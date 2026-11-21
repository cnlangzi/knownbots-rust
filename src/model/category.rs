//! The coarse bucket the crate groups a crawler into.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The table of the categories and of their wire labels.
///
/// The macro generates the type and everything that reads a label, so that a
/// variant cannot be added without its label.
macro_rules! bot_categories {
    ($($variant:ident => $label:literal, $doc:literal,)*) => {
        /// The coarse bucket the crate groups a crawler into.
        ///
        /// The category of a crawler is a fact of that crawler, stated in its
        /// definition: the crate never derives it. The category is coarse — the
        /// fine-grained description of a crawler is its [`BotKind`], which says
        /// what the crawler does on the web.
        ///
        /// The wire label of a category is a contract a host persists: the
        /// labels of the vocabulary are `search_engine`, `ai_bot`, `social`,
        /// `monitoring` and `ads_bot`.
        ///
        /// [`BotKind`]: crate::BotKind
        #[non_exhaustive]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum BotCategory {
            $(
                #[doc = concat!($doc, " The wire label is `", $label, "`.")]
                $variant,
            )*
        }

        impl BotCategory {
            /// Every category of the vocabulary.
            pub const ALL: &[BotCategory] = &[$(BotCategory::$variant,)*];

            /// The wire label of the category.
            ///
            /// The label is the form a host persists: it does not change inside
            /// a release line.
            pub fn as_str(&self) -> &'static str {
                match self {
                    $(BotCategory::$variant => $label,)*
                }
            }
        }

        impl std::fmt::Display for BotCategory {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl Serialize for BotCategory {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for BotCategory {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let label = String::deserialize(deserializer)?;
                Self::ALL
                    .iter()
                    .copied()
                    .find(|category| category.as_str() == label)
                    .ok_or_else(|| D::Error::custom(format!("unknown bot category {label:?}")))
            }
        }
    };
}

bot_categories! {
    SearchEngine => "search_engine",
        "A crawler of a general web search engine.",
    AiBot => "ai_bot",
        "A crawler that fetches for an assistant or collects content for the training of a model.",
    Social => "social",
        "A crawler of a network of sites.",
    Monitoring => "monitoring",
        "A crawler that watches a site, checks its security or collects its pages.",
    AdsBot => "ads_bot",
        "A crawler that checks the landing pages of an advertisement.",
}
