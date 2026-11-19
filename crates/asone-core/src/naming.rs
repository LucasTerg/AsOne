//! Nazwy plików wynikowych przyjazne dla adresów URL.

/// Zamienia dowolny tekst na „slug”: tylko ASCII, małe litery i cyfry,
/// a każda seria spacji/znaków specjalnych staje się pojedynczym myślnikiem.
///
/// Polskie (i inne) znaki diakrytyczne są transliterowane (`ą→a`, `ł→l`, `ż→z`…).
pub fn slugify(raw: &str) -> String {
    let ascii = deunicode::deunicode(raw);
    let mut out = String::with_capacity(ascii.len());
    let mut pending_dash = false;
    for ch in ascii.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// Buduje nazwę pliku wynikowego, np. `("123-marka model", 1, "jpg")` → `123-marka-model-1.jpg`.
///
/// `index` to numer porządkowy liczony od 1 (pozycja zakładki ze zdjęciem).
/// Gdy nazwa bazowa po oczyszczeniu jest pusta, zwracany jest sam numer (`1.jpg`).
pub fn build_filename(raw_input: &str, index: usize, ext: &str) -> String {
    let base = slugify(raw_input);
    let ext = ext.trim_start_matches('.');
    if base.is_empty() {
        format!("{index}.{ext}")
    } else {
        format!("{base}-{index}.{ext}")
    }
}

/// Nazwy dla `count` zdjęć numerowanych od `start` (pole „Start nr”).
pub fn plan_names_from(raw_input: &str, start: usize, count: usize) -> Vec<String> {
    (0..count).map(|i| build_filename(raw_input, start + i, "jpg")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn przyklad_ze_specyfikacji() {
        assert_eq!(build_filename("123-marka model", 1, "jpg"), "123-marka-model-1.jpg");
        assert_eq!(build_filename("123-marka model", 2, "jpg"), "123-marka-model-2.jpg");
        assert_eq!(build_filename("123-marka model", 3, ".jpg"), "123-marka-model-3.jpg");
    }

    #[test]
    fn polskie_znaki() {
        assert_eq!(slugify("Zażółć gęślą jaźń"), "zazolc-gesla-jazn");
        assert_eq!(slugify("ŁÓDŹ Śnieżka Ćma"), "lodz-sniezka-cma");
    }

    #[test]
    fn znaki_specjalne_i_spacje() {
        assert_eq!(slugify("  Buty -- Nike!!  Air  "), "buty-nike-air");
        assert_eq!(slugify("a/b\\c_d.e,f"), "a-b-c-d-e-f");
        assert_eq!(slugify("---"), "");
    }

    #[test]
    fn numeracja_od_start_nr() {
        assert_eq!(plan_names_from("buty", 4, 2), vec!["buty-4.jpg", "buty-5.jpg"]);
    }

    #[test]
    fn pusta_nazwa() {
        assert_eq!(build_filename("", 4, "jpg"), "4.jpg");
        assert_eq!(build_filename("  !!  ", 1, "jpg"), "1.jpg");
    }
}
