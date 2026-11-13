//! The parsers of the published lists.
//!
//! A parser reads the document a vendor publishes and answers the prefixes of
//! it. The parser tells two failures apart: a body that is not the document the
//! vendor publishes — an error page, an empty body — and a document whose shape
//! changed. The first is the fault of the environment, the second is a fact the
//! crate has to follow.

use std::net::Ipv4Addr;
use std::net::Ipv6Addr;

use ipnet::{IpNet, Ipv4Net, Ipv6Net};
use serde_json::Value;

use crate::error::ParseError;

/// The parsers the crate knows.
pub fn is_known_parser(parser: &str) -> bool {
    matches!(
        parser,
        "google" | "openai" | "uptimerobot" | "ahrefs" | "amazon" | "txt"
    )
}

/// Turns a body into prefixes with the named parser.
///
/// An unknown parser reads the body as plain text.
pub fn parse(parser: &str, body: &[u8]) -> Result<Vec<IpNet>, ParseError> {
    match parser {
        "google" => parse_prefixes(body, "prefixes", &["ipv4Prefix", "ipv6Prefix"]),
        "openai" => parse_prefixes(body, "prefixes", &["prefix"]),
        "uptimerobot" => parse_prefixes(body, "prefixes", &["ip_prefix", "ipv6_prefix"]),
        "ahrefs" => parse_ahrefs(body),
        "amazon" => parse_amazon(body),
        _ => parse_txt(body),
    }
}

/// Reads a document that holds a list of entries under `list_key`.
///
/// Every entry is read on its own: an entry that holds no usable prefix is
/// skipped, the other entries of the document still count.
fn parse_prefixes(body: &[u8], list_key: &str, keys: &[&str]) -> Result<Vec<IpNet>, ParseError> {
    let document: Value = serde_json::from_slice(body).map_err(|error| {
        ParseError::NotADocument(format!("the body is not a json document: {error}"))
    })?;
    let entries = document
        .get(list_key)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ParseError::UnexpectedShape(format!("the document holds no `{list_key}` list"))
        })?;
    let mut prefixes = Vec::new();
    for entry in entries {
        for key in keys {
            if let Some(value) = entry.get(*key).and_then(Value::as_str) {
                push_addr_or_cidr(&mut prefixes, value);
            }
        }
    }
    Ok(prefixes)
}

/// Reads the list of the addresses of the Ahrefs crawler.
fn parse_ahrefs(body: &[u8]) -> Result<Vec<IpNet>, ParseError> {
    let document: Value = serde_json::from_slice(body).map_err(|error| {
        ParseError::NotADocument(format!("the body is not a json document: {error}"))
    })?;
    let entries = document
        .get("ips")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            ParseError::UnexpectedShape("the document holds no `ips` list".to_string())
        })?;
    let mut prefixes = Vec::new();
    for entry in entries {
        if let Some(value) = entry.get("ip_address").and_then(Value::as_str) {
            push_addr_or_cidr(&mut prefixes, value);
        }
    }
    Ok(prefixes)
}

/// Reads the list of the addresses of the Amazon crawler.
///
/// Amazon publishes a page that carries a json document. The document is
/// preferred; a page that lists the addresses as literals is the fallback.
fn parse_amazon(body: &[u8]) -> Result<Vec<IpNet>, ParseError> {
    let text = String::from_utf8_lossy(body);
    if let Some(document) = extract_json_document(&text) {
        let document: Value = serde_json::from_str(document).map_err(|error| {
            ParseError::NotADocument(format!("the page holds no readable json document: {error}"))
        })?;
        let entries = document
            .get("prefixes")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                ParseError::UnexpectedShape("the document holds no `prefixes` list".to_string())
            })?;
        let mut prefixes = Vec::new();
        for entry in entries {
            if let Some(value) = entry.get("ipv4Prefix").and_then(Value::as_str) {
                push_addr_or_cidr(&mut prefixes, value);
            }
        }
        if prefixes.is_empty() {
            return Err(ParseError::UnexpectedShape(
                "the `prefixes` list holds no usable prefix".to_string(),
            ));
        }
        return Ok(prefixes);
    }
    let mut prefixes = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        let is_address_character = character.is_ascii_digit() || character == '.';
        if is_address_character {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(begin) = start.take() {
            push_addr_or_cidr(&mut prefixes, &text[begin..index]);
        }
    }
    if let Some(begin) = start {
        push_addr_or_cidr(&mut prefixes, &text[begin..]);
    }
    if prefixes.is_empty() {
        return Err(ParseError::NotADocument(
            "the page holds no address list".to_string(),
        ));
    }
    // Deduplicate while preserving the order.
    prefixes.sort();
    prefixes.dedup();
    Ok(prefixes)
}

/// Finds the json document the page of Amazon carries.
fn extract_json_document(text: &str) -> Option<&str> {
    // The document looks like `{"creationTime": "...", "prefixes": [...]}`.
    let start = text.find("\"prefixes\"")?;
    let object_start = text[..start].rfind('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    for (offset, byte) in bytes[object_start..].iter().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[object_start..object_start + offset + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Reads a body that lists the addresses line by line.
fn parse_txt(body: &[u8]) -> Result<Vec<IpNet>, ParseError> {
    let text = String::from_utf8_lossy(body);
    let mut prefixes = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        push_addr_or_cidr(&mut prefixes, line);
    }
    if prefixes.is_empty() {
        return Err(ParseError::NotADocument(
            "the body holds no address".to_string(),
        ));
    }
    Ok(prefixes)
}

/// Adds the prefix of an entry, if the entry holds one.
fn push_addr_or_cidr(prefixes: &mut Vec<IpNet>, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    if let Ok(net) = value.parse::<IpNet>() {
        prefixes.push(net);
        return;
    }
    if let Ok(address) = value.parse::<Ipv4Addr>() {
        prefixes.push(IpNet::V4(Ipv4Net::new(address, 32).expect("a /32 network")));
        return;
    }
    if let Ok(address) = value.parse::<Ipv6Addr>() {
        prefixes.push(IpNet::V6(
            Ipv6Net::new(address, 128).expect("a /128 network"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[test]
    fn google_parser_v4_v6() {
        let body =
            br#"{"prefixes":[{"ipv4Prefix":"66.249.64.0/19"},{"ipv6Prefix":"2001:4860::/32"}]}"#;
        let nets = parse("google", body).unwrap();
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains(&"66.249.66.1".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn openai_parser() {
        let body = br#"{"prefixes":[{"prefix":"132.196.86.0/24"}]}"#;
        let nets = parse("openai", body).unwrap();
        assert_eq!(nets.len(), 1);
    }

    #[test]
    fn txt_parser_bare_ip() {
        let body = b"1.2.3.4\n# comment\n5.6.7.0/24\n";
        let nets = parse("txt", body).unwrap();
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains(&"1.2.3.4".parse::<IpAddr>().unwrap()));
    }

    #[test]
    fn uptimerobot_parser() {
        let body = br#"{"prefixes":[{"ip_prefix":"3.12.251.153/32"},{"ipv6_prefix":"2600::/32"}]}"#;
        let nets = parse("uptimerobot", body).unwrap();
        assert_eq!(nets.len(), 2);
    }

    #[test]
    fn ahrefs_parser() {
        let body = br#"{"ips":[{"ip_address":"5.39.1.224"},{"ip_address":"not-an-address"}]}"#;
        let nets = parse("ahrefs", body).unwrap();
        assert_eq!(nets.len(), 1);
    }

    /// A body that is not the document of the vendor is not a document at all.
    #[test]
    fn html_body_is_not_a_document() {
        let error = parse("google", b"<html><body>error</body></html>").unwrap_err();
        assert!(matches!(error, ParseError::NotADocument(_)));
    }

    /// An empty body is not a document either.
    #[test]
    fn empty_body_is_not_a_document() {
        let error = parse("google", b"").unwrap_err();
        assert!(matches!(error, ParseError::NotADocument(_)));
    }

    /// A document without the expected list changed its shape.
    #[test]
    fn document_without_the_list_changed_its_shape() {
        let error = parse("google", br#"{"creationTime":"2026-01-01"}"#).unwrap_err();
        assert!(matches!(error, ParseError::UnexpectedShape(_)));
    }

    /// One entry that holds no usable prefix does not discard the others.
    #[test]
    fn a_broken_entry_does_not_discard_the_document() {
        let body = br#"{"prefixes":[{"ipv4Prefix":"1.2.3.0/24"},{"ipv4Prefix":"not-a-cidr"},{"ipv6Prefix":"2001:db8::/32"}]}"#;
        let nets = parse("google", body).unwrap();
        assert_eq!(nets.len(), 2);
        assert!(nets[0].contains(&"1.2.3.1".parse::<IpAddr>().unwrap()));
        assert!(nets[1].contains(&"2001:db8::1".parse::<IpAddr>().unwrap()));
    }

    /// An entry that is not an object at all is skipped as well.
    #[test]
    fn an_entry_that_is_not_an_object_is_skipped() {
        let body = br#"{"prefixes":["1.2.3.0/24",{"ipv4Prefix":"5.6.7.0/24"}]}"#;
        let nets = parse("google", body).unwrap();
        assert_eq!(nets.len(), 1);
    }

    /// A body that holds no address is not the list.
    #[test]
    fn a_text_body_without_an_address_is_not_a_document() {
        let error = parse("txt", b"<html><body>error</body></html>").unwrap_err();
        assert!(matches!(error, ParseError::NotADocument(_)));
    }

    /// The page of Amazon carries the document; the fallback reads the literals.
    #[test]
    fn amazon_parser_reads_the_document_of_the_page() {
        let body =
            br#"<html><script>{"prefixes":[{"ipv4Prefix":"3.81.245.0/24"}]}</script></html>"#;
        let nets = parse("amazon", body).unwrap();
        assert_eq!(nets.len(), 1);
    }

    #[test]
    fn amazon_parser_falls_back_to_the_literals_of_the_page() {
        let body = b"<html>the addresses are 3.81.245.78 and 3.81.245.78</html>";
        let nets = parse("amazon", body).unwrap();
        assert_eq!(nets.len(), 1);
    }
}
