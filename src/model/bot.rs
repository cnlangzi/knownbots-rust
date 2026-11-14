//! The registry entry of one bot.

use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use ipnet::IpNet;
use parking_lot::RwLock;
use serde::Deserialize;

use crate::error::ConfigError;
use crate::index::prefix;
use crate::model::lru::FailLru;
use crate::model::rdns::RdnsCache;
use crate::model::registry::vendor_for;

/// The category of a bot, taken from the `kind` field of its definition.
///
/// The category is the coarse classification the upstream publishes. It says
/// what kind of crawler a bot is, not who owns it — that is [`Bot::vendor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Default)]
pub enum BotKind {
    /// A crawler of a search engine.
    SearchEngine,
    /// A crawler that fetches the pages of a network of sites.
    SocialMedia,
    /// A crawler that collects content for the training of a model.
    AITraining,
    /// A crawler that fetches content for an assistant.
    AIAssist,
    /// A crawler that trains a model and serves an assistant.
    AIMixed,
    /// A crawler of an engine optimisation service.
    SEO,
    /// A crawler that watches the availability of a site.
    Monitor,
    /// A crawler that checks the security of a site.
    Security,
    /// A crawler that collects pages without a published address space.
    Scraper,
    /// A bot whose definition names no category.
    #[default]
    #[serde(other)]
    Unknown,
}

/// A bot of the registry.
///
/// The prefixes of the bot live behind a lock, so that the background refresh
/// can swap them without blocking the readers of the request path for long.
///
/// The type is read-only for the host: a bot comes from the registry of the
/// crate, never from the host.
#[non_exhaustive]
pub struct Bot {
    /// The id of the bot in the registry (`googlebot`).
    pub name: String,
    /// The category of the bot.
    pub kind: BotKind,
    /// The parser that reads the published list of the bot.
    pub parser: String,
    /// The marker the `User-Agent` of a request has to carry.
    pub ua: String,
    /// The organisation that owns the bot.
    pub vendor: String,
    /// The documents the bot publishes its address space in.
    pub urls: Vec<String>,
    /// The prefixes the definition states itself.
    pub custom: Vec<IpNet>,
    /// The domains the reverse DNS of the bot has to end in.
    pub domains: Vec<String>,
    /// The address the definition was confirmed with.
    ///
    /// The address is a sample of the crawler: its reverse name resolves under
    /// one of the [`domains`](Bot::domains) and resolves forward back to the
    /// address itself. The health check reads it to judge the source of a
    /// crawler that is verified by its reverse DNS.
    pub sample: Option<IpAddr>,
    /// Whether the bot carries a reverse DNS of its own.
    pub rdns: bool,
    /// The prefixes of the bot: the ones of its definition and the ones of the
    /// documents it published.
    pub(crate) prefixes: RwLock<Vec<IpNet>>,
    /// The reverse DNS cache of the bot.
    pub(crate) rdns_cache: Option<Arc<RdnsCache>>,
    /// The addresses whose reverse lookup already failed.
    pub(crate) fail_cache: Option<Arc<FailLru>>,
    /// When the address space was loaded.
    pub(crate) loaded_at: RwLock<Option<SystemTime>>,
}

impl Bot {
    /// Builds a bot from its definition.
    pub(crate) fn from_definition(definition: BotDefinition) -> Self {
        // An empty parser means "no structured format" — plain CIDR lines.
        // Substituting the name of the bot used to look like a custom parser
        // and then silently hit the text fallback inside the parser.
        let parser = if definition.parser.is_empty() {
            "txt".to_string()
        } else {
            definition.parser
        };
        if !definition.urls.is_empty() && !crate::feed::is_known_parser(&parser) {
            tracing::warn!(
                bot = %definition.name,
                parser = %parser,
                "the parser of the bot is unknown; its list is parsed as plain text"
            );
        }
        let vendor = if definition.vendor.is_empty() {
            let fallback = vendor_for(&definition.name);
            if fallback == "Unknown" {
                tracing::warn!(
                    bot = %definition.name,
                    "the definition names no vendor; the vendor of the bot stays unknown"
                );
            }
            fallback.to_string()
        } else {
            definition.vendor
        };
        let mut custom = Vec::new();
        for cidr in &definition.custom {
            match cidr.parse::<IpNet>() {
                Ok(net) => custom.push(net),
                Err(error) => tracing::warn!(
                    bot = %definition.name,
                    cidr = %cidr,
                    %error,
                    "the definition holds a prefix that is not a CIDR; it is skipped"
                ),
            }
        }
        let sample = match definition.sample.as_deref() {
            None => None,
            Some(value) => match value.parse::<IpAddr>() {
                Ok(address) => Some(address),
                Err(error) => {
                    tracing::warn!(
                        bot = %definition.name,
                        sample = %value,
                        %error,
                        "the definition holds a sample that is not an address; it is skipped"
                    );
                    None
                }
            },
        };
        Self {
            name: definition.name,
            kind: definition.kind,
            parser,
            ua: definition.ua,
            vendor,
            urls: definition.urls,
            custom: custom.clone(),
            domains: definition.domains,
            sample,
            rdns: definition.rdns,
            prefixes: RwLock::new(custom),
            rdns_cache: None,
            fail_cache: None,
            loaded_at: RwLock::new(None),
        }
    }

    /// The prefixes of the bot: the ones of its definition and the ones of the
    /// documents it published.
    pub fn prefixes(&self) -> Vec<IpNet> {
        self.prefixes.read().clone()
    }

    /// The number of the prefixes of the bot.
    pub fn prefix_count(&self) -> usize {
        self.prefixes.read().len()
    }

    /// When the address space of the bot was loaded.
    ///
    /// The instant is the moment the document that holds the prefixes was
    /// written, so that a host can tell how old the address space is.
    pub fn loaded_at(&self) -> Option<SystemTime> {
        *self.loaded_at.read()
    }

    /// Whether the address is covered by the prefixes of the bot.
    pub fn contains_ip(&self, ip: IpAddr) -> bool {
        let prefixes = self.prefixes.read();
        prefix::covers(&prefixes, ip)
    }

    /// Replaces the downloaded prefixes while keeping the ones of the definition.
    ///
    /// The merged set holds every prefix once. The cache of a previous run
    /// holds the prefixes of the definition as well, so the merge has to drop
    /// the duplicates — otherwise the set grows with every restart.
    pub fn store_downloaded(&self, downloaded: Vec<IpNet>) {
        if downloaded.is_empty() {
            // A refresh that yields nothing never wipes a working set.
            return;
        }
        let mut merged = self.custom.clone();
        merged.extend(downloaded);
        merged.sort();
        merged.dedup();
        *self.prefixes.write() = merged;
        *self.loaded_at.write() = Some(SystemTime::now());
    }

    /// Loads the prefixes of a previous run from `path`.
    ///
    /// A file that cannot be read is ignored: the bot then starts cold and the
    /// first refresh fills its address space.
    pub fn load_cached_ips(&self, path: &Path) {
        let Ok(data) = std::fs::read_to_string(path) else {
            return;
        };
        let mut nets = Vec::new();
        for line in data.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Ok(net) = line.parse::<IpNet>() {
                nets.push(net);
            }
        }
        if !nets.is_empty() {
            self.store_downloaded(nets);
            // The age of the address space is the age of the file it came from.
            if let Ok(modified) = std::fs::metadata(path).and_then(|metadata| metadata.modified()) {
                *self.loaded_at.write() = Some(modified);
            }
        }
    }

    /// Persists the prefixes of the bot to `path`.
    pub fn persist_ips(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| ConfigError::Io(error.to_string()))?;
        }
        let prefixes = self.prefixes.read();
        let mut body = String::new();
        for net in prefixes.iter() {
            body.push_str(&net.to_string());
            body.push('\n');
        }
        std::fs::write(path, body)
            .map_err(|error| ConfigError::Io(format!("write {}: {error}", path.display())))?;
        Ok(())
    }
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("parser", &self.parser)
            .field("ua", &self.ua)
            .field("vendor", &self.vendor)
            .field("urls", &self.urls)
            .field("prefixes", &self.prefix_count())
            .field("rdns", &self.rdns)
            .finish()
    }
}

/// The definition of a bot, as it is written in a YAML file.
#[derive(Debug, Deserialize)]
pub(crate) struct BotDefinition {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) kind: BotKind,
    #[serde(default)]
    pub(crate) parser: String,
    #[serde(default)]
    pub(crate) ua: String,
    #[serde(default)]
    pub(crate) vendor: String,
    #[serde(default)]
    pub(crate) urls: Vec<String>,
    #[serde(default)]
    pub(crate) custom: Vec<String>,
    #[serde(default)]
    pub(crate) domains: Vec<String>,
    #[serde(default)]
    pub(crate) sample: Option<String>,
    #[serde(default)]
    pub(crate) rdns: bool,
}

/// A definition that was skipped at load time.
///
/// The operator dropped a file into the override directory of the registry that
/// the loader could not turn into a bot — a YAML syntax error, or a document
/// without the required `name` / `ua` fields. The file is skipped and reported
/// here, so that a host can tell the operator *why* the bot they configured is
/// missing.
///
/// A malformed **or incomplete** **embedded** definition is not an error entry
/// but a hard failure: it is a regression in the repository that holds the
/// definition, not an operator mistake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotConfigError {
    /// The path of the offending file. For an embedded definition this is the
    /// compiled-in file name (e.g. `"googlebot.yaml"`).
    pub path: String,
    /// Why the file was skipped.
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn definition() -> BotDefinition {
        BotDefinition {
            name: "testbot".into(),
            kind: BotKind::SearchEngine,
            parser: "google".into(),
            ua: "TestBot".into(),
            vendor: "Example".into(),
            urls: vec!["https://example.invalid/ips.json".into()],
            custom: vec!["10.0.0.0/8".into()],
            domains: Vec::new(),
            sample: None,
            rdns: false,
        }
    }

    fn bot_with_custom() -> Bot {
        Bot::from_definition(definition())
    }

    /// The sample of a definition is the address the crawler was confirmed
    /// with. A sample that is not an address is skipped, the way a prefix that
    /// is not a CIDR is.
    #[test]
    fn the_sample_of_the_definition_is_an_address() {
        let mut confirmed = definition();
        confirmed.sample = Some("162.142.125.1".into());
        let bot = Bot::from_definition(confirmed);
        assert_eq!(bot.sample, Some("162.142.125.1".parse::<IpAddr>().unwrap()));

        let mut broken = definition();
        broken.sample = Some("not-an-address".into());
        let bot = Bot::from_definition(broken);
        assert_eq!(bot.sample, None);
    }

    /// The prefixes of the definition are added once, however often the cache of
    /// a previous run is read back. The cache holds the prefixes of the
    /// definition as well, so the merge has to drop the duplicates.
    #[test]
    fn the_prefixes_of_the_definition_are_added_once() {
        let bot = bot_with_custom();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("ips.txt");

        bot.store_downloaded(vec!["1.2.3.0/24".parse().unwrap()]);
        bot.persist_ips(&path).unwrap();

        for _ in 0..3 {
            bot.load_cached_ips(&path);
        }

        let prefixes = bot.prefixes.read();
        assert_eq!(prefixes.len(), 2, "the prefixes grew: {prefixes:?}");
    }
}
