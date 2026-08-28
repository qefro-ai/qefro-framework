use crate::error::{QefroError, QefroResult};

/// Validate that `name` is a safe SQL identifier (unquoted internally, quoted
/// at the SQL boundary). Never pass untrusted input through this without
/// validation — the query builder rejects anything that fails.
pub fn assert_safe_ident(name: &str) -> QefroResult<()> {
    if name.is_empty() || name.len() > 63 {
        return Err(QefroError::bad_request(format!(
            "invalid identifier '{name}': length must be 1..=63"
        )));
    }
    let mut chars = name.chars();
    let first = chars.next().expect("non-empty");
    if !first.is_ascii_lowercase() && first != '_' {
        return Err(QefroError::bad_request(format!(
            "invalid identifier '{name}': must start with a lowercase letter or underscore"
        )));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        return Err(QefroError::bad_request(format!(
            "invalid identifier '{name}': only [a-z0-9_] are allowed"
        )));
    }
    Ok(())
}

/// Quote a validated identifier for PostgreSQL.
pub fn quote_ident(name: &str) -> QefroResult<String> {
    assert_safe_ident(name)?;
    Ok(format!("\"{name}\""))
}

pub fn snake_case(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else if c == '-' || c == ' ' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out.to_ascii_lowercase()
}

pub fn kebab_case(name: &str) -> String {
    snake_case(name).replace('_', "-")
}

pub fn slugify(name: &str) -> String {
    kebab_case(name)
}

/// Naive English pluralization for URL slugs: Customer -> customers, MenuItem -> menu-items.
pub fn to_plural_slug(name: &str) -> String {
    let kebab = kebab_case(name);
    if kebab.ends_with('s') {
        kebab
    } else if kebab.ends_with('y') && kebab.len() > 1 {
        let stem = &kebab[..kebab.len() - 1];
        if stem.ends_with(|c: char| "aeiou".contains(c)) {
            format!("{kebab}s")
        } else {
            format!("{stem}ies")
        }
    } else {
        format!("{kebab}s")
    }
}

/// Tiny Levenshtein distance for "did you mean" suggestions.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

pub fn suggest_similar<'a>(
    needle: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let needle_l = needle.to_ascii_lowercase();
    if needle_l.is_empty() {
        return None;
    }
    let mut contains: Option<(usize, String)> = None;
    let mut best: Option<(usize, String)> = None;
    for c in candidates {
        let cl = c.to_ascii_lowercase();
        if cl == needle_l {
            continue;
        }
        if cl.contains(&needle_l) {
            let score = cl.len().saturating_sub(needle_l.len());
            if contains.as_ref().map(|(s, _)| score < *s).unwrap_or(true) {
                contains = Some((score, c.to_string()));
            }
        }
        let dist = levenshtein(&needle_l, &cl);
        if best.as_ref().map(|(d, _)| dist < *d).unwrap_or(true) {
            best = Some((dist, c.to_string()));
        }
    }
    if let Some((_, name)) = contains {
        return Some(name);
    }
    best.filter(|(d, _)| *d > 0 && *d <= 4).map(|(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsafe_idents() {
        assert!(assert_safe_ident("customers").is_ok());
        assert!(assert_safe_ident("menu_items").is_ok());
        assert!(assert_safe_ident("Customers").is_err());
        assert!(assert_safe_ident("menu-items").is_err());
        assert!(assert_safe_ident("drop table").is_err());
        assert!(assert_safe_ident("id;delete").is_err());
        assert!(assert_safe_ident("").is_err());
    }

    #[test]
    fn slug_helpers() {
        assert_eq!(to_plural_slug("Customer"), "customers");
        assert_eq!(to_plural_slug("MenuItem"), "menu-items");
        assert_eq!(to_plural_slug("Opportunity"), "opportunities");
        assert_eq!(snake_case("MenuItem"), "menu_item");
    }

    #[test]
    fn did_you_mean() {
        let names = ["DiningTable", "Customer", "Reservation"];
        assert_eq!(
            suggest_similar("Table", names).as_deref(),
            Some("DiningTable")
        );
        assert!(suggest_similar("zzzzzzzz", names).is_none());
    }
}
