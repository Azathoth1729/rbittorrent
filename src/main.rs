pub(crate) mod value;
pub(crate) mod args;

use std::fmt::format;
use anyhow::{anyhow, Context};
use serde_json;
use clap::Parser;
use crate::args::{Args, Command};


///
///
/// # Arguments 
///
/// * `encoded_value`: bencoded string, may be very long
///
/// returns: Result of a pair (json Value, rest of input string)
fn decode_bencoded_value(encoded_value: &str) -> anyhow::Result<(serde_json::Value, &str)> {
    let first_char = encoded_value.chars().next().context("encoded_value exhausted!")?;
    match first_char {
        'i' => { decode_bencoded_int(encoded_value) }
        '0'..='9' => { decode_bencoded_string(encoded_value) }
        'l' => { todo!() }
        'd' => { todo!() }
        _ => { Err(anyhow!("Encounter an invalid char: {}", encoded_value)) }
    }
}

// Example: "5:hello" -> "hello"
fn decode_bencoded_string(encoded_string: &str) -> anyhow::Result<(serde_json::Value, &str)> {
    let (len, rest) = encoded_string.split_once(':')
        .with_context(|| format!("Can't split_once encoded_value: {} by `:`", encoded_string))
        .and_then(|(len_str, rest)| {
            let len = len_str.parse::<usize>().with_context(|| format!("Can't parse str: {} before `:` delimiter which should be a usize", &len_str))?;
            if len > rest.len() {
                Err(anyhow!("Parsed len {} is bigger than rest.len {}", len, rest.len()))
            } else {
                Ok((len, rest))
            }
        })?;
    Ok((serde_json::Value::String((&rest[..len]).to_string()), &rest[len..]))
}

// Example: "i42e" -> 42
// Example: "i0e" -> 0
// Example: "i-1e" -> -1
fn decode_bencoded_int(encoded_int: &str) -> anyhow::Result<(serde_json::Value, &str)> {
    encoded_int.strip_prefix('i').with_context(|| format!("str: {} can't strip prefix `i`", encoded_int))?
        .split_once('e').with_context(|| format!("Can't split_once encoded_value: {} by `:`", encoded_int))
        .and_then(|(int_str, rest)| {
            if int_str.strip_prefix("-0").is_some() {
                return Err(anyhow!("i-0*e is invalid"));
            }
            let int = int_str.parse::<isize>().with_context(|| format!("Can't parse str : {} before `:` delimiter which should be a usize", &int_str))?;
            let int_str = int_str.strip_prefix('-').or(Some(int_str)).unwrap();
            if int_str.starts_with('0') && int != 0 {
                return Err(anyhow!("i(-)0*e is invalid"));
            }
            Ok((serde_json::Value::Number(int.into()), rest))
        })
}

// Usage: your_bittorrent.sh decode "<encoded_value>"
fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Decode { msg } => {
            let (decoded_value, _) = decode_bencoded_value(&msg).with_context(|| format!("Failed to decode {}", &msg))?;
            println!("{}", decoded_value.to_string());
        }
    }
    Ok(())
}
