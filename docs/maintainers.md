# Maintainers

Every crawler of the registry depends on an outside source: a published list of
addresses, or the DNS of a vendor. A crawler may depend on more than one of
them. The health check reads every source of the registry and writes the
outcome of the run to a report.

## The run

The check runs daily at 04:23 UTC, in the workflow
`.github/workflows/provider-health.yml`. The workflow runs the check and renders
the report:

```sh
cargo test --all-features --test provider_health -- --ignored
scripts/provider-health.sh target/provider-health/report.json
```

The check is an ignored test of the crate, so the ordinary suite of the crate
never depends on the uptime of a vendor. The command above runs it by hand; it
reaches the public internet, and every source it reads is the source a host of
the crate reads.

## The report

The run writes the report to `target/provider-health/report.json`. The report
holds one row per source of a crawler: a crawler that depends on two sources
holds two rows, and each of them carries its own state.

```json
{
  "checked_at": 1763012345,
  "sources": [
    {
      "name": "googlebot",
      "vendor": "Google",
      "kind": "SearchEngine",
      "source": "list",
      "state": "healthy",
      "fault": null,
      "prefixes": 1307,
      "detail": null
    }
  ]
}
```

| field | meaning |
|---|---|
| `checked_at` | The Unix time of the start of the run. |
| `name` | The id of the crawler in the registry. |
| `vendor` | The organisation that owns the crawler. |
| `kind` | The category of the crawler. |
| `source` | The source of the row: `list` (a published address space), `dns` (the reverse DNS of the vendor) or `static` (the crawler states its own prefixes). |
| `state` | `healthy`, `failed`, `skipped` (no upstream to check) or `pending` (the check did not reach the source). |
| `fault` | `environment` (the upstream or the network is at fault) or `crate` (the list arrived in a shape the crate mis-reads) — the two failures of the crate. |
| `prefixes` | The number of prefixes the crawler holds after the check. Every row of a crawler carries the same number. |
| `detail` | The reason of the outcome. |

## The judgement of a reverse DNS

A crawler whose source is the reverse DNS of its vendor is judged by the
address its definition states as its `sample:`, when it states one. The reverse
name of the sample has to resolve, has to be under one of the crawler's
`domains:`, and has to resolve forward back to the sample — the three
conditions the crate needs to answer for a reverse record. A crawler that
states no sample is judged by its domains, which are resolved in the forward
direction.

The sample is the stronger judgement. The zones of the crawlers that hold only
reverse names carry no address records of their own, so a domain that does not
resolve forward says nothing about the reverse names it holds. A sample that
stops confirming is a sample the definition has to replace.

## The issue

The script renders the table of the report and keeps an issue labelled
`provider-health` open while a source of the registry is unhealthy. The issue
holds the table of the run; it is closed when every source is healthy again.
The script exits with a non-zero status when a source is unhealthy, so the
workflow fails with it.
