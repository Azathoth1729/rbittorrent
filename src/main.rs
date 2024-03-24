use anyhow::Context;
use clap::Parser;

use crate::{args::{Args, Command}, torrent::*};

pub(crate) mod args;
pub(crate) mod hashes;
pub(crate) mod torrent;
pub(crate) mod de;

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Decode { msg } => {
            let decoded_value = de::decode_cmd(&msg)?;
            println!("{:?}", decoded_value);
        }
        Command::Info { path } => {
            let torrent_f = std::fs::read(path).context("read torrent file")?;
            let torrent: Torrent = serde_bencode::from_bytes(&torrent_f).context("parse torrent file")?;

            eprintln!("{torrent:?}");
            println!("Tracker URL: P{}", torrent.announce);
            if let Keys::SingleFile { length } = torrent.info.keys {
                println!("Length: {}", length)
            }
        }
    }
    Ok(())
}
