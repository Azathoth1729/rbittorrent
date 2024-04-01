use clap::{Parser, Subcommand};
use std::net::SocketAddrV4;
use std::path::PathBuf;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
#[clap(rename_all = "snake_case")]
pub enum Command {
    Decode {
        msg: String,
    },
    Info {
        path: PathBuf,
    },
    Peers {
        path: PathBuf,
    },
    Handshake {
        path: PathBuf,
        peer_ip: SocketAddrV4,
    },
    DownloadPiece {
        #[arg(short)]
        output: PathBuf,
        path: PathBuf,
        piece_index: usize,
    },
}
