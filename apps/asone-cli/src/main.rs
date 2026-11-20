//! `asone-cli` – AsOne bez okna: eksport paczek zdjęć, podgląd nazw, inspekcja i serwer MCP.

mod mcp;
mod options;
mod workspace;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use asone_core::batch::{BatchEntry, export_batch};
use asone_core::imageio::{expand_paths, probe_path};
use clap::{Parser, Subcommand};
use serde_json::{Value, json};

use crate::options::EditOptions;

#[derive(Parser)]
#[command(
    name = "asone-cli",
    version,
    about = "AsOne bez okna: zdjęcia produktowe do internetu (Progressive JPEG bez metadanych)",
    after_help = "Przykład:\n  asone-cli export -n \"Kubek ASONE Classic\" --max-long-side 1200 --margin 40 --square IMG_*.jpg\n  → web/Kubek-ASONE-Classic-1.jpg, web/Kubek-ASONE-Classic-2.jpg, …\n\nLimity domyślnie: 3000×3600 px i 3 MB (--max-width/--max-height/--max-file-mb, --no-limits)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Eksport paczki: pliki w podanej kolejności → <nazwa>-1.jpg, <nazwa>-2.jpg, …
    Export(ExportArgs),
    /// Podgląd nazw plików dla nazwy bazowej.
    Names {
        /// Nazwa bazowa, np. "Kubek ASONE Classic".
        name: String,
        /// Liczba zdjęć.
        #[arg(short, long, default_value_t = 3)]
        count: usize,
        /// Numer pierwszego pliku.
        #[arg(long, default_value_t = 1)]
        start: usize,
        /// Wynik jako JSON.
        #[arg(long)]
        json: bool,
    },
    /// Informacje o obrazach: format, wymiary (po orientacji EXIF), przezroczystość, metadane.
    Inspect {
        /// Pliki lub katalogi.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Wynik jako JSON.
        #[arg(long)]
        json: bool,
    },
    /// Serwer MCP na stdio (dla agentów AI, np. `claude mcp add asone -- asone-cli mcp …`).
    Mcp {
        /// Katalog, z którego agent może czytać zdjęcia.
        #[arg(long, value_name = "DIR")]
        automation_read_root: Option<PathBuf>,
        /// Katalog, w którym agent może zapisywać eksporty.
        #[arg(long, value_name = "DIR")]
        automation_write_root: Option<PathBuf>,
    },
}

#[derive(clap::Args)]
struct ExportArgs {
    /// Pliki lub katalogi; kolejność wyznacza numerację.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Nazwa bazowa paczki, np. "Kubek ASONE Classic".
    #[arg(short, long)]
    name: String,
    /// Folder wynikowy (domyślnie podfolder „web” obok pierwszego pliku).
    #[arg(short, long, value_name = "DIR")]
    out: Option<PathBuf>,
    #[command(flatten)]
    options: EditOptions,
    /// Liczba wątków (domyślnie wszystkie rdzenie).
    #[arg(short, long)]
    jobs: Option<usize>,
    /// Wynik jako JSON.
    #[arg(long)]
    json: bool,
}

/// Raport eksportu w JSON (wspólny dla CLI i MCP).
pub(crate) fn report_json(entries: &[BatchEntry], out_dir: &Path) -> Value {
    let files: Vec<Value> = entries
        .iter()
        .map(|e| match &e.result {
            Ok(s) => json!({
                "index": e.index, "source": e.source, "file": e.file_name,
                "width": s.size[0], "height": s.size[1], "bytes": s.bytes, "quality": s.quality,
                "adjustments": s.notes
            }),
            Err(err) => json!({ "index": e.index, "source": e.source, "file": e.file_name, "error": err }),
        })
        .collect();
    let saved = entries.iter().filter(|e| e.result.is_ok()).count();
    json!({
        "outDir": out_dir,
        "encoder": asone_core::jpeg::ENCODER_NAME,
        "saved": saved,
        "failed": entries.len() - saved,
        "files": files,
    })
}

fn export(args: ExportArgs) -> Result<bool, String> {
    let inputs = expand_paths(args.inputs.clone());
    if inputs.is_empty() {
        return Err("Brak obsługiwanych obrazów (JPG, PNG, WebP, TIFF, BMP)".into());
    }
    let opts = args.options.to_batch(&args.name)?;
    let out_dir = match args.out {
        Some(d) => d,
        None => inputs[0].parent().unwrap_or(Path::new(".")).join("web"),
    };
    let threads = args.jobs.unwrap_or_else(|| std::thread::available_parallelism().map_or(2, |n| n.get()));
    let quiet = args.json;
    let entries = export_batch(&inputs, &out_dir, &opts, threads, |e| {
        if quiet {
            return;
        }
        let src = e.source.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match &e.result {
            Ok(s) => {
                eprintln!(
                    "✔ {:<32} {:>5}×{:<5} {:>7.1} KB  q{:<3} ← {src}",
                    e.file_name,
                    s.size[0],
                    s.size[1],
                    s.bytes as f64 / 1024.0,
                    s.quality
                );
                for n in &s.notes {
                    eprintln!("    ↳ {n}");
                }
            }
            Err(err) => eprintln!("✖ {:<32} {err}  ← {src}", e.file_name),
        }
    });
    let report = report_json(&entries, &out_dir);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default());
    } else {
        eprintln!(
            "Zapisano {}/{} plików ({}) do {}",
            report["saved"], entries.len(), asone_core::jpeg::ENCODER_NAME, out_dir.display()
        );
    }
    Ok(report["failed"] == 0)
}

fn inspect(inputs: Vec<PathBuf>, as_json: bool) -> Result<bool, String> {
    let files = expand_paths(inputs);
    if files.is_empty() {
        return Err("Brak obsługiwanych obrazów".into());
    }
    let mut all_ok = true;
    let mut rows = Vec::new();
    for f in files {
        match probe_path(&f) {
            Ok(i) => {
                if !as_json {
                    let meta = [(i.has_exif, "EXIF"), (i.has_icc, "ICC")]
                        .iter()
                        .filter(|(has, _)| *has)
                        .map(|(_, n)| *n)
                        .collect::<Vec<_>>()
                        .join("+");
                    println!(
                        "{}  {}  {}×{}  {:.1} KB{}{}",
                        f.display(),
                        i.format,
                        i.width,
                        i.height,
                        i.bytes as f64 / 1024.0,
                        if i.has_alpha { "  przezroczystość" } else { "" },
                        if meta.is_empty() { String::new() } else { format!("  metadane: {meta}") }
                    );
                }
                rows.push(json!({
                    "path": f, "format": i.format, "width": i.width, "height": i.height,
                    "hasAlpha": i.has_alpha, "hasExif": i.has_exif, "hasIcc": i.has_icc, "bytes": i.bytes
                }));
            }
            Err(e) => {
                all_ok = false;
                if !as_json {
                    eprintln!("{}: {e}", f.display());
                }
                rows.push(json!({ "path": f, "error": e }));
            }
        }
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_default());
    }
    Ok(all_ok)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Export(args) => export(args),
        Command::Names { name, count, start, json } => {
            if asone_core::naming::slugify(&name).is_empty() {
                Err("Nazwa bazowa po oczyszczeniu jest pusta".into())
            } else {
                let names = asone_core::naming::plan_names_from(&name, start.max(1), count);
                if json {
                    println!("{}", serde_json::to_string_pretty(&names).unwrap_or_default());
                } else {
                    names.iter().for_each(|n| println!("{n}"));
                }
                Ok(true)
            }
        }
        Command::Inspect { inputs, json } => inspect(inputs, json),
        Command::Mcp { automation_read_root, automation_write_root } => {
            match workspace::Workspace::new(automation_read_root.as_deref(), automation_write_root.as_deref()) {
                Err(e) => {
                    eprintln!("asone-cli: {e}");
                    return ExitCode::from(2);
                }
                Ok(ws) => {
                    eprintln!("asone-cli mcp: {}", ws.describe());
                    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                        Ok(r) => r,
                        Err(e) => {
                            eprintln!("asone-cli: {e}");
                            return ExitCode::FAILURE;
                        }
                    };
                    runtime.block_on(mcp::serve_stdio(ws)).map(|()| true)
                }
            }
        }
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("asone-cli: {e}");
            ExitCode::from(2)
        }
    }
}
