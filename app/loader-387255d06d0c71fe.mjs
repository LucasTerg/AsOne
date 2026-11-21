// Karta startowa AsOne (inicjalizator Trunka: data-initializer="web/loader.mjs").
//
// * pasek rzeczywistego transferu WebAssembly (MB pobrane / całkowity rozmiar, %),
// * przełącznik ⚡ WebGPU / ⚙ WebGL2 (?backend=webgpu, ?backend=webgl2, ?webgl=1) – zachowuje
//   pozostałe parametry; brak WebGPU w przeglądarce → od razu WebGL2, błąd WebGPU → aplikacja sama
//   przeładowuje się z ?webgl=1,
// * pliki z sieci: ?file=…, ?url=… (można powtarzać), ?sample (przykładowe zdjęcia), ?new (pusto);
//   pobieranie kaskadowe: bezpośrednio → https://lmk.one/api/cors?url= → https://corsproxy.io/?url=.

const $ = (id) => document.getElementById(id);
const params = new URLSearchParams(location.search);
const backendParam = (params.get("backend") || "").toLowerCase();
const hasWebGpu = typeof navigator !== "undefined" && "gpu" in navigator;
const webglMode = params.has("webgl") || backendParam === "webgl2" || backendParam === "webgl" || !hasWebGpu;

const MB = 1024 * 1024;
const SAMPLES = ["sample/produkt-1.jpg", "sample/produkt-2.jpg", "sample/produkt-3.png"];
const CORS_PROXIES = [
  (u) => `https://lmk.one/api/cors?url=${encodeURIComponent(u)}`,
  (u) => `https://corsproxy.io/?url=${encodeURIComponent(u)}`,
];

function setStatus(text) {
  const el = $("status_text");
  if (el) el.textContent = text;
}

function setProgress(current, total) {
  const pct = total > 0 ? Math.min(100, Math.max(0, Math.round((current / total) * 100))) : 0;
  const bar = $("progress_bar");
  if (bar) bar.style.width = pct + "%";
  const p = $("progress_pct");
  if (p) p.textContent = pct + "%";
  const b = $("progress_bytes");
  if (b) {
    b.textContent = total > 0
      ? `${(current / MB).toFixed(1)} MB / ${(total / MB).toFixed(1)} MB`
      : `${(current / MB).toFixed(1)} MB`;
  }
}

function showError(err) {
  setStatus("Nie udało się uruchomić AsOne");
  const box = $("error_box");
  if (!box) return;
  const msg = String((err && (err.message || err)) || "Nieznany błąd")
    .replace(/[<>&]/g, (c) => ({ "<": "&lt;", ">": "&gt;", "&": "&amp;" })[c]);
  box.style.display = "block";
  box.innerHTML = `<strong>Wystąpił problem:</strong><br>${msg}<br><br>` +
    `<a href="${withParams({ webgl: "1", backend: null })}">Uruchom w trybie zgodności WebGL2</a> | ` +
    `<a href="javascript:location.reload()">Odśwież</a>`;
}

/** Adres strony z podmienionymi parametrami (null = usuń), pozostałe (np. ?file=) zostają. */
function withParams(changes) {
  const p = new URLSearchParams(location.search);
  for (const [k, v] of Object.entries(changes)) {
    if (v === null) p.delete(k);
    else p.set(k, v);
  }
  const q = p.toString();
  return q ? `?${q}` : location.pathname;
}

// Przełącznik backendu.
(() => {
  const gpu = $("btn_webgpu");
  const gl = $("btn_webgl");
  if (gpu) {
    gpu.href = withParams({ backend: "webgpu", webgl: null });
    gpu.classList.toggle("active", !webglMode);
    if (!hasWebGpu) gpu.title = "Ta przeglądarka nie obsługuje WebGPU – używany jest WebGL2";
  }
  if (gl) {
    gl.href = withParams({ webgl: "1", backend: null });
    gl.classList.toggle("active", webglMode);
  }
})();

/** Pobieranie z kaskadą CORS: bezpośrednio, potem przez proxy (tylko dla adresów z innych domen). */
export async function fetchWithCorsFallback(url) {
  const target = new URL(url, location.href);
  const candidates = [target.href];
  if (/^https?:$/.test(target.protocol) && target.origin !== location.origin) {
    for (const proxy of CORS_PROXIES) candidates.push(proxy(target.href));
  }
  let lastError = null;
  for (const candidate of candidates) {
    try {
      const res = await fetch(candidate);
      if (res.ok) return res;
      lastError = new Error(`HTTP ${res.status}`);
    } catch (e) {
      lastError = e;
    }
    console.info(`[AsOne] ${candidate} – ${lastError?.message || lastError}; próba kolejnego źródła…`);
  }
  throw lastError || new Error("nie udało się pobrać pliku");
}

function fileNameFor(url, res) {
  const cd = res.headers.get("content-disposition") || "";
  const m = cd.match(/filename\*?=(?:UTF-8'')?"?([^";]+)"?/i);
  let name = m ? decodeURIComponent(m[1]) : "";
  if (!name) {
    try {
      name = decodeURIComponent(new URL(url, location.href).pathname.split("/").pop() || "");
    } catch (_) {}
  }
  if (!name) name = "zdjecie";
  if (!/\.(jpe?g|png|webp|tiff?|bmp)$/i.test(name)) {
    const type = (res.headers.get("content-type") || "").toLowerCase();
    const ext = type.includes("png") ? "png" : type.includes("webp") ? "webp" : type.includes("tiff") ? "tif" : "jpg";
    name += "." + ext;
  }
  return name;
}

async function bindings() {
  for (let i = 0; i < 100; i++) {
    if (window.wasmBindings?.asone_open_file) return window.wasmBindings;
    await new Promise((r) => setTimeout(r, 50));
  }
  throw new Error("brak powiązań WebAssembly");
}

/** ?file= / ?url= / ?sample → pliki trafiają do aplikacji (lista + zakładki). */
async function openFromParams() {
  if (params.has("new") || params.has("empty")) return;
  const urls = [...params.getAll("file"), ...params.getAll("url")];
  if (params.has("sample")) urls.push(...SAMPLES);
  if (urls.length === 0) return;
  const b = await bindings();
  for (const url of urls) {
    try {
      const res = await fetchWithCorsFallback(url);
      const bytes = new Uint8Array(await res.arrayBuffer());
      b.asone_open_file(fileNameFor(url, res), bytes);
    } catch (e) {
      console.warn("[AsOne] Nie udało się pobrać", url, e);
      b.asone_notify(`Nie udało się pobrać ${url}: ${e?.message || e}`, true);
    }
  }
}

function hideLoader() {
  const el = $("asone_loading");
  if (!el) return;
  el.style.opacity = "0";
  setTimeout(() => el.remove(), 260);
}

// Sygnały z aplikacji (src/web.rs).
window.addEventListener("asone-ready", () => {
  setStatus("Gotowe");
  // Chwila na pierwszą klatkę egui pod kartą – bez błysku.
  requestAnimationFrame(() => requestAnimationFrame(() => {
    hideLoader();
    openFromParams();
  }));
});
window.addEventListener("asone-error", (e) => showError(e.detail));

// Inicjalizator Trunka.
export default function asoneLoader() {
  return {
    onStart: () => setStatus("Pobieranie silnika AsOne (WebAssembly)…"),
    onProgress: ({ current, total }) => setProgress(current, total),
    onComplete: () => setStatus(webglMode ? "Uruchamianie (WebGL2)…" : "Uruchamianie GPU (WebGPU)…"),
    onSuccess: () => {},
    onFailure: (error) => {
      console.error("[AsOne] start:", error);
      showError(error);
    },
  };
}
