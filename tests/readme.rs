//! The README of the crate, held in step with the crate itself.
//!
//! The README is the documentation of the crate: the examples in it are the
//! doctests of the crate, and the table of the crawlers it holds has to match
//! the registry the crate loads.

use std::path::Path;

use knownbots::Bot;

/// The header row of the table of the crawlers.
const HEADER: &str = "| bot | vendor | kind | marker | source |";

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
        let cells: Vec<String> = line
            .split('|')
            .skip(1)
            .map(|cell| cell.trim().to_string())
            .collect();
        // The separator of the table holds no cell.
        if cells.first().is_some_and(|cell| cell == "---") {
            continue;
        }
        // The row closes with a `|`, so its last field is empty.
        let cells = &cells[..cells.len() - 1];
        assert_eq!(
            cells.len(),
            5,
            "a row of the table holds five cells: {line}"
        );
        rows.push(cells.to_vec());
    }
    rows
}

/// The kind of the source a crawler depends on.
fn source_of(bot: &Bot) -> &'static str {
    if !bot.urls.is_empty() {
        "list"
    } else if bot.rdns {
        "dns"
    } else {
        "static"
    }
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
                source_of(bot).to_string(),
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
