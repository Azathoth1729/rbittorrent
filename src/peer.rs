use crate::common::AsBytes;
use bytes::{Buf, BufMut, BytesMut};
use serde::{
    de::{Error, Visitor},
    Deserialize, Serialize, Serializer,
};
use std::{
    fmt::Formatter,
    net::{Ipv4Addr, SocketAddrV4},
};
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum MessageTag {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub tag: MessageTag,
    pub payload: Vec<u8>,
}

pub struct MessageFramer {}

#[derive(Debug)]
#[repr(C)]
pub struct MessageRequest {
    index: [u8; 4],
    begin: [u8; 4],
    length: [u8; 4],
}

#[derive(Debug)]
#[repr(C)]
pub struct MessagePiece {
    index: [u8; 4],
    begin: [u8; 4],
    block: [u8],
}

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

impl Handshake {
    const MEM_SIZE: usize = std::mem::size_of::<Self>();
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            length: 19,
            bittorrent: *b"BitTorrent protocol",
            reserved: [0; 8],
            info_hash,
            peer_id,
        }
    }
    // pub fn as_bytes_mut(&mut self) -> &mut [u8; Self::MEM_SIZE] {
    //      let self_as_bytes = self as *mut Self as *mut [u8; Self::MEM_SIZE];
    //      // Safety: Handshake is a POD with repr(c)
    //      unsafe { &mut *self_as_bytes }
    //  }
}

impl AsBytes for Handshake {}

impl Decoder for MessageFramer {
    type Item = Message;
    type Error = std::io::Error;
    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        const MAX: usize = 1 << 16;

        // eprintln!("decode happened. src.len={}", src.len());

        if src.len() < 4 {
            // Not enough data to read length marker.
            return Ok(None);
        }

        // Read length marker.
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_be_bytes(length_bytes) as usize;

        if length == 0 {
            // This is a heartbeat message, discard it
            src.advance(4);
            // And then try again in case the buffer has more message
            return self.decode(src);
        }

        // Check that the length is not too large to avoid a denial of
        // service attack where the server runs out of memory.
        if length > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            ));
        }

        if src.len() < 5 {
            // Not enough data to read tag marker.
            return Ok(None);
        }

        if src.len() < 4 + length {
            // The full string has not yet arrived.
            //
            // We reserve more space in the buffer. This is not strictly
            // necessary, but is a good idea performance-wise.
            src.reserve(4 + length - src.len());

            // We inform the Framed that we need more bytes to form the next
            // frame.
            return Ok(None);
        }

        // eprintln!("length: {length}");

        // Use advance to modify src such that it no longer contains
        // this frame.
        let tag = src[4];
        let data = src[5..5 + length - 1].to_vec();
        src.advance(4 + length);

        Ok(Some(Message {
            tag: tag
                .try_into()
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?,
            payload: data,
        }))
    }
}

impl Encoder<Message> for MessageFramer {
    type Error = std::io::Error;

    fn encode(&mut self, item: Message, dst: &mut BytesMut) -> Result<(), Self::Error> {
        const MAX: usize = 8 * 1024 * 1024;

        // Don't send a message if it is longer than the other end will
        // accept.
        if item.len() > MAX {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", item.len()),
            ));
        }

        // Convert the length into a byte array.
        // The cast to u32 cannot overflow due to the length check above.
        let len_slice = u32::to_be_bytes(item.len() as u32);

        // Reserve space in the buffer.
        dst.reserve(4 + item.len());

        // Write the length and string to the buffer.
        dst.extend_from_slice(&len_slice);
        dst.put_u8(item.tag as u8);
        dst.extend_from_slice(item.payload.as_slice());
        Ok(())
    }
}

impl Message {
    pub fn new(tag: MessageTag, payload: Vec<u8>) -> Self {
        Self { tag, payload }
    }
    pub fn len(&self) -> usize {
        1 /* tag */ + self.payload.len()
    }
}

impl MessageRequest {
    pub fn new(index: u32, begin: u32, length: u32) -> Self {
        Self {
            index: index.to_be_bytes(),
            begin: begin.to_be_bytes(),
            length: length.to_be_bytes(),
        }
    }
    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }
    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }
    pub fn length(&self) -> u32 {
        u32::from_be_bytes(self.length)
    }

    // #[allow(dead_code)]
    // pub fn as_bytes(&self) -> &[u8; std::mem::size_of::<Self>()] {
    //     let self_as_bytes = self as *const Self as *const [u8; std::mem::size_of::<Self>()];
    //     unsafe { &*self_as_bytes }
    // }
    //
    // pub fn as_bytes_mut(&mut self) -> &mut [u8; std::mem::size_of::<Self>()] {
    //     let self_as_bytes = self as *mut Self as *mut [u8; std::mem::size_of::<Self>()];
    //     // Safety: Handshake is a POD with repr(c)
    //     unsafe { &mut *self_as_bytes }
    // }
}

impl MessagePiece {
    // pub fn new(index: u32, begin: u32, block: [u8]) -> Self {
    //     Self {
    //         index: index.to_be_bytes(),
    //         begin: begin.to_be_bytes(),
    //         block,
    //     }
    // }
    pub fn index(&self) -> u32 {
        u32::from_be_bytes(self.index)
    }
    pub fn begin(&self) -> u32 {
        u32::from_be_bytes(self.begin)
    }
    pub fn block(&self) -> &[u8] {
        &self.block
    }
    
    pub fn try_from_bytes(data: &[u8]) -> anyhow::Result<&Self> {
        // MessagePiece {
        //     index: [0,0,0,0],
        //     begin: [0,0,0,0],
        //     block: [0,0,0,0],
        // }
        todo!()
    }
}
impl AsBytes for MessageRequest {}

impl TryFrom<u8> for MessageTag {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageTag::Choke),
            1 => Ok(MessageTag::Unchoke),
            2 => Ok(MessageTag::Interested),
            3 => Ok(MessageTag::NotInterested),
            4 => Ok(MessageTag::Have),
            5 => Ok(MessageTag::Bitfield),
            6 => Ok(MessageTag::Request),
            7 => Ok(MessageTag::Piece),
            8 => Ok(MessageTag::Cancel),
            _ => Err(format!("Unknown message type: {}.", value)),
        }
    }
}
