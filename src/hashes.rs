use std::fmt::Formatter;
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer};

#[derive(Debug, Clone)]
pub struct Hashes(Vec<[u8; 20]>);

struct HashStrVisitor;

impl<'de> Visitor<'de> for HashStrVisitor {
    type Value = Hashes;
    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        formatter.write_str("a byte string whose length is a multiple of 20")
    }

    fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E> where E: Error {
        if v.len() % 20 != 0 {
            Err(E::custom(format!("length is {}", v.len())))
        } else {
            // TODO: use array_chunks when stable
            Ok(
                Hashes(v.chunks_exact(20)
                    .map(|slice_20| {
                        slice_20.try_into().expect("guaranteed to be length 20")
                    }).collect())
            )
        }
    }
}

impl<'de> Deserialize<'de> for Hashes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_bytes(HashStrVisitor)
    }
}