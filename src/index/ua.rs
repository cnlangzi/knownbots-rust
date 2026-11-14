//! The matching of a `User-Agent` against the markers of the registry.
//!
//! Official bots publish a fixed casing (`Googlebot`, never `googlebot`). Case
//! folding would accept forgeries that merely change case, so matching is
//! byte-exact.
//!
//! A match must also sit on a word boundary — alphanumeric on either side
//! disqualifies (`MyGooglebot` does not match `Googlebot`). A hyphen is a
//! boundary, so `AdsBot-Google-Mobile` contains the word `AdsBot`. When
//! several markers match, the **longest** wins so that the more specific bot
//! is preferred.
//!
//! Matching is a linear scan over the table of the registry (~59 entries). That
//! beats a per-byte index once the longest match forces a full pass anyway, and
//! keeps the request path branch-predictable.

use crate::model::bot::Bot;

/// True when `c` is ASCII alphanumeric.
#[inline]
fn is_alnum(c: u8) -> bool {
    c.is_ascii_alphanumeric()
}

/// Case-sensitive word-boundary search for `word` inside `text`.
pub fn contains_word(text: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let text_bytes = text.as_bytes();
    let word_bytes = word.as_bytes();
    let mut index = 0;
    while index + word_bytes.len() <= text_bytes.len() {
        if let Some(offset) = find_slice(&text_bytes[index..], word_bytes) {
            let position = index + offset;
            let before_ok = position == 0 || !is_alnum(text_bytes[position - 1]);
            let after_ok = position + word_bytes.len() == text_bytes.len()
                || !is_alnum(text_bytes[position + word_bytes.len()]);
            if before_ok && after_ok {
                return true;
            }
            index = position + 1;
        } else {
            break;
        }
    }
    false
}

fn find_slice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Finds the bot whose marker matches `ua`, preferring the longest marker when
/// several match.
///
/// The answer is the position of the bot in `bots`.
pub fn find_bot_by_ua(ua: &str, bots: &[Bot]) -> Option<usize> {
    if ua.is_empty() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None; // (length, position)
    for (position, bot) in bots.iter().enumerate() {
        if bot.ua.is_empty() {
            continue;
        }
        if contains_word(ua, &bot.ua) {
            let length = bot.ua.len();
            match best {
                Some((best_length, _)) if best_length >= length => {}
                _ => best = Some((length, position)),
            }
        }
    }
    best.map(|(_, position)| position)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::bot::{Bot, BotKind};
    use parking_lot::RwLock;

    fn bot(name: &str, ua: &str) -> Bot {
        Bot {
            name: name.into(),
            kind: BotKind::SearchEngine,
            parser: "txt".into(),
            ua: ua.into(),
            vendor: "Test".into(),
            urls: Vec::new(),
            custom: Vec::new(),
            domains: Vec::new(),
            rdns: false,
            prefixes: RwLock::new(Vec::new()),
            rdns_cache: None,
            fail_cache: None,
            loaded_at: RwLock::new(None),
        }
    }

    #[test]
    fn contains_word_exact() {
        assert!(contains_word(
            "Mozilla/5.0 (compatible; Googlebot/2.1)",
            "Googlebot"
        ));
    }

    #[test]
    fn contains_word_rejects_wrong_case() {
        assert!(!contains_word(
            "Mozilla/5.0 (compatible; googlebot/2.1)",
            "Googlebot"
        ));
    }

    #[test]
    fn contains_word_rejects_alphanumeric_prefix() {
        assert!(!contains_word("MyGooglebot/1.0", "Googlebot"));
    }

    #[test]
    fn contains_word_hyphen_is_boundary() {
        assert!(contains_word(
            "Mozilla/5.0 (compatible; AdsBot-Google-Mobile)",
            "AdsBot"
        ));
        assert!(contains_word(
            "Mozilla/5.0 (compatible; AdsBot-Google-Mobile)",
            "AdsBot-Google-Mobile"
        ));
    }

    #[test]
    fn longest_marker_wins() {
        let bots = vec![
            bot("adsbot", "AdsBot"),
            bot("adsbot-mobile", "AdsBot-Google-Mobile"),
        ];
        let ua = "Mozilla/5.0 (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)";
        let position = find_bot_by_ua(ua, &bots).expect("a match");
        assert_eq!(bots[position].name, "adsbot-mobile");
    }

    #[test]
    fn inspection_tool_not_googlebot() {
        let bots = vec![
            bot("googlebot", "Googlebot"),
            bot("google-inspectiontool", "Google-InspectionTool"),
        ];
        let ua = "Mozilla/5.0 (compatible; Google-InspectionTool/1.0)";
        let position = find_bot_by_ua(ua, &bots).expect("a match");
        assert_eq!(bots[position].name, "google-inspectiontool");
    }
}
