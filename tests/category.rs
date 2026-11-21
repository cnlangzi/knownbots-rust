//! The vocabulary of the categories of a crawler.
//!
//! The crate owns the coarse buckets and their wire labels: a host that
//! persists a category uses the label of the crate. The tests hold the labels
//! fixed, so that a host that stored one of them keeps reading it.

use knownbots::BotCategory;

/// The labels of the vocabulary, with the category each of them belongs to.
const LABELS: [(&str, BotCategory); 5] = [
    ("search_engine", BotCategory::SearchEngine),
    ("ai_bot", BotCategory::AiBot),
    ("social", BotCategory::Social),
    ("monitoring", BotCategory::Monitoring),
    ("ads_bot", BotCategory::AdsBot),
];

/// The vocabulary holds the five buckets and nothing else.
#[test]
fn the_vocabulary_holds_five_categories() {
    assert_eq!(BotCategory::ALL.len(), 5);

    let mut labels: Vec<&str> = BotCategory::ALL.iter().map(|c| c.as_str()).collect();
    labels.sort_unstable();
    let mut expected: Vec<&str> = LABELS.iter().map(|(label, _)| *label).collect();
    expected.sort_unstable();
    assert_eq!(labels, expected);
}

/// The label of a category is the one the host persists.
///
/// The label is a contract: a host stores it in its own data, so it does not
/// change inside a release line.
#[test]
fn the_label_of_every_category_is_fixed() {
    for (label, category) in LABELS {
        assert_eq!(category.as_str(), label);
    }
}

/// The rendered category is its label.
#[test]
fn the_display_of_a_category_is_its_label() {
    for (label, category) in LABELS {
        assert_eq!(format!("{category}"), label);
        assert_eq!(format!("{category}"), category.as_str());
    }
}

/// The serialized category is its label, and the label reads back as the
/// category.
#[test]
fn the_serde_form_of_a_category_is_its_label() {
    for (label, category) in LABELS {
        assert_eq!(
            serde_json::to_string(&category).unwrap(),
            format!("\"{label}\"")
        );
        assert_eq!(
            serde_json::from_str::<BotCategory>(&format!("\"{label}\"")).unwrap(),
            category
        );
    }
}

/// The deserialization reads a string it owns.
///
/// A borrowed label fails at runtime when the document is read through a value
/// or when the label carries an escape, so the crate reads a string it owns.
#[test]
fn the_deserialization_reads_a_string_it_owns() {
    assert_eq!(
        serde_json::from_value::<BotCategory>(serde_json::json!("social")).unwrap(),
        BotCategory::Social
    );
    assert_eq!(
        serde_json::from_str::<BotCategory>(r#""social""#).unwrap(),
        BotCategory::Social
    );
    // The label `social`, with the `i` written as an escape.
    assert_eq!(
        serde_json::from_str::<BotCategory>(r#""soc\u0069al""#).unwrap(),
        BotCategory::Social
    );
}

/// A label the crate does not know is an error that names it.
#[test]
fn an_unknown_label_is_reported() {
    let error = serde_json::from_str::<BotCategory>("\"nope\"")
        .expect_err("a label the crate does not know must fail");
    let message = error.to_string();
    assert!(
        message.contains("nope"),
        "the error must name the label: {message}"
    );
}

/// A host matches a category with a fallback arm.
///
/// The vocabulary of the crate may grow, and a new bucket must not break the
/// match of a host: the host styles a category it does not know neutrally.
#[test]
fn a_host_matches_a_category_with_a_fallback_arm() {
    fn style(category: BotCategory) -> &'static str {
        match category {
            BotCategory::SearchEngine => "search",
            BotCategory::AiBot => "ai",
            BotCategory::Social => "social",
            BotCategory::Monitoring => "monitoring",
            BotCategory::AdsBot => "ads",
            _ => "other",
        }
    }

    for (_, category) in LABELS {
        assert_ne!(
            style(category),
            "other",
            "the host knows the category {}",
            category.as_str()
        );
    }
}
