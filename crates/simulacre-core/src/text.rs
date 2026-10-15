//! Small text utilities: template filling, ASCII folding for handles,
//! French gender agreement.

use std::collections::HashMap;

/// Replaces `{slot}` markers. Unknown slots are left visible on purpose so a
/// template/data mismatch shows up in tests instead of silently vanishing.
pub fn fill(template: &str, slots: &HashMap<&str, String>) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        match rest[start..].find('}') {
            Some(end) => {
                let key = &rest[start + 1..start + end];
                match slots.get(key) {
                    Some(v) => out.push_str(v),
                    None => out.push_str(&rest[start..=start + end]),
                }
                rest = &rest[start + end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Folds to lowercase-able ASCII the way people actually type names into
/// username fields: `Müller` → `mueller`, `Hélène` → `helene`, `Yılmaz` → `yilmaz`.
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'ä' => out.push_str("ae"),
            'Ä' => out.push_str("Ae"),
            'ö' => out.push_str("oe"),
            'Ö' => out.push_str("Oe"),
            'ü' => out.push_str("ue"),
            'Ü' => out.push_str("Ue"),
            'ß' => out.push_str("ss"),
            'æ' => out.push_str("ae"),
            'œ' => out.push_str("oe"),
            'Œ' => out.push_str("Oe"),
            'à' | 'á' | 'â' | 'ã' | 'å' => out.push('a'),
            'À' | 'Á' | 'Â' | 'Ã' | 'Å' => out.push('A'),
            'ç' => out.push('c'),
            'Ç' => out.push('C'),
            'è' | 'é' | 'ê' | 'ë' => out.push('e'),
            'È' | 'É' | 'Ê' | 'Ë' => out.push('E'),
            'ì' | 'í' | 'î' | 'ï' | 'ı' => out.push('i'),
            'Ì' | 'Í' | 'Î' | 'Ï' | 'İ' => out.push('I'),
            'ñ' => out.push('n'),
            'ò' | 'ó' | 'ô' | 'õ' => out.push('o'),
            'Ò' | 'Ó' | 'Ô' | 'Õ' => out.push('O'),
            'ù' | 'ú' | 'û' => out.push('u'),
            'Ù' | 'Ú' | 'Û' => out.push('U'),
            'ÿ' | 'ý' => out.push('y'),
            'ş' => out.push('s'),
            'Ş' => out.push('S'),
            'ğ' => out.push('g'),
            c if c.is_ascii() => out.push(c),
            _ => {}
        }
    }
    out
}

/// ASCII letters/digits only, lowercased: the "stem" used for handle
/// comparisons.
pub fn stem(s: &str) -> String {
    fold(s).chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect()
}

/// French inclusive forms used in the UI: `né·e` → `né` / `née`.
pub fn agree(word_m: &str, word_f: &str, female: bool) -> String {
    if female { word_f } else { word_m }.to_string()
}

/// "in <place>" with the article contracted the way each language requires:
/// fr `à les Salins` → `aux Salins`, de `in die Neustadt` → `in der Neustadt`.
pub fn prep_in(lang: &str, place: &str) -> String {
    match lang {
        "fr" => {
            if let Some(r) = place.strip_prefix("le ") {
                format!("au {r}")
            } else if let Some(r) = place.strip_prefix("les ") {
                format!("aux {r}")
            } else {
                format!("à {place}")
            }
        }
        "de" => {
            if let Some(r) = place.strip_prefix("der ").or_else(|| place.strip_prefix("das ")) {
                format!("im {r}")
            } else if let Some(r) = place.strip_prefix("die ") {
                format!("in der {r}")
            } else {
                format!("in {place}")
            }
        }
        _ => format!("in {place}"),
    }
}

/// French "de <x>" with contraction: `de le FC Nantes` → `du FC Nantes`.
pub fn prep_de_fr(x: &str) -> String {
    if let Some(r) = x.strip_prefix("le ") {
        format!("du {r}")
    } else if let Some(r) = x.strip_prefix("les ") {
        format!("des {r}")
    } else if x.starts_with(|c: char| "aeiouyéèêàâîïôûAEIOUYÉ".contains(c)) {
        format!("d'{x}")
    } else {
        format!("de {x}")
    }
}

/// Lowercases the first letter of a common noun used mid-sentence
/// (`Menuisière` → `menuisière`) but leaves acronyms alone (`IT support`).
pub fn lc_first(s: &str) -> String {
    let mut c = s.chars();
    match (c.next(), s.chars().nth(1)) {
        (Some(f), Some(n)) if f.is_uppercase() && n.is_lowercase() => f.to_lowercase().collect::<String>() + c.as_str(),
        _ => s.to_string(),
    }
}

/// Drops a leading article from an institution name: `l'Université de Lille`
/// → `Université de Lille`, `die TU Dresden` → `TU Dresden`.
pub fn strip_article(s: &str) -> &str {
    for a in ["l'", "l’", "la ", "le ", "les ", "the ", "die ", "der ", "das "] {
        if let Some(r) = s.strip_prefix(a) {
            return r;
        }
    }
    s
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// Length of the longest common substring, used to catch handles that share a
/// recognisable chunk (`jdupont87` vs `dupont_j`).
pub fn longest_common_substring(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut best = 0;
    let mut prev = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        let mut cur = vec![0usize; b.len() + 1];
        for j in 1..=b.len() {
            if a[i - 1] == b[j - 1] {
                cur[j] = prev[j - 1] + 1;
                best = best.max(cur[j]);
            }
        }
        prev = cur;
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_known_and_keeps_unknown() {
        let mut s = HashMap::new();
        s.insert("a", "1".to_string());
        assert_eq!(fill("x{a}y{b}z", &s), "x1y{b}z");
        assert_eq!(fill("no slots", &s), "no slots");
        assert_eq!(fill("dangling {a", &s), "dangling {a");
    }

    #[test]
    fn fold_handles_european_names() {
        assert_eq!(fold("Müller"), "Mueller");
        assert_eq!(fold("Hélène Lefèvre"), "Helene Lefevre");
        assert_eq!(fold("Yılmaz"), "Yilmaz");
        assert_eq!(fold("Weiß"), "Weiss");
        assert_eq!(stem("Jean-Moulin 87!"), "jeanmoulin87");
    }

    #[test]
    fn prepositions() {
        assert_eq!(prep_in("fr", "les Salins"), "aux Salins");
        assert_eq!(prep_in("fr", "la Croix-Rousse"), "à la Croix-Rousse");
        assert_eq!(prep_in("fr", "Monplaisir"), "à Monplaisir");
        assert_eq!(prep_in("de", "die Neustadt"), "in der Neustadt");
        assert_eq!(prep_in("de", "das Viertel"), "im Viertel");
        assert_eq!(prep_in("de", "Linden"), "in Linden");
        assert_eq!(prep_in("en", "the Fan"), "in the Fan");
        assert_eq!(prep_de_fr("le FC Nantes"), "du FC Nantes");
        assert_eq!(prep_de_fr("l'OL"), "de l'OL");
        assert_eq!(prep_de_fr("Lyon"), "de Lyon");
        assert_eq!(prep_de_fr("Angers"), "d'Angers");
        assert_eq!(lc_first("Menuisière"), "menuisière");
        assert_eq!(lc_first("IT support technician"), "IT support technician");
    }

    #[test]
    fn articles() {
        assert_eq!(strip_article("l'Université de Lille"), "Université de Lille");
        assert_eq!(strip_article("die TU Dresden"), "TU Dresden");
        assert_eq!(strip_article("Nantes Université"), "Nantes Université");
    }

    #[test]
    fn lcs() {
        assert_eq!(longest_common_substring("jdupont87", "dupont_j"), 6);
        assert_eq!(longest_common_substring("abc", "xyz"), 0);
    }
}
