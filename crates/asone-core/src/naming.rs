//! Nazwy plików wynikowych przyjazne dla adresów URL.

/// Zamienia tekst na nazwę pliku: litery ASCII (wielkość liter zostaje) i cyfry, znaki
/// diakrytyczne transliterowane, każda seria innych znaków → jeden myślnik.
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
            out.push(ch);
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
    build_filename_ver(raw_input, index, None, ext)
}

/// Jak [`build_filename`], z dopiskiem wersji: `Some(2)` → `123-marka-2-ver2.jpg`.
pub fn build_filename_ver(raw_input: &str, index: usize, version: Option<u8>, ext: &str) -> String {
    let base = slugify(raw_input);
    let ext = ext.trim_start_matches('.');
    let ver = version.map(|v| format!("-ver{v}")).unwrap_or_default();
    if base.is_empty() {
        format!("{index}{ver}.{ext}")
    } else {
        format!("{base}-{index}{ver}.{ext}")
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
    fn wzor_nazwa_marka_model() {
        assert_eq!(build_filename("Maska LED ŻÓŁW Pure 1234 2.5 mm", 1, "jpg"), "Maska-LED-ZOLW-Pure-1234-2-5-mm-1.jpg");
        assert_eq!(
            build_filename_ver("Maska LED ŻÓŁW Pure 1234 2.5 mm", 1, Some(2), "jpg"),
            "Maska-LED-ZOLW-Pure-1234-2-5-mm-1-ver2.jpg"
        );
        assert_eq!(build_filename("123-marka model", 2, ".jpg"), "123-marka-model-2.jpg");
    }

    #[test]
    fn znaki_diakrytyczne() {
        assert_eq!(slugify("Zażółć gęślą jaźń"), "Zazolc-gesla-jazn");
        assert_eq!(slugify("Größe Öl"), "Grosse-Ol");
        assert_eq!(slugify("Příliš žluťoučký kůň"), "Prilis-zlutoucky-kun");
    }

    #[test]
    fn znaki_specjalne_i_spacje() {
        assert_eq!(slugify("  Buty -- Nike!!  Air  "), "Buty-Nike-Air");
        assert_eq!(slugify("Lampa (UV) 48W+ 100% #1 $5"), "Lampa-UV-48W-100-1-5");
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
        assert_eq!(build_filename_ver("", 3, Some(4), "png"), "3-ver4.png");
        assert_eq!(build_filename("  !!  ", 1, "jpg"), "1.jpg");
    }
}
