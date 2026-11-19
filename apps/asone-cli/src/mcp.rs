//! Serwer MCP (oficjalne SDK `rmcp`, stdio). Nazwy narzędzi w `snake_case`, jak w programach Craft.
//!
//! Dostęp do plików wyłącznie pod `--automation-read-root` (wejście) i `--automation-write-root`
//! (wyjście). Bez katalogu dane uprawnienie nie istnieje. Oryginały nigdy nie są nadpisywane.

use std::path::PathBuf;
use std::sync::Arc;

use asone_core::batch::{BatchOptions, export_batch, process_image};
use asone_core::imageio::{decode_path, fit_within, probe_path, resize_rgba};
use asone_core::naming::slugify;
use base64::Engine as _;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::ToolCallContext;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt, tool, tool_handler, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::options::EditOptions;
use crate::workspace::Workspace;

const INSTRUCTIONS: &str = "AsOne przygotowuje paczki zdjęć produktowych do internetu: kadr (także auto do obiektu), marginesy, \
białe tło, kwadrat 1:1, zmniejszenie (Lanczos3) i Progressive JPEG bez metadanych (EXIF/GPS/ICC). \
Limity domyślnie zawsze pilnowane: maks. 3000×3600 px i 3 MB (options.max_width/max_height/max_file_mb, no_limits). \
Nazwy: `123-marka model` → `123-marka-model-1.jpg`, `-2.jpg`… w kolejności `inputs`. \
Najpierw `names_preview` i `images_inspect`, potem `render_preview` dla jednego zdjęcia, na końcu `batch_export`. \
Ścieżki wejściowe są względne do katalogu odczytu, `out_dir` – do katalogu zapisu.";

const MAX_INPUTS: usize = 1000;
const MAX_PREVIEW_SIDE: u32 = 2048;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamesParams {
    /// Nazwa bazowa paczki, np. „123-marka model”.
    pub base_name: String,
    /// Liczba zdjęć (1–1000).
    pub count: usize,
    /// Numer pierwszego pliku (domyślnie 1).
    #[serde(default)]
    pub start_number: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InspectParams {
    /// Ścieżki obrazów (względem katalogu odczytu).
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportParams {
    /// Nazwa bazowa paczki, np. „123-marka model”.
    pub base_name: String,
    /// Obrazy w kolejności numeracji (względem katalogu odczytu).
    pub inputs: Vec<String>,
    /// Folder wynikowy względem katalogu zapisu (domyślnie „web”). Istniejące pliki `.jpg`
    /// o tych samych nazwach są zastępowane; oryginały nigdy.
    #[serde(default)]
    pub out_dir: Option<String>,
    /// Ustawienia edycji i kompresji.
    #[serde(default)]
    pub options: EditOptions,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PreviewParams {
    /// Obraz (względem katalogu odczytu).
    pub path: String,
    /// Ustawienia edycji – jak w `batch_export`.
    #[serde(default)]
    pub options: EditOptions,
    /// Dłuższy bok podglądu PNG (domyślnie 1024, maks. 2048).
    #[serde(default)]
    pub max_side: Option<u32>,
}

#[derive(Clone)]
pub struct AsOneMcp {
    ws: Arc<Workspace>,
    tool_router: ToolRouter<Self>,
}

fn ok_json(v: &Value) -> CallToolResult {
    CallToolResult::success(vec![ContentBlock::text(
        serde_json::to_string_pretty(v).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
    )])
}

fn fail(e: impl std::fmt::Display) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(e.to_string())])
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tokio::task::spawn_blocking(f).await.map_err(|e| format!("zadanie przerwane: {e}"))
}

#[tool_router]
impl AsOneMcp {
    pub fn new(ws: Workspace) -> Self {
        Self { ws: Arc::new(ws), tool_router: Self::tool_router() }
    }

    #[tool(
        description = "Podgląd nazw plików dla nazwy bazowej: slug (ASCII, małe litery, myślniki) i numeracja `-1`, `-2`… Nie dotyka plików.",
        annotations(title = "Podgląd nazw", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn names_preview(&self, Parameters(p): Parameters<NamesParams>) -> Result<CallToolResult, McpError> {
        if !(1..=MAX_INPUTS).contains(&p.count) {
            return Ok(fail(format!("count: zakres 1–{MAX_INPUTS}")));
        }
        let slug = slugify(&p.base_name);
        if slug.is_empty() {
            return Ok(fail("Nazwa bazowa po oczyszczeniu jest pusta"));
        }
        let start = p.start_number.unwrap_or(1).max(1);
        Ok(ok_json(&json!({ "slug": slug, "names": asone_core::naming::plan_names_from(&p.base_name, start, p.count) })))
    }

    #[tool(
        description = "Informacje o obrazach z katalogu odczytu: format, wymiary po orientacji EXIF, przezroczystość, obecność EXIF/ICC, rozmiar pliku.",
        annotations(title = "Inspekcja obrazów", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn images_inspect(&self, Parameters(p): Parameters<InspectParams>) -> Result<CallToolResult, McpError> {
        if p.paths.is_empty() || p.paths.len() > MAX_INPUTS {
            return Ok(fail(format!("paths: od 1 do {MAX_INPUTS} pozycji")));
        }
        let ws = self.ws.clone();
        let rows = blocking(move || {
            p.paths
                .iter()
                .map(|path| match ws.resolve_read(path).and_then(|abs| probe_path(&abs)) {
                    Ok(i) => json!({
                        "path": path, "format": i.format, "width": i.width, "height": i.height,
                        "hasAlpha": i.has_alpha, "hasExif": i.has_exif, "hasIcc": i.has_icc, "bytes": i.bytes
                    }),
                    Err(e) => json!({ "path": path, "error": e }),
                })
                .collect::<Vec<_>>()
        })
        .await;
        Ok(match rows {
            Ok(rows) => ok_json(&Value::Array(rows)),
            Err(e) => fail(e),
        })
    }

    #[tool(
        description = "Renderuje jedno zdjęcie z ustawieniami (kadr, marginesy, tło, kwadrat) i zwraca podgląd PNG – bez zapisu na dysk.",
        annotations(title = "Podgląd wyniku", read_only_hint = true, destructive_hint = false, idempotent_hint = true, open_world_hint = false)
    )]
    async fn render_preview(&self, Parameters(p): Parameters<PreviewParams>) -> Result<CallToolResult, McpError> {
        let max_side = p.max_side.unwrap_or(1024).clamp(16, MAX_PREVIEW_SIDE);
        let opts = match p.options.to_batch("podglad") {
            Ok(o) => o,
            Err(e) => return Ok(fail(e)),
        };
        let abs = match self.ws.resolve_read(&p.path) {
            Ok(a) => a,
            Err(e) => return Ok(fail(e)),
        };
        let result = blocking(move || render_png(&abs, &opts, max_side)).await.and_then(|r| r);
        Ok(match result {
            Ok((png, note)) => CallToolResult::success(vec![
                ContentBlock::image(base64::engine::general_purpose::STANDARD.encode(png), "image/png"),
                ContentBlock::text(note.to_string()),
            ]),
            Err(e) => fail(e),
        })
    }

    #[tool(
        description = "Eksport paczki („Zapisz z zakładek”): `inputs` w podanej kolejności → `<slug>-1.jpg`, `<slug>-2.jpg`… w `out_dir` (pod katalogiem zapisu). \
Progressive JPEG bez metadanych; opcjonalnie kadr, marginesy, tło, kwadrat 1:1 i zmniejszenie Lanczos3. Zastępuje wcześniejsze eksporty o tych samych nazwach, nigdy oryginałów.",
        annotations(title = "Eksport paczki", read_only_hint = false, destructive_hint = true, idempotent_hint = true, open_world_hint = false)
    )]
    async fn batch_export(&self, Parameters(p): Parameters<ExportParams>) -> Result<CallToolResult, McpError> {
        if p.inputs.is_empty() || p.inputs.len() > MAX_INPUTS {
            return Ok(fail(format!("inputs: od 1 do {MAX_INPUTS} pozycji")));
        }
        let opts = match p.options.to_batch(&p.base_name) {
            Ok(o) => o,
            Err(e) => return Ok(fail(e)),
        };
        let inputs: Result<Vec<PathBuf>, String> = p.inputs.iter().map(|i| self.ws.resolve_read(i)).collect();
        let inputs = match inputs {
            Ok(i) => i,
            Err(e) => return Ok(fail(e)),
        };
        let out_dir = match self.ws.resolve_write_dir(p.out_dir.as_deref().unwrap_or("web")) {
            Ok(d) => d,
            Err(e) => return Ok(fail(e)),
        };
        let result = blocking(move || {
            let threads = std::thread::available_parallelism().map_or(2, |n| n.get());
            let entries = export_batch(&inputs, &out_dir, &opts, threads, |_| {});
            crate::report_json(&entries, &out_dir)
        })
        .await;
        Ok(match result {
            Ok(report) => {
                let failed = report["failed"].as_u64().unwrap_or(0);
                let mut r = ok_json(&report);
                if failed > 0 && report["saved"].as_u64() == Some(0) {
                    r.is_error = Some(true);
                }
                r
            }
            Err(e) => fail(e),
        })
    }
}

fn render_png(path: &std::path::Path, opts: &BatchOptions, max_side: u32) -> Result<(Vec<u8>, Value), String> {
    let img = decode_path(path)?;
    let processed = process_image(&img, opts)?;
    // Podgląd pokazuje dokładnie to, co trafi do pliku: dekodujemy wynikowy JPEG.
    let out = image::load_from_memory(&processed.jpeg).map_err(|e| e.to_string())?.to_rgba8();
    let size = fit_within([out.width(), out.height()], max_side);
    let small = resize_rgba(&out, size, asone_core::imageio::lanczos3())?;
    let mut png = Vec::new();
    small
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    let note = json!({
        "source": { "width": processed.source_size[0], "height": processed.source_size[1] },
        "output": { "width": processed.size[0], "height": processed.size[1], "jpegBytes": processed.jpeg.len(), "quality": processed.quality },
        "adjustments": processed.notes,
        "preview": { "width": size[0], "height": size[1] },
    });
    Ok((png, note))
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AsOneMcp {
    /// Jak w programach Craft: nieznany argument → JSON-RPC `-32602` z nazwą klucza, przed wykonaniem.
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        if let Some(tool) = self.tool_router.get(&request.name) {
            let properties = tool.input_schema.get("properties").and_then(Value::as_object);
            if let Some(key) = request
                .arguments
                .as_ref()
                .and_then(|args| args.keys().find(|k| properties.is_none_or(|p| !p.contains_key(*k))))
            {
                return Err(McpError::invalid_params(
                    format!("unknown argument `{key}` for {}", request.name),
                    None,
                ));
            }
        }
        self.tool_router.call(ToolCallContext::new(self, request, context)).await
    }

    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("asone", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// Obsługuje MCP na stdin/stdout aż do rozłączenia klienta.
pub async fn serve_stdio(ws: Workspace) -> Result<(), String> {
    let service = AsOneMcp::new(ws)
        .serve(rmcp::transport::stdio())
        .await
        .map_err(|e| format!("MCP: {e}"))?;
    service.waiting().await.map_err(|e| format!("MCP: {e}"))?;
    Ok(())
}
