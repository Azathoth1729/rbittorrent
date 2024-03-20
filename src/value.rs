use std::collections::HashMap;

/// Represents any valid Bencode value.
#[derive(Clone)]
pub enum BenCode {
    Int(i64),
    String(String),
    Array(Vec<BenCode>),
    Dict(HashMap<String, BenCode>)
}