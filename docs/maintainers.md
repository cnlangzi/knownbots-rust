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

A crawler whose vendor publishes reverse names that never confirm forward is not
carried by the reverse DNS of its vendor: the registry would carry a name that
matches traffic the crate can never verify. Such a crawler is carried by the
address space its vendor publishes — as a `static` crawler — or not at all.

## The judgement of a static crawler

A crawler whose source is the prefixes its own definition states has no
upstream for the check to read, so its row is `skipped`. Its address space is
what the definition states, and nothing notices when the vendor changes it: a
network the vendor drops stays in the definition, and a network the vendor adds
is missing from it. A request from an address outside the stated networks fails
closed, as it does for every crawler whose address space the crate does not
cover.

## The issue

The script renders the table of the report and keeps an issue labelled
`provider-health` open while a source of the registry is unhealthy. The issue
holds the table of the run; it is closed when every source is healthy again.
The script exits with a non-zero status when a source is unhealthy, so the
workflow fails with it.

## The admission of a crawler

A crawler enters the registry on evidence. Its token has to appear on a page
of its vendor — a documentation page or the vendor's own `robots.txt`; a
directory of crawlers run by a third party is somebody's snapshot and is not a
source. An address of the crawler has to carry a reverse name that ends in a
domain the vendor owns, and that name has to resolve forward to the address
itself. The definition names the domain and the address, and the health check
judges the crawler by that address.

A token that does not meet every one of these conditions is not in the
registry, and the registry says nothing about it: a request that claims such a
crawler is an ordinary request.

### The crawlers the registry carries

The kind of a crawler follows from the wording of the page of its vendor that
supplied the token. The line that decides it is quoted in the last column.

| token | vendor | kind | the page of the vendor | the line that decides the kind |
|---|---|---|---|---|
| `CensysInspect` | Censys | Security | https://docs.censys.com/docs/opt-out-of-data-collection | "Censys regularly probes every public IP address and website, curate and enrich the resulting data, and make it intelligible to defenders." |
| `DataForSeoBot` | DataForSEO | SEO | https://dataforseo.com/dataforseo-bot | "DataForSEO Bot is constantly crawling the web to add new links to our backlink database and check links that are already included." |
| `HetrixTools` | HetrixTools | Monitor | https://hetrixtools.com/uptime-monitor-bot/ | "A HetrixTools user added your website as an uptime monitor. Our bot checks that one address on a schedule, and alerts them when it stops responding" |
| `HySpider` | Tencent | AITraining | https://hy.tencent.com/polices/hy-crawlers | "Hy 抓取的内容可能用于训练 Hy 的基础模型。" — the content HySpider crawls may be used to train the foundation models of Hy |
| `KimiBot` | Moonshot | AITraining | https://www.kimi.com/policies/kimi-crawlers | "KimiBot crawls content potentially used to train Kimi's foundation models." |
| `Kimi-SearchBot` | Moonshot | AIAssist | https://www.kimi.com/policies/kimi-crawlers | "Kimi-SearchBot powers Kimi's search features: it analyzes pages for relevance and builds the search index." |
| `Kimi-User` | Moonshot | AIAssist | https://www.kimi.com/policies/kimi-crawlers | "Kimi-User handles user-initiated actions in Kimi (for example, asking Kimi to summarize a specific article or answer a question that requires live web retrieval)." |
| `meta-externalfetcher` | Meta | AIAssist | https://developers.facebook.com/docs/sharing/webmasters/web-crawlers/ | "fetches individual links at a user's request and supports product functions such as evaluating and improving agentic AI capabilities" |
| `meta-webindexer` | Meta | AIAssist | https://developers.facebook.com/docs/sharing/webmasters/web-crawlers/ | "navigates the web to improve Meta AI search result quality for users ... helps us cite and link to your content in Meta AI's responses" |
| `SEOkicks` | SEOkicks | SEO | https://www.seokicks.de/robot.html | "Der SEOkicks Crawler durchsucht Websites gezielt nach externen Verlinkungen zu anderen Domains und erfasst diese inkl. Linktext und Linkattributen in einer Datenbank." — the SEOkicks crawler looks for the external links of a website and records them, with their link text and their link attributes, in a database |
| `SeznamBot` | Seznam | SearchEngine | https://o-seznam.cz/napoveda/vyhledavani/en/seznambot-crawler/ | "SeznamBot is a robot (also known as web crawler or web spider) – a program that retrieves content for our search engine" |
| `Sitebulb` | Sitebulb | SEO | https://support.sitebulb.com/en/articles/9993764-sitebulb-s-user-agent | "Sitebulb is a website crawling tool designed to help SEO professionals efficiently audit websites and find opportunities for improvement." |
| `Yeti` | Naver | SearchEngine | https://searchadvisor.naver.com/guide/seo-basic-firewall | "일반적인 네이버 검색 서비스를 위해 웹 문서를 수집하는 검색로봇은 Yeti라고 불리우며" — the search robot that collects web documents for the Naver search service is called Yeti |

The vendors of `SeznamBot` and of `Yeti` publish an address space of their own
— https://search.seznam.cz/ipranges/seznambot.json and
https://searchadvisor.naver.com/doc/naverbot.json — and the definitions of
the two crawlers name the reverse DNS of their vendors only; a definition that
carries the list would state two sources, where the registry states one.

### The tokens the registry does not carry

The condition a token failed is named in the third column. The evidence is the
reverse name of an address and its forward lookup, and the page of the vendor
that was searched.

| token | vendor | the condition it failed | the evidence |
|---|---|---|---|
| `FacebookBot` | Meta | no page of the vendor names the token | the crawler page https://developers.facebook.com/docs/sharing/webmasters/web-crawlers/ names `facebookexternalhit`, `meta-webindexer`, `meta-externalads`, `meta-externalagent` and `meta-externalfetcher`, and the older page https://developers.facebook.com/docs/sharing/bot/ redirects to it; neither names `FacebookBot`. The addresses of the crawlers of Meta carry the reverse name `corpout3.snc1.tfbnw.net` for `66.220.144.40`, which resolves forward to `66.220.144.40`; no address of a `FacebookBot` was found |
| `BingPreview` | Microsoft | no User-Agent the vendor publishes carries the token | the crawler table of the vendor (https://www.bing.com/webmasters/help/which-crawlers-does-bing-use-8c184ec0, served as plain text by https://www.bing.com/webmasters/api/help/htmlcontent?ArticleId=8c184ec0) names the crawler "BingPreview" — "BingPreview generates page snapshots for Bing." — but the User-Agent strings it publishes for that crawler carry the token `bingbot`, which the registry holds under `bingbot`; the User-Agent that carried the token was "Mozilla/5.0 (Windows NT 6.1; WOW64) AppleWebKit/534+ (KHTML, like Gecko) BingPreview/1.0b", published by the post of the vendor that the table links to (https://blogs.bing.com/webmaster/2012/10/26/page-snapshots-in-bing-windows-8-app-to-bring-new-crawl-traffic-to-sites/, whose copy is archived at https://web.archive.org/web/20231211024110/https://blogs.bing.com/webmaster/2012/10/26/page-snapshots-in-bing-windows-8-app-to-bring-new-crawl-traffic-to-sites/), and that post is no longer published. The DNS side holds: the address `40.77.167.1` carries the reverse name `msnbot-40-77-167-1.search.msn.com`, which resolves forward to `40.77.167.1` |
| `DotBot` | Moz | no reverse name ends in a domain of the vendor | the page https://moz.com/help/moz-procedures/crawlers/dotbot names the crawler ("Dotbot is Moz's web crawler, it gathers web data for the Moz Link Index"), but the address `216.244.66.237` of the crawler carries the reverse name `216-244-66-237.reverse.wowrack.com`, which carries no address record and does not forward-confirm |
| `Qwantify` | Qwant | no page of the vendor publishes the token | the crawler page of the vendor (https://help.qwant.com/hc/en-us/articles/51146433634833) names the crawler "QwantBot" and publishes the User-Agents "Mozilla/5.0 (compatible; Qwantbot/1.0_12345; +https://help.qwant.com/bot/)" and "Mozilla/5.0 (compatible; Qwantbot-news/2.0; +https://help.qwant.com/bot/)"; the token survives only on the legal page https://about.qwant.com/en/legal/classement/, which calls the indexing robots of the "Qwantify" family. The reverse name of `194.187.171.99` is `qwantbot-99-171-187-194.qwant.com`, and the forward lookup answers a different address on every resolver (104.16.251.55, 128.242.245.29, 199.59.149.210), so it does not forward-confirm |
| `360Spider` | Qihoo 360 | no reverse name ends in a domain of the vendor | the page http://www.so.com/help/help_3_2.html names the token ("网页搜索 360Spider - 图片搜索 360Spider-Image - 视频搜索 360Spider-Video") and http://www.so.com/help/spider_ip.html publishes the ranges of the crawler, but the address `42.236.12.130` of those ranges carries the reverse name `hn.kd.ny.adsl`, which carries no address record and does not forward-confirm |
| `Bravebot` | Brave | no page of the vendor names the token | the crawler page of the vendor (https://search.brave.com/help/brave-search-crawler) states that the crawler "does not advertise a differentiated user agent because we must avoid discrimination from websites that allow only Google to crawl them", and no page of https://brave.com/ carries the token; the address `69.63.180.173` of https://search.brave.com/ carries no reverse name |
| `Diffbot` | Diffbot | no reverse name ends in a domain of the vendor | the page https://www.diffbot.com/docs/crawl/faq/robots-txt names the token in the table of the User-Agents, but the address `216.218.141.226` of the crawler carries no reverse name, and `216.218.147.201` carries the reverse name `201.subnet192.147.218.216.in-addr.arpa`, which carries no address record and does not forward-confirm |
| `AI2Bot` | Allen Institute for AI | no reverse name ends in a domain of the vendor | the page https://allenai.org/crawler publishes the User-Agent "Mozilla/5.0 (compatible) AI2Bot (+https://www.allenai.org/crawler)", but the address `207.148.6.129` of the crawler carries the reverse name `bc-prod-web-03.tempurl.host`, which resolves forward to `207.148.6.129` — the zone of a hosting provider, not of the vendor |
| `cohere-ai` | Cohere | no page of the vendor names the token | https://docs.cohere.com/docs/cohere-web-crawlers states "We do not use Cohere bots or user agents for the purpose of crawling or scraping web content to train generative AI foundation models at this time", and its table of bots names no token; the address `34.96.76.122` of https://api.cohere.com/ carries the reverse name `122.76.96.34.bc.googleusercontent.com`, which resolves forward to `34.96.76.122` — the zone of a hosting provider, not of the vendor |
| `cohere-training-data-crawler` | Cohere | no page of the vendor names the token | the token is on no page of https://docs.cohere.com/ and on no page of https://cohere.com/; only the crawler page https://docs.cohere.com/docs/cohere-web-crawlers is published, and it names no token. The address `34.96.76.122` of https://api.cohere.com/ carries the reverse name `122.76.96.34.bc.googleusercontent.com`, which resolves forward to `34.96.76.122` — the zone of a hosting provider, not of the vendor |
| `GrokBot` | xAI | no page of the vendor names the token | https://x.ai/robots.txt names no crawler, and the pages of https://docs.x.ai/ describe the Grok Bot product, not a crawler; the address `54.89.135.129` of https://grok.com/ carries the reverse name `ec2-54-89-135-129.compute-1.amazonaws.com`, which resolves forward to `54.89.135.129` — the zone of a hosting provider, not of the vendor |
| `xAI-Bot` | xAI | no page of the vendor names the token | https://x.ai/robots.txt names no crawler, and no page of https://docs.x.ai/ carries the token; the address `54.89.135.129` of https://grok.com/ carries the reverse name `ec2-54-89-135-129.compute-1.amazonaws.com`, which resolves forward to `54.89.135.129` — the zone of a hosting provider, not of the vendor |
| `DeepSeekBot` | DeepSeek | no page of the vendor names the token | https://www.deepseek.com/robots.txt names no crawler, and no page of https://www.deepseek.com/ or of https://api-docs.deepseek.com/ carries the token; the address `116.205.40.113` of https://api.deepseek.com/ carries the reverse name `ecs-116-205-40-113.compute.hwclouds-dns.com`, which resolves forward to `116.205.40.113` — the zone of a hosting provider, not of the vendor |
| `Timpibot` | Timpi | no page of the vendor names the token | https://timpi.io/robots.txt names no crawler, and no page of https://timpi.io/ carries the token; the address `80.241.212.221` of https://timpi.com/ carries the reverse name `vmi360447.contaboserver.net`, which resolves forward to `80.241.212.221` — the zone of a hosting provider, not of the vendor |
| `omgilibot` | Webz.io | no reverse name ends in a domain of the vendor | the page https://webz.io/blog/web-data/what-is-the-omgili-bot-and-why-is-it-crawling-your-website/ names the crawler and publishes the User-Agent "omgili/0.5 +https://omgili.com", but the address `109.226.36.107` of https://omgili.com/ carries the reverse name `static.109.226.36.107.ccc.net.il`, which carries no address record and does not forward-confirm |
| `img2dataset` | Hugging Face | no page of the vendor names the token | https://huggingface.co/robots.txt names no crawler and no page of https://huggingface.co/ carries the token; the address `118.193.202.219` of https://huggingface.co/ carries no reverse name |
| `Meltwater` | Meltwater | no page of the vendor names the token | https://www.meltwater.com/robots.txt names no crawler and no page of the sitemap of the vendor (350 pages) carries the token; the address `168.245.73.208` of the vendor carries the reverse name `o11.crm.meltwater.com`, which resolves forward to `168.245.73.208` — a host of the CRM product, not of the crawler |
| `Seekr` | Seekr | no page of the vendor names the token | https://www.seekr.com/robots.txt names no crawler and no page of the sitemap of the vendor carries the token; the address `3.129.232.104` of https://seekr.com/ carries the reverse name `ec2-3-129-232-104.us-east-2.compute.amazonaws.com`, which resolves forward to `3.129.232.104` — the zone of a hosting provider, not of the vendor |
| `PhindBot` | Phind | no page of the vendor names the token | every page of the vendor (https://www.phind.com/, https://www.phind.com/robots, https://www.phind.com/bot) answers with HTTP 403 to every reader, and the addresses of the vendor sit behind Cloudflare; the address `104.18.5.54` of https://www.phind.com/ carries no reverse name |
| `ProRataInc` | Gist | no reverse name ends in a domain of the vendor | the page https://platform.gist.ai/docs/prorata-ai-crawler-bot names the token ("The user-agent string that Prorata.ai uses is `ProRataInc`") and publishes the addresses `172.190.46.235` and `172.171.95.51`, but neither of them carries a reverse name |
| `iaskspider` | iAsk | no page of the vendor names the token | no page of https://iask.ai/ names the crawler; the address `123.253.106.157` of the crawler carries the reverse name `123.253.106.157.ptr.rootnetworks.com`, which carries no address record and does not forward-confirm |
| `Expanse` | Palo Alto Networks | no page of the vendor names the token, and no reverse name ends in a domain of the vendor | the page https://cortex-docs.paloaltonetworks.com/cortex-xpanse/reference/scanning-activity.md publishes the ranges of the scanner but names "Cortex Xpanse", not `Expanse`; the address `35.203.210.10` of those ranges carries the reverse name `10.210.203.35.bc.googleusercontent.com`, which resolves forward to `35.203.210.10` — the zone of a hosting provider, not of the vendor |
| `Sucuri` | Sucuri | no page of the vendor publishes the token | the page https://docs.sucuri.net/website-monitoring/server-side-scanner/ publishes the User-Agent "Sucuri Server Side Scanner", not the token `Sucuri`; the address `192.124.249.16` of the vendor carries the reverse name `cloudproxy10016.sucuri.net`, which resolves forward to `192.124.249.16` among five addresses of the range — the DNS side holds, the token does not |
| `Wordfence` | Wordfence | no page of the vendor names the token, and no reverse name ends in a domain of the vendor | the pages of the vendor (https://www.wordfence.com/threat-intelligence/, https://www.wordfence.com/help/faq/, https://www.wordfence.com/blog/) answer with HTTP 202 behind a challenge of the network, and https://www.wordfence.com/robots.txt names no crawler; the address `54.190.24.63` of the vendor carries the reverse name `ec2-54-190-24-63.us-west-2.compute.amazonaws.com`, which resolves forward to `54.190.24.63` — the zone of a hosting provider, not of the vendor |
| `Screaming Frog SEO Spider` | Screaming Frog | no address of the crawler carries a reverse name | the page https://www.screamingfrog.co.uk/seo-spider/user-agent/ names the token ("Its user agent is ‘Screaming Frog SEO Spider’"), but the SEO Spider is a desktop application that crawls from the machine of the user, so the vendor operates no address of a crawler; the address `104.26.2.186` of https://www.screamingfrog.co.uk/ carries no reverse name |
| `Sistrix` | Sistrix | no page of the vendor names the token | https://www.sistrix.com/robots.txt names no crawler and no page of the sitemaps of the vendor carries the token; the address `167.235.94.202` of https://www.sistrix.com/ carries the reverse name `www.sistrix.com`, which resolves forward to `167.235.94.202` — the address of the web server, not of a crawler |
| `SerpstatBot` | Serpstat | no page of the vendor publishes the token | the FAQ of the vendor (https://serpstat.com/page/serpstatbot-faq/) names the crawler "Serpstatbot" and states "Serpstatbot does not have a set of static IP addresses"; the token `SerpstatBot` is on no page of the vendor. The address `78.46.102.85` of https://serpstat.com/ carries the reverse name `serpstat.com`, which resolves forward to `78.46.102.85` — the address of the web server, not of a crawler |
| `BLEXBot` | BLEX | no page of the vendor names the token | https://blex.com/robots.txt names no crawler of the vendor and no page of https://blex.com/ carries the token; the address `206.196.113.24` of https://blex.com/ carries the reverse name `static-206-196-113-24.primary.net`, which carries no address record and does not forward-confirm |
| `GTmetrix` | GTmetrix | no page of the vendor names the token | https://gtmetrix.com/robots.txt names no crawler of the vendor and no page of the sitemap of the vendor carries the token; the address `104.20.27.222` of https://gtmetrix.com/ carries no reverse name |
| `Chrome-Lighthouse` | Google | no page of the vendor names the token | Google's page of its crawlers (https://developers.google.com/crawling/docs/crawlers-fetchers/overview-google-crawlers) and the Lighthouse documentation (https://developer.chrome.com/docs/lighthouse/) name no such crawler; the address `66.249.66.1` of the crawlers of Google carries the reverse name `crawl-66-249-66-1.googlebot.com`, which resolves forward to `66.249.66.1` — the address of another crawler of the vendor |
| `StatusCake` | StatusCake | no reverse name resolves forward to its address | the page https://www.statuscake.com/kb/knowledge-base/testing-websites-using-incapsula/ publishes the User-Agent "Mozilla/5.0 (Windows NT 6.2; WOW64) AppleWebKit/537.4 (KHTML, like Gecko) Chrome/98 Safari/537.4 (StatusCake)", but the address `167.99.242.41` carries the reverse name `de6.pagespeed.statuscake.com`, which carries no address record and does not forward-confirm |
| `Site24x7` | Site24x7 | no page of the vendor names the token | no page of https://www.site24x7.com/ publishes a User-Agent of the crawler; the address `203.29.243.116` of the monitoring nodes of the vendor carries the reverse name `au20.enduserexp.com`, which resolves forward to `203.29.243.116` — a domain of the Zoho group, not of Site24x7 |
| `BetterStack` | Better Stack | no page of the vendor publishes the token | the FAQ of https://betterstack.com/docs/uptime/frequently-asked-questions/ publishes the User-Agent "Better Stack Better Uptime Bot", not `BetterStack`; the address `167.254.229.55` of the vendor carries the reverse name `edge-us-west-1.uptime.betterstack.com`, which resolves forward to `167.254.229.55` — the DNS side holds, the token does not |
| `archive.org_bot` | Internet Archive | no reverse name resolves forward to its address | the page https://archive.org/details/archive.org_bot names the token ("User Agent archive.org_bot is used for our wide crawl of the web."), but the address `207.241.237.179` carries the reverse name `crawl910.us.archive.org`, and the forward lookup answers a different address on every resolver (31.13.95.35, 118.193.202.219, 157.240.10.36), so the name never resolves back to the address it names |
| `RyteBot` | Semrush | no reverse name ends in a domain of the vendor | the page of the vendor https://www.semrush.com/bot.html names the token ("To block RyteBot from crawling your site for tools provided by Ryte.com"), but the crawler belongs to Ryte and runs on shared infrastructure: the 42 addresses it was seen on carry the reverse name of an Amazon instance, of a Google App Engine instance, of an unrelated host or no reverse name at all, and none of them carries a name under `semrush.com`. The addresses and their reverse names are listed in https://github.com/cnlangzi/knownbots-rust/issues/51 |
| `SemrushBot-ESI` | Semrush | no reverse name ends in a domain of the vendor | the page of the vendor https://www.semrush.com/bot.html names the token ("To block SemrushBot-ESI from crawling your site for services provided by Semrush Enterprise Site Intelligence"), but the vendor names no host for the product: `1.esi.bot.semrush.com` answers the wildcard of the zone, `34.120.45.191`, whose reverse name is `191.45.120.34.bc.googleusercontent.com` — the zone of a hosting provider, not of the vendor — and no address of the crawler was observed |
| `SemrushBot-FT` | Semrush | no reverse name ends in a domain of the vendor | the page of the vendor https://www.semrush.com/bot.html names the token ("To block SemrushBot-FT from crawling your site for Plagiarism Checker and similar tools"), but the vendor names no host for the product: `1.ft.bot.semrush.com` answers the wildcard of the zone, `34.120.45.191`, whose reverse name is `191.45.120.34.bc.googleusercontent.com` — the zone of a hosting provider, not of the vendor — and no address of the crawler was observed |
| `SemrushBot-OCOB` | Semrush | no reverse name ends in a domain of the vendor | the page of the vendor https://www.semrush.com/bot.html names the token ("To block SemrushBot-OCOB from crawling your site for Content Toolkit"), but the vendor names no host for the product: `1.ocob.bot.semrush.com` answers the wildcard of the zone, `34.120.45.191`, whose reverse name is `191.45.120.34.bc.googleusercontent.com` — the zone of a hosting provider, not of the vendor — and no address of the crawler was observed |
