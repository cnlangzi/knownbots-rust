//! The verification model: the marker of the request and the address of the
//! request both have to hold.

use std::net::IpAddr;
use std::sync::Arc;

use knownbots::{Validator, ValidatorOptions, VerifyStatus};

/// A validator with an empty cache.
fn validator() -> (tempfile::TempDir, Arc<Validator>) {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        ..ValidatorOptions::default()
    })
    .expect("a validator");
    (directory, validator)
}

fn ip(address: &str) -> IpAddr {
    address.parse().expect("an address")
}

#[test]
fn a_claim_that_names_a_known_crawler_verifies_with_its_address() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("googlebot", "66.249.64.0/19")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        ip("66.249.66.1"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "googlebot");
    assert_eq!(result.vendor, "Google");
}

#[test]
fn a_claim_without_the_address_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("googlebot", "66.249.64.0/19")
        .unwrap();
    let result = validator.verify("Mozilla/5.0 (compatible; Googlebot/2.1)", ip("203.0.113.7"));
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}

#[test]
fn a_claim_that_names_no_crawler_is_unknown() {
    let (_directory, validator) = validator();
    let result = validator.verify("Mozilla/5.0 Chrome/120.0", ip("203.0.113.7"));
    assert_eq!(result.status, VerifyStatus::Unknown);
    assert_eq!(result.name, "");
}

/// The official markers are published in a fixed casing. A forgery that only
/// changes the case must not verify.
#[test]
fn the_marker_is_case_sensitive() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("googlebot", "66.249.64.0/19")
        .unwrap();
    let result = validator.verify("Mozilla/5.0 (compatible; googlebot/2.1)", ip("66.249.66.1"));
    assert_eq!(result.status, VerifyStatus::Unknown);
}

/// The marker has to sit on a word boundary.
#[test]
fn the_marker_has_to_sit_on_a_word_boundary() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("googlebot", "66.249.64.0/19")
        .unwrap();
    let result = validator.verify("MyGooglebot/1.0", ip("66.249.66.1"));
    assert_eq!(result.status, VerifyStatus::Unknown);
}

/// Several markers can match one request; the longest one wins.
#[test]
fn the_longest_marker_wins() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("adsbot-mobile", "66.249.64.0/19")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)",
        ip("66.249.66.1"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "adsbot-mobile");
}

/// The shorter marker of the pair must not claim the request either.
#[test]
fn the_shorter_marker_does_not_claim_the_request() {
    let (_directory, validator) = validator();
    validator.seed_prefix("adsbot", "66.249.64.0/19").unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)",
        ip("66.249.66.1"),
    );
    assert!(!result.is_verified());
    assert_eq!(result.status, VerifyStatus::Failed);
}

#[test]
fn a_crawler_that_is_verified_by_its_reverse_dns_verifies_with_its_hostname() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("baiduspider", "220.181.108.94", "crawl.baidu.com")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Baiduspider/2.0; +http://www.baidu.com/search/spider.html)",
        ip("220.181.108.94"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "baiduspider");
}

#[test]
fn a_hostname_outside_the_domains_of_the_crawler_does_not_verify() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("baiduspider", "220.181.108.94", "crawl.example.com")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Baiduspider/2.0)",
        ip("220.181.108.94"),
    );
    assert!(!result.is_verified());
    assert_eq!(result.status, VerifyStatus::Failed);
}

/// A cold cache never verifies the request that triggered the lookup.
#[test]
fn a_cold_cache_does_not_verify_the_request() {
    let (_directory, validator) = validator();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Baiduspider/2.0)",
        ip("220.181.108.94"),
    );
    assert!(!result.is_verified());
    #[cfg(feature = "full")]
    assert_eq!(result.status, VerifyStatus::Pending);
    #[cfg(not(feature = "full"))]
    assert_eq!(result.status, VerifyStatus::Failed);
}

/// The prefixes of the definition of a crawler are its address space.
#[test]
fn the_prefixes_of_the_definition_verify_the_request() {
    let (_directory, validator) = validator();
    let result = validator.verify(
        "facebookexternalhit/1.1 (+http://www.facebook.com/externalhit_uatext.php)",
        ip("31.13.24.10"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "facebookexternalhit");
}

/// The marker of the crawler of OpenAI sits inside the marker of the crawler of
/// Google, so the longest marker has to claim the request.
#[test]
fn the_adsbot_of_openai_is_not_the_adsbot_of_google() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("oai-adsbot", "130.131.229.128/25")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; OAI-AdsBot/1.0; +https://openai.com/adsbot",
        ip("130.131.229.129"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "oai-adsbot");
    assert_eq!(result.vendor, "OpenAI");
}

/// The marker of the crawler of Google is not the marker of the crawler of
/// OpenAI.
#[test]
fn the_adsbot_of_google_stays_the_adsbot_of_google() {
    let (_directory, validator) = validator();
    validator.seed_prefix("adsbot", "66.249.64.0/19").unwrap();
    let result = validator.verify(
        "AdsBot-Google (+http://www.google.com/adsbot.html)",
        ip("66.249.66.1"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "adsbot");
}

/// The crawler that fetches the pages a user asks ChatGPT about is verified by
/// the address space the vendor publishes.
#[test]
fn the_chatgpt_user_is_verified_by_the_address_space_openai_publishes() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("chatgpt-user", "104.208.184.192/28")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; ChatGPT-User/1.0; +https://openai.com/bot",
        ip("104.208.184.193"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "chatgpt-user");
}

/// An address outside the published address space is not the crawler, even when
/// the request names it.
#[test]
fn a_chatgpt_user_claim_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("chatgpt-user", "104.208.184.192/28")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; ChatGPT-User/1.0; +https://openai.com/bot",
        ip("203.0.113.7"),
    );
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}

/// The crawler that improves the search results of Claude is verified by the
/// address space Anthropic publishes.
#[test]
fn the_claude_searchbot_is_verified_by_the_address_space_anthropic_publishes() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("claude-searchbot", "216.73.216.0/22")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Claude-SearchBot/1.0; +https://claude.com/bots)",
        ip("216.73.216.5"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "claude-searchbot");
    assert_eq!(result.vendor, "Anthropic");
}

/// The crawler that fetches the pages a user asks Claude about is verified by
/// the address space Anthropic publishes.
#[test]
fn the_claude_user_is_verified_by_the_address_space_anthropic_publishes() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("claude-user", "34.162.191.81/32")
        .unwrap();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Claude-User/1.0; +https://claude.com/bots)",
        ip("34.162.191.81"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "claude-user");
    assert_eq!(result.vendor, "Anthropic");
}

/// Each crawler of Anthropic holds its own address space: a prefix of one does
/// not make a request that names another the crawler it claims to be.
///
/// The three crawlers download the same document, so they carry the same
/// prefixes — but the marker decides which crawler a request claims to be, and
/// the address decides whether it may claim it.
#[test]
fn the_crawlers_of_anthropic_hold_their_own_address_space() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("claude-searchbot", "216.73.216.0/22")
        .unwrap();
    validator
        .seed_prefix("claude-user", "34.162.191.81/32")
        .unwrap();

    let training = validator.verify(
        "Mozilla/5.0 (compatible; ClaudeBot/1.0; +https://claude.com/bots)",
        ip("216.73.216.5"),
    );
    assert!(!training.is_verified());
    assert_eq!(training.status, VerifyStatus::Failed);

    let search = validator.verify(
        "Mozilla/5.0 (compatible; Claude-SearchBot/1.0; +https://claude.com/bots)",
        ip("34.162.191.81"),
    );
    assert!(!search.is_verified());
    assert_eq!(search.status, VerifyStatus::Failed);

    let user = validator.verify(
        "Mozilla/5.0 (compatible; Claude-User/1.0; +https://claude.com/bots)",
        ip("216.73.216.5"),
    );
    assert!(!user.is_verified());
    assert_eq!(user.status, VerifyStatus::Failed);
}
