//! The crawlers the registry verifies by the reverse DNS of their vendor.
//!
//! The table is shared by the tests that read it, so that the fields of a
//! definition and the reverse name of its sample are stated once.

use knownbots::BotKind;

/// A crawler the registry verifies by the reverse DNS of its vendor.
///
/// The table of the crawlers is shared by the tests, so that the fields of a
/// definition and the reverse name of its sample are stated once.
pub struct DnsCrawler {
    /// The id of the crawler in the registry.
    pub name: &'static str,
    /// The category of the crawler.
    pub kind: BotKind,
    /// The organisation that owns the crawler.
    pub vendor: &'static str,
    /// The marker the `User-Agent` of a request has to carry.
    pub marker: &'static str,
    /// The domains the reverse name of the crawler has to end in.
    pub domains: &'static [&'static str],
    /// The address the crawler was confirmed with.
    pub sample: &'static str,
    /// The reverse name of the sample.
    pub hostname: &'static str,
}

impl DnsCrawler {
    /// Whether the reverse name of the sample ends in one of the domains of the
    /// crawler.
    pub fn sample_is_under_a_domain(&self) -> bool {
        let domains: Vec<String> = self
            .domains
            .iter()
            .map(|domain| domain.to_string())
            .collect();
        knownbots::match_domain(self.hostname, &domains)
    }
}

/// The crawlers the registry verifies by the reverse DNS of their vendor.
pub const DNS_CRAWLERS: &[DnsCrawler] = &[
    DnsCrawler {
        name: "censysinspect",
        kind: BotKind::Security,
        vendor: "Censys",
        marker: "CensysInspect",
        domains: &["censys-scanner.com"],
        sample: "162.142.125.1",
        hostname: "1.125.142.162.censys-scanner.com",
    },
    DnsCrawler {
        name: "dataforseobot",
        kind: BotKind::SEO,
        vendor: "DataForSEO",
        marker: "DataForSeoBot",
        domains: &["dataforseo.com"],
        sample: "136.243.228.176",
        hostname: "crawling-gateway-136-243-228-176.dataforseo.com",
    },
    DnsCrawler {
        name: "hetrixtools",
        kind: BotKind::Monitor,
        vendor: "HetrixTools",
        marker: "HetrixTools",
        domains: &["hetrixtools.com"],
        sample: "52.207.41.187",
        hostname: "wk1.hetrixtools.com",
    },
    DnsCrawler {
        name: "meta-externalfetcher",
        kind: BotKind::AIAssist,
        vendor: "Meta",
        marker: "meta-externalfetcher",
        domains: &["fbsv.net", "tfbnw.net", "facebook.com"],
        sample: "66.220.144.40",
        hostname: "corpout3.snc1.tfbnw.net",
    },
    DnsCrawler {
        name: "meta-webindexer",
        kind: BotKind::AIAssist,
        vendor: "Meta",
        marker: "meta-webindexer",
        domains: &["fbsv.net", "tfbnw.net", "facebook.com"],
        sample: "66.220.144.40",
        hostname: "corpout3.snc1.tfbnw.net",
    },
    DnsCrawler {
        name: "semrushbot",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SemrushBot",
        domains: &["semrush.com"],
        sample: "185.191.171.2",
        hostname: "2.bl.bot.semrush.com",
    },
    DnsCrawler {
        name: "semrushbot-ba",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SemrushBot-BA",
        domains: &["semrush.com"],
        sample: "85.208.98.16",
        hostname: "8.bat.bot.semrush.com",
    },
    DnsCrawler {
        name: "semrushbot-si",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SemrushBot-SI",
        domains: &["semrush.com"],
        sample: "85.208.98.49",
        hostname: "1.si.bot.semrush.com",
    },
    DnsCrawler {
        name: "semrushbot-swa",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SemrushBot-SWA",
        domains: &["semrush.com"],
        sample: "85.208.98.52",
        hostname: "1.swa.bot.semrush.com",
    },
    DnsCrawler {
        name: "siteauditbot",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SiteAuditBot",
        domains: &["semrush.com"],
        sample: "85.208.98.200",
        hostname: "72.siteaudit.bot.semrush.com",
    },
    DnsCrawler {
        name: "splitsignalbot",
        kind: BotKind::SEO,
        vendor: "Semrush",
        marker: "SplitSignalBot",
        domains: &["semrush.com"],
        sample: "85.208.98.48",
        hostname: "1.ss.bot.semrush.com",
    },
    DnsCrawler {
        name: "seznambot",
        kind: BotKind::SearchEngine,
        vendor: "Seznam",
        marker: "SeznamBot",
        domains: &["seznam.cz"],
        sample: "77.75.77.95",
        hostname: "fulltextrobot-77-75-77-95.seznam.cz",
    },
    DnsCrawler {
        name: "seokicks",
        kind: BotKind::SEO,
        vendor: "SEOkicks",
        marker: "SEOkicks",
        domains: &["seokicks.de"],
        sample: "65.21.226.48",
        hostname: "cp1.seokicks.de",
    },
    DnsCrawler {
        name: "sitebulb",
        kind: BotKind::SEO,
        vendor: "Sitebulb",
        marker: "Sitebulb",
        domains: &["sitebulb.com"],
        sample: "78.40.58.130",
        hostname: "crawl-78-40-58-130.uk.sitebulb.com",
    },
    DnsCrawler {
        name: "yeti",
        kind: BotKind::SearchEngine,
        vendor: "Naver",
        marker: "Yeti",
        domains: &["naver.com"],
        sample: "125.209.235.169",
        hostname: "crawl.125-209-235-169.web.naver.com",
    },
];
