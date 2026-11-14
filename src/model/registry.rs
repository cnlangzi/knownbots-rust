//! The registry of the bots and the loading of its definitions.

use std::collections::HashMap;
use std::path::Path;

use crate::error::ConfigError;
use crate::model::bot::{Bot, BotConfigError, BotDefinition};

/// The bot definitions that are compiled into the crate.
///
/// Every file of the `conf.d/` directory of the crate is listed here, so that no
/// definition of the repository is inert.
pub const EMBEDDED_CONFIGS: &[(&str, &str)] = &[
    (
        "adsbot-mobile.yaml",
        include_str!("../../conf.d/adsbot-mobile.yaml"),
    ),
    ("adsbot.yaml", include_str!("../../conf.d/adsbot.yaml")),
    (
        "ahrefsbot.yaml",
        include_str!("../../conf.d/ahrefsbot.yaml"),
    ),
    (
        "amazonbot.yaml",
        include_str!("../../conf.d/amazonbot.yaml"),
    ),
    (
        "amzn-searchbot.yaml",
        include_str!("../../conf.d/amzn-searchbot.yaml"),
    ),
    (
        "amzn-user.yaml",
        include_str!("../../conf.d/amzn-user.yaml"),
    ),
    (
        "apis-google.yaml",
        include_str!("../../conf.d/apis-google.yaml"),
    ),
    ("applebot.yaml", include_str!("../../conf.d/applebot.yaml")),
    (
        "baiduspider.yaml",
        include_str!("../../conf.d/baiduspider.yaml"),
    ),
    ("bingbot.yaml", include_str!("../../conf.d/bingbot.yaml")),
    (
        "bytespider.yaml",
        include_str!("../../conf.d/bytespider.yaml"),
    ),
    ("ccbot.yaml", include_str!("../../conf.d/ccbot.yaml")),
    (
        "chatgpt-user.yaml",
        include_str!("../../conf.d/chatgpt-user.yaml"),
    ),
    (
        "claude-searchbot.yaml",
        include_str!("../../conf.d/claude-searchbot.yaml"),
    ),
    (
        "claude-user.yaml",
        include_str!("../../conf.d/claude-user.yaml"),
    ),
    (
        "claudebot.yaml",
        include_str!("../../conf.d/claudebot.yaml"),
    ),
    (
        "cloudflare-healthchecks.yaml",
        include_str!("../../conf.d/cloudflare-healthchecks.yaml"),
    ),
    (
        "discordbot.yaml",
        include_str!("../../conf.d/discordbot.yaml"),
    ),
    (
        "duckduckbot.yaml",
        include_str!("../../conf.d/duckduckbot.yaml"),
    ),
    (
        "facebookexternalhit.yaml",
        include_str!("../../conf.d/facebookexternalhit.yaml"),
    ),
    (
        "feedfetcher.yaml",
        include_str!("../../conf.d/feedfetcher.yaml"),
    ),
    (
        "google-agent.yaml",
        include_str!("../../conf.d/google-agent.yaml"),
    ),
    (
        "google-cloudvertexbot.yaml",
        include_str!("../../conf.d/google-cloudvertexbot.yaml"),
    ),
    (
        "google-cws.yaml",
        include_str!("../../conf.d/google-cws.yaml"),
    ),
    (
        "google-gemininotebook.yaml",
        include_str!("../../conf.d/google-gemininotebook.yaml"),
    ),
    (
        "google-inspectiontool.yaml",
        include_str!("../../conf.d/google-inspectiontool.yaml"),
    ),
    (
        "google-pinpoint.yaml",
        include_str!("../../conf.d/google-pinpoint.yaml"),
    ),
    (
        "google-read-aloud.yaml",
        include_str!("../../conf.d/google-read-aloud.yaml"),
    ),
    (
        "google-safety.yaml",
        include_str!("../../conf.d/google-safety.yaml"),
    ),
    (
        "google-site-verification.yaml",
        include_str!("../../conf.d/google-site-verification.yaml"),
    ),
    (
        "google-storebot.yaml",
        include_str!("../../conf.d/google-storebot.yaml"),
    ),
    (
        "googlebot.yaml",
        include_str!("../../conf.d/googlebot.yaml"),
    ),
    (
        "googlemessages.yaml",
        include_str!("../../conf.d/googlemessages.yaml"),
    ),
    (
        "googleother.yaml",
        include_str!("../../conf.d/googleother.yaml"),
    ),
    (
        "googleproducer.yaml",
        include_str!("../../conf.d/googleproducer.yaml"),
    ),
    ("gptbot.yaml", include_str!("../../conf.d/gptbot.yaml")),
    (
        "linkedinbot.yaml",
        include_str!("../../conf.d/linkedinbot.yaml"),
    ),
    (
        "mediapartners-google.yaml",
        include_str!("../../conf.d/mediapartners-google.yaml"),
    ),
    (
        "meta-externalagent.yaml",
        include_str!("../../conf.d/meta-externalagent.yaml"),
    ),
    (
        "mistralai-index.yaml",
        include_str!("../../conf.d/mistralai-index.yaml"),
    ),
    (
        "mistralai-user.yaml",
        include_str!("../../conf.d/mistralai-user.yaml"),
    ),
    ("mj12bot.yaml", include_str!("../../conf.d/mj12bot.yaml")),
    (
        "oai-adsbot.yaml",
        include_str!("../../conf.d/oai-adsbot.yaml"),
    ),
    (
        "oai-searchbot.yaml",
        include_str!("../../conf.d/oai-searchbot.yaml"),
    ),
    (
        "perplexitybot.yaml",
        include_str!("../../conf.d/perplexitybot.yaml"),
    ),
    (
        "perplexity-user.yaml",
        include_str!("../../conf.d/perplexity-user.yaml"),
    ),
    ("petalbot.yaml", include_str!("../../conf.d/petalbot.yaml")),
    ("pingdom.yaml", include_str!("../../conf.d/pingdom.yaml")),
    (
        "pinterestbot.yaml",
        include_str!("../../conf.d/pinterestbot.yaml"),
    ),
    (
        "redditbot.yaml",
        include_str!("../../conf.d/redditbot.yaml"),
    ),
    (
        "semrushbot-backlinks.yaml",
        include_str!("../../conf.d/semrushbot-backlinks.yaml"),
    ),
    (
        "semrushbot.yaml",
        include_str!("../../conf.d/semrushbot.yaml"),
    ),
    ("slackbot.yaml", include_str!("../../conf.d/slackbot.yaml")),
    ("sogou.yaml", include_str!("../../conf.d/sogou.yaml")),
    (
        "telegrambot.yaml",
        include_str!("../../conf.d/telegrambot.yaml"),
    ),
    (
        "twitterbot.yaml",
        include_str!("../../conf.d/twitterbot.yaml"),
    ),
    (
        "uptimerobot.yaml",
        include_str!("../../conf.d/uptimerobot.yaml"),
    ),
    ("whatsapp.yaml", include_str!("../../conf.d/whatsapp.yaml")),
    (
        "yandexbot.yaml",
        include_str!("../../conf.d/yandexbot.yaml"),
    ),
];

/// What the loader made of one definition.
enum Loaded {
    /// The definition describes a bot.
    Bot(Box<Bot>),
    /// The definition was skipped; the payload says why.
    Skipped(String),
}

/// Parses one definition.
///
/// A syntax error is an error — the caller decides whether that is fatal (an
/// embedded definition) or skippable (an operator override). A document that
/// parses but lacks `name` / `ua` is skipped with the reason, so that the
/// operator learns why their bot is missing.
fn parse_definition(data: &str, filename: &str) -> Result<Loaded, ConfigError> {
    let definition: BotDefinition = serde_yaml::from_str(data).map_err(|error| {
        ConfigError::Registry(format!("parse bot config {filename}: {error:#}"))
    })?;
    if definition.name.is_empty() {
        return Ok(Loaded::Skipped("missing required 'name' field".to_string()));
    }
    if definition.ua.is_empty() {
        return Ok(Loaded::Skipped("missing required 'ua' field".to_string()));
    }
    Ok(Loaded::Bot(Box::new(Bot::from_definition(definition))))
}

/// Loads the embedded definitions.
///
/// A malformed **or incomplete** embedded definition is **fatal**: it is a
/// regression in the repository that holds the definition, not an operator
/// error, and must never reach production silently. Skipping it would drop the
/// bot from the registry — its traffic would stop being recognised, and nothing
/// on the running host would say so.
fn load_embedded(configs: &[(&str, &str)]) -> Result<Vec<Bot>, ConfigError> {
    let mut by_name: HashMap<String, Bot> = HashMap::new();
    for (filename, data) in configs {
        match parse_definition(data, filename)? {
            Loaded::Bot(bot) => {
                by_name.insert(bot.name.clone(), *bot);
            }
            Loaded::Skipped(reason) => {
                return Err(ConfigError::Registry(format!(
                    "embedded bot config {filename} is incomplete: {reason}"
                )));
            }
        }
    }
    Ok(by_name.into_values().collect())
}

/// Loads the embedded definitions, then overlays any `*.yaml` / `*.yml` from
/// `override_dir/conf.d` (the same name wins).
///
/// Operator files that cannot be turned into a bot are **skipped**, logged as a
/// warning and returned in the error list — a typo in an override file must not
/// take the host down. A malformed or incomplete *embedded* definition stays
/// fatal.
pub fn load_bots_with_errors(
    override_dir: Option<&Path>,
) -> Result<(Vec<Bot>, Vec<BotConfigError>), ConfigError> {
    let mut errors = Vec::new();
    let mut by_name: HashMap<String, Bot> = load_embedded(EMBEDDED_CONFIGS)?
        .into_iter()
        .map(|bot| (bot.name.clone(), bot))
        .collect();

    if let Some(root) = override_dir {
        let conf_d = root.join("conf.d");
        if conf_d.is_dir() {
            for entry in std::fs::read_dir(&conf_d)
                .map_err(|error| ConfigError::Io(format!("read {}: {error}", conf_d.display())))?
            {
                let entry = entry.map_err(|error| ConfigError::Io(error.to_string()))?;
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if ext != "yaml" && ext != "yml" {
                    continue;
                }
                let label = path.display().to_string();
                let data = match std::fs::read_to_string(&path) {
                    Ok(data) => data,
                    Err(error) => {
                        // An unreadable file is as much an operator error as an
                        // unparsable one — skip it instead of refusing to start.
                        let reason = format!("read {label}: {error}");
                        tracing::warn!("knownbots: skip bot config {label}: {error}");
                        errors.push(BotConfigError {
                            path: label,
                            message: reason,
                        });
                        continue;
                    }
                };
                let name = path
                    .file_name()
                    .and_then(|file_name| file_name.to_str())
                    .unwrap_or("custom.yaml");
                match parse_definition(&data, name) {
                    Ok(Loaded::Bot(bot)) => {
                        tracing::info!("knownbots: custom config overrides bot {}", bot.name);
                        by_name.insert(bot.name.clone(), *bot);
                    }
                    Ok(Loaded::Skipped(reason)) => {
                        tracing::warn!("knownbots: skip bot config {label}: {reason}");
                        errors.push(BotConfigError {
                            path: label,
                            message: reason,
                        });
                    }
                    Err(error) => {
                        let reason = format!("{error}");
                        tracing::warn!("knownbots: skip bot config {label}: {reason}");
                        errors.push(BotConfigError {
                            path: label,
                            message: reason,
                        });
                    }
                }
            }
        }
    }

    let mut bots: Vec<Bot> = by_name.into_values().collect();
    // A stable order keeps the reports of a host reproducible.
    bots.sort_by(|left, right| left.name.cmp(&right.name));
    Ok((bots, errors))
}

/// Loads the embedded definitions, then overlays any `*.yaml` / `*.yml` from
/// `override_dir/conf.d` (the same name wins).
///
/// Skipped operator files are logged as warnings. Use [`load_bots_with_errors`]
/// to also get them back.
pub fn load_bots(override_dir: Option<&Path>) -> Result<Vec<Bot>, ConfigError> {
    let (bots, _errors) = load_bots_with_errors(override_dir)?;
    Ok(bots)
}

/// The bots that are backed by a document and hold no prefix yet.
///
/// A fresh process (no cache yet) sits in this window until the first refresh
/// succeeds. A claim against one of them fails closed, unless the bot carries a
/// reverse DNS as well: that path decides until the document arrives.
pub(crate) fn count_cold_url_bots(bots: &[Bot]) -> usize {
    bots.iter()
        .filter(|bot| !bot.urls.is_empty() && bot.prefixes.read().is_empty())
        .count()
}

/// The organisation that owns a bot, guessed from its name.
///
/// The definition of a bot names its vendor; the guess is the fallback for a
/// definition that does not.
pub fn vendor_for(bot_name: &str) -> &'static str {
    match bot_name {
        n if n.starts_with("google")
            || n.starts_with("adsbot")
            || n.starts_with("apis-")
            || n.starts_with("mediapartners")
            || n.starts_with("feedfetcher") =>
        {
            "Google"
        }
        "bingbot" => "Microsoft",
        "baiduspider" => "Baidu",
        "yandexbot" => "Yandex",
        "duckduckbot" => "DuckDuckGo",
        "sogou" => "Sogou",
        "applebot" | "applebot-extended" => "Apple",
        "petalbot" => "Huawei",
        "gptbot" | "chatgpt-user" | "oai-searchbot" | "oai-adsbot" => "OpenAI",
        n if n.starts_with("claude") => "Anthropic",
        "amazonbot" | "amzn-searchbot" | "amzn-user" => "Amazon",
        "meta-externalagent" | "facebookexternalhit" | "whatsapp" => "Meta",
        "perplexity-user" | "perplexitybot" => "Perplexity",
        n if n.starts_with("mistralai") => "Mistral",
        "bytespider" => "ByteDance",
        "ccbot" => "CommonCrawl",
        "linkedinbot" => "LinkedIn",
        "twitterbot" => "Twitter",
        "pinterestbot" => "Pinterest",
        "telegrambot" => "Telegram",
        "slackbot" => "Slack",
        "discordbot" => "Discord",
        "redditbot" => "Reddit",
        "uptimerobot" => "UptimeRobot",
        "pingdom" => "Pingdom",
        n if n.starts_with("cloudflare") => "Cloudflare",
        n if n.starts_with("semrush") => "Semrush",
        "ahrefsbot" => "Ahrefs",
        "mj12bot" => "Majestic",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feed::is_known_parser;
    use crate::index::prefix;
    use ipnet::IpNet;
    use std::collections::HashSet;
    use std::net::IpAddr;

    #[test]
    fn load_embedded_has_googlebot() {
        let bots = load_bots(None).unwrap();
        assert!(bots.iter().any(|bot| bot.name == "googlebot"));
        assert!(bots.iter().any(|bot| bot.name == "claudebot"));
        assert!(bots.iter().any(|bot| bot.name == "mediapartners-google"));
        assert!(bots.len() >= 40);
    }

    #[test]
    fn embedded_bots_declare_vendor() {
        let bots = load_bots(None).unwrap();
        assert!(count_cold_url_bots(&bots) > 0);
        for bot in &bots {
            assert_ne!(
                bot.vendor, "Unknown",
                "bot {} has no vendor; add it to the YAML",
                bot.name
            );
        }
    }

    /// The registry id is the id of the bot, not the marker and not a second
    /// spelling of it.
    #[test]
    fn registry_name_is_independent_of_ua_marker() {
        let bots = load_bots(None).unwrap();
        let bing = bots.iter().find(|bot| bot.name == "bingbot").unwrap();
        assert_eq!(bing.ua, "bingbot");
        let google = bots.iter().find(|bot| bot.name == "googlebot").unwrap();
        assert_eq!(google.ua, "Googlebot");
        let inspection = bots
            .iter()
            .find(|bot| bot.name == "google-inspectiontool")
            .unwrap();
        assert_eq!(inspection.ua, "Google-InspectionTool");
    }

    #[test]
    fn empty_parser_defaults_to_txt_not_bot_name() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(
            conf.join("custombot.yaml"),
            "name: custombot\nua: \"CustomBot\"\nvendor: \"Example\"\n",
        )
        .unwrap();
        let bots = load_bots(Some(directory.path())).unwrap();
        let bot = bots.iter().find(|bot| bot.name == "custombot").unwrap();
        assert_eq!(bot.parser, "txt");
        assert_eq!(bot.vendor, "Example");
        assert_eq!(bot.name, "custombot");
        assert_eq!(bot.ua, "CustomBot");
    }

    #[test]
    fn unknown_parser_name_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(
            conf.join("typo.yaml"),
            "name: typobot\nua: \"TypoBot\"\nparser: googel\nurls:\n  - \"https://example.invalid/ips.json\"\n",
        )
        .unwrap();
        let bots = load_bots(Some(directory.path())).unwrap();
        let bot = bots.iter().find(|bot| bot.name == "typobot").unwrap();
        assert_eq!(bot.parser, "googel");
        assert!(!is_known_parser(&bot.parser));
    }

    /// A typo in an operator override file must not take the host down: the file
    /// is skipped, the rest of the registry loads, and the file is reported so
    /// that the host can show *why* the bot the operator configured is missing.
    #[test]
    fn malformed_override_file_is_skipped_and_reported() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        let broken = conf.join("broken.yaml");
        std::fs::write(&broken, "name: brokenbot\n  ua: [unclosed\n").unwrap();
        std::fs::write(
            conf.join("goodbot.yaml"),
            "name: goodbot\nua: \"GoodBot\"\nvendor: \"Example\"\n",
        )
        .unwrap();

        let (bots, errors) = load_bots_with_errors(Some(directory.path())).unwrap();
        assert!(!bots.iter().any(|bot| bot.name == "brokenbot"));
        assert!(bots.iter().any(|bot| bot.name == "goodbot"));
        assert!(bots.iter().any(|bot| bot.name == "googlebot"));
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert_eq!(errors[0].path, broken.display().to_string());
        assert!(
            errors[0].message.contains("parse bot config"),
            "message must name the parse failure: {}",
            errors[0].message
        );
    }

    /// A file that parses but lacks `name` / `ua` is skipped too — and
    /// reported, so the operator learns why their bot is missing instead of
    /// just not seeing it.
    #[test]
    fn incomplete_override_file_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        let path = conf.join("nameless.yaml");
        std::fs::write(&path, "name: \"\"\nua: \"Nameless\"\n").unwrap();

        let (bots, errors) = load_bots_with_errors(Some(directory.path())).unwrap();
        assert!(!bots.iter().any(|bot| bot.ua == "Nameless"));
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert_eq!(errors[0].path, path.display().to_string());
        assert!(
            errors[0].message.contains("name"),
            "message must name the missing field: {}",
            errors[0].message
        );
    }

    /// A YAML that omits a required field entirely is a parse failure —
    /// reported the same way, not fatal.
    #[test]
    fn override_file_without_a_name_field_is_reported() {
        let directory = tempfile::tempdir().unwrap();
        let conf = directory.path().join("conf.d");
        std::fs::create_dir_all(&conf).unwrap();
        let path = conf.join("nameless.yaml");
        std::fs::write(&path, "ua: \"Nameless\"\n").unwrap();

        let (bots, errors) = load_bots_with_errors(Some(directory.path())).unwrap();
        assert!(!bots.iter().any(|bot| bot.ua == "Nameless"));
        assert_eq!(errors.len(), 1, "errors: {errors:?}");
        assert_eq!(errors[0].path, path.display().to_string());
        assert!(
            errors[0].message.contains("name"),
            "message must name the missing field: {}",
            errors[0].message
        );
    }

    /// A malformed **embedded** definition is a regression in the repository
    /// that holds it, not an operator error — it stays fatal so that a broken
    /// build cannot reach production silently.
    #[test]
    fn malformed_embedded_config_is_fatal() {
        let error = load_embedded(&[("broken.yaml", "name: brokenbot\n  ua: [unclosed\n")])
            .expect_err("a broken embedded definition must fail the loader");
        assert!(
            format!("{error}").contains("parse bot config broken.yaml"),
            "error must name the file: {error}"
        );
    }

    /// An embedded definition that parses but lacks `name` / `ua` is just as
    /// much a regression: skipping it would drop the bot from the registry and
    /// stop its traffic from being recognised, with nothing on the running host
    /// to say so.
    #[test]
    fn incomplete_embedded_config_is_fatal() {
        let error = load_embedded(&[("googlebot.yaml", "name: \"\"\nua: \"Googlebot\"\n")])
            .expect_err("an incomplete embedded definition must fail the loader");
        assert!(
            format!("{error}").contains("googlebot.yaml"),
            "error must name the file: {error}"
        );
    }

    /// Every definition the crate ships must be complete — the check that keeps
    /// the fatal path above from firing in production.
    #[test]
    fn every_shipped_embedded_config_is_complete() {
        let bots = load_embedded(EMBEDDED_CONFIGS)
            .expect("every shipped embedded definition must be complete");
        assert!(bots.len() >= 40, "loaded {} bots", bots.len());
        assert!(bots.iter().any(|bot| bot.name == "googlebot"));
    }

    /// True when a `domains:` entry can still match a PTR record.
    ///
    /// [`crate::model::rdns::match_domain`] normalises every entry by stripping
    /// its trailing dots and lower-casing it, then compares it against the
    /// hostname the same way. An entry that is empty after that (the empty
    /// string, or nothing but dots) can never equal a hostname, and one carrying
    /// whitespace can never be a suffix of one — so a bot whose `domains:` holds
    /// only such entries is as unmatchable as a bot with no list at all.
    fn domain_is_usable(domain: &str) -> bool {
        let domain = domain.trim_end_matches('.');
        !domain.is_empty() && !domain.contains(char::is_whitespace)
    }

    /// [`domain_is_usable`] must agree with the comparison it stands in for: a
    /// domain it calls unusable has to be one that no PTR record can ever match.
    #[test]
    fn domain_usability_mirrors_the_ptr_comparison() {
        assert!(domain_is_usable("example.com"));
        assert!(domain_is_usable("example.com."));
        assert!(crate::model::rdns::match_domain(
            "crawl-66-249-66-1.example.com",
            &["example.com".to_string()]
        ));

        for unusable in ["", ".", "..."] {
            assert!(!domain_is_usable(unusable), "{unusable:?}");
            assert!(
                !crate::model::rdns::match_domain("crawl.example.com", &[unusable.to_string()]),
                "{unusable:?} must not match a hostname"
            );
        }

        assert!(!domain_is_usable(" example.com"));
        assert!(!domain_is_usable("example .com"));
    }

    /// The contract every embedded definition has to satisfy.
    ///
    /// The rules can be broken **silently**: the loader only warns (or never
    /// reads the file at all), the bot then never verifies a request, its
    /// traffic never reaches the log of the host, and nothing on the running
    /// host reports an error. The rule of the unique name is silent in its
    /// *duplicate* half — the loader keeps the last bot of a duplicated name —
    /// and loud in its *empty* half, like the rule of the marker:
    /// [`load_embedded`] treats an incomplete embedded definition as fatal and
    /// the host refuses to start.
    ///
    /// Every violation is collected, so one run reports every problem instead
    /// of only the first.
    #[test]
    fn embedded_bot_configs_satisfy_the_contract() {
        let mut violations: Vec<String> = Vec::new();
        // Registry id → the file that declared it, so a duplicate names both.
        let mut seen_names: HashMap<String, &str> = HashMap::new();

        for &(filename, data) in EMBEDDED_CONFIGS {
            // The document as the crate's own definition sees it. A document
            // that does not even fit the struct is broken in a way the loader
            // reports loudly (it refuses to start), so say what serde says —
            // the field it names is the defect.
            let definition: BotDefinition = match serde_yaml::from_str(data) {
                Ok(definition) => definition,
                Err(error) => {
                    violations.push(format!("{filename}: not a valid bot config: {error:#}"));
                    continue;
                }
            };

            // The `name` is non-empty and unique across the registry. The
            // loader keeps the *last* bot of a duplicated name, so without this
            // rule one bot silently replaces the other.
            if definition.name.is_empty() {
                violations.push(format!("{filename}: missing name"));
            } else if let Some(first) = seen_names.insert(definition.name.clone(), filename) {
                violations.push(format!(
                    "{filename}: duplicate bot name {:?} (already declared by {first})",
                    definition.name
                ));
            }

            // The `ua` is non-empty. An empty marker matches no User-Agent, so
            // the bot would never verify.
            if definition.ua.is_empty() {
                violations.push(format!("{filename}: missing ua"));
            }

            // An RDNS bot needs a domain list it can actually match. Without
            // one no PTR record can ever match, so the bot is permanently
            // unverified.
            if definition.rdns {
                if definition.domains.is_empty() {
                    violations.push(format!("{filename}: rdns bot without domains"));
                } else if !definition
                    .domains
                    .iter()
                    .any(|domain| domain_is_usable(domain))
                {
                    violations.push(format!(
                        "{filename}: rdns bot with no usable domain — every entry matches no PTR record"
                    ));
                }
            }

            // Every `custom:` entry must be a CIDR. The loader skips a bad one
            // with a warning, so the bot would silently cover less address
            // space than the definition claims.
            for cidr in &definition.custom {
                if let Err(error) = cidr.parse::<IpNet>() {
                    violations.push(format!("{filename}: invalid custom CIDR {cidr:?}: {error}"));
                }
            }

            // A stated `sample:` must be an address. The loader skips a bad one
            // with a warning, so the health check would judge the bot by no
            // sample at all.
            if let Some(sample) = &definition.sample
                && let Err(error) = sample.parse::<IpAddr>()
            {
                violations.push(format!("{filename}: invalid sample {sample:?}: {error}"));
            }

            // The definition is accepted by the loader.
            match parse_definition(data, filename) {
                Ok(Loaded::Bot(_)) => {}
                Ok(Loaded::Skipped(reason)) => {
                    violations.push(format!("{filename}: the loader skips it: {reason}"));
                }
                Err(error) => {
                    violations.push(format!("{filename}: the loader rejects it: {error}"));
                }
            }

            // The parser the loader will use for the download is the
            // *effective* one: an absent `parser:` means the text parser. An
            // unknown parser only warns at load time; the list is then parsed
            // as plain text and the refresh ends as "broken".
            let parser = if definition.parser.is_empty() {
                "txt"
            } else {
                definition.parser.as_str()
            };
            if !definition.urls.is_empty() && !is_known_parser(parser) {
                violations.push(format!(
                    "{filename}: unknown parser {parser:?} for a URL-backed bot"
                ));
            }
        }

        // Every definition file in `conf.d/` must be compiled into the binary.
        // A file that is not listed in `EMBEDDED_CONFIGS` is never parsed, so
        // it does nothing at all.
        let conf_d = Path::new(env!("CARGO_MANIFEST_DIR")).join("conf.d");
        let embedded: HashSet<&str> = EMBEDDED_CONFIGS.iter().map(|(name, _)| *name).collect();
        let mut files: Vec<String> = std::fs::read_dir(&conf_d)
            .expect("conf.d/ must be readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".yaml") || name.ends_with(".yml"))
            .collect();
        files.sort();
        for file in files {
            if !embedded.contains(file.as_str()) {
                violations.push(format!(
                    "{file}: definition file not embedded — list it in EMBEDDED_CONFIGS \
                     (src/model/registry.rs)"
                ));
            }
        }

        assert!(
            violations.is_empty(),
            "the embedded bot definitions break the contract ({} problem(s)):\n  - {}",
            violations.len(),
            violations.join("\n  - "),
        );
    }

    /// The prefix lookup of the crate answers for the prefixes of a bot.
    #[test]
    fn prefix_lookup_covers_the_address_space() {
        let prefixes: Vec<IpNet> = vec![
            "66.249.64.0/19".parse().unwrap(),
            "2001:4860::/32".parse().unwrap(),
        ];
        assert!(prefix::covers(
            &prefixes,
            "66.249.66.1".parse::<std::net::IpAddr>().unwrap()
        ));
        assert!(!prefix::covers(
            &prefixes,
            "203.0.113.7".parse::<std::net::IpAddr>().unwrap()
        ));
        assert!(prefix::covers(
            &prefixes,
            "2001:4860::1".parse::<std::net::IpAddr>().unwrap()
        ));
    }
}
