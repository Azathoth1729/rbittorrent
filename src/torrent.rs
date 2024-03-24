use serde::{Deserialize, Serialize};
use crate::hashes;

/// Metainfo files (also known as .torrent files) are bencoded dictionaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Torrent {
    /// The URL of the tracker.
    pub announce: String,
    pub info: Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    /// The suggested name to save the file (or directory) as. It is purely advisory.
    ///
    /// In the single file case, the name key is the name of a file, 
    /// In the multiple file case, it's the name of a directory.
    pub name: String,

    /// The number of bytes in each piece the file is split into.
    ///
    /// For the purposes of transfer, files are split into fixed-size pieces which are all the same length except for possibly the last one which may be truncated.
    /// piece length is almost always a power of two, most commonly 2^18 = 256 K (BitTorrent prior to version 3.2 uses 2 20 = 1 M as default).
    #[serde(rename = "piece length")]
    pub plength: usize,

    /// Each entry of `pieces` is the SHA1 hash of the piece at the corresponding index.
    pub pieces: hashes::Hashes,

    #[serde(flatten)]
    pub keys: Keys,
}

/// There is a key `length` or a key `files`, but not both or neither.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Keys {
    /// If `length` is present then the download represents a single file
    SingleFile {
        /// The length of the file in bytes.
        length: usize,
    },
    /// Otherwise it represents a set of files which go in a directory structure.
    ///
    /// For the purposes of the other keys in `Info`, the multi-file case is treated as only having
    /// a single file by concatenating the files in the order they appear in the files list.
    MultiFile {
        files: Vec<TorrentFile>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFile {
    /// The length of the file in bytes.
    length: usize,
    /// Subdirectory names for this file, the last of which is the actual file name 
    /// (a zero length list is an error case).
    path: Vec<String>,
}
