//! "Did you mean ...?" for what is typed into the CLI and syncix.toml.
//!
//! Why: a slip such as `syncix staus`, `set Box Color oragne`, `new Prat` or
//! `ls Workspace.Tycons` ended in a bare "not found" or "not a colour", and an AI driving
//! the CLI guessed again, often wrong. The closest known spellings are offered instead.
//! Nothing is corrected silently: a guess applied without asking is how a wrong value
//! lands in a place unnoticed ("rad" could be red or tan).

/// Edit distance between two spellings, case-insensitive: inserting, deleting or changing
/// a letter, or swapping two neighbours ("oragne"), counts one each.
pub fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().flat_map(char::to_lowercase).collect();
    let b: Vec<char> = b.chars().flat_map(char::to_lowercase).collect();
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, cell) in d[0].iter_mut().enumerate() {
        *cell = j;
    }
    for (i, ca) in a.iter().enumerate() {
        for (j, cb) in b.iter().enumerate() {
            let mut best = (d[i][j + 1] + 1)
                .min(d[i + 1][j] + 1)
                .min(d[i][j] + usize::from(ca != cb));
            if i > 0 && j > 0 && *ca == b[j - 1] && a[i - 1] == *cb {
                best = best.min(d[i - 1][j - 1] + 1);
            }
            d[i + 1][j + 1] = best;
        }
    }
    d[a.len()][b.len()]
}

/// How far a spelling may be from what was typed and still be offered: one slip in a
/// short word, more in a long one. Two letters are too few to guess from.
fn allowed(typed_len: usize) -> usize {
    match typed_len {
        0..=2 => 0,
        3..=4 => 1,
        5..=8 => 2,
        _ => 3,
    }
}

/// The known spellings closest to `typed`, best first, at most three. A known name that
/// starts with what was typed counts too ("Transp" -> Transparency), after every near
/// spelling. Empty when nothing is close enough.
pub fn closest<'a>(typed: &str, candidates: impl IntoIterator<Item = &'a str>) -> Vec<&'a str> {
    let typed = typed.trim();
    let typed_len = typed.chars().count();
    let limit = allowed(typed_len);
    let lower = typed.to_lowercase();

    let mut scored: Vec<(usize, &'a str)> = Vec::new();
    for candidate in candidates {
        let length_gap = candidate.chars().count().abs_diff(typed_len);
        let score = if length_gap <= limit && distance(typed, candidate) <= limit {
            distance(typed, candidate)
        } else if typed_len >= 4 && candidate.to_lowercase().starts_with(&lower) {
            limit + 1
        } else {
            continue;
        };
        if !scored.iter().any(|(_, known)| known.eq_ignore_ascii_case(candidate)) {
            scored.push((score, candidate));
        }
    }
    scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.len().cmp(&b.1.len())).then(a.1.cmp(b.1)));
    // Only the best tier: "Size" beats "Side" and "Sine" for "Szie", so they are not offered.
    let best = scored.first().map(|(score, _)| *score);
    scored
        .into_iter()
        .filter(|(score, _)| Some(*score) == best)
        .take(3)
        .map(|(_, candidate)| candidate)
        .collect()
}

/// "Did you mean X?", "Did you mean X or Y?", "Did you mean X, Y or Z?"; None for none.
pub fn did_you_mean<S: AsRef<str>>(options: &[S]) -> Option<String> {
    let names: Vec<&str> = options.iter().map(AsRef::as_ref).collect();
    match names.as_slice() {
        [] => None,
        [one] => Some(format!("Did you mean {}?", one)),
        [init @ .., last] => Some(format!("Did you mean {} or {}?", init.join(", "), last)),
    }
}

/// closest + did_you_mean in one step.
pub fn hint<'a>(typed: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    did_you_mean(&closest(typed, candidates))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slips_are_one_step_apart() {
        assert_eq!(distance("oragne", "orange"), 1);
        assert_eq!(distance("Szie", "Size"), 1);
        assert_eq!(distance("staus", "status"), 1);
        assert_eq!(distance("PART", "part"), 0);
        assert_eq!(distance("", "abc"), 3);
    }

    #[test]
    fn the_closest_spelling_is_offered() {
        let colours = ["red", "orange", "purple", "green"];
        assert_eq!(closest("oragne", colours), ["orange"]);
        assert_eq!(closest("gren", colours), ["green"]);
        assert!(closest("banana", colours).is_empty());
        // Only the best tier is offered.
        assert_eq!(closest("Szie", ["Size", "Side", "Shape"]), ["Size"]);
        // Too short to guess from.
        assert!(closest("rd", colours).is_empty());
    }

    #[test]
    fn a_typed_beginning_finds_the_long_name() {
        assert_eq!(closest("Transp", ["Transparency", "Anchored"]), ["Transparency"]);
        assert!(closest("Tra", ["Transparency"]).is_empty());
    }

    #[test]
    fn the_sentence_lists_every_option() {
        assert_eq!(did_you_mean::<&str>(&[]), None);
        assert_eq!(did_you_mean(&["a"]).unwrap(), "Did you mean a?");
        assert_eq!(did_you_mean(&["a", "b"]).unwrap(), "Did you mean a or b?");
        assert_eq!(did_you_mean(&["a", "b", "c"]).unwrap(), "Did you mean a, b or c?");
    }
}
