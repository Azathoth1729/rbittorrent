use anyhow::Context;
use clap::Parser;
use hex;

use crate::{
    args::{Args, Command},
    torrent::{Keys, Torrent},
    tracker::TrackerRequest,
    tracker::TrackerResponse,
};

pub(crate) mod args;
pub(crate) mod de;
pub(crate) mod hashes;
pub(crate) mod torrent;
pub(crate) mod tracker;

const PEER_ID: &str = "00112233445566778899";

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
            println!("Tracker URL: P{}", torrent.announce);

            let length = if let Keys::SingleFile { length } = torrent.info.keys {
                length
            } else {
                todo!();
            };

            println!("Length: {}", length);

            let info_hash = torrent.info_hash()?;
            println!("Info Hash: {}", hex::encode(&info_hash));
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
            let info_hash = torrent.info_hash()?;

            let length = if let Keys::SingleFile { length } = torrent.info.keys {
                length
            } else {
                todo!();
            };
            let request = TrackerRequest {
                info_hash,
                peer_id: String::from(PEER_ID),
                port: 6881,
                uploaded: 0,
                downloaded: 0,
                left: length,
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
    }
    Ok(())
}
