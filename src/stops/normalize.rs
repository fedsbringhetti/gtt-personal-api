//! Normalization of stop names and search text, so that `C.SO VITT. EMANUELE`, `corso vittorio
//! emanuele` and `c.so vittorio emanuele` all become `corso vittorio emanuele`.

/// Abbreviations found in real stop names, with their expansion. An entry ending with `.` also
/// matches when glued to the next word (`v.cavagnolo`); one without a trailing dot must be a
/// whole token or be followed by a dot. Longer entries come before their prefixes.
const ABBREVIATIONS: &[(&str, &str)] = &[
    ("circ.ne", "circonvallazione"),
    ("str.le", "stradale"),
    ("f.lli", "fratelli"),
    ("p.zza", "piazza"),
    ("distr.", "distributore"),
    ("staz.", "stazione"),
    ("stab.", "stabilimento"),
    ("borg.", "borgata"),
    ("canc.", "cancello"),
    ("fraz.", "frazione"),
    ("incr.", "incrocio"),
    ("mons.", "monsignor"),
    ("vitt.", "vittorio"),
    ("c.so", "corso"),
    ("p.za", "piazza"),
    ("p.le", "piazzale"),
    ("p.te", "ponte"),
    ("l.go", "largo"),
    ("v.le", "viale"),
    ("s.da", "strada"),
    ("b.da", "bormida"),
    ("m.to", "monferrato"),
    ("m.vi", "mondovi"),
    ("t.se", "torinese"),
    ("ang.", "angolo"),
    ("civ.", "civico"),
    ("loc.", "localita"),
    ("mad.", "madonna"),
    ("reg.", "regione"),
    ("str.", "strada"),
    ("sup.", "superiore"),
    ("s.p.", "sp"),
    ("s.s.", "ss"),
    ("bv.", "bivio"),
    ("fr.", "frazione"),
    ("c.", "corso"),
    ("p.", "piazza"),
    ("s.", "san"),
    ("v.", "via"),
];

/// Whole words treated as the same word, so `santa anna`, `sant'anna` and `s.anna` match, and
/// abbreviations written without dots (`cso`, `pza`) are expanded too.
fn canonical(word: &str) -> &str {
    match word {
        "santa" | "santo" | "sant" => "san",
        "cso" => "corso",
        "pza" | "pzza" => "piazza",
        other => other,
    }
}

/// Lowercase, accents removed, abbreviations expanded, punctuation turned into single spaces.
/// The result contains only `[a-z0-9]` words separated by one space.
pub fn normalize(text: &str) -> String {
    let folded: String = text
        .chars()
        .flat_map(char::to_lowercase)
        .map(fold)
        .collect();
    let mut words: Vec<&str> = Vec::new();

    for mut rest in folded.split_whitespace() {
        while !rest.is_empty() {
            if let Some((abbreviation, expansion)) = ABBREVIATIONS
                .iter()
                .find(|(abbreviation, _)| starts_with_abbreviation(rest, abbreviation))
            {
                words.push(expansion);
                rest = &rest[abbreviation.len()..];
                continue;
            }
            let (word, tail) = rest.split_once('.').unwrap_or((rest, ""));
            if !word.is_empty() {
                words.push(canonical(word));
            }
            rest = tail;
        }
    }

    words.join(" ")
}

fn starts_with_abbreviation(text: &str, abbreviation: &str) -> bool {
    if !text.starts_with(abbreviation) {
        return false;
    }
    abbreviation.ends_with('.')
        || text.len() == abbreviation.len()
        || text[abbreviation.len()..].starts_with('.')
}

/// Maps a lowercase character to ASCII: accented letters lose the accent, dots are kept for
/// abbreviation detection, anything else that is not `[a-z0-9]` becomes a space.
fn fold(c: char) -> char {
    match c {
        'a'..='z' | '0'..='9' | '.' => c,
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ç' => 'c',
        'ñ' => 'n',
        _ => ' ',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercases_and_removes_punctuation() {
        assert_eq!(normalize("PORTA NUOVA"), "porta nuova");
        assert_eq!(
            normalize("To - Porta Nuova (Ingresso Via Sacchi)"),
            "to porta nuova ingresso via sacchi"
        );
        assert_eq!(normalize("  A/B\u{a0}C  "), "a b c");
    }

    #[test]
    fn removes_accents() {
        assert_eq!(normalize("LOCALITÀ PRÈ"), "localita pre");
        assert_eq!(normalize("Mondovì"), "mondovi");
    }

    #[test]
    fn expands_abbreviations() {
        assert_eq!(
            normalize("ASTI (C.SO FERRARIS/V. TORRICELLI)"),
            "asti corso ferraris via torricelli"
        );
        assert_eq!(
            normalize("ALFIANO NATTA (P.ZA VITT.EMANUELE III)"),
            "alfiano natta piazza vittorio emanuele iii"
        );
        assert_eq!(normalize("NIZZA M.TO"), "nizza monferrato");
        assert_eq!(normalize("MONTALDO M.VI'"), "montaldo mondovi");
        assert_eq!(
            normalize("Piobesi T.Se - V.Torino 18"),
            "piobesi torinese via torino 18"
        );
    }

    #[test]
    fn expands_abbreviations_glued_to_the_next_word() {
        assert_eq!(
            normalize("TO-c.so G.Cesare-v.Cavagnolo"),
            "to corso g cesare via cavagnolo"
        );
        assert_eq!(normalize("Via S.Anna 67"), "via san anna 67");
    }

    #[test]
    fn prefers_longer_abbreviations() {
        assert_eq!(normalize("V.LE MARTIRI"), "viale martiri");
        assert_eq!(normalize("STR.LE TORINO"), "stradale torino");
        assert_eq!(normalize("S.P. 146"), "sp 146");
        assert_eq!(normalize("S.S.24"), "ss 24");
    }

    #[test]
    fn does_not_expand_inside_words() {
        assert_eq!(normalize("CORSO VITTORIO"), "corso vittorio");
        assert_eq!(normalize("VIA SAVONAROLA"), "via savonarola");
        assert_eq!(normalize("PIAZZA"), "piazza");
    }

    #[test]
    fn saint_variants_become_the_same_word() {
        assert_eq!(normalize("SANTA RITA"), "san rita");
        assert_eq!(normalize("Sant'Anna"), "san anna");
        assert_eq!(normalize("S. Anna"), "san anna");
        assert_eq!(normalize("SANTENA"), "santena");
    }

    #[test]
    fn expands_abbreviations_written_without_dots() {
        assert_eq!(
            normalize("To - Cso Vittorio Emanuele 67"),
            "to corso vittorio emanuele 67"
        );
        assert_eq!(normalize("PZA CASTELLO"), "piazza castello");
    }

    #[test]
    fn splits_numbers_on_dots() {
        assert_eq!(
            normalize("BORGIALLO (P.ZA 16.3.1948)"),
            "borgiallo piazza 16 3 1948"
        );
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(normalize(""), "");
        assert_eq!(normalize(" - / . "), "");
    }
}
