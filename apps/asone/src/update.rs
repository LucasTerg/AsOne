//! Samoaktualizacja wersji AppImage.
//!
//! Gdy AsOne działa jako AppImage (zmienna `APPIMAGE` od środowiska AppImage), sprawdza najnowsze
//! wydanie na GitHubie, pobiera nowy plik AppImage dla tej samej architektury, weryfikuje sumę
//! SHA-256 z `SHA256SUMS.txt` z wydania i podmienia plik **pod tą samą ścieżką** – skróty w menu
//! (KDE trzyma w pamięci ścieżkę z pliku .desktop), skróty Steam i na pulpicie dalej działają.
//! Działająca kopia czyta ze starego pliku do zamknięcia – „Uruchom ponownie” startuje nową.
//! AppImage ma też wbudowaną informację o aktualizacji (zsync) dla AppImageUpdate / GearLever.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Najnowsze wydanie (bez wersji roboczych).
const LATEST: &str = "https://api.github.com/repos/LucasTerg/AsOne/releases/latest";
/// Strona wydań – dla wersji innych niż AppImage.
pub const RELEASES_PAGE: &str = "https://github.com/LucasTerg/AsOne/releases/latest";

/// Nowsza wersja do pobrania.
#[derive(Clone, Debug)]
pub struct Release {
    pub version: String,
    pub appimage_name: String,
    pub appimage_url: String,
    pub size: u64,
    pub sums_url: Option<String>,
}

/// Ścieżka działającego pliku AppImage (`None` – inna wersja: Flatpak, tar.gz, Windows…).
pub fn appimage_path() -> Option<PathBuf> {
    std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file())
}

/// `latest` nowsza niż `current` (wersje `a.b.c`, ewentualne „v” na początku pomijane).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |v: &str| -> Vec<u64> { v.trim().trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect() };
    parse(latest) > parse(current)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(600))).build().into()
}

/// Najnowsze wydanie, jeśli jest nowsze od tej wersji i ma AppImage dla tej architektury.
pub fn check() -> Result<Option<Release>, String> {
    let json = latest_json()?;
    let tag = json.get("tag_name").and_then(Value::as_str).ok_or("Sprawdzanie aktualizacji: brak wersji w odpowiedzi")?;
    if !is_newer(tag, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    release_from(&json).map(Some)
}

fn latest_json() -> Result<Value, String> {
    let mut resp = agent()
        .get(LATEST)
        .header("User-Agent", "AsOne-updater")
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("Sprawdzanie aktualizacji: {e}"))?;
    let text = resp.body_mut().read_to_string().map_err(|e| format!("Sprawdzanie aktualizacji: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("Sprawdzanie aktualizacji: {e}"))
}

/// Plik AppImage tej architektury i sumy kontrolne z opisu wydania (JSON API GitHuba).
fn release_from(json: &Value) -> Result<Release, String> {
    let tag = json.get("tag_name").and_then(Value::as_str).ok_or("Sprawdzanie aktualizacji: brak wersji w odpowiedzi")?;
    let version = tag.trim_start_matches('v').to_string();
    let assets = json.get("assets").and_then(Value::as_array).cloned().unwrap_or_default();
    let find = |name: &str| assets.iter().find(|a| a.get("name").and_then(Value::as_str) == Some(name));
    let appimage_name = format!("AsOne-{version}-{}.AppImage", std::env::consts::ARCH);
    let asset = find(&appimage_name).ok_or_else(|| format!("Wydanie {version} nie ma pliku {appimage_name}"))?;
    let url = |a: &Value| a.get("browser_download_url").and_then(Value::as_str).map(String::from);
    Ok(Release {
        version,
        appimage_url: url(asset).ok_or("Sprawdzanie aktualizacji: brak adresu pliku")?,
        size: asset.get("size").and_then(Value::as_u64).unwrap_or(0),
        sums_url: find("SHA256SUMS.txt").and_then(url),
        appimage_name,
    })
}

/// Plik tymczasowy pobierania: ukryty, obok AppImage (ten sam system plików – podmiana jest atomowa).
fn part_path(current: &Path) -> PathBuf {
    let name = current.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "AsOne.AppImage".into());
    current.with_file_name(format!(".{name}.part"))
}

/// Pobiera nowy AppImage, sprawdza SHA-256 i podmienia plik pod tą samą ścieżką (zwraca ją).
/// `progress` – 0–1000.
pub fn install(release: &Release, current: &Path, progress: &AtomicU32) -> Result<PathBuf, String> {
    let target = current.to_path_buf();
    let part = part_path(current);
    let result = (|| -> Result<(), String> {
        let mut resp = agent()
            .get(&release.appimage_url)
            .header("User-Agent", "AsOne-updater")
            .call()
            .map_err(|e| format!("Pobieranie: {e}"))?;
        let total = resp
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(release.size)
            .max(1);
        let mut reader = resp.body_mut().with_config().limit(1 << 31).reader();
        let mut file = std::fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = 0u64;
        loop {
            let n = reader.read(&mut buf).map_err(|e| format!("Pobieranie: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            file.write_all(&buf[..n]).map_err(|e| format!("Zapis: {e}"))?;
            done += n as u64;
            progress.store(((done * 1000) / total).min(999) as u32, Ordering::Relaxed);
        }
        file.flush().map_err(|e| format!("Zapis: {e}"))?;
        drop(file);
        if let Some(sums) = &release.sums_url {
            let mut resp = agent().get(sums).header("User-Agent", "AsOne-updater").call().map_err(|e| format!("Sumy kontrolne: {e}"))?;
            let text = resp.body_mut().read_to_string().map_err(|e| format!("Sumy kontrolne: {e}"))?;
            let expected = expected_sum(&text, &release.appimage_name).ok_or("Sumy kontrolne: brak pliku w SHA256SUMS.txt")?;
            let got: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
            if !got.eq_ignore_ascii_case(&expected) {
                return Err("Pobrany plik ma złą sumę kontrolną – aktualizacja przerwana".into());
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o755)).map_err(|e| format!("Uprawnienia: {e}"))?;
        }
        std::fs::rename(&part, &target).map_err(|e| format!("Zapis: {e}"))?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    progress.store(1000, Ordering::Relaxed);
    Ok(target)
}

/// Suma z pliku `SHA256SUMS.txt` (`<hex>  <nazwa>`, opcjonalnie `*<nazwa>`).
fn expected_sum(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim().trim_start_matches('*') == name).then(|| hash.trim().to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porownanie_wersji() {
        assert!(is_newer("0.1.16", "0.1.15"));
        assert!(is_newer("v0.2.0", "0.1.99"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("0.1.15", "0.1.15"));
        assert!(!is_newer("0.1.14", "0.1.15"));
    }

    #[test]
    fn suma_z_pliku() {
        let sums = "aa11  AsOne-0.1.16-x86_64.AppImage\nbb22 *AsOne-0.1.16-aarch64.AppImage\n";
        assert_eq!(expected_sum(sums, "AsOne-0.1.16-x86_64.AppImage").as_deref(), Some("aa11"));
        assert_eq!(expected_sum(sums, "AsOne-0.1.16-aarch64.AppImage").as_deref(), Some("bb22"));
        assert_eq!(expected_sum(sums, "inny"), None);
    }

    /// Prawdziwe pobranie najnowszego AppImage (ok. 26 MB) w miejsce pliku testowego.
    /// `cargo test -p asone aktualizacja_na_zywo -- --ignored --nocapture`
    #[test]
    #[ignore = "pobiera najnowsze wydanie z GitHuba"]
    fn aktualizacja_na_zywo() {
        let release = release_from(&latest_json().unwrap()).unwrap();
        let dir = std::env::temp_dir().join(format!("asone-update-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let current = dir.join("AsOne-test-x86_64.appimage");
        std::fs::write(&current, b"stara wersja").unwrap();
        let progress = AtomicU32::new(0);
        let path = install(&release, &current, &progress).unwrap();
        assert_eq!(path, current, "ta sama ścieżka");
        let bytes = std::fs::read(&path).unwrap();
        eprintln!("{}: {} B, postęp {}", release.appimage_name, bytes.len(), progress.load(Ordering::Relaxed));
        assert_eq!(bytes.len() as u64, release.size);
        assert_eq!(&bytes[..4], b"\x7fELF");
        assert!(!part_path(&current).exists(), "bez pliku tymczasowego");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o111, 0o111);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn plik_tymczasowy_obok() {
        assert_eq!(
            part_path(Path::new("/apps/AsOne-0.1.18-x86_64.appimage")),
            PathBuf::from("/apps/.AsOne-0.1.18-x86_64.appimage.part")
        );
    }
}
