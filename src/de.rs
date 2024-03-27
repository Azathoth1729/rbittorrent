use anyhow::{anyhow, Context};
use serde_bencode::value::Value as BencodeValue;
use std::collections::HashMap;

pub fn decode_cmd(encoded_value: &str) -> anyhow::Result<BencodeValue> {
    let (value, rest) = decode_bencoded_value(encoded_value)?;
    if rest.is_empty() {
        Ok(value)
    } else {
        Err(anyhow!("still have decode str: {}", rest))
    }
}

///
///
/// # Arguments
///
/// * `encoded_value`: bencoded string, may be very long
///
/// returns: Result of a pair (json Value, rest of input string)
fn decode_bencoded_value(encoded_value: &str) -> anyhow::Result<(BencodeValue, &str)> {
    let first_char = encoded_value
        .chars()
        .next()
        .context("encoded_value exhausted!")?;
    // let mut peeker = encoded_value.chars().peekable();
    // if peeker.peek().unwrap() == &'i' {}
    match first_char {
        'i' => decode_bencoded_int(encoded_value),
        '0'..='9' => decode_bencoded_string(encoded_value),
        'l' => {
            let mut values = Vec::new();
            let mut remainder = &encoded_value[1..];
            while remainder.chars().next() != Some('e') {
                let (value, rest) = decode_bencoded_value(remainder)?;
                values.push(value);
                remainder = rest;
            }
            let remainder = remainder
                .strip_prefix('e')
                .with_context(|| format!("Can't strip prefix `e` of str: {}", remainder))?;
            Ok((BencodeValue::List(values), remainder))
        }
        'd' => {
            let mut map = HashMap::new();
            let mut remainder = &encoded_value[1..];
            while remainder.chars().next() != Some('e') {
                let decoded = decode_bencoded_value(remainder)?;
                if let (BencodeValue::Bytes(key), rest) = decoded {
                    let (value, rest) = decode_bencoded_value(rest).with_context(|| {
                        format!(
                            "Can't decoded when parsed value of map, str: {}\nprev key is: {:?}",
                            rest, key
                        )
                    })?;
                    map.insert(key, value);
                    remainder = rest;
                } else {
                    return Err(anyhow!("decoded value: {:?} is not string", decoded.0));
                }
            }
            let remainder = remainder
                .strip_prefix('e')
                .with_context(|| format!("Can't strip prefix `e` of str: {}", remainder))?;
            Ok((BencodeValue::Dict(map), remainder))
        }
        _ => Err(anyhow!("Encounter an invalid char: {}", encoded_value)),
    }
}

// Example: "5:hello" -> "hello"
fn decode_bencoded_string(encoded_string: &str) -> anyhow::Result<(BencodeValue, &str)> {
    let (len, rest) = encoded_string
        .split_once(':')
        .with_context(|| format!("Can't split_once encoded_value: {} by `:`", encoded_string))
        .and_then(|(len_str, rest)| {
            let len = len_str.parse::<usize>().with_context(|| {
                format!(
                    "Can't parse str: {} before `:` delimiter which should be a usize",
                    &len_str
                )
            })?;
            if len > rest.len() {
                Err(anyhow!(
                    "Parsed len {} is bigger than rest.len {}",
                    len,
                    rest.len()
                ))
            } else {
                Ok((len, rest))
            }
        })?;
    Ok((BencodeValue::Bytes((&rest[..len]).into()), &rest[len..]))
}

// Example: "i42e" -> 42
// Example: "i0e" -> 0
// Example: "i-1e" -> -1
fn decode_bencoded_int(encoded_int: &str) -> anyhow::Result<(BencodeValue, &str)> {
    encoded_int
        .strip_prefix('i')
        .with_context(|| format!("str: {} can't strip prefix `i`", encoded_int))?
        .split_once('e')
        .with_context(|| format!("Can't split_once encoded_value: {} by `:`", encoded_int))
        .and_then(|(int_str, rest)| {
            if int_str.strip_prefix("-0").is_some() {
                return Err(anyhow!("i-0*e is invalid"));
            }
            let int = int_str.parse::<i64>().with_context(|| {
                format!(
                    "Can't parse str : {} before `:` delimiter which should be a usize",
                    &int_str
                )
            })?;
            let int_str = int_str.strip_prefix('-').or(Some(int_str)).unwrap();
            if int_str.starts_with('0') && int != 0 {
                return Err(anyhow!("i(-)0*e is invalid"));
            }
            Ok((BencodeValue::Int(int), rest))
        })
}
