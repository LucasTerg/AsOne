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
