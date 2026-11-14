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

/// The User-Agent Mistral publishes for the crawler that fetches the pages a
/// user asks about.
const MISTRAL_USER_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; MistralAI-User/1.0; +https://docs.mistral.ai/robots)";

/// The User-Agent Mistral publishes for the crawler that indexes the web.
const MISTRAL_INDEX_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko; compatible; MistralAI-Index/1.0; +https://docs.mistral.ai/robots)";

/// The User-Agent Common Crawl publishes for the crawler that fetches the web.
const CCBOT_UA: &str = "CCBot/2.0 (https://commoncrawl.org/faq/)";

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

/// The three crawlers of Anthropic share one published address space.
///
/// The vendor publishes one document for ClaudeBot, Claude-SearchBot and
/// Claude-User, and every crawler that names it downloads it for itself — so
/// they hold the same prefixes. The marker decides which crawler a request
/// claims to be, and the address decides whether it may claim it.
#[test]
fn the_crawlers_of_anthropic_share_the_published_address_space() {
    let (_directory, validator) = validator();
    for bot in ["claudebot", "claude-searchbot", "claude-user"] {
        validator.seed_prefix(bot, "216.73.216.0/22").unwrap();
    }

    let training = validator.verify(
        "Mozilla/5.0 (compatible; ClaudeBot/1.0)",
        ip("216.73.216.5"),
    );
    assert!(training.is_verified());
    assert_eq!(training.name, "claudebot");
    assert_eq!(training.vendor, "Anthropic");

    let search = validator.verify(
        "Mozilla/5.0 (compatible; Claude-SearchBot/1.0)",
        ip("216.73.216.5"),
    );
    assert!(search.is_verified());
    assert_eq!(search.name, "claude-searchbot");
    assert_eq!(search.vendor, "Anthropic");

    let user = validator.verify(
        "Mozilla/5.0 (compatible; Claude-User/1.0)",
        ip("216.73.216.5"),
    );
    assert!(user.is_verified());
    assert_eq!(user.name, "claude-user");
    assert_eq!(user.vendor, "Anthropic");
}

/// A request is verified against the address space of the crawler its marker
/// names, not against the address space of another crawler of the same vendor.
///
/// The prefix is seeded into one crawler alone, so the request that names
/// another one has nothing to be verified against — and fails closed.
#[test]
fn a_request_is_verified_against_the_address_space_of_the_crawler_its_marker_names() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("claude-searchbot", "216.73.216.0/22")
        .unwrap();

    let search = validator.verify(
        "Mozilla/5.0 (compatible; Claude-SearchBot/1.0)",
        ip("216.73.216.5"),
    );
    assert!(search.is_verified());

    let training = validator.verify(
        "Mozilla/5.0 (compatible; ClaudeBot/1.0)",
        ip("216.73.216.5"),
    );
    assert!(!training.is_verified());
    assert_eq!(training.status, VerifyStatus::Failed);
}

/// An address outside the published address space is not a crawler of
/// Anthropic, even when the request names one.
#[test]
fn a_claude_claim_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    for bot in ["claudebot", "claude-searchbot", "claude-user"] {
        validator.seed_prefix(bot, "216.73.216.0/22").unwrap();
    }

    for ua in [
        "Mozilla/5.0 (compatible; ClaudeBot/1.0)",
        "Mozilla/5.0 (compatible; Claude-SearchBot/1.0)",
        "Mozilla/5.0 (compatible; Claude-User/1.0)",
    ] {
        let result = validator.verify(ua, ip("203.0.113.7"));
        assert_eq!(result.status, VerifyStatus::Failed);
        assert!(!result.is_verified());
    }
}

/// The two crawlers of Mistral are verified by the address space the vendor
/// publishes for them.
///
/// The vendor publishes a document of its own for each of the two crawlers, so
/// a request is verified against the address space of the crawler its marker
/// names.
#[test]
fn the_crawlers_of_mistral_are_verified_by_the_address_space_mistral_publishes() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("mistralai-user", "20.240.160.161/32")
        .unwrap();
    validator
        .seed_prefix("mistralai-index", "135.225.57.92/32")
        .unwrap();

    let user = validator.verify(MISTRAL_USER_UA, ip("20.240.160.161"));
    assert!(user.is_verified());
    assert_eq!(user.name, "mistralai-user");
    assert_eq!(user.vendor, "Mistral");

    let index = validator.verify(MISTRAL_INDEX_UA, ip("135.225.57.92"));
    assert!(index.is_verified());
    assert_eq!(index.name, "mistralai-index");
    assert_eq!(index.vendor, "Mistral");
}

/// The address of one crawler of Mistral does not make the request a claim of
/// the other one.
///
/// The vendor publishes a document of its own for each of the two crawlers, so
/// the address space of the one is not the address space of the other. A
/// request that names one crawler and comes from the address of the other
/// claims a crawler it is not.
#[test]
fn the_crawlers_of_mistral_do_not_share_their_address_space() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("mistralai-user", "20.240.160.161/32")
        .unwrap();
    validator
        .seed_prefix("mistralai-index", "135.225.57.92/32")
        .unwrap();

    let index = validator.verify(MISTRAL_INDEX_UA, ip("20.240.160.161"));
    assert_eq!(index.status, VerifyStatus::Failed);
    assert!(!index.is_verified());

    let user = validator.verify(MISTRAL_USER_UA, ip("135.225.57.92"));
    assert_eq!(user.status, VerifyStatus::Failed);
    assert!(!user.is_verified());
}

/// An address outside the published address space is not a crawler of Mistral,
/// even when the request names one.
#[test]
fn a_mistral_claim_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("mistralai-user", "20.240.160.161/32")
        .unwrap();
    validator
        .seed_prefix("mistralai-index", "135.225.57.92/32")
        .unwrap();

    for ua in [MISTRAL_USER_UA, MISTRAL_INDEX_UA] {
        let result = validator.verify(ua, ip("203.0.113.7"));
        assert_eq!(result.status, VerifyStatus::Failed);
        assert!(!result.is_verified());
    }
}

/// The crawler of Common Crawl is verified by the address space the vendor
/// publishes and by the reverse DNS of its vendor.
///
/// The two paths are complementary: the published list is the only path that
/// covers the IPv6 addresses of the crawler — the vendor keeps no reverse
/// record for them — and the vendor recommends the reverse DNS in addition to
/// the list for the IPv4 addresses. A definition that drops one of them drops a
/// path the vendor offers.
#[test]
fn the_crawler_of_common_crawl_is_verified_by_both_of_its_paths() {
    let bots = knownbots::load_bots(None).expect("the embedded registry must load");
    let ccbot = bots
        .iter()
        .find(|bot| bot.name == "ccbot")
        .expect("ccbot in the registry");
    assert_eq!(
        ccbot.urls,
        ["https://index.commoncrawl.org/ccbot.json"],
        "the definition names no published address space"
    );
    assert!(ccbot.rdns, "the definition names no reverse DNS");
    assert_eq!(ccbot.domains, ["commoncrawl.org"]);

    let (_directory, ipv4) = validator();
    ipv4.seed_prefix("ccbot", "3.41.188.32/29").unwrap();
    let published = ipv4.verify(CCBOT_UA, ip("3.41.188.33"));
    assert!(published.is_verified());
    assert_eq!(published.name, "ccbot");

    // The vendor keeps no reverse record for the IPv6 addresses of its
    // crawler, so the published list is the only path that covers them.
    let (_directory, ipv6) = validator();
    ipv6.seed_prefix("ccbot", "2600:1f28:365:8000::/56")
        .unwrap();
    let published = ipv6.verify(CCBOT_UA, ip("2600:1f28:365:8000::1"));
    assert!(published.is_verified());
    assert_eq!(published.name, "ccbot");

    // The reverse DNS confirms a request from an address the published address
    // space does not cover.
    let (_directory, reverse) = validator();
    reverse
        .seed_rdns("ccbot", "203.0.113.7", "crawl.commoncrawl.org")
        .unwrap();
    let result = reverse.verify(CCBOT_UA, ip("203.0.113.7"));
    assert!(result.is_verified());
    assert_eq!(result.name, "ccbot");
    assert_eq!(result.vendor, "CommonCrawl");
}

/// A hostname outside the domains of the crawler does not confirm the request,
/// even when the crawler carries a published address space as well.
#[test]
fn a_crawl_of_common_crawl_outside_its_domains_does_not_verify() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("ccbot", "203.0.113.7", "crawl.example.com")
        .unwrap();
    let result = validator.verify(CCBOT_UA, ip("203.0.113.7"));
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}
