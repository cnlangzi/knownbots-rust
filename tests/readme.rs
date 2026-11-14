//! The README of the crate, held in step with the crate itself.
//!
//! The README is the documentation of the crate: its examples are the doctests
//! of the crate, and the facts it states about the crate are checked here.

mod common;

use std::path::Path;

use common::{Source, Sources};

/// The header row of the table of the crawlers.
const HEADER: &str = "| bot | vendor | kind | marker | source |";

/// The line that opens the list of the parsers of the crate.
const PARSERS: &str = "The parsers the crate knows are:";

/// The README of the crate.
fn readme() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    std::fs::read_to_string(&path).expect("the README must be readable")
}

/// The rows of the table of the crawlers, in the order of the README.
fn readme_rows(markdown: &str) -> Vec<Vec<String>> {
    let mut lines = markdown.lines();

    // The table is located by its header row, so that a renamed heading does
    // not break the test.
    let _header = lines
        .find(|line| line.trim() == HEADER)
        .expect("the README must hold the table of the crawlers");

    let mut rows = Vec::new();
    for line in lines {
        let line = line.trim();
        if !line.starts_with('|') {
            break;
        }
        // A row opens and closes with a `|`, so the cells are what lies
        // between them. A row that holds more than the five cells of the
        // header is not a row of this table.
        let cells: Vec<String> = line
            .trim_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_string())
            .collect();
        // The separator of the table holds no cell.
        if cells.first().is_some_and(|cell| cell == "---") {
            continue;
        }
        assert_eq!(
            cells.len(),
            5,
            "a row of the table holds five cells: {line}"
        );
        rows.push(cells);
    }
    rows
}

/// The rows of the table of the crawlers, as the registry holds them.
fn registry_rows() -> Vec<Vec<String>> {
    let bots = knownbots::load_bots(None).expect("the embedded registry must load");
    bots.iter()
        .map(|bot| {
            vec![
                bot.name.clone(),
                bot.vendor.clone(),
                format!("{:?}", bot.kind),
                bot.ua.clone(),
                Sources::of(bot).to_string(),
            ]
        })
        .collect()
}

/// The table of the crawlers of the README holds the registry of the crate,
/// field by field and row by row.
///
/// The table tells a host what the crate knows, so it has to be right: a
/// crawler that the registry gained and the README does not name is a crawler
/// no host can recognise.
#[test]
fn the_table_of_the_crawlers_holds_the_registry() {
    let rows = readme_rows(&readme());
    let expected = registry_rows();

    assert_eq!(
        rows.len(),
        expected.len(),
        "the table of the README holds {} rows, the registry holds {}",
        rows.len(),
        expected.len()
    );

    for (index, (row, expected)) in rows.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            row, expected,
            "row {index} of the table of the README does not match the registry"
        );
    }
}

/// A crawler the table calls `static` states its own prefixes.
///
/// The label is a promise: a crawler with no list and no reverse DNS that
/// states no prefix has no address space at all, and calling it `static`
/// would hide that.
#[test]
fn a_static_crawler_states_its_own_prefixes() {
    let bots = knownbots::load_bots(None).expect("the embedded registry must load");
    for bot in &bots {
        if Sources::of(bot).contains(&Source::Static) {
            assert!(
                !bot.custom.is_empty(),
                "the crawler {} has no address space at all",
                bot.name
            );
        }
    }
}

/// The parsers the README names are the parsers the crate knows.
///
/// The README is the documentation of the crate, so a parser it names that the
/// crate does not know is a promise the crate does not keep.
#[test]
fn the_parsers_of_the_readme_are_known_to_the_crate() {
    let markdown = readme();
    let lines: Vec<&str> = markdown.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.starts_with(PARSERS))
        .expect("the README must name the parsers of the crate");

    let mut parsers = Vec::new();
    for line in &lines[start + 1..] {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(name) = line.strip_prefix("- `") else {
            break;
        };
        parsers.push(name.split('`').next().unwrap_or_default().to_string());
    }

    assert!(!parsers.is_empty(), "the README must name the parsers");
    for parser in parsers {
        assert!(
            knownbots::feed::is_known_parser(&parser),
            "the README names the parser {parser}, which the crate does not know"
        );
    }
}
