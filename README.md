# knownbots

[![crates.io](https://img.shields.io/crates/v/knownbots.svg)](https://crates.io/crates/knownbots)
[![docs.rs](https://img.shields.io/docsrs/knownbots.svg)](https://docs.rs/knownbots)

- [Install](#install)
- [Quick start](#quick-start)
- [The result of a verification](#the-result-of-a-verification)
- [The verification model](#the-verification-model)
- [Configuration](#configuration)
- [The registry](#the-registry)
- [Crawlers of your own](#crawlers-of-your-own)
- [Observability](#observability)
- [The parts you can use on their own](#the-parts-you-can-use-on-their-own)
- [How the lists are fetched](#how-the-lists-are-fetched)
- [Maintainers](#maintainers)
- [License](#license)

`knownbots` verifies whether a request that claims to be a known crawler really
is one. A crawler proves its identity in two steps: it names itself in the
`User-Agent` header, and it comes from an address the vendor published for that
crawler. Only a `Verified` result says that the request is the crawler it
claims to be; a request that cannot be verified fails closed. The crate is a
Rust port of the upstream library
[`github.com/cnlangzi/knownbots`](https://github.com/cnlangzi/knownbots); the
API of the `0.1` line is a promise of compatibility *within* the line.

## Install

The crate needs Rust **1.88** or newer.

```sh
cargo add knownbots --features full
```

The `full` feature compiles in the network parts of the crate: the download of
the published lists and the reverse DNS lookups. A host that only matches the
`User-Agent` of a request against the registry of the crate depends on it
without the feature:

```sh
cargo add knownbots
```

| | default build | `full` build |
|---|---|---|
| the registry and the `User-Agent` matching | yes | yes |
| the prefixes a definition states itself (`custom:`) | yes | yes |
| the lists cached on disk (`<root>/<bot>/ips.txt`) | yes | yes |
| the download of the published lists | no | yes |
| the reverse DNS lookups | no | yes |
| the worker thread | no | yes |

Without the `full` feature the crate holds **no downloaded address space**:
only the prefixes a definition states itself and the lists that are cached on
disk can match. The items that exist only with the feature are:

- `Validator::start_background`
- `Validator::refresh`
- `Validator::refresh_bot`
- `knownbots::HttpClient`
- `knownbots::reqwest`
- the `http_client` field of `ValidatorOptions`
- `knownbots::feed::download_prefixes`
- `knownbots::feed::Downloaded`

## Quick start

The validator is the handle a host keeps. It holds the registry, the caches and
the background worker. The root of the validator is the directory of the host:
the crate keeps its caches there.

```rust,no_run
use std::net::IpAddr;

use knownbots::{Validator, ValidatorOptions, VerifyStatus};

fn main() {
    let validator = Validator::new(ValidatorOptions {
        root: "/var/lib/knownbots".into(),
        ..ValidatorOptions::default()
    })
    .expect("the validator");

    // The background work — the download of the lists and the reverse lookups
    // — runs on a thread of the crate that has a runtime of its own, so the
    // host needs no runtime for it.
    #[cfg(feature = "full")]
    let _ = validator.start_background();

    let result = validator.verify(
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "66.249.66.1".parse::<IpAddr>().unwrap(),
    );

    match result.status {
        VerifyStatus::Verified => println!("{} of {}", result.name, result.vendor),
        VerifyStatus::Failed => println!("the claim is false"),
        VerifyStatus::Pending => println!("the reverse DNS decides; ask again later"),
        VerifyStatus::Unknown => println!("no crawler of the registry claims this request"),
    }
}
```

The method `Validator::start_background` answers `false` when the worker was
already started and when the thread could not be started; it never panics. The
method `Validator::shutdown` stops the background work.

## The result of a verification

| status | meaning | what the host does |
|---|---|---|
| `Verified` | The marker matched and the address belongs to the crawler. | Treat the request as the crawler it claims to be. |
| `Failed` | The marker matched, the address does not belong to the crawler. | Treat the request as an ordinary one — the claim is false. |
| `Pending` | The marker matched, the reverse DNS decides and the cache is cold. | Treat the request as an ordinary one until the cache holds the answer; a later request may succeed. |
| `Unknown` | The marker names no crawler of the registry. | Treat the request as an ordinary one. |

Only `Verified` says that the request is the crawler it claims to be.

The identity of the crawler — the `name` and the `vendor` of the result — is
filled **only** for a `Verified` result. Every other status leaves the two
fields empty, so a host never reads the name of a crawler it did not verify.

The two builds differ in one place: a cold reverse-DNS cache answers `Pending`
with the `full` feature and `Failed` without it, because the lookup that would
fill the cache is part of the feature.

A verified request, with the address space of the crawler seeded instead of
downloaded:

```rust
use std::net::IpAddr;

use knownbots::{Validator, VerifyStatus};

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let validator = Validator::with_root(directory.path()).unwrap();

    // The seam for the fixtures of an offline installation.
    validator.seed_prefix("googlebot", "66.249.64.0/19").unwrap();

    let result = validator.verify(
        "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
        "66.249.66.1".parse::<IpAddr>().unwrap(),
    );

    assert_eq!(result.status, VerifyStatus::Verified);
    assert_eq!(result.name, "googlebot");
    assert_eq!(result.vendor, "Google");
}
```

A request that carries the marker of no crawler:

```rust
use std::net::IpAddr;

use knownbots::{Validator, VerifyStatus};

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let validator = Validator::with_root(directory.path()).unwrap();

    let result = validator.verify(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0 Safari/537.36",
        "203.0.113.7".parse::<IpAddr>().unwrap(),
    );

    assert_eq!(result.status, VerifyStatus::Unknown);
    assert!(result.name.is_empty());
    assert!(result.vendor.is_empty());
}
```

Three seams fill the state of a crawler without a network access:
`Validator::seed_prefix` seeds a prefix into the address space of a crawler,
`Validator::seed_rdns` seeds a reverse DNS entry into its cache, and
`Validator::seed_refresh_error` records the failure of a refresh. A host uses
them for the fixtures of its own tests and for an offline installation.

## The verification model

The verification is synchronous and allocation-light. It never awaits and it
never holds a lock that a background task holds for long.

1. The `User-Agent` of the request is matched against the markers of the
   registry. The match is **byte-exact** and sits on a **word boundary**: the
   official markers are published in a fixed casing (`Googlebot`, never
   `googlebot`), and `MyGooglebot` does not match `Googlebot`. When several
   markers match, the **longest** one wins, so `Applebot-Extended` is preferred
   over `Applebot`.
2. The address of the request has to fall into the address space of the matched
   crawler. The address space is the set of prefixes the vendor published, plus
   the prefixes the definition of the crawler states itself.
3. A crawler that is verified by its **reverse DNS** instead is confirmed by the
   hostname of the address: the hostname has to end in one of the domains of the
   crawler, and a forward lookup has to confirm it.

A request that cannot be verified **fails closed**. A reverse lookup that is not
cached yet never happens on the request path: the lookup is queued, the request
answers `Pending`, and a later request may succeed. A host that cannot wait
treats a `Pending` result like a `Failed` one and asks again on a later
request, when the on-disk cache of the crawler holds the answer.

## Configuration

| option | default | meaning |
|---|---|---|
| `root` | `./knownbots` | The directory that holds the caches and the definitions of the host. |
| `fail_limit` | `1000` | How many failed reverse lookups a crawler remembers. |
| `refresh_interval` | `24h` | How often the published lists are refreshed. |
| `refresh_on_start` | `true` | Whether the worker refreshes the lists as soon as it starts. |
| `http_client` | none | The client the lists are downloaded with. The crate builds one when the host supplies none. Only with the `full` feature. |

Build the options with `..ValidatorOptions::default()`, so that an option the
crate adds later does not break the host.

A host that wants to keep the caches in its own directory points the root at
it. `Validator::with_root` builds a validator with the default options and
that directory:

```rust
use knownbots::Validator;

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let validator = Validator::with_root(directory.path()).unwrap();

    println!("{} crawlers in the registry", validator.bot_count());
    for bot in validator.bots() {
        println!("{} of {}", bot.name, bot.vendor);
    }

    // The background work of the crate stops here.
    validator.shutdown();
}
```

The crate writes three things under the root:

| path | holds |
|---|---|
| `<root>/<bot>/ips.txt` | The address space of the crawler: the prefixes of its definition and of the lists it published. |
| `<root>/<bot>/rdns.txt` | The cache of the reverse lookups of the crawler. |
| `<root>/conf.d/` | The definitions of the host. |

The default root is `./knownbots`, relative to the working directory of the
process. A host that serves traffic points the root at a directory of its own.

## The registry

The registry holds the crawlers the crate knows. Every crawler is a YAML file
of the `conf.d/` directory of this repository, and every file of that directory
is compiled into the crate.

| bot | vendor | kind | marker | source |
|---|---|---|---|---|
| adsbot | Google | Monitor | AdsBot | list |
| adsbot-mobile | Google | Monitor | AdsBot-Google-Mobile | list |
| ahrefsbot | Ahrefs | SEO | AhrefsBot | list |
| amazonbot | Amazon | AITraining | Amazonbot | list |
| apis-google | Google | Monitor | APIs-Google | list |
| applebot | Apple | SearchEngine | Applebot | list |
| baiduspider | Baidu | SearchEngine | Baiduspider | dns |
| bingbot | Microsoft | SearchEngine | bingbot | list |
| bytespider | ByteDance | AITraining | Bytespider | dns |
| ccbot | CommonCrawl | AITraining | CCBot | dns |
| chatgpt-user | OpenAI | AIAssist | ChatGPT-User | dns |
| claudebot | Anthropic | AITraining | ClaudeBot | list |
| cloudflare-healthchecks | Cloudflare | Monitor | Cloudflare-Healthchecks | list |
| discordbot | Discord | SocialMedia | Discordbot | dns |
| duckduckbot | DuckDuckGo | SearchEngine | DuckDuckBot | list |
| facebookexternalhit | Meta | SocialMedia | facebookexternalhit | static |
| feedfetcher | Google | SearchEngine | FeedFetcher | list |
| google-inspectiontool | Google | SearchEngine | Google-InspectionTool | list |
| google-storebot | Google | SEO | Storebot-Google | list |
| googlebot | Google | SearchEngine | Googlebot | list |
| googleother | Google | SearchEngine | GoogleOther | list |
| gptbot | OpenAI | AITraining | GPTBot | list |
| linkedinbot | LinkedIn | SocialMedia | LinkedInBot | dns |
| mediapartners-google | Google | Monitor | Mediapartners-Google | list |
| meta-externalagent | Meta | AITraining | meta-externalagent | dns |
| mj12bot | Majestic | SEO | MJ12bot | dns |
| oai-searchbot | OpenAI | AIAssist | OAI-SearchBot | list |
| perplexity-user | Perplexity | AIAssist | Perplexity-User | dns |
| perplexitybot | Perplexity | AITraining | PerplexityBot | dns |
| petalbot | Huawei | AITraining | PetalBot | dns |
| pingdom | Pingdom | Monitor | Pingdom | list |
| pinterestbot | Pinterest | SocialMedia | Pinterestbot | dns |
| redditbot | Reddit | SocialMedia | redditbot | dns |
| semrushbot | Semrush | SEO | SemrushBot | dns |
| semrushbot-ba | Semrush | SEO | SemrushBot-BA | dns |
| slackbot | Slack | SocialMedia | Slackbot | dns |
| sogou | Sogou | SearchEngine | Sogou | dns |
| telegrambot | Telegram | SocialMedia | TelegramBot | dns |
| twitterbot | Twitter | SocialMedia | Twitterbot | dns |
| uptimerobot | UptimeRobot | Monitor | UptimeRobot | list |
| whatsapp | Meta | SocialMedia | WhatsApp | dns |
| yandexbot | Yandex | SearchEngine | YandexBot | dns |

The `source` column tells where the address space of a crawler comes from:
`list` (a published document the crate downloads), `dns` (the reverse DNS of
the vendor) or `static` (the crawler states its own prefixes).

A definition is a YAML document:

```yaml
kind: SearchEngine
name: googlebot
vendor: "Google"
parser: google
ua: "Googlebot"
urls:
  - "https://developers.google.com/static/search/apis/ipranges/googlebot.json"
```

| field | meaning |
|---|---|
| `kind` | The category of the crawler. |
| `name` | The id of the crawler in the registry. |
| `vendor` | The organisation that owns the crawler. |
| `parser` | The parser that reads the published list. Absent means the text parser. |
| `ua` | The marker the `User-Agent` of a request has to carry. |
| `urls` | The documents the crawler publishes its address space in. |
| `custom` | The prefixes the definition states itself. |
| `domains` | The domains the reverse DNS of the crawler has to end in. |
| `rdns` | Whether the crawler is verified by its reverse DNS. |

The parsers the crate knows are:

- `google`
- `openai`
- `uptimerobot`
- `ahrefs`
- `amazon`
- `txt` — plain CIDR lines

A definition without a `parser` is read with the text parser.

A crawler that is verified by its reverse DNS names the domains its addresses
resolve to:

```yaml
kind: SearchEngine
name: baiduspider
vendor: "Baidu"
ua: "Baiduspider"
rdns: true
domains:
  - "baidu.com"
  - "baidu.jp"
```

## Crawlers of your own

A host adds crawlers of its own by dropping YAML files into the `conf.d/`
directory of the root of the validator — `<root>/conf.d/`. The files may carry
the extension `.yaml` or `.yml`.

The `name:` field of the definition decides which definition is replaced: a
file that names a crawler of the registry replaces that definition, a file that
names a crawler of its own adds it. The name of the file itself is only a
label — it appears in the error messages of a definition that was skipped.

A definition of the host that cannot be read or parsed is skipped and reported
by `Validator::config_errors()`. A typo in a definition of the host never
takes the host down; the rest of the registry loads as usual.

An incomplete embedded definition is the opposite: it fails the construction
of the validator, because a crawler that silently disappears from the registry
stops recognising its traffic without a word.

## Observability

| call | answers |
|---|---|
| `Validator::refresh_status()` | The state of the address space of every crawler, with the instant it was loaded. |
| `Validator::last_refresh_error(name)` | Why the last refresh of a crawler failed. `None` means the refresh succeeded or never ran. |
| `Validator::config_errors()` | The definitions of the host that were skipped, with the reason of every one. |
| `Validator::rdns_overflow()` | The reverse lookups that were dropped because the queue was full. |
| `Validator::bots()` | The registry the verifier knows: the embedded definitions and the ones the host dropped into `<root>/conf.d/`. |
| `Validator::bot_count()` | The number of crawlers of the registry. |
| `Validator::covered_ip(name)` | An address inside the address space of a crawler, when the crawler holds one. |

```rust
use knownbots::Validator;

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let validator = Validator::with_root(directory.path()).unwrap();

    for status in validator.refresh_status() {
        println!(
            "{}: {:?} ({} prefixes)",
            status.name, status.state, status.prefixes
        );
    }
}
```

The state of a crawler is `Ready` when its address space is loaded, `Cold`
when it is backed by a document that was never downloaded, and `Failed` when
the last refresh failed. A crawler that is verified by its reverse DNS has no
document to download and is always `Ready`.

## The parts you can use on their own

The crate is a library of small parts as well as a whole. A host that only
matches the `User-Agent` of a request, or that reads a published list of its
own, uses these parts without a validator:

```rust
use knownbots::{contains_word, find_bot_by_ua, load_bots, match_domain, vendor_for};

fn main() {
    let bots = load_bots(None).unwrap();

    // The marker is matched byte-exact, on a word boundary.
    assert!(contains_word(
        "Mozilla/5.0 (compatible; Googlebot/2.1)",
        "Googlebot"
    ));
    assert!(!contains_word("MyGooglebot/1.0", "Googlebot"));

    // The longest marker wins.
    let position = find_bot_by_ua(
        "Mozilla/5.0 (compatible; AdsBot-Google-Mobile; +http://www.google.com/mobile/adsbot.html)",
        &bots,
    )
    .expect("a match");
    assert_eq!(bots[position].name, "adsbot-mobile");

    // The vendor of a crawler, and the domain of its reverse DNS.
    assert_eq!(vendor_for("bingbot"), "Microsoft");
    assert!(match_domain(
        "crawl-66-249-66-1.googlebot.com",
        &["googlebot.com".to_string()]
    ));

    // The parsers of the published lists.
    assert!(knownbots::feed::is_known_parser("google"));
    let prefixes = knownbots::feed::parse(
        "google",
        br#"{"prefixes":[{"ipv4Prefix":"66.249.64.0/19"}]}"#,
    )
    .expect("the list of the crawler");
    assert_eq!(prefixes.len(), 1);
}
```

- `load_bots` reads the registry of the crate. `load_bots_with_errors` reads it
  as well and returns the definitions that were skipped.
- `find_bot_by_ua` answers the position of the crawler of a marker in a
  registry.
- `contains_word` is the match of the crate: byte-exact, on a word boundary.
- `vendor_for` guesses the vendor of a crawler from its id.
- `match_domain` answers whether a hostname is the domain or a subdomain of it.
- `feed::parse` turns the body of a published list into prefixes;
  `feed::is_known_parser` answers whether the crate knows a parser.
- A crawler of the registry is a `Bot`; its category is a `BotKind`.

## How the lists are fetched

The download of a published list is not a single step: the body is fetched, it
is decoded, and it is read by the parser of the crawler. The crate decodes the
bodies that are compressed with gzip, brotli, zstd or deflate.

The two failures of a download are told apart, because the two mean different
things:

| failure | meaning |
|---|---|
| `RefreshFailure::Unreachable` | The environment: the vendor refused the request, the network was down, the endpoint answered with something that is not the list. |
| `RefreshFailure::Broken` | This crate: the list arrived in a shape the crate mis-reads, or the transfer failed in a way that points at the client of the crate. |

A body that is not a JSON document at all — an error page, an empty body — is
`Unreachable`: the endpoint served something that is not the list. A JSON
document that lost its top-level key is `Broken`: the shape of the list changed
and the crate has to follow it. A list that holds no usable prefix at all is
`Broken` as well — the document was read, but it yielded nothing.

An entry of a list that holds no usable prefix is skipped; the other entries of
the document still count. A crawler whose refresh failed keeps the prefixes it
had, so it keeps verifying from the cache.

The download itself is `knownbots::feed::download_prefixes`; it answers a
`knownbots::feed::Downloaded` that holds the prefixes it read and the failure
that stopped the rest.

## Maintainers

The health of the sources of the registry is described in
[the maintainer documentation](https://github.com/cnlangzi/knownbots-rust/blob/main/docs/maintainers.md).

## License

MIT.
