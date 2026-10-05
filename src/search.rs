//! Fuzzy name matching shared by the workspace index and the features that
//! search it. Lower scores are better matches.

/// Lower is better: 0 exact, 1 prefix, 2 substring, 3 subsequence; `None` for no match.
/// `query` must already be lowercase.
pub(crate) fn fuzzy_score(query: &str, name: &str) -> Option<u8> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) || !name.is_ascii() {
        return score_lowercase(query, &name.to_lowercase());
    }
    score_lowercase(query, name)
}

/// Same as [`fuzzy_score`] for a name that is already lowercase.
pub(crate) fn score_lowercase(query: &str, name: &str) -> Option<u8> {
    if !is_subsequence(query, name) {
        return None;
    }
    Some(if query.is_empty() {
        3
    } else if name == query {
        0
    } else if name.starts_with(query) {
        1
    } else if name.contains(query) {
        2
    } else {
        3
    })
}

fn is_subsequence(query: &str, name: &str) -> bool {
    let mut want = query.as_bytes().iter().peekable();
    for b in name.bytes() {
        match want.peek() {
            Some(&&q) if q == b => {
                want.next();
            }
            Some(_) => {}
            None => break,
        }
    }
    want.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::fuzzy_score;

    #[test]
    fn fuzzy_ranks() {
        assert_eq!(fuzzy_score("point", "point"), Some(0));
        assert_eq!(fuzzy_score("po", "Point-X"), Some(1));
        assert_eq!(fuzzy_score("int", "point-x"), Some(2));
        assert_eq!(fuzzy_score("mkp", "make-point"), Some(3));
        assert_eq!(fuzzy_score("mkp", "mapcar-safe"), None);
        assert_eq!(fuzzy_score("", "x"), Some(3));
        assert_eq!(fuzzy_score("ö", "Öl"), Some(1));
        assert_eq!(fuzzy_score("öx", "Ölx"), Some(3));
    }
}
