//! Uprawnienia do plików dla MCP: wszystko pod `--automation-read-root` / `--automation-write-root`.
//!
//! Ścieżki są względne do katalogu głównego albo bezwzględne, ale wewnątrz niego.
//! Ucieczki (`..`, dowiązania prowadzące poza katalog) są odrzucane.

use std::path::{Component, Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Workspace {
    read_root: Option<PathBuf>,
    write_root: Option<PathBuf>,
}

fn canonical_root(path: &Path, what: &str) -> Result<PathBuf, String> {
    let c = path
        .canonicalize()
        .map_err(|e| format!("{what} {}: {e}", path.display()))?;
    if !c.is_dir() {
        return Err(format!("{what} {} nie jest katalogiem", c.display()));
    }
    Ok(c)
}

impl Workspace {
    pub fn new(read_root: Option<&Path>, write_root: Option<&Path>) -> Result<Self, String> {
        Ok(Self {
            read_root: read_root.map(|p| canonical_root(p, "--automation-read-root")).transpose()?,
            write_root: write_root.map(|p| canonical_root(p, "--automation-write-root")).transpose()?,
        })
    }

    pub fn describe(&self) -> String {
        let show = |r: &Option<PathBuf>| r.as_ref().map_or("brak".to_string(), |p| p.display().to_string());
        format!("odczyt: {}, zapis: {}", show(&self.read_root), show(&self.write_root))
    }

    /// Istniejący plik do odczytu, wewnątrz katalogu odczytu.
    pub fn resolve_read(&self, path: &str) -> Result<PathBuf, String> {
        let root = self
            .read_root
            .as_ref()
            .ok_or("Brak uprawnień do odczytu: uruchom serwer z --automation-read-root")?;
        let joined = join(root, path)?;
        let c = joined
            .canonicalize()
            .map_err(|e| format!("{path}: {e}"))?;
        if !c.starts_with(root) {
            return Err(format!("{path}: poza katalogiem odczytu"));
        }
        if !c.is_file() {
            return Err(format!("{path}: to nie jest plik"));
        }
        Ok(c)
    }

    /// Katalog do zapisu (tworzony w razie potrzeby), wewnątrz katalogu zapisu.
    pub fn resolve_write_dir(&self, path: &str) -> Result<PathBuf, String> {
        let root = self
            .write_root
            .as_ref()
            .ok_or("Brak uprawnień do zapisu: uruchom serwer z --automation-write-root")?;
        let joined = join(root, path)?;
        std::fs::create_dir_all(&joined).map_err(|e| format!("{path}: {e}"))?;
        let c = joined.canonicalize().map_err(|e| format!("{path}: {e}"))?;
        if !c.starts_with(root) {
            return Err(format!("{path}: poza katalogiem zapisu"));
        }
        Ok(c)
    }
}

/// Łączy ścieżkę z katalogiem głównym, odrzucając `..` i ścieżki bezwzględne spoza niego.
fn join(root: &Path, path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(format!("{path}: „..” jest niedozwolone"));
    }
    if p.is_absolute() {
        if !p.starts_with(root) {
            return Err(format!("{path}: ścieżka bezwzględna poza dozwolonym katalogiem"));
        }
        return Ok(p.to_path_buf());
    }
    Ok(root.join(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn granice_katalogow() {
        let base = std::env::temp_dir().join(format!("asone-ws-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let inside = base.join("projekt");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::write(inside.join("a.jpg"), b"x").unwrap();
        std::fs::write(base.join("sekret.txt"), b"x").unwrap();

        let ws = Workspace::new(Some(&inside), Some(&inside)).unwrap();
        assert!(ws.resolve_read("a.jpg").is_ok());
        assert!(ws.resolve_read(inside.join("a.jpg").to_str().unwrap()).is_ok());
        assert!(ws.resolve_read("../sekret.txt").is_err());
        assert!(ws.resolve_read(base.join("sekret.txt").to_str().unwrap()).is_err());
        assert!(ws.resolve_read("brak.jpg").is_err());
        let out = ws.resolve_write_dir("web").unwrap();
        assert!(out.ends_with("web") && out.is_dir());
        assert!(ws.resolve_write_dir("../poza").is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("sekret.txt"), inside.join("link.jpg")).unwrap();
            assert!(ws.resolve_read("link.jpg").is_err(), "dowiązanie poza katalog");
        }

        let none = Workspace::new(None, None).unwrap();
        assert!(none.resolve_read("a.jpg").is_err());
        assert!(none.resolve_write_dir("web").is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}
