# AsOne – zdjęcia produktowe do internetu

Lekki edytor (Rust + `wgpu` + `egui_dock`) do jednego zadania: szybko przygotować paczkę ~10 zdjęć
produktu do publikacji w sieci. Kadr, marginesy, białe tło, kwadrat 1:1, usuwanie tła (gumka,
prostokąt, interfejs pod AI), nazwy przyjazne dla URL i eksport do Progressive JPEG bez metadanych.

Specyfikacja: [`specyfikacja_i_opis_projektu.md`](specyfikacja_i_opis_projektu.md),
zadanie: [`prompt_do_budowy_aplikacji.md`](prompt_do_budowy_aplikacji.md).

## Struktura (workspace jak w programach Craft)

| Ścieżka | Co to jest |
|---|---|
| `crates/asone-core` | Rdzeń bez GUI: `naming` (slug + numeracja), `edits` (kadr, marginesy, tło, maska), `imageio` (dekodowanie z orientacją EXIF, Lanczos3), `compose` (kompozycja CPU), `jpeg` (MozJPEG / `jpeg-encoder`), `batch` (eksport wsadowy z ochroną oryginałów). |
| `apps/asone` | Aplikacja okienkowa: `main.rs`, `lib.rs`, `app.rs`, `document.rs`, `renderer.rs`, `shader.wgsl`, `export.rs`, `canvas.rs`, `tabs.rs`, `loader.rs`, `ai.rs`, `platform.rs`, `theme.rs`. |
| `apps/asone-cli` | `asone-cli`: `export`, `names`, `inspect`, `mcp` (serwer MCP na `rmcp`). |

## Przepływ pracy

1. Przeciągnij zdjęcia (albo folder) na okno → **Lista plików** (miniatury).
2. Kliknij miniaturę → zdjęcie otwiera się w osobnej zakładce `#1`, `#2`, …
3. Wpisz nazwę paczki, np. `123-marka model` → pliki `123-marka-model-1.jpg`, `-2.jpg`, …
4. Przeciągnij zakładki, żeby zmienić kolejność – numeracja przelicza się od razu.
5. Kadruj (C), dodaj marginesy, białe tło, „Dopasuj do kwadratu 1:1”, usuń tło gumką (E) lub prostokątem (M).
6. **Zapisz z zakładek (N)** albo `Ctrl+S` – eksport tylko otwartych zakładek, w ich kolejności.

Domyślny folder zapisu to `web/` obok zdjęć (zmiana: przycisk „Folder” w górnym pasku).
Oryginały nigdy nie są modyfikowane ani nadpisywane.

### Skróty

| Klawisz | Działanie |
|---|---|
| `H` / `C` / `E` / `M` | Przesuwanie / Kadrowanie / Gumka / Wycinanie prostokątem |
| kółko myszy | zoom w punkt kursora |
| `Spacja`+LPM, środkowy przycisk | przesuwanie widoku |
| `Ctrl+0` / `Ctrl+1` | dopasuj do okna / 100 % |
| `[` `]` | rozmiar pędzla |
| LPM / PPM (lub `Alt`+LPM) | gumka: usuń / przywróć |
| `Delete` / `Shift+Delete` / `Esc` | wytnij / przywróć zaznaczenie, odznacz |
| `Enter` lub dwuklik | zatwierdź kadr |
| `Ctrl+S` | Zapisz z zakładek |

## Budowanie

```sh
cargo run --release -p asone            # aplikacja okienkowa
cargo run --release -p asone -- *.jpg   # od razu z plikami w zakładkach
cargo build --release -p asone-cli      # CLI + MCP
cargo test --workspace                  # testy (bez GPU)
```

MozJPEG to biblioteka C – potrzebny jest kompilator C (`cc`/`gcc`/`clang`/MSVC) i `cmake`.
Funkcja `nasm` włącza wstawki SIMD (szybsza kompresja), jeśli `nasm` jest zainstalowany:
`cargo build --release -p asone --features nasm`. Bez kompilatora C:
`--no-default-features` (czysto rustowy `jpeg-encoder`, też Progressive JPEG bez metadanych).

### Platformy

| System | Jak | Uwagi |
|---|---|---|
| **Linux** (x86_64, aarch64) | `cargo build --release -p asone` | Główna platforma. Wayland i X11, Vulkan, awaryjnie OpenGL. Okna wyboru plików przez portal XDG. |
| **FreeBSD** | jak Linux | Vulkan (Mesa), X11/Wayland. |
| **Windows** (x86_64, ARM64) | jak wyżej (MSVC) | DirectX 12 / Vulkan; ARM64 = laptopy ze Snapdragonem. |
| **macOS** (Apple Silicon, Intel) | jak wyżej (Xcode CLT) | Metal. |
| **Android** | `xbuild` lub `cargo-apk`, biblioteka `cdylib` z `android_main` | Bez systemowego okna wyboru: folder wpisuje się w liście plików (domyślnie `DCIM/Camera`), zapis do `Pictures/AsOne`. Aplikacja potrzebuje uprawnienia do zdjęć (`READ_MEDIA_IMAGES`). |
| **Przeglądarka (wasm)** | `cd apps/asone && trunk serve --release` | WebGPU lub WebGL2. Pliki wrzuca się przeciąganiem / przyciskiem, wyniki trafiają do „Pobranych”. Kompresja: `jpeg-encoder` (MozJPEG to kod C). |
| **ChromeOS** | wersja **Linux** w „Środowisku programistycznym Linux” (Crostini) albo wersja przeglądarkowa | Osobny build nie jest potrzebny. W Crostini działa pełna wersja (D&D, foldery); bez akceleracji Vulkan `wgpu` przełącza się na OpenGL. Na Chromebookach ARM – target `aarch64-unknown-linux-gnu`. APK z Androida też zadziała, ale wersja Linux jest wygodniejsza. |

Sprawdzanie kompilacji na inne systemy (bez kompilatora C dla nich):

```sh
for t in x86_64-pc-windows-msvc aarch64-pc-windows-msvc aarch64-apple-darwin x86_64-unknown-freebsd \
         aarch64-linux-android wasm32-unknown-unknown aarch64-unknown-linux-gnu; do
  cargo check -p asone --target $t --no-default-features
done
```

## CLI

```sh
asone-cli names "123-marka model" -c 3
asone-cli inspect zdjecia/
asone-cli export -n "123-marka model" --max-long-side 1200 --margin 40 --square IMG_*.jpg
asone-cli export -n "buty" -o web --background "#FFFFFF" --crop 0.1,0,0.9,1 --json a.jpg b.png
```

Pliki są numerowane w kolejności podania. Kod wyjścia: 0 – wszystko zapisane, 1 – część plików
z błędem, 2 – błąd użycia.

## MCP

```sh
cargo install --path apps/asone-cli --locked
claude mcp add asone -- asone-cli mcp --automation-read-root /praca/zdjecia --automation-write-root /praca/zdjecia
```

```json
{"mcpServers":{"asone":{"command":"asone-cli","args":["mcp","--automation-read-root","/praca/zdjecia","--automation-write-root","/praca/zdjecia"]}}}
```

| Narzędzie | Opis |
|---|---|
| `names_preview` | slug i lista nazw dla `base_name` + `count` (tylko odczyt) |
| `images_inspect` | format, wymiary po EXIF, przezroczystość, EXIF/ICC, rozmiar (tylko odczyt) |
| `render_preview` | podgląd PNG wyniku dla jednego zdjęcia z `options` (tylko odczyt) |
| `batch_export` | eksport `inputs` → `out_dir` (domyślnie `web`) z `options` |

`options`: `quality`, `max_long_side`, `margin`, `margin_top|right|bottom|left`, `square`,
`background` (`#RRGGBB`), `crop` (`[l, t, r, b]` w 0–1).

Zasady jak w programach Craft: dostęp do plików tylko pod podanymi katalogami (bez katalogu –
brak uprawnienia), ścieżki z `..` i dowiązania prowadzące na zewnątrz są odrzucane, nieznane
argumenty dają JSON-RPC `-32602`, błędy wykonania – `isError: true`. Eksport zastępuje tylko
wcześniejsze pliki `.jpg` o tych samych nazwach, nigdy oryginałów.

## Usuwanie tła AI

`apps/asone/src/ai.rs` definiuje `BackgroundRemover` i wątek roboczy. Przycisk „Usuń tło (AI)”
działa już w całym potoku (pomniejszona kopia → model → maska skalowana Lanczos3 → GPU), ale
`OnnxBackgroundRemover` jest zaślepką: po dodaniu `ort` i pliku `models/rmbg-1.4.onnx` wystarczy
zaimplementować `predict_mask` (instrukcja w komentarzu modułu).

## Ograniczenia

- Profile ICC (np. Display P3 z iPhone'a) nie są konwertowane do sRGB – są usuwane, więc kolory
  takich zdjęć mogą się lekko przesunąć.
- Każde otwarte zdjęcie trzyma teksturę z mipmapami i maskę w VRAM/RAM (~130 MB dla 24 Mpx).
- Brak Undo/Redo (Etap 4) i prawdziwego modelu AI (Etap 5).
