//! „Zmień tylko nazwę”: nowe nazwy plików na dysku według wklejonej nazwy, bez ruszania
//! zawartości.
//!
//! Plik zostaje w swoim folderze i zachowuje rozszerzenie (małymi literami):
//! `IMG_0042.JPG` → `123-marka-model-3.jpg` (albo `…-3-ver2.jpg` z dopiskiem wersji).
//! Zmiana idzie w dwóch krokach przez nazwy tymczasowe, więc zamiana kolejności
//! (`a-1` ↔ `a-2`) działa, a pliki spoza zestawu nigdy nie są nadpisywane.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::naming::build_filename_ver;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenamePlan {
    pub from: PathBuf,
    pub to: PathBuf,
}

impl RenamePlan {
    pub fn is_noop(&self) -> bool {
        self.from == self.to
    }
}

/// Plan dla plików w podanej kolejności, numerowanych od `start`; `files` = (ścieżka, dopisek wersji).
pub fn plan_rename(base_name: &str, start: usize, files: &[(PathBuf, Option<u8>)]) -> Vec<RenamePlan> {
    files
        .iter()
        .enumerate()
        .map(|(i, (from, version))| {
            let ext = from.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_else(|| "jpg".into());
            let name = build_filename_ver(base_name, start + i, *version, &ext);
            RenamePlan { from: from.clone(), to: from.with_file_name(name) }
        })
        .collect()
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string())
}

/// Wykonuje plan. Wynik dla każdej pozycji: nowa ścieżka albo opis błędu (plik zostaje bez zmian).
pub fn rename_files(plans: &[RenamePlan]) -> Vec<Result<PathBuf, String>> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let sources: HashSet<PathBuf> = plans.iter().map(|p| canon(&p.from)).collect();
    let mut results: Vec<Option<Result<PathBuf, String>>> = vec![None; plans.len()];
    let mut targets = HashSet::new();
    let mut active = Vec::new();

    for (i, p) in plans.iter().enumerate() {
        let to_name = file_name(&p.to);
        if p.is_noop() {
            results[i] = Some(Ok(p.to.clone()));
        } else if !p.from.is_file() {
            results[i] = Some(Err(format!("{}: brak pliku", file_name(&p.from))));
        } else if !targets.insert(p.to.clone()) {
            results[i] = Some(Err(format!("{to_name}: ta sama nazwa dla dwóch plików")));
        } else if p.to.exists() && !sources.contains(&canon(&p.to)) {
            results[i] = Some(Err(format!("{to_name} już istnieje – pominięto, nic nie nadpisano")));
        } else {
            active.push(i);
        }
    }

    // Krok 1: nazwy tymczasowe.
    let pid = std::process::id();
    let mut temps = Vec::new();
    for i in active {
        let p = &plans[i];
        let temp = p.from.with_file_name(format!(".asone-zmiana-nazwy-{pid}-{i}.tmp"));
        match std::fs::rename(&p.from, &temp) {
            Ok(()) => temps.push((i, temp)),
            Err(e) => results[i] = Some(Err(format!("{}: {e}", file_name(&p.from)))),
        }
    }
    // Krok 2: nazwy docelowe (bez nadpisywania; przy błędzie powrót do starej nazwy).
    for (i, temp) in temps {
        let p = &plans[i];
        let outcome = if p.to.exists() {
            Err(format!("{} już istnieje – pominięto, nic nie nadpisano", file_name(&p.to)))
        } else {
            std::fs::rename(&temp, &p.to).map(|()| p.to.clone()).map_err(|e| format!("{}: {e}", file_name(&p.to)))
        };
        if outcome.is_err() && std::fs::rename(&temp, &p.from).is_err() {
            results[i] = Some(Err(format!(
                "{}: nie udało się przywrócić nazwy – plik jest teraz jako {}",
                file_name(&p.from),
                temp.display()
            )));
            continue;
        }
        results[i] = Some(outcome);
    }
    results.into_iter().map(|r| r.unwrap_or_else(|| Err("pominięto".into()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("asone-rename-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        p
    }

    fn read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).unwrap()
    }

    #[test]
    fn plan_z_rozszerzeniem_i_wersja() {
        let files = vec![(PathBuf::from("/x/IMG_1.JPG"), None), (PathBuf::from("/y/b.png"), Some(2))];
        let plan = plan_rename("123-Marka Model", 5, &files);
        assert_eq!(plan[0].to, PathBuf::from("/x/123-Marka-Model-5.jpg"));
        assert_eq!(plan[1].to, PathBuf::from("/y/123-Marka-Model-6-ver2.png"));
    }

    #[test]
    fn zmiana_nazw_i_zamiana_kolejnosci() {
        let dir = temp_dir("swap");
        let a = write(&dir, "a-1.jpg", "A");
        let b = write(&dir, "a-2.jpg", "B");
        let c = write(&dir, "IMG.JPG", "C");
        // Nowa kolejność: b, a, c → a-1 = B, a-2 = A, a-3 = C.
        let plan = plan_rename("a", 1, &[(b, None), (a, None), (c, None)]);
        let res = rename_files(&plan);
        assert!(res.iter().all(Result::is_ok), "{res:?}");
        assert_eq!(read(&dir, "a-1.jpg"), "B");
        assert_eq!(read(&dir, "a-2.jpg"), "A");
        assert_eq!(read(&dir, "a-3.jpg"), "C");
        assert!(!dir.join("IMG.JPG").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nie_nadpisuje_obcych_plikow() {
        let dir = temp_dir("clobber");
        let x = write(&dir, "zdjecie.jpg", "X");
        write(&dir, "a-1.jpg", "OBCY");
        let res = rename_files(&plan_rename("a", 1, &[(x, None)]));
        assert!(res[0].as_ref().unwrap_err().contains("już istnieje"));
        assert_eq!(read(&dir, "a-1.jpg"), "OBCY");
        assert_eq!(read(&dir, "zdjecie.jpg"), "X");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn bez_zmian_gdy_nazwa_juz_dobra() {
        let dir = temp_dir("noop");
        let a = write(&dir, "a-1.jpg", "A");
        let plan = plan_rename("a", 1, &[(a.clone(), None)]);
        assert!(plan[0].is_noop());
        assert_eq!(rename_files(&plan), vec![Ok(a)]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
