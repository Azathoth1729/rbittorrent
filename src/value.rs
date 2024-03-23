use std::collections::HashMap;

/// Represents any valid Bencode value.
#[warn(dead_code)]
pub enum BenCode {
    Int(i64),
    String(String),
    Array(Vec<BenCode>),
    Dict(HashMap<String, BenCode>),
}