use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

const SUGGESTION_COUNT: usize = 5;
const SUFFIX_BONUS: i64 = 1000;

#[derive(Debug, Clone)]
pub struct SignalNotFound {
    pub signal: String,
    pub suggestions: Vec<String>,
}

impl std::fmt::Display for SignalNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.suggestions.is_empty() {
            write!(f, "signal '{}' not found", self.signal)
        } else {
            write!(
                f,
                "signal '{}' not found. Similar signals (hierarchical names): {}. Which one did you mean?",
                self.signal,
                self.suggestions
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

impl std::error::Error for SignalNotFound {}

pub struct FuzzyMatch;

impl FuzzyMatch {
    pub fn find_top_n(signal: &str, candidates: &[String]) -> Vec<String> {
        let matcher = SkimMatcherV2::default();
        let mut scored: Vec<(i64, &String)> = candidates
            .iter()
            .filter_map(|c| {
                matcher.fuzzy_match(c, signal).map(|score| {
                    let suffix_bonus = if c.ends_with(signal) { SUFFIX_BONUS } else { 0 };
                    (score.saturating_add(suffix_bonus), c)
                })
            })
            .collect();
        scored.sort_by_key(|(score, _)| *score);
        scored
            .into_iter()
            .rev()
            .take(SUGGESTION_COUNT)
            .map(|(_, name)| name.clone())
            .collect()
    }
}
