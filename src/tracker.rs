use crate::peer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerRequest {
    /// The info hash of the torrent
    /// 20 bytes long, will need to be URL encoded
    /// Note: this is NOT the hexadecimal representation, which is 40 bytes long
    #[serde(skip_serializing)]
    pub info_hash: [u8; 20],
    /// A unique identifier for your client
    /// A string of length 20 that you get to pick.
    pub peer_id: String,
    /// The port your client is listening on
    pub port: u16,
    /// The total amount uploaded so far
    pub uploaded: usize,
    /// The total amount downloaded so far
    pub downloaded: usize,
    /// The number of bytes left to download
    pub left: usize,
    /// Whether the peer list should use the compact representation

    /// The compact representation is more commonly used in the wild,
    /// the non-compact representation is mostly supported for backward-compatibility.
    pub compact: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackerResponse {
    /// An integer, indicating how often your client should make a request to the tracker in seconds.
    pub interval: usize,
    /// A string, which contains list of peers that your client can connect to.
    /// Each peer is represented using 6 bytes.
    /// The first 4 bytes are the peer's IP address and the last 2 bytes are the peer's port number.
    pub peers: peer::Peers,
}
