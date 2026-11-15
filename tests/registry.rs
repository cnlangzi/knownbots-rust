//! The registry of the bots: the embedded definitions, the definitions an
//! operator overrides, and the contract the definitions have to satisfy.

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use common::{Source, Sources};
use knownbots::{BotKind, EMBEDDED_CONFIGS, load_bots, load_bots_with_errors, vendor_for};

#[cfg(feature = "full")]
use knownbots::{Validator, ValidatorOptions};

/// Every file of the `conf.d/` directory of the crate.
fn definition_files() -> Vec<String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("conf.d");
    let mut files: Vec<String> = std::fs::read_dir(&directory)
        .expect("conf.d/ must be readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".yaml") || name.ends_with(".yml"))
        .collect();
    files.sort();
    files
}

/// Every source file of the crate.
fn source_files(directory: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(directory).expect("src/ must be readable") {
        let path = entry.expect("the entry must be readable").path();
        if path.is_dir() {
            files.extend(source_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}

/// The paragraph of the documentation of the provider-health check that states
/// the census of the registry.
///
/// The paragraph names the number of the crawlers, and the number of those
/// that depend on each source. It is read as text, so that the numbers it
/// holds are the numbers the registry holds.
fn census_of_the_health_check() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/provider_health.rs");
    let source = std::fs::read_to_string(&path).expect("the module must be readable");
    let documentation = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim())
        .collect::<Vec<_>>()
        .join("\n");
    documentation
        .split("\n\n")
        .map(|paragraph| paragraph.replace('\n', " "))
        .find(|paragraph| {
            paragraph
                .chars()
                .any(|character| character.is_ascii_digit())
        })
        .unwrap_or_default()
}

/// The numbers a text holds, in the order of the text.
fn numbers(text: &str) -> Vec<usize> {
    let mut numbers = Vec::new();
    let mut digits = String::new();
    for character in text.chars() {
        if character.is_ascii_digit() {
            digits.push(character);
            continue;
        }
        if !digits.is_empty() {
            numbers.push(digits.parse().expect("a number"));
            digits.clear();
        }
    }
    if !digits.is_empty() {
        numbers.push(digits.parse().expect("a number"));
    }
    numbers
}

#[test]
fn the_embedded_definitions_load() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    assert_eq!(bots.len(), 64, "loaded {} bots", bots.len());
    assert!(bots.iter().any(|bot| bot.name == "googlebot"));
    assert!(bots.iter().any(|bot| bot.name == "claudebot"));
    assert!(bots.iter().any(|bot| bot.name == "mediapartners-google"));
}

/// The counts the documentation of the provider-health check states.
///
/// The check reads every source of the registry, so the numbers it names have
/// to be the numbers the registry holds — the check itself only runs against
/// the internet.
///
/// The numbers are read out of the documentation of the check, so that the two
/// cannot drift apart: a number that drifts still reads well, and nobody
/// learns that it lies.
#[test]
fn the_counts_of_the_registry_are_the_counts_the_health_check_names() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    let count = |source: Source| {
        bots.iter()
            .filter(|bot| Sources::of(bot).contains(&source))
            .count()
    };

    let registry = vec![
        bots.len(),
        count(Source::List),
        count(Source::Dns),
        count(Source::Static),
    ];
    let census = numbers(&census_of_the_health_check());
    assert_eq!(
        census, registry,
        "the documentation of the health check names {census:?} — the crawlers, the list-backed, the DNS-backed and the static ones — the registry holds {registry:?}"
    );
}

/// The crawler of Common Crawl depends on two sources.
///
/// The vendor publishes an address space for the crawler and keeps the reverse
/// DNS of its addresses, so the definition names both of them.
#[test]
fn the_crawler_of_common_crawl_depends_on_two_sources() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    let ccbot = bots
        .iter()
        .find(|bot| bot.name == "ccbot")
        .expect("ccbot in the registry");
    let sources = Sources::of(ccbot);
    assert!(
        sources.contains(&Source::List),
        "the crawler names no published address space"
    );
    assert!(
        sources.contains(&Source::Dns),
        "the crawler names no reverse DNS"
    );
    assert_eq!(sources.to_string(), "list+dns");
}

/// The size of the registry is the size the documentation of the matching
/// names.
///
/// The module documentation of the matching names the size of the table it
/// scans over. A number that drifts from the registry drifts silently — the
/// documentation still reads well and nobody learns that it lies — so the
/// number is taken out of the documentation and held to the registry.
#[test]
fn the_size_of_the_registry_is_the_size_the_documentation_names() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/index/ua.rs");
    let source = std::fs::read_to_string(&path).expect("the module must be readable");
    let documentation: String = source
        .lines()
        .take_while(|line| line.starts_with("//!"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut sizes: Vec<usize> = Vec::new();
    let mut rest = documentation.as_str();
    while let Some(index) = rest.find('~') {
        rest = &rest[index + 1..];
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || !rest[digits.len()..].trim_start().starts_with("entries") {
            continue;
        }
        sizes.push(digits.parse().expect("a number of entries"));
    }

    assert_eq!(
        sizes.len(),
        1,
        "the module documentation names the size of the registry once: {sizes:?}"
    );
    assert_eq!(
        sizes[0],
        bots.len(),
        "the module documentation names {} entries, the registry holds {}",
        sizes[0],
        bots.len()
    );
}

#[test]
fn every_definition_file_is_embedded() {
    let embedded: HashSet<&str> = EMBEDDED_CONFIGS.iter().map(|(name, _)| *name).collect();
    let files = definition_files();
    assert!(!files.is_empty());
    for file in &files {
        assert!(
            embedded.contains(file.as_str()),
            "{file} is not embedded — list it in EMBEDDED_CONFIGS (src/model/registry.rs)"
        );
    }
}

#[test]
fn every_embedded_definition_is_a_file_of_the_directory() {
    let files: HashSet<String> = definition_files().into_iter().collect();
    for (name, _) in EMBEDDED_CONFIGS {
        assert!(
            files.contains(*name),
            "{name} is embedded but does not exist in conf.d/"
        );
    }
}

#[test]
fn the_names_of_the_registry_are_unique() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    let names: HashSet<&str> = bots.iter().map(|bot| bot.name.as_str()).collect();
    assert_eq!(names.len(), bots.len());
}

#[test]
fn the_markers_of_the_registry_are_not_empty() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    for bot in &bots {
        assert!(!bot.ua.is_empty(), "bot {} has no marker", bot.name);
    }
}

#[test]
fn an_override_replaces_the_definition_of_the_bot() {
    let directory = tempfile::tempdir().unwrap();
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).unwrap();
    std::fs::write(
        conf.join("googlebot.yaml"),
        "name: googlebot\nua: \"Googlebot\"\nvendor: \"Example\"\ncustom:\n  - \"66.249.64.0/19\"\n",
    )
    .unwrap();

    let bots = load_bots(Some(directory.path())).expect("the override must load");
    let google = bots.iter().find(|bot| bot.name == "googlebot").unwrap();
    assert_eq!(google.vendor, "Example");
    assert_eq!(google.custom.len(), 1);
}

#[test]
fn a_malformed_override_is_skipped_and_reported() {
    let directory = tempfile::tempdir().unwrap();
    let conf = directory.path().join("conf.d");
    std::fs::create_dir_all(&conf).unwrap();
    let broken = conf.join("broken.yaml");
    std::fs::write(&broken, "name: brokenbot\n  ua: [unclosed\n").unwrap();

    let (bots, errors) =
        load_bots_with_errors(Some(directory.path())).expect("the registry must load");
    assert!(!bots.iter().any(|bot| bot.name == "brokenbot"));
    assert!(bots.iter().any(|bot| bot.name == "googlebot"));
    assert_eq!(errors.len(), 1, "errors: {errors:?}");
    assert_eq!(errors[0].path, broken.display().to_string());
}

#[test]
fn the_vendor_of_a_bot_is_guessed_from_its_name() {
    assert_eq!(vendor_for("googlebot"), "Google");
    assert_eq!(vendor_for("bingbot"), "Microsoft");
    assert_eq!(vendor_for("claudebot"), "Anthropic");
    assert_eq!(vendor_for("claude-searchbot"), "Anthropic");
    assert_eq!(vendor_for("claude-user"), "Anthropic");
    assert_eq!(vendor_for("oai-adsbot"), "OpenAI");
    assert_eq!(vendor_for("amzn-searchbot"), "Amazon");
    assert_eq!(vendor_for("amzn-user"), "Amazon");
    assert_eq!(vendor_for("mistralai-user"), "Mistral");
    assert_eq!(vendor_for("mistralai-index"), "Mistral");
    assert_eq!(vendor_for("duckassistbot"), "DuckDuckGo");
    assert_eq!(vendor_for("youbot"), "You.com");
    assert_eq!(vendor_for("siteauditbot"), "Semrush");
    assert_eq!(vendor_for("splitsignalbot"), "Semrush");
    assert_eq!(vendor_for("rytebot"), "Semrush");
    assert_eq!(vendor_for("a-bot-that-does-not-exist"), "Unknown");
}

/// The definitions of the crawlers of OpenAI hold the fields the vendor
/// publishes.
///
/// A crawler the registry does not name is a crawler no host can recognise, and
/// a crawler that is verified by a source the vendor does not publish is
/// verified by a rule broader than the vendor's.
#[test]
fn the_definitions_of_the_crawlers_of_openai_hold_their_fields() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let adsbot = bots
        .iter()
        .find(|bot| bot.name == "oai-adsbot")
        .expect("oai-adsbot in the registry");
    assert_eq!(adsbot.kind, BotKind::Monitor);
    assert_eq!(adsbot.vendor, "OpenAI");
    assert_eq!(adsbot.ua, "OAI-AdsBot");
    assert_eq!(adsbot.parser, "google");
    assert_eq!(adsbot.urls, ["https://openai.com/adsbot.json"]);

    let chatgpt = bots
        .iter()
        .find(|bot| bot.name == "chatgpt-user")
        .expect("chatgpt-user in the registry");
    assert_eq!(chatgpt.kind, BotKind::AIAssist);
    assert_eq!(chatgpt.vendor, "OpenAI");
    assert_eq!(chatgpt.ua, "ChatGPT-User");
    assert_eq!(chatgpt.parser, "google");
    assert_eq!(chatgpt.urls, ["https://openai.com/chatgpt-user.json"]);
    assert!(
        !chatgpt.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        chatgpt.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
}

/// The definitions of the crawlers of Anthropic hold the fields the vendor
/// publishes.
///
/// A crawler the registry does not name is a crawler no host can recognise, and
/// a crawler that is verified by a source the vendor does not publish is
/// verified by a rule broader than the vendor's.
#[test]
fn the_definitions_of_the_crawlers_of_anthropic_hold_their_fields() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let searchbot = bots
        .iter()
        .find(|bot| bot.name == "claude-searchbot")
        .expect("claude-searchbot in the registry");
    assert_eq!(searchbot.kind, BotKind::AIAssist);
    assert_eq!(searchbot.vendor, "Anthropic");
    assert_eq!(searchbot.ua, "Claude-SearchBot");
    assert_eq!(searchbot.parser, "google");
    assert_eq!(searchbot.urls, ["https://claude.com/crawling/bots.json"]);
    assert!(
        !searchbot.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        searchbot.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );

    let user = bots
        .iter()
        .find(|bot| bot.name == "claude-user")
        .expect("claude-user in the registry");
    assert_eq!(user.kind, BotKind::AIAssist);
    assert_eq!(user.vendor, "Anthropic");
    assert_eq!(user.ua, "Claude-User");
    assert_eq!(user.parser, "google");
    assert_eq!(user.urls, ["https://claude.com/crawling/bots.json"]);
    assert!(!user.rdns, "the crawler is verified by its published list");
    assert!(
        user.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );

    let claudebot = bots
        .iter()
        .find(|bot| bot.name == "claudebot")
        .expect("claudebot in the registry");
    assert_eq!(claudebot.kind, BotKind::AITraining);
    assert_eq!(claudebot.vendor, "Anthropic");
    assert_eq!(claudebot.ua, "ClaudeBot");
    assert_eq!(claudebot.parser, "google");
    assert_eq!(claudebot.urls, ["https://claude.com/crawling/bots.json"]);
    assert!(
        !claudebot.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        claudebot.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
}

/// The definitions of the crawlers of Mistral hold the fields the vendor
/// publishes.
///
/// A crawler the registry does not name is a crawler no host can recognise, and
/// a crawler that is verified by a source the vendor does not publish is
/// verified by a rule broader than the vendor's.
#[test]
fn the_definitions_of_the_crawlers_of_mistral_hold_their_fields() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let index = bots
        .iter()
        .find(|bot| bot.name == "mistralai-index")
        .expect("mistralai-index in the registry");
    assert_eq!(index.kind, BotKind::AIAssist);
    assert_eq!(index.vendor, "Mistral");
    assert_eq!(index.ua, "MistralAI-Index");
    assert_eq!(index.parser, "google");
    assert_eq!(index.urls, ["https://mistral.ai/mistralai-index-ips.json"]);
    assert!(!index.rdns, "the crawler is verified by its published list");
    assert!(
        index.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
    assert!(
        index.custom.is_empty(),
        "the crawler states no prefix of its own"
    );

    let user = bots
        .iter()
        .find(|bot| bot.name == "mistralai-user")
        .expect("mistralai-user in the registry");
    assert_eq!(user.kind, BotKind::AIAssist);
    assert_eq!(user.vendor, "Mistral");
    assert_eq!(user.ua, "MistralAI-User");
    assert_eq!(user.parser, "google");
    assert_eq!(user.urls, ["https://mistral.ai/mistralai-user-ips.json"]);
    assert!(!user.rdns, "the crawler is verified by its published list");
    assert!(
        user.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
    assert!(
        user.custom.is_empty(),
        "the crawler states no prefix of its own"
    );
}

/// The definitions of the crawlers of Amazon hold the fields the vendor
/// publishes.
///
/// A crawler the registry does not name is a crawler no host can recognise, and
/// a crawler that is verified by a source the vendor does not publish is
/// verified by a rule broader than the vendor's.
#[test]
fn the_definitions_of_the_crawlers_of_amazon_hold_their_fields() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let searchbot = bots
        .iter()
        .find(|bot| bot.name == "amzn-searchbot")
        .expect("amzn-searchbot in the registry");
    assert_eq!(searchbot.kind, BotKind::AIAssist);
    assert_eq!(searchbot.vendor, "Amazon");
    assert_eq!(searchbot.ua, "Amzn-SearchBot");
    assert_eq!(searchbot.parser, "amazon");
    assert_eq!(
        searchbot.urls,
        ["https://developer.amazon.com/amazonbot/searchbot-ip-addresses"]
    );
    assert!(
        !searchbot.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        searchbot.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );

    let user = bots
        .iter()
        .find(|bot| bot.name == "amzn-user")
        .expect("amzn-user in the registry");
    assert_eq!(user.kind, BotKind::AIAssist);
    assert_eq!(user.vendor, "Amazon");
    assert_eq!(user.ua, "Amzn-User");
    assert_eq!(user.parser, "amazon");
    assert_eq!(
        user.urls,
        ["https://developer.amazon.com/amazonbot/live-ip-addresses"]
    );
    assert!(!user.rdns, "the crawler is verified by its published list");
    assert!(
        user.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
}

/// The definitions of the crawlers of DuckDuckGo hold the fields the vendor
/// publishes.
///
/// A crawler the registry does not name is a crawler no host can recognise, and
/// a crawler that is verified by a source the vendor does not publish is
/// verified by a rule broader than the vendor's.
#[test]
fn the_definitions_of_the_crawlers_of_duckduck_go_hold_their_fields() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let assist = bots
        .iter()
        .find(|bot| bot.name == "duckassistbot")
        .expect("duckassistbot in the registry");
    assert_eq!(assist.kind, BotKind::AIAssist);
    assert_eq!(assist.vendor, "DuckDuckGo");
    assert_eq!(assist.ua, "DuckAssistBot");
    assert_eq!(assist.parser, "google");
    assert_eq!(assist.urls, ["https://duckduckgo.com/duckassistbot.json"]);
    assert!(
        !assist.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        assist.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
    assert!(
        assist.custom.is_empty(),
        "the crawler states no prefix of its own"
    );

    let search = bots
        .iter()
        .find(|bot| bot.name == "duckduckbot")
        .expect("duckduckbot in the registry");
    assert_eq!(search.kind, BotKind::SearchEngine);
    assert_eq!(search.vendor, "DuckDuckGo");
    assert_eq!(search.ua, "DuckDuckBot");
    assert_eq!(search.parser, "google");
    assert_eq!(search.urls, ["https://duckduckgo.com/duckduckbot.json"]);
    assert!(
        !search.rdns,
        "the crawler is verified by its published list"
    );
    assert!(
        search.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
}

/// The definition of the crawler of You.com holds the fields the vendor
/// publishes.
///
/// The vendor states the address space of the crawler as one network and
/// publishes no machine-readable list of it, so the definition holds the
/// network itself. A definition that named a broader address space would
/// verify a request that the vendor does not own.
#[test]
fn the_definition_of_the_crawler_of_you_com_holds_the_fields_the_vendor_publishes() {
    let bots = load_bots(None).expect("the embedded definitions must load");

    let youbot = bots
        .iter()
        .find(|bot| bot.name == "youbot")
        .expect("youbot in the registry");
    assert_eq!(youbot.kind, BotKind::SearchEngine);
    assert_eq!(youbot.vendor, "You.com");
    assert_eq!(youbot.ua, "YouBot");
    assert!(
        youbot.urls.is_empty(),
        "the vendor publishes no machine-readable list of the address space"
    );
    assert!(
        !youbot.rdns,
        "the crawler is verified by its stated prefixes"
    );
    assert!(
        youbot.domains.is_empty(),
        "the crawler has no reverse DNS source"
    );
    let prefixes: Vec<String> = youbot.custom.iter().map(|net| net.to_string()).collect();
    assert_eq!(prefixes, ["68.67.112.0/24"]);
}

#[test]
fn the_categories_of_the_registry_are_known() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    for bot in &bots {
        assert_ne!(
            bot.kind,
            BotKind::Unknown,
            "bot {} names no category",
            bot.name
        );
    }
}

/// The API of the crate carries the registry, the validator and the result of a
/// verification. The test names every one of them, so that a rename cannot
/// pass unnoticed.
#[test]
#[allow(clippy::type_complexity)]
fn the_api_carries_the_registry_the_validator_and_the_result() {
    use knownbots::{
        Bot, BotConfigError, BotKind, BotRefreshState, BotRefreshStatus, ConfigError,
        RefreshFailure, Validator, ValidatorOptions, VerifyResult, VerifyStatus, contains_word,
        find_bot_by_ua, load_bots, load_bots_with_errors, match_domain, vendor_for,
    };

    fn assert_type<T>() {}

    assert_type::<Bot>();
    assert_type::<BotKind>();
    assert_type::<BotConfigError>();
    assert_type::<BotRefreshState>();
    assert_type::<BotRefreshStatus>();
    assert_type::<RefreshFailure>();
    assert_type::<Validator>();
    assert_type::<ValidatorOptions>();
    assert_type::<VerifyResult>();
    assert_type::<VerifyStatus>();

    let _: fn(Option<&Path>) -> Result<Vec<Bot>, ConfigError> = load_bots;
    let _: fn(Option<&Path>) -> Result<(Vec<Bot>, Vec<BotConfigError>), ConfigError> =
        load_bots_with_errors;
    let _: fn(&str) -> &'static str = vendor_for;
    let _: fn(&str, &[String]) -> bool = match_domain;
    let _: fn(&str, &str) -> bool = contains_word;
    let _: fn(&str, &[Bot]) -> Option<usize> = find_bot_by_ua;
}

/// The library carries the category of a bot, but not the vocabulary of the
/// host that uses it: the wire labels of an admin panel belong to that host.
#[test]
fn the_api_carries_no_category_label() {
    // The labels are searched as the string literals they would be written as,
    // so that a word in a comment does not fail the suite.
    let labels = [
        "\"search_engine\"",
        "\"ads_bot\"",
        "\"ai_bot\"",
        "\"social\"",
        "\"monitoring\"",
    ];
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for file in source_files(&source) {
        let text = std::fs::read_to_string(&file).expect("the source must be readable");
        for label in labels {
            assert!(
                !text.contains(label),
                "{} holds the label {label}, which belongs to the host of the crate",
                file.display()
            );
        }
    }
}

/// The background work runs on a thread of the crate, so the host needs no
/// runtime for it — and the thread stops when the validator is dropped.
#[cfg(feature = "full")]
#[test]
fn the_background_work_needs_no_runtime_of_the_host() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let validator = Validator::new(ValidatorOptions {
        root: directory.path().to_path_buf(),
        // The worker does not touch the network in this test.
        refresh_on_start: false,
        ..ValidatorOptions::default()
    })
    .expect("a validator");

    assert!(validator.start_background(), "the worker must start");
    // The second call does nothing.
    assert!(!validator.start_background());

    validator.shutdown();
    drop(validator);
}
