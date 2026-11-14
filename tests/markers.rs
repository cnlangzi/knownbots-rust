//! The markers of the registry, held to the User-Agents the vendors publish.
//!
//! A marker is the token a request has to carry in its `User-Agent` for the
//! crate to recognise the crawler. It is matched byte-exact on a word
//! boundary, so a marker that no request carries matches nothing: the crawler
//! never verifies, its traffic never reaches the log of the host, and nothing
//! reports the defect.
//!
//! `published-uas.yaml` holds the User-Agent every vendor publishes for a
//! crawler of the registry, with the page of the vendor that publishes it. The
//! guard below holds the two in step: the table and the registry name the same
//! crawlers, the marker of every crawler is a word of the published
//! User-Agent of its crawler, and every published User-Agent is attributed to
//! the crawler that publishes it.
//!
//! The table is a snapshot of the documentation of the vendors, and it reaches
//! no network: this guard cannot notice that a vendor has renamed its token,
//! and neither can the provider-health workflow, which reads the sources of
//! the registry and not the User-Agents of its crawlers.

use std::collections::BTreeMap;
use std::path::Path;

use knownbots::{contains_word, find_bot_by_ua, load_bots};
use serde::Deserialize;

/// One entry of the table: the page of the vendor and the User-Agents it publishes.
#[derive(Debug, Deserialize)]
struct Published {
    /// The page of the vendor that publishes the User-Agents.
    url: String,
    /// The User-Agents the vendor publishes for the crawler.
    ua: Vec<String>,
}

/// The table of the published User-Agents.
fn table() -> BTreeMap<String, Published> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/published-uas.yaml");
    let data = std::fs::read_to_string(&path)
        .expect("the table of the published User-Agents must be readable");
    serde_yaml::from_str(&data).expect("the table of the published User-Agents must parse")
}

/// The markers of the registry are the tokens the vendors publish.
///
/// The rule can be broken **silently**: a marker that no request carries
/// matches nothing, and the crawler it names sits in the registry advertising a
/// traffic it can never recognise. Every violation is collected, so that one
/// run reports every problem instead of only the first.
#[test]
fn the_marker_of_every_crawler_is_a_word_of_the_user_agent_its_vendor_publishes() {
    let bots = load_bots(None).expect("the embedded registry must load");
    let published = table();
    let mut violations: Vec<String> = Vec::new();

    // The table and the registry name the same crawlers.
    for bot in &bots {
        if !published.contains_key(&bot.name) {
            violations.push(format!(
                "{} has no published User-Agent in the table",
                bot.name
            ));
        }
    }
    for name in published.keys() {
        if !bots.iter().any(|bot| &bot.name == name) {
            violations.push(format!(
                "the table names {name}, which the registry does not know"
            ));
        }
    }

    for bot in &bots {
        let Some(entry) = published.get(&bot.name) else {
            continue;
        };
        if entry.url.is_empty() {
            violations.push(format!("{} has no page of a vendor in the table", bot.name));
        } else if !entry.url.starts_with("https://") {
            violations.push(format!(
                "{} names no page of a vendor it can be checked against: {}",
                bot.name, entry.url
            ));
        }
        if entry.ua.is_empty() {
            violations.push(format!(
                "{} has no published User-Agent in the table",
                bot.name
            ));
        }
        for ua in &entry.ua {
            // The marker of the crawler is a word of the published User-Agent.
            if !contains_word(ua, &bot.ua) {
                violations.push(format!(
                    "the marker {:?} of {} is not a word of the published User-Agent {ua:?}",
                    bot.ua, bot.name
                ));
                continue;
            }
            // The published User-Agent belongs to the crawler that publishes it.
            match find_bot_by_ua(ua, &bots) {
                Some(position) if bots[position].name == bot.name => {}
                Some(position) => violations.push(format!(
                    "the published User-Agent {ua:?} of {} is attributed to {}",
                    bot.name, bots[position].name
                )),
                None => violations.push(format!(
                    "the published User-Agent {ua:?} of {} matches no crawler",
                    bot.name
                )),
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the markers of the registry break the tokens the vendors publish ({} problem(s)):\n  - {}",
        violations.len(),
        violations.join("\n  - "),
    );
}
