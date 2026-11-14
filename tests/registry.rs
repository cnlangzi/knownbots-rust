//! The registry of the bots: the embedded definitions, the definitions an
//! operator overrides, and the contract the definitions have to satisfy.

mod common;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use common::Source;
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

#[test]
fn the_embedded_definitions_load() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    assert_eq!(bots.len(), 53, "loaded {} bots", bots.len());
    assert!(bots.iter().any(|bot| bot.name == "googlebot"));
    assert!(bots.iter().any(|bot| bot.name == "claudebot"));
    assert!(bots.iter().any(|bot| bot.name == "mediapartners-google"));
}

/// The counts the documentation of the provider-health check states.
///
/// The check reads every source of the registry, so the numbers it names have
/// to be the numbers the registry holds — the check itself only runs against
/// the internet.
#[test]
fn the_counts_of_the_registry_are_the_counts_the_health_check_names() {
    let bots = load_bots(None).expect("the embedded definitions must load");
    let count = |source: Source| bots.iter().filter(|bot| Source::of(bot) == source).count();
    assert_eq!(count(Source::List), 32, "the registry holds other lists");
    assert_eq!(
        count(Source::Dns),
        20,
        "the registry holds other reverse DNS"
    );
    assert_eq!(
        count(Source::Static),
        1,
        "the registry holds other static crawlers"
    );
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
    assert_eq!(vendor_for("oai-adsbot"), "OpenAI");
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
