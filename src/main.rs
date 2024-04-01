#![feature(generic_const_exprs)]

use anyhow::Context;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use sha1::{Digest, Sha1};
use std::net::SocketAddrV4;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::common::AsBytes;
use crate::peer::{Message, MessageFramer, MessagePiece, MessageRequest, MessageTag};
use crate::{
    args::{Args, Command},
    peer::Handshake,
    torrent::Torrent,
    tracker::TrackerRequest,
    tracker::TrackerResponse,
};

pub(crate) mod args;
pub(crate) mod common;
pub(crate) mod de;
pub(crate) mod hashes;
pub(crate) mod peer;
pub(crate) mod torrent;
pub(crate) mod tracker;

const PEER_ID: &str = "00112233445566778899";
const PEER_ID_BYTES: [u8; 20] = *b"00112233445566778899";

const PIECE_BLOCK_MAX: usize = 1 << 14;

async fn get_tracker_info(
    torrent: &Torrent,
    self_peer_id: &str,
) -> anyhow::Result<TrackerResponse> {
    let request = TrackerRequest {
        info_hash: torrent.info_hash()?,
        peer_id: String::from(self_peer_id),
        port: 6881,
        uploaded: 0,
        downloaded: 0,
        left: torrent.info.keys.length(),
        compact: 1,
    };

    let mut tracker_url =
        reqwest::Url::parse(&torrent.announce).context("parse tracker announce url")?;
    let mut url_params =
        serde_urlencoded::to_string(&request).context("url-encode tracker parameters")?;

    let hexed_info_hash_str = &request.info_hash.map(|byte| hex::encode(&[byte])).join("%");

    url_params.push_str(format!("&info_hash=%{}", hexed_info_hash_str).as_str());

    tracker_url.set_query(Some(&url_params));
    eprintln!("get_tracker_info by url:\n{}", tracker_url);

    let response = reqwest::get(tracker_url).await.context("fetch tracker")?;
    let response = response.bytes().await.context("fetch tracker response")?;
    let response: TrackerResponse =
        serde_bencode::from_bytes(&response).context("parse tracker response")?;
    Ok(response)
}

async fn make_handshake(
    torrent: &Torrent,
    peer_ip: &SocketAddrV4,
) -> anyhow::Result<(Handshake, TcpStream)> {
    let mut tcp_stream: TcpStream = tokio::net::TcpStream::connect(peer_ip)
        .await
        .with_context(|| format!("connect to peer: {}", peer_ip))?;

    let mut handshake = Handshake::new(torrent.info_hash()?, PEER_ID_BYTES);
    {
        let handshake_bytes = handshake.as_bytes_mut();
        tcp_stream
            .write_all(handshake_bytes)
            .await
            .context("write handshake")?;
        tcp_stream
            .read_exact(handshake_bytes)
            .await
            .context("read handshake")?;
    }
    Ok((handshake, tcp_stream))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Decode { msg } => {
            let decoded_value = de::decode_cmd(&msg)?;
            println!("{:?}", decoded_value);
        }
        Command::Info { path } => {
            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            eprintln!("{torrent:?}");
            println!("Tracker URL: {}", torrent.announce);
            println!("Length: {}", torrent.info.keys.length());
            println!("Info Hash: {}", hex::encode(torrent.info_hash()?));
            println!("Piece Length: {}", torrent.info.plength);
            println!("Piece Hashes:");

            for hash in torrent.info.pieces.0 {
                println!("{}", hex::encode(&hash));
            }
        }
        Command::Peers { path } => {
            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            let response = get_tracker_info(&torrent, PEER_ID).await?;

            for peer in response.peers.0 {
                println!("{}", peer);
            }
        }
        Command::Handshake { path, peer_ip } => {
            println!("Handshake with peer_ip: {}", peer_ip);

            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;
            let info_hash = torrent.info_hash()?;

            let (handshake, _) = make_handshake(&torrent, &peer_ip).await?;
            assert_eq!(handshake.length, 19);
            assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");
            assert_eq!(handshake.info_hash, info_hash);

            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        }
        Command::DownloadPiece {
            output,
            path,
            piece_index,
        } => {
            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;
            eprintln!("torrent info: {:?}", &torrent.info);
            assert!(piece_index < torrent.info.pieces.0.len());
            let response = get_tracker_info(&torrent, PEER_ID).await?;

            let to_connect_peer = response.peers.0[0];
            let (handshake, tcp_stream) = make_handshake(&torrent, &to_connect_peer).await?;
            assert_eq!(handshake.length, 19);
            assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");
            assert_eq!(handshake.info_hash, torrent.info_hash()?);

            // let framer = MessageFramer {};
            let mut stream = tokio_util::codec::Framed::new(tcp_stream, MessageFramer {});
            let bitfield_msg = stream
                .next()
                .await
                .expect("peer always sends a bitfields")
                .context("peer message was invalid")?;
            eprintln!("bitfield_msg: {:#?}", bitfield_msg);
            assert_eq!(bitfield_msg.tag, MessageTag::Bitfield);
            // assert!(bitfield_msg.payload.is_empty());
            stream
                .send(Message::new(MessageTag::Interested, Vec::new()))
                .await
                .context("send interested message")?;

            let unchoke_msg = stream
                .next()
                .await
                .expect("peer always sends a bitfields")
                .context("peer message was invalid")?;
            assert_eq!(unchoke_msg.tag, MessageTag::Unchoke);
            assert!(unchoke_msg.payload.is_empty());

            let piece_size = if piece_index == torrent.info.pieces.0.len() - 1 {
                let rem = torrent.info.keys.length() % torrent.info.plength;
                if rem == 0 {
                    torrent.info.plength
                } else {
                    rem
                }
            } else {
                torrent.info.plength
            };
            let mut all_blocks: Vec<u8> = Vec::with_capacity(piece_size);
            // piece_size / PIECE_BLOCK_MAX round up
            let nblocks = (piece_size + (PIECE_BLOCK_MAX - 1)) / PIECE_BLOCK_MAX;
            eprintln!("{nblocks} blocks of at most {PIECE_BLOCK_MAX} to reach {piece_size}");
            // let
            for block_idx in 0..nblocks {
                let block_size = if block_idx == nblocks - 1 {
                    let rem = piece_size % PIECE_BLOCK_MAX;
                    if rem == 0 {
                        PIECE_BLOCK_MAX
                    } else {
                        rem
                    }
                } else {
                    PIECE_BLOCK_MAX
                };
                eprintln!("block_size: {block_size} ");

                let message_request = MessageRequest::new(
                    piece_index as u32,
                    (block_idx * PIECE_BLOCK_MAX) as u32,
                    block_size as u32,
                );
                stream
                    .send(Message::new(
                        MessageTag::Request,
                        Vec::from(message_request.as_bytes()),
                    ))
                    .await
                    .with_context(|| format!("send request for block {block_idx}"))?;

                let piece_msg = stream
                    .next()
                    .await
                    .expect("peer should send a piece")
                    .context("peer message was invalid")?;
                assert_eq!(piece_msg.tag, MessageTag::Piece);
                assert!(!piece_msg.payload.is_empty());
                let a = &piece_msg.payload[..];
                // eprintln!("{}",std::mem::size_of::<MessagePiece>());
                let msg_piece = (&piece_msg.payload[..piece_msg.payload.len() - 8]) as *const [u8]
                    as *const MessagePiece;
                let msg_piece = unsafe { &*msg_piece };
                assert_eq!(msg_piece.index() as usize, piece_index);
                assert_eq!(msg_piece.begin() as usize, block_idx * PIECE_BLOCK_MAX);
                assert_eq!(
                    msg_piece.block().len(),
                    block_size,
                    "on iteration {} ",
                    block_idx
                );
                eprintln!(
                    "msg_piece:\n\
                     index: {}\n\
                     begin: {}\n\
                     block.len: {}",
                    msg_piece.index(),
                    msg_piece.begin(),
                    msg_piece.block().len()
                );
                all_blocks.extend(msg_piece.block());
            }
            assert_eq!(all_blocks.len(), piece_size);
            // eprintln!("piece_size: {}", piece_size);
            // eprintln!("all_blocks.len: {:#?}", all_blocks.len());

            let mut hasher = Sha1::new();
            hasher.update(&all_blocks);
            let hash: [u8; 20] = hasher
                .finalize()
                .try_into()
                .context("received data' sha1 hash should be equal to piece_hash")?;
            assert_eq!(hash, torrent.info.pieces.0[piece_index]);

            std::fs::create_dir_all("./tmp")?;

            tokio::fs::write(&output, all_blocks)
                .await
                .context("write out downloaded piece")?;
            println!("Piece {piece_index} downloaded to {}.", output.display());
        }
    }
    Ok(())
}
