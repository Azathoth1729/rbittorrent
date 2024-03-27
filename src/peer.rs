use serde::de::{Error, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt::Formatter;
use std::net::{Ipv4Addr, SocketAddrV4};

#[repr(C)]
#[derive(Debug, PartialEq)]
pub struct Handshake {
    /// length of the protocol string (BitTorrent protocol) which is 19 (1 byte)
    pub length: u8,
    /// the string BitTorrent protocol (19 bytes)
    pub bittorrent: [u8; 19],
    /// eight reserved bytes, which are all set to zero (8 bytes)
    pub reserved: [u8; 8],
    /// sha1 info_hash (20 bytes) (NOT the hexadecimal representation, which is 40 bytes long)
    pub info_hash: [u8; 20],
    /// peer id (20 bytes)
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            length: 19,
            bittorrent: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash,
            peer_id,
        }
    }
    #[allow(dead_code)]
    pub fn as_bytes(&self) -> &[u8; std::mem::size_of::<Handshake>()] {
        let handshake_bytes =
            self as *const Handshake as *const [u8; std::mem::size_of::<Handshake>()];
        unsafe { &*handshake_bytes }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8; std::mem::size_of::<Handshake>()] {
        let handshake_bytes = self as *mut Handshake as *mut [u8; std::mem::size_of::<Handshake>()];
        // Safety: Handshake is a POD with repr(c)
        unsafe { &mut *handshake_bytes }
    }
}

#[derive(Debug, Clone)]
pub struct Peers(pub Vec<SocketAddrV4>);

pub struct PeersVisitor;

impl<'de> Visitor<'de> for PeersVisitor {
    type Value = Peers;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str(
            "6 bytes, the first 4 bytes are the peer's IP address \
            and the last 2 bytes are the peer's port number.",
        )
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if v.len() % 6 != 0 {
            Err(E::custom(format!("length is {}", v.len())))
        } else {
            // TODO: use array_chunks when stable
            Ok(Peers(
                v.chunks_exact(6)
                    .map(|slice_6| {
                        SocketAddrV4::new(
                            Ipv4Addr::new(slice_6[0], slice_6[1], slice_6[2], slice_6[3]),
                            u16::from_be_bytes([slice_6[4], slice_6[5]]),
                        )
                    })
                    .collect(),
            ))
        }
    }
}

impl<'de> Deserialize<'de> for Peers {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_bytes(PeersVisitor)
    }
}

impl Serialize for Peers {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut single_slice = Vec::with_capacity(6 * self.0.len());
        for peer in &self.0 {
            single_slice.extend(peer.ip().octets());
            single_slice.extend(peer.port().to_be_bytes());
        }
        serializer.serialize_bytes(&single_slice)
    }
}
