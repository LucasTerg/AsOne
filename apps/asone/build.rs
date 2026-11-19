//! Statystyki „O programie › Modele” liczone z `git log` przy budowaniu.
//!
//! Model = stopka `Co-Authored-By: <nazwa> <e-mail>` commita (firma z domeny e-maila).
//! Wiersze +/− bez `vendor/`, `Cargo.lock` i plików binarnych. Bez gita (np. paczka źródeł)
//! tabela jest pusta, a budowanie się udaje.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Default)]
struct Stat {
    company: String,
    model: String,
    commits: u32,
    added: u64,
    removed: u64,
    first: String,
    last: String,
}

fn company(email: &str) -> String {
    let domain = email.rsplit('@').next().unwrap_or("").to_lowercase();
    match domain.as_str() {
        "anthropic.com" => "Anthropic".into(),
        "google.com" | "deepmind.com" => "Google".into(),
        "openai.com" => "OpenAI".into(),
        "users.noreply.github.com" | "github.com" => "GitHub".into(),
        "" => "—".into(),
        d => d.into(),
    }
}

fn excluded(path: &str) -> bool {
    path.starts_with("vendor/") || path.ends_with("Cargo.lock") || path.starts_with("dist/")
}

fn collect(root: &Path) -> Option<(Vec<Stat>, u32, String)> {
    let out = Command::new("git")
        .args(["-c", "safe.directory=*", "-C"])
        .arg(root)
        .args([
            "log",
            "--no-merges",
            "--date=short",
            "--numstat",
            "--format=%x1e%h%x1f%ad%x1f%(trailers:key=Co-authored-by,valueonly,separator=%x1d)",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut by_model: BTreeMap<String, Stat> = BTreeMap::new();
    let mut total = 0u32;
    let mut head = String::new();
    for record in text.split('\x1e').filter(|r| !r.trim().is_empty()) {
        let mut lines = record.lines();
        let mut fields = lines.next()?.split('\x1f');
        let (hash, date, trailers) = (fields.next()?, fields.next()?, fields.next().unwrap_or(""));
        if head.is_empty() {
            head = format!("{hash} · {date}");
        }
        total += 1;
        // Pierwszy współautor-AI (nazwa <e-mail>); bez stopki – osobna grupa.
        let coauthor = trailers.split('\x1d').map(str::trim).find(|t| !t.is_empty());
        let (company, model) = match coauthor {
            Some(t) => {
                let (name, email) = match t.split_once('<') {
                    Some((n, e)) => (n.trim(), e.trim_end_matches('>').trim()),
                    None => (t, ""),
                };
                (company(email), name.to_string())
            }
            None => ("—".to_string(), "bez stopki Co-Authored-By".to_string()),
        };
        let stat = by_model.entry(format!("{company}\x1f{model}")).or_insert_with(|| Stat {
            company,
            model,
            last: date.to_string(), // git log idzie od najnowszych
            ..Stat::default()
        });
        stat.commits += 1;
        stat.first = date.to_string();
        for line in lines {
            let mut parts = line.split('\t');
            let (Some(a), Some(r), Some(path)) = (parts.next(), parts.next(), parts.next()) else { continue };
            if excluded(path) {
                continue;
            }
            if let (Ok(a), Ok(r)) = (a.parse::<u64>(), r.parse::<u64>()) {
                stat.added += a;
                stat.removed += r;
            }
        }
    }
    let mut stats: Vec<Stat> = by_model.into_values().collect();
    stats.sort_by(|a, b| b.commits.cmp(&a.commits).then(b.added.cmp(&a.added)));
    Some((stats, total, head))
}

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../..");
    println!("cargo:rerun-if-changed=build.rs");
    let git = root.join(".git");
    for p in ["HEAD", "logs/HEAD", "packed-refs", "refs/heads"] {
        let path = git.join(p);
        if path.exists() {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    let (stats, total, head) = collect(&root).unwrap_or_default();
    let mut code = String::from("pub const MODEL_STATS: &[ModelStat] = &[\n");
    for s in &stats {
        let _ = writeln!(
            code,
            "    ModelStat {{ company: {:?}, model: {:?}, commits: {}, added: {}, removed: {}, first: {:?}, last: {:?} }},",
            s.company, s.model, s.commits, s.added, s.removed, s.first, s.last
        );
    }
    let _ = writeln!(code, "];\npub const TOTAL_COMMITS: u32 = {total};\npub const STATS_HEAD: &str = {head:?};");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("git_stats.rs");
    std::fs::write(out, code).unwrap();
}
