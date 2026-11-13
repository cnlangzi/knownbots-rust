# knownbots

[![crates.io](https://img.shields.io/crates/v/knownbots.svg)](https://crates.io/crates/knownbots)

`knownbots` verifies whether a request that claims to be a known crawler really
is one.

A crawler proves its identity in two steps: it names itself in the `User-Agent`
header, and it comes from an address the vendor published for that crawler. The
crate checks both steps. A request that names a known crawler but comes from an
address the vendor does not publish is not that crawler.

The crate is a Rust port of the upstream library
[`github.com/cnlangzi/knownbots`](https://github.com/cnlangzi/knownbots).

## Install

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
answers `Pending`, and a later request may succeed.

| status | meaning |
|---|---|
| `Verified` | The marker matched and the address belongs to the crawler. |
| `Failed` | The marker matched, the address does not belong to the crawler. |
| `Pending` | The marker matched, the reverse DNS decides and the cache is cold. |
| `Unknown` | The marker names no crawler of the registry. |

Only `Verified` says that the request is the crawler it claims to be.

## Getting an answer

```rust
use std::net::IpAddr;

use knownbots::{Validator, ValidatorOptions, VerifyStatus};

let validator = Validator::new(ValidatorOptions {
    root: "/var/lib/knownbots".into(),
    ..ValidatorOptions::default()
})
.expect("the validator");

let result = validator.verify(
    "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)",
    "66.249.66.1".parse::<IpAddr>().unwrap(),
);

match result.status {
    VerifyStatus::Verified => println!("{} of {}", result.name, result.vendor),
    _ => println!("not a known crawler"),
}
```

The background work — the download of the lists and the reverse lookups — starts
separately, once a runtime exists:

```rust
validator.start_background();
```

## The registry

The registry holds the crawlers the crate knows. Every crawler is a YAML file
of the `conf.d/` directory of this repository, and every file of that directory
is compiled into the binary.

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

The parsers the crate knows are `google`, `openai`, `uptimerobot`, `ahrefs`,
`amazon` and `txt` (plain CIDR lines).

A host may add crawlers of its own: a YAML file of the `conf.d/` directory of
the root of the validator replaces the definition of the crawler with the same
name. A definition that cannot be read is skipped and reported by
`Validator::config_errors()` — a typo in a definition of the host never takes
the host down.

## The lists

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

## The background work

The scheduler refreshes the lists of the crawlers that are backed by a document.
The first pass runs immediately, so that the crawlers are usable within seconds
of the start of the process. A failed refresh is retried with an exponential
backoff.

The reverse lookups run on the runtime of the host, never on the request path.
The answers are cached on disk, so that a restart does not pay for them again.

## Observability

A host that watches the lists reads the state of the registry:

```rust
for status in validator.refresh_status() {
    println!(
        "{}: {:?} ({} prefixes)",
        status.name, status.state, status.prefixes
    );
}
```

| call | answers |
|---|---|
| `Validator::refresh_status()` | The state of the address space of every crawler. |
| `Validator::last_refresh_error(name)` | Why the last refresh of a crawler failed. |
| `Validator::config_errors()` | The definitions of the host that were skipped. |
| `Validator::rdns_overflow()` | The reverse lookups that were dropped because the queue was full. |

## Configuration

| option | default | meaning |
|---|---|---|
| `root` | `./knownbots` | The directory that holds the caches and the definitions of the host. |
| `fail_limit` | `1000` | How many failed reverse lookups a crawler remembers. |
| `refresh_interval` | `24h` | How often the published lists are refreshed. |

## License

MIT.
