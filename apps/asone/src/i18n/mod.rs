//! Język interfejsu: polski albo angielski – domyślnie według systemu (polski system → polski,
//! każdy inny → angielski), zmiana w Preferencjach › Interfejs.
//!
//! Teksty w kodzie są po polsku i przechodzą przez [`t!`]: `t!("Zapisz")` albo z wartościami
//! `t!("Zapisano {n} plików", n = count)`. Przy angielskim [`tr`] zwraca odpowiednik z tabeli
//! [`en::EN`]; brak wpisu → tekst polski (test `wszystkie_napisy_maja_tlumaczenie` pilnuje tabeli).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

mod core;
mod en;

/// Wybór języka w ustawieniach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// Jak w systemie.
    #[default]
    Auto,
    Polish,
    English,
}

impl Language {
    pub const ALL: [Language; 3] = [Language::Auto, Language::Polish, Language::English];

    /// Nazwa w obu językach – czytelna bez względu na bieżący język.
    pub fn label(self) -> &'static str {
        match self {
            Language::Auto => tr("Jak w systemie"),
            Language::Polish => "Polski",
            Language::English => "English",
        }
    }
}

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Ustawia język interfejsu (`Auto` – według języka systemu).
pub fn set_language(lang: Language) {
    let english = match lang {
        Language::Auto => !system_is_polish(),
        Language::Polish => false,
        Language::English => true,
    };
    ENGLISH.store(english, Ordering::Relaxed);
}

/// Interfejs po angielsku.
pub fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Język systemu (LANG / LC_* na Linuksie, ustawienia systemu na Windows, macOS i Androidzie,
/// język przeglądarki w wersji web).
fn system_is_polish() -> bool {
    sys_locale::get_locales().next().is_some_and(|l| is_polish_locale(&l))
}

fn is_polish_locale(locale: &str) -> bool {
    let l = locale.to_ascii_lowercase();
    l == "pl" || l.starts_with("pl-") || l.starts_with("pl_") || l.starts_with("pl.")
}

/// Tekst w bieżącym języku (`pl` – tekst polski z kodu).
pub fn tr(pl: &'static str) -> &'static str {
    if !is_english() {
        return pl;
    }
    static TABLE: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    TABLE.get_or_init(|| en::EN.iter().copied().collect()).get(pl).copied().unwrap_or(pl)
}

/// Tekst z `asone-core` (metadane, uwagi i błędy eksportu, wczytywania, zmiany nazw) w bieżącym
/// języku – tabela wzorców [`core::EN_CORE`]; nieznany tekst (np. dane z pliku) zostaje bez zmian.
pub fn tr_core(s: &str) -> std::borrow::Cow<'_, str> {
    if !is_english() {
        return std::borrow::Cow::Borrowed(s);
    }
    match core_translate(s, 0) {
        Some(t) => std::borrow::Cow::Owned(t),
        None => std::borrow::Cow::Borrowed(s),
    }
}

fn core_translate(s: &str, depth: u8) -> Option<String> {
    if s.contains('\n') {
        let out = s.split('\n').map(|l| core_translate(l, depth).unwrap_or_else(|| l.to_string())).collect::<Vec<_>>().join("\n");
        return (out != s).then_some(out);
    }
    for (pl, en) in core::EN_CORE {
        if let Some(values) = match_pattern(pl, s) {
            let mut out = (*en).to_string();
            for (i, v) in values.iter().enumerate() {
                // Wartość bywa sama tekstem z rdzenia (np. „ – nadal za duży”) – jeden poziom głębiej.
                let v = if depth == 0 { core_translate(v, 1).unwrap_or_else(|| v.clone()) } else { v.clone() };
                out = out.replace(&format!("{{{i}}}"), &v);
            }
            return Some(out);
        }
    }
    // Panel Metadane: „Etykieta (NazwaTagu)” – tłumaczona sama etykieta.
    if depth == 0
        && let Some((label, tag)) = s.rsplit_once(" (")
        && tag.ends_with(')')
        && let Some(t) = core_translate(label, 1)
    {
        return Some(format!("{t} ({tag}"));
    }
    None
}

/// Dopasowanie do wzorca z `{}` (dowolny tekst); zwraca wartości w kolejności.
fn match_pattern(pattern: &str, s: &str) -> Option<Vec<String>> {
    let lits: Vec<&str> = pattern.split("{}").collect();
    if lits.len() == 1 {
        return (pattern == s).then(Vec::new);
    }
    let mut rest = s.strip_prefix(lits[0])?;
    let mut values = Vec::with_capacity(lits.len() - 1);
    let last = lits.len() - 2;
    for (k, lit) in lits[1..].iter().enumerate() {
        if k == last {
            values.push(rest.strip_suffix(lit)?.to_string());
        } else {
            if lit.is_empty() {
                return None; // dwa `{}` obok siebie – niejednoznaczne
            }
            let i = rest.find(lit)?;
            values.push(rest[..i].to_string());
            rest = &rest[i + lit.len()..];
        }
    }
    Some(values)
}

/// Tekst w bieżącym języku: `t!("Zapisz")` → `&'static str`;
/// `t!("Zapisano {n} plików", n = count)` → `String` (wartości wstawiane po tłumaczeniu).
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::i18n::tr($s)
    };
    ($s:literal, $($k:ident = $v:expr),+ $(,)?) => {{
        let mut out = ::std::string::String::from($crate::i18n::tr($s));
        $(
            out = out.replace(concat!("{", stringify!($k), "}"), &::std::string::ToString::to_string(&$v));
        )+
        out
    }};
}

/// Liczba z rzeczownikiem: polski (1 plik, 2 pliki, 5 plików) albo angielski (1 file, 2 files).
pub fn count(n: usize, one: &'static str, few: &'static str, many: &'static str, en_one: &'static str, en_many: &'static str) -> String {
    let word = if is_english() {
        if n == 1 { en_one } else { en_many }
    } else if n == 1 {
        one
    } else if (2..=4).contains(&(n % 10)) && !(12..=14).contains(&(n % 100)) {
        few
    } else {
        many
    };
    format!("{n} {word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jezyk_systemu() {
        assert!(is_polish_locale("pl-PL"));
        assert!(is_polish_locale("pl_PL.UTF-8"));
        assert!(is_polish_locale("pl"));
        assert!(!is_polish_locale("en-US"));
        assert!(!is_polish_locale("C"));
        assert!(!is_polish_locale("plx"));
    }

    #[test]
    fn liczebniki() {
        ENGLISH.store(false, Ordering::Relaxed);
        assert_eq!(count(1, "plik", "pliki", "plików", "file", "files"), "1 plik");
        assert_eq!(count(3, "plik", "pliki", "plików", "file", "files"), "3 pliki");
        assert_eq!(count(12, "plik", "pliki", "plików", "file", "files"), "12 plików");
        assert_eq!(count(22, "plik", "pliki", "plików", "file", "files"), "22 pliki");
    }

    /// Każdy `t!("…")` w kodzie aplikacji ma wpis w tabeli angielskiej, a tabela nie ma
    /// zbędnych ani podwójnych wpisów.
    #[test]
    fn wszystkie_napisy_maja_tlumaczenie() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut used = std::collections::BTreeSet::new();
        let mut stack = vec![dir];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") && !p.ends_with("i18n/en.rs") {
                    let src = std::fs::read_to_string(&p).unwrap();
                    // Bez bloków testów (tu m.in. wzorce wyszukiwania samego testu).
                    let code = src.split("#[cfg(test)]\nmod tests").next().unwrap_or(&src);
                    collect_keys(code, &mut used);
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        for (pl, en) in en::EN {
            assert!(seen.insert(*pl), "podwójny wpis: {pl:?}");
            assert!(!en.is_empty(), "puste tłumaczenie: {pl:?}");
            assert_eq!(placeholders(pl), placeholders(en), "inne {{…}} w tłumaczeniu: {pl:?} → {en:?}");
        }
        let missing: Vec<_> = used.iter().filter(|k| !seen.contains(k.as_str())).collect();
        assert!(missing.is_empty(), "bez tłumaczenia ({}): {missing:#?}", missing.len());
        let unused: Vec<_> = en::EN.iter().map(|(pl, _)| *pl).filter(|pl| !used.contains(*pl)).collect();
        assert!(unused.is_empty(), "zbędne wpisy ({}): {unused:#?}", unused.len());
    }

    #[test]
    fn teksty_rdzenia() {
        // Bez przełączania języka (testy biegną równolegle) – samo tłumaczenie wzorców.
        let t = |s: &str| core_translate(s, 0).unwrap_or_else(|| s.to_string());
        assert_eq!(t("Błąd zapisu: brak miejsca"), "Write error: brak miejsca");
        assert_eq!(
            t("jakość 90 → 65 i zmniejszono 4000×3000 → 2000×1500 (limit 3.0 MB) – nadal za duży"),
            "quality 90 → 65 and downscaled 4000×3000 → 2000×1500 (limit 3.0 MB) – still too large"
        );
        assert_eq!(t("jakość 90 → 80 (limit 3.0 MB)"), "quality 90 → 80 (limit 3.0 MB)");
        assert_eq!(t("Opis (ImageDescription)"), "Description (ImageDescription)");
        assert_eq!(t("Szerokość (EXIF) (PixelXDimension)"), "Width (EXIF) (PixelXDimension)");
        assert_eq!(t("a.jpg: ta sama nazwa dla dwóch plików"), "a.jpg: the same name for two files");
        assert_eq!(t("Lokalizacja GPS: 52.1, 21.0"), "GPS location: 52.1, 21.0");
        assert_eq!(t("Dowolny opis ze zdjęcia"), "Dowolny opis ze zdjęcia");
    }

    /// Każdy wpis tabeli rdzenia występuje w kodzie `asone-core` (po zamianie wstawek na `{}`),
    /// a tłumaczenie używa wszystkich wartości.
    #[test]
    fn teksty_rdzenia_sa_w_rdzeniu() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../crates/asone-core/src");
        let mut lits = std::collections::HashSet::new();
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let src = std::fs::read_to_string(e.path()).unwrap();
            let code = src.split("#[cfg(test)]\nmod tests").next().unwrap_or(&src);
            // Wiersz po wierszu (bez komentarzy); znak '"' w kodzie rozbiłby parowanie cudzysłowów.
            for line in code.lines().filter(|l| !l.trim_start().starts_with("//")) {
                let line = line.replace("'\"'", "' '");
                let mut chars = line.chars();
                while chars.by_ref().any(|c| c == '"') {
                    let mut lit = String::new();
                    while let Some(c) = chars.next() {
                        match c {
                            '"' => break,
                            '\\' => {
                                if let Some(n) = chars.next() {
                                    lit.push(n);
                                }
                            }
                            c => lit.push(c),
                        }
                    }
                    let mut norm = String::new();
                    let mut depth = 0;
                    for c in lit.chars() {
                        match c {
                            '{' => {
                                depth += 1;
                                if depth == 1 {
                                    norm.push_str("{}");
                                }
                            }
                            '}' if depth > 0 => depth -= 1,
                            _ if depth > 0 => {}
                            c => norm.push(c),
                        }
                    }
                    lits.insert(norm);
                }
            }
        }
        for (pl, en) in core::EN_CORE {
            assert!(lits.contains(*pl), "brak w asone-core: {pl:?}");
            let n = pl.matches("{}").count();
            for i in 0..n {
                assert!(en.contains(&format!("{{{i}}}")), "{en:?} nie używa {{{i}}}");
            }
        }
    }

    fn placeholders(s: &str) -> Vec<String> {
        let mut v: Vec<String> = s.split('{').skip(1).filter_map(|p| p.split_once('}').map(|(k, _)| k.to_string())).collect();
        v.sort();
        v
    }

    /// Klucze z `t!("…")` i `tr("…")` (z prostymi sekwencjami \n, \", \\).
    fn collect_keys(src: &str, out: &mut std::collections::BTreeSet<String>) {
        for pat in ["t!(", "tr("] {
            let mut rest = src;
            while let Some(i) = rest.find(pat) {
                // Tekst może zaczynać się w następnej linii: `t!(\n    "…"`.
                let Some(after) = rest[i + pat.len()..].trim_start().strip_prefix('"') else {
                    rest = &rest[i + pat.len()..];
                    continue;
                };
                let mut key = String::new();
                let mut chars = after.chars();
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => match chars.next() {
                            Some('n') => key.push('\n'),
                            Some('t') => key.push('\t'),
                            // Kontynuacja wiersza: znak nowej linii i wcięcie nie należą do tekstu.
                            Some('\n') => {
                                while chars.clone().next().is_some_and(char::is_whitespace) {
                                    chars.next();
                                }
                            }
                            Some(o) => key.push(o),
                            None => break,
                        },
                        c => key.push(c),
                    }
                }
                // Tylko całe słowo (`format!(` kończy się na `t!(`, `push_str(` na `tr(`);
                // `fn tr(pl: …)` i przykłady w komentarzach dokumentacji pomijamy.
                let before = &rest[..i];
                let word_start = !before.ends_with(|c: char| c.is_alphanumeric() || c == '_');
                let line = before.rsplit('\n').next().unwrap_or("");
                if word_start && !line.trim_start().starts_with("//") && !line.contains("fn tr") {
                    out.insert(key);
                }
                rest = after;
            }
        }
    }
}
