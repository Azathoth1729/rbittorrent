use anyhow::Context;
use clap::Parser;
use hex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    args::{Args, Command},
    peer::Handshake,
    torrent::Torrent,
    tracker::TrackerRequest,
    tracker::TrackerResponse,
};

pub(crate) mod args;
pub(crate) mod de;
pub(crate) mod hashes;
pub(crate) mod peer;
pub(crate) mod torrent;
pub(crate) mod tracker;

const PEER_ID: &str = "00112233445566778899";
const PEER_ID_BYTES: [u8; 20] = *b"00112233445566778899";

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

            let request = TrackerRequest {
                info_hash: torrent.info_hash()?,
                peer_id: String::from(PEER_ID),
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
            eprintln!("url with params:\n{}", tracker_url);

            let response = reqwest::get(tracker_url).await.context("fetch tracker")?;

            let response = response.bytes().await.context("fetch tracker response")?;
            let response: TrackerResponse =
                serde_bencode::from_bytes(&response).context("parse tracker response")?;
            for peer in response.peers.0 {
                println!("{}", peer);
            }
        }
        Command::Handshake { path, peer_ip } => {
            println!("peer_ip: {}", peer_ip);

            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent =
                serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;
            let info_hash = torrent.info_hash()?;

            let mut peer_stream = tokio::net::TcpStream::connect(peer_ip)
                .await
                .with_context(|| format!("connect to peer: {}", peer_ip))?;

            let mut handshake = Handshake::new(info_hash, PEER_ID_BYTES);
            {
                let handshake_bytes = handshake.as_bytes_mut();
                peer_stream
                    .write_all(handshake_bytes)
                    .await
                    .context("write handshake")?;
                peer_stream
                    .read_exact(handshake_bytes)
                    .await
                    .context("read handshake")?;
            }
            assert_eq!(handshake.length, 19);
            assert_eq!(handshake.bittorrent, *b"BitTorrent protocol");
            assert_eq!(handshake.info_hash, info_hash);

            println!("Peer ID: {}", hex::encode(handshake.peer_id));
        }
    }
    Ok(())
}
