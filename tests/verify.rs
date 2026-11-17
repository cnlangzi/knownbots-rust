//! The verification model: the marker of the request and the address of the
//! request both have to hold.

#[path = "common/dns.rs"]
mod dns;

use std::net::IpAddr;
use std::sync::Arc;

use dns::DNS_CRAWLERS;
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

/// The User-Agent DuckDuckGo publishes for the crawler that fetches the pages
/// of its AI-assisted answers.
const DUCK_ASSIST_UA: &str = "DuckAssistBot/1.2; (+http://duckduckgo.com/duckassistbot.html)";

/// The User-Agent DuckDuckGo publishes for the crawler that improves its
/// search index.
const DUCK_DUCK_UA: &str = "DuckDuckBot/1.1; (+http://duckduckgo.com/duckduckbot.html)";

/// A User-Agent that carries the marker the vendor publishes.
///
/// The vendor publishes no User-Agent header of its own — it names the crawler
/// and the token `MojeekBot` — so the string below carries that token.
const MOJEEK_UA: &str = "Mozilla/5.0 (compatible; MojeekBot; +https://www.mojeek.com/bot.html)";

/// The User-Agent Semrush publishes for the crawler of its On Page SEO Checker.
const SEMRUSH_SI_UA: &str =
    "Mozilla/5.0 (compatible; SemrushBot-SI/0.97; +http://www.semrush.com/bot.html)";

/// The User-Agent Semrush publishes for its main crawler.
const SEMRUSH_UA: &str =
    "Mozilla/5.0 (compatible; SemrushBot/7~bl; +http://www.semrush.com/bot.html)";

/// The User-Agent Moonshot publishes for the crawler that trains the models of
/// Kimi.
const KIMI_BOT_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; KimiBot/1.0; +https://www.kimi.com/policies/kimi-crawlers";

/// The User-Agent Moonshot publishes for the crawler that builds the search
/// index of Kimi.
const KIMI_SEARCHBOT_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; Kimi-SearchBot/1.0; +https://www.kimi.com/policies/kimi-crawlers";

/// The User-Agent Moonshot publishes for the crawler that fetches the pages a
/// user asks Kimi about.
const KIMI_USER_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; Kimi-User/1.0; +https://www.kimi.com/policies/kimi-crawlers";

/// The User-Agent Tencent publishes for the crawler that trains the models of
/// Hy.
const HYSPIDER_UA: &str = "Mozilla/5.0 AppleWebKit/537.36 (KHTML, like Gecko); compatible; HySpider/1.0; +https://hy.tencent.com/polices/hy-crawlers";

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

/// The crawler of ByteDance is verified by the address space the vendor
/// publishes.
///
/// The vendor publishes the networks of the crawler and keeps the reverse names
/// of its addresses, but the names never confirm forward — the crate cannot
/// verify the crawler by them, so the request is verified by the networks
/// instead.
#[test]
fn the_crawler_of_bytedance_is_verified_by_the_address_space_it_publishes() {
    let (_directory, validator) = validator();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Bytespider; https://zhanzhang.toutiao.com/)",
        ip("110.249.201.10"),
    );
    assert!(result.is_verified());
    assert_eq!(result.name, "bytespider");
    assert_eq!(result.vendor, "ByteDance");
}

/// An address outside the published address space is not the crawler, even when
/// the request names it.
///
/// The crawler carries no reverse DNS, so the request fails closed at once: no
/// reverse lookup is queued for it, and the answer is `Failed` and never
/// `Pending`.
#[test]
fn a_bytespider_claim_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    let result = validator.verify(
        "Mozilla/5.0 (compatible; Bytespider; https://zhanzhang.toutiao.com/)",
        ip("203.0.113.7"),
    );
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
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

/// The crawler of DuckDuckGo that fetches the pages of the AI-assisted answers
/// is verified by the address space the vendor publishes for it.
///
/// The vendor publishes a document of its own for the crawler, so a request is
/// verified against the address space of the crawler its marker names.
#[test]
fn the_crawler_of_duckduck_go_is_verified_by_the_address_space_it_publishes() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("duckassistbot", "104.43.54.127/32")
        .unwrap();

    let result = validator.verify(DUCK_ASSIST_UA, ip("104.43.54.127"));
    assert!(result.is_verified());
    assert_eq!(result.name, "duckassistbot");
    assert_eq!(result.vendor, "DuckDuckGo");
}

/// An address outside the published address space is not a crawler of
/// DuckDuckGo, even when the request names one.
#[test]
fn a_duck_assist_claim_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("duckassistbot", "104.43.54.127/32")
        .unwrap();

    let result = validator.verify(DUCK_ASSIST_UA, ip("203.0.113.7"));
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}

/// The two crawlers of DuckDuckGo are told apart by their markers.
///
/// The vendor publishes a document for each of the two crawlers, and the
/// marker of one never claims the request of the other.
#[test]
fn the_two_crawlers_of_duckduck_go_are_told_apart_by_their_markers() {
    let (_directory, validator) = validator();
    validator
        .seed_prefix("duckassistbot", "104.43.54.127/32")
        .unwrap();
    validator
        .seed_prefix("duckduckbot", "104.43.54.127/32")
        .unwrap();

    let assist = validator.verify(DUCK_ASSIST_UA, ip("104.43.54.127"));
    assert_eq!(assist.name, "duckassistbot");

    let search = validator.verify(DUCK_DUCK_UA, ip("104.43.54.127"));
    assert_eq!(search.name, "duckduckbot");
}

/// The crawler of Mojeek is backed by the list the vendor publishes and by its
/// reverse DNS: the definition names both, and each path confirms a request.
#[test]
fn the_mojeek_crawler_is_verified_by_its_list_and_by_its_reverse_dns() {
    let (_directory, validator) = validator();

    let bot = validator
        .bots()
        .iter()
        .find(|bot| bot.name == "mojeekbot")
        .expect("mojeekbot in the registry");
    assert!(!bot.urls.is_empty(), "the crawler names no published list");
    assert!(bot.rdns, "the crawler carries no reverse DNS");

    // The address space the vendor publishes, seeded.
    validator
        .seed_prefix("mojeekbot", "5.102.173.64/28")
        .unwrap();
    let result = validator.verify(MOJEEK_UA, ip("5.102.173.71"));
    assert!(result.is_verified());
    assert_eq!(result.name, "mojeekbot");

    // The reverse DNS confirms a request from an address the published address
    // space does not cover.
    validator
        .seed_rdns("mojeekbot", "203.0.113.7", "crawl-203-0-113-7.mojeek.com")
        .unwrap();
    let result = validator.verify(MOJEEK_UA, ip("203.0.113.7"));
    assert!(result.is_verified());
    assert_eq!(result.name, "mojeekbot");
}

/// The crawlers of Semrush are told apart by their markers.
///
/// The marker of the crawler of the On Page SEO Checker holds the marker of the
/// main crawler of the vendor, so the longest marker has to claim the request —
/// and the main crawler keeps the requests that carry its own marker.
#[test]
fn the_crawlers_of_semrush_are_told_apart_by_their_markers() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("semrushbot-si", "85.208.98.53", "2.si.bot.semrush.com")
        .unwrap();
    validator
        .seed_rdns("semrushbot", "85.208.98.25", "1.lbt.bot.semrush.com")
        .unwrap();

    let si = validator.verify(SEMRUSH_SI_UA, ip("85.208.98.53"));
    assert!(si.is_verified());
    assert_eq!(si.name, "semrushbot-si");
    assert_eq!(si.vendor, "Semrush");

    let main = validator.verify(SEMRUSH_UA, ip("85.208.98.25"));
    assert!(main.is_verified());
    assert_eq!(main.name, "semrushbot");
    assert_eq!(main.vendor, "Semrush");
}

/// The six crawlers of Semrush share one address space.
///
/// The vendor keeps the reverse DNS of all of its crawlers under
/// `semrush.com`, so the address space of a crawler of Semrush is the address
/// space of the vendor: the address decides whether a request may claim a
/// crawler of Semrush at all, and the marker decides which one it claims to be.
#[test]
fn the_crawlers_of_semrush_share_the_address_space_of_their_vendor() {
    let (_directory, validator) = validator();
    for bot in [
        "semrushbot",
        "semrushbot-ba",
        "semrushbot-si",
        "semrushbot-swa",
        "siteauditbot",
        "splitsignalbot",
    ] {
        validator
            .seed_rdns(bot, "85.208.98.25", "1.lbt.bot.semrush.com")
            .unwrap();
    }

    // The one address confirms a request that names any of the crawlers of the
    // vendor.
    for (name, marker) in [
        ("semrushbot", "SemrushBot"),
        ("semrushbot-ba", "SemrushBot-BA"),
        ("semrushbot-si", "SemrushBot-SI"),
        ("semrushbot-swa", "SemrushBot-SWA"),
        ("siteauditbot", "SiteAuditBot"),
        ("splitsignalbot", "SplitSignalBot"),
    ] {
        let ua =
            format!("Mozilla/5.0 (compatible; {marker}/1.0; +http://www.semrush.com/bot.html)");
        let result = validator.verify(&ua, ip("85.208.98.25"));
        assert!(result.is_verified());
        assert_eq!(result.name, name);
    }
}

/// A hostname outside the domains of the vendor does not confirm a Semrush
/// crawler, even when the request names one.
#[test]
fn a_semrush_claim_from_an_address_outside_the_domains_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("semrushbot-si", "85.208.98.53", "2.si.bot.example.com")
        .unwrap();

    let result = validator.verify(SEMRUSH_SI_UA, ip("85.208.98.53"));
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}

/// The registry carries no crawler for the tokens of the crawlers of Semrush
/// whose traffic it can never verify.
///
/// The vendor publishes no address space for them and names no host of its own
/// for their product, so a request that carries one of their tokens is never
/// verified as the crawler it names.
///
/// `RyteBot` names no crawler of the registry, so the request is an ordinary
/// request. The three tokens that begin with `SemrushBot` are words of the
/// token of the main crawler — a hyphen is a boundary, so `SemrushBot-ESI`
/// carries the word `SemrushBot` — so the main crawler claims the request and
/// the address decides: a request from an address of the vendor is verified as
/// the main crawler, and a request from any other address fails closed.
#[test]
fn a_claim_of_a_crawler_of_semrush_the_registry_does_not_carry_is_never_verified_as_it() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("semrushbot", "85.208.98.25", "1.lbt.bot.semrush.com")
        .unwrap();
    validator
        .seed_rdns("semrushbot", "203.0.113.7", "203.0.113.7.example.com")
        .unwrap();

    let ryte = validator.verify("Mozilla/5.0 (compatible; RyteBot/1.0)", ip("203.0.113.7"));
    assert_eq!(ryte.status, VerifyStatus::Unknown);
    assert_eq!(ryte.name, "");

    for token in ["SemrushBot-ESI", "SemrushBot-FT", "SemrushBot-OCOB"] {
        let ua = format!("Mozilla/5.0 (compatible; {token}/1.0; +http://www.semrush.com/bot.html)");

        let outside = validator.verify(&ua, ip("203.0.113.7"));
        assert_eq!(outside.status, VerifyStatus::Failed);
        assert_eq!(outside.name, "");

        let inside = validator.verify(&ua, ip("85.208.98.25"));
        assert_eq!(inside.name, "semrushbot");
    }
}

/// The crawlers that are known only by their reverse DNS are verified by the
/// reverse names of their vendors.
///
/// Each of them names a domain the vendor owns, and an address whose reverse
/// name ends in that domain is the address of the crawler.
#[test]
fn the_crawlers_of_the_reverse_dns_are_verified_by_their_reverse_names() {
    let (_directory, validator) = validator();

    for crawler in DNS_CRAWLERS {
        validator
            .seed_rdns(crawler.name, crawler.sample, crawler.hostname)
            .unwrap();

        assert!(
            crawler.sample_is_under_a_domain(),
            "the reverse name of the sample of {} is under none of its domains",
            crawler.name
        );

        let ua = format!("Mozilla/5.0 (compatible; {}/1.0)", crawler.marker);
        let result = validator.verify(&ua, ip(crawler.sample));
        assert!(result.is_verified(), "{} was not verified", crawler.name);
        assert_eq!(result.name, crawler.name);
        assert_eq!(result.vendor, crawler.vendor);
        assert_eq!(result.kind, crawler.kind);
    }
}

/// A hostname outside the domains of the vendor does not confirm a crawler that
/// is known only by its reverse DNS, even when the request names one.
#[test]
fn a_claim_of_a_crawler_of_the_reverse_dns_from_an_address_outside_the_domains_fails_closed() {
    let (_directory, validator) = validator();
    validator
        .seed_rdns("seznambot", "203.0.113.7", "crawler.example.com")
        .unwrap();

    let result = validator.verify(
        "Mozilla/5.0 (compatible; SeznamBot/4.0; +https://o-seznam.cz/napoveda/vyhledavani/en/seznambot-crawler/)",
        ip("203.0.113.7"),
    );
    assert_eq!(result.status, VerifyStatus::Failed);
    assert!(!result.is_verified());
}

/// The four crawlers of Moonshot and of Tencent are verified by the address
/// space their vendors publish.
///
/// Each vendor publishes a document for every one of its crawlers, and a
/// request is verified against the address space of the crawler its marker
/// names. The three documents of Moonshot hold the same prefixes today — the
/// vendor publishes one address space for its three crawlers — so the three
/// crawlers share it; when the vendor gives the three crawlers address spaces
/// of their own, the registry follows without a change.
#[test]
fn the_crawlers_of_moonshot_and_of_tencent_are_verified_by_the_address_space_they_publish() {
    let (_directory, validator) = validator();
    for bot in ["kimbot", "kimi-searchbot", "kimi-user"] {
        validator.seed_prefix(bot, "47.79.253.42/32").unwrap();
    }
    validator.seed_prefix("hyspider", "43.144.87.0/24").unwrap();

    let training = validator.verify(KIMI_BOT_UA, ip("47.79.253.42"));
    assert!(training.is_verified());
    assert_eq!(training.name, "kimbot");
    assert_eq!(training.vendor, "Moonshot");

    let search = validator.verify(KIMI_SEARCHBOT_UA, ip("47.79.253.42"));
    assert!(search.is_verified());
    assert_eq!(search.name, "kimi-searchbot");
    assert_eq!(search.vendor, "Moonshot");

    let user = validator.verify(KIMI_USER_UA, ip("47.79.253.42"));
    assert!(user.is_verified());
    assert_eq!(user.name, "kimi-user");
    assert_eq!(user.vendor, "Moonshot");

    let hy = validator.verify(HYSPIDER_UA, ip("43.144.87.10"));
    assert!(hy.is_verified());
    assert_eq!(hy.name, "hyspider");
    assert_eq!(hy.vendor, "Tencent");
}

/// An address outside the published address space is not a crawler of Moonshot
/// or of Tencent, even when the request names one.
#[test]
fn a_claim_of_a_crawler_of_moonshot_or_of_tencent_from_an_unknown_address_fails_closed() {
    let (_directory, validator) = validator();
    for bot in ["kimbot", "kimi-searchbot", "kimi-user"] {
        validator.seed_prefix(bot, "47.79.253.42/32").unwrap();
    }
    validator.seed_prefix("hyspider", "43.144.87.0/24").unwrap();

    for ua in [KIMI_BOT_UA, KIMI_SEARCHBOT_UA, KIMI_USER_UA, HYSPIDER_UA] {
        let result = validator.verify(ua, ip("203.0.113.7"));
        assert_eq!(result.status, VerifyStatus::Failed);
        assert!(!result.is_verified());
    }
}

/// Every crawler of Moonshot is verified against the document of its own.
///
/// The vendor publishes a document for every one of its crawlers, and each
/// crawler is verified against the document it names. The three documents hold
/// the same prefixes today, so the seeds below are synthetic: they give each
/// crawler an address that the other two do not hold, to show that the
/// documents are read per crawler. When the vendor gives the three crawlers
/// address spaces of their own, a request that names one crawler and comes
/// from the address of another claims a crawler it is not, and it fails closed.
#[test]
fn every_crawler_of_moonshot_is_verified_against_its_own_document() {
    let (_directory, validator) = validator();
    validator.seed_prefix("kimbot", "198.51.100.10/32").unwrap();
    validator
        .seed_prefix("kimi-searchbot", "198.51.100.20/32")
        .unwrap();
    validator
        .seed_prefix("kimi-user", "198.51.100.30/32")
        .unwrap();

    let search = validator.verify(KIMI_SEARCHBOT_UA, ip("198.51.100.10"));
    assert_eq!(search.status, VerifyStatus::Failed);
    assert!(!search.is_verified());

    let training = validator.verify(KIMI_BOT_UA, ip("198.51.100.20"));
    assert_eq!(training.status, VerifyStatus::Failed);
    assert!(!training.is_verified());

    let user = validator.verify(KIMI_USER_UA, ip("198.51.100.30"));
    assert!(user.is_verified());
    assert_eq!(user.name, "kimi-user");
    assert_eq!(user.vendor, "Moonshot");
}
