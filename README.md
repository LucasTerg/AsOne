# AsOne – zdjęcia produktowe do internetu

Lekki edytor (Rust + `wgpu` + `egui_dock`) do jednego zadania: szybko przygotować paczkę ~10 zdjęć
produktu do publikacji w sieci. Kadr, marginesy, białe tło, kwadrat 1:1, usuwanie tła (lokalny
model AI, gumka, prostokąt), nazwy przyjazne dla URL i eksport do Progressive JPEG bez metadanych
z limitami pilnowanymi przy każdym zapisie.

## Struktura (workspace jak w programach Craft)

| Ścieżka | Co to jest |
|---|---|
| `crates/asone-core` | Rdzeń bez GUI: `naming` (slug + numeracja), `edits` (kadr, marginesy, tło, maska), `imageio` (dekodowanie z orientacją EXIF, Lanczos3), `compose` (kompozycja CPU), `jpeg` (MozJPEG / `jpeg-encoder`), `finish` (limity 3000×3600 / 3 MB), `autocrop`, `archive` (ZIP, PDF), `batch` (eksport wsadowy z ochroną oryginałów). |
| `apps/asone` | Aplikacja okienkowa: `app/` (stan, panele, inspektor, pliki), `document.rs`, `renderer.rs` + `shader.wgsl`, `canvas.rs`, `tabs.rs`, `export.rs`, `loader.rs`, `ai.rs` (RMBG przez ONNX Runtime), `titlebar.rs`, `prefs.rs`, `settings.rs`, `theme.rs`, `platform.rs`, `web.rs`; wersja web: `index.html`, `web/loader.mjs`. |
| `packaging/linux` | AppImage i Flatpak: `package.sh`, `AppRun`, `.desktop`, metadane AppStream, ikony. |
| `apps/asone-cli` | `asone-cli`: `export`, `names`, `inspect`, `mcp` (serwer MCP na `rmcp`). |

## Przepływ pracy

1. Przeciągnij zdjęcia (albo folder) na okno → **Lista plików** (miniatury).
2. Kliknij miniaturę → zdjęcie otwiera się w osobnej zakładce `#1`, `#2`, … (ponowny klik zamyka
   zakładkę). „Odznacz” zamyka wszystkie zakładki, „Wyczyść listę” usuwa zdjęcia z listy.
3. Wklej nazwę paczki (przycisk „Wklej”), np. `123-marka model` → `123-marka-model-1.jpg`, `-2.jpg`, …
   („Start nr” zmienia pierwszy numer).
4. Przeciągnij miniatury na liście albo zakładki, żeby zmienić kolejność – numeracja przelicza się od razu.
5. Kadruj (C), dodaj marginesy, białe tło, „Dopasuj do kwadratu 1:1”, usuń tło gumką (E) lub prostokątem (M).
6. **Zapisz z zakładek (N)** albo `Ctrl+S` – eksport tylko otwartych zakładek, w ich kolejności.

Domyślny folder zapisu to `web/` obok zdjęć (zmiana: przycisk „Folder” w górnym pasku).
Oryginały nigdy nie są modyfikowane ani nadpisywane.

**Ustawienia › Preferencje** (jak w PhotoCrafcie): *Interfejs* (16 motywów, w tym Sepia, kolor
płótna, skala interfejsu − / +, systemowy pasek tytułu, „Ukryj”), *Eksport i limity* (maks. 3000 ×
3600 px i 3 MB, pilnowane przy każdym zapisie – najpierw spada jakość, potem wymiary),
*Usuwanie tła (AI)*. **Ustawienia › Ukryj** chowa: Edytuj w GIMP, Usuń z dysku, Inspekcja tła,
Maska aktywna (ta i tak pokazuje się tylko dla PNG z przezroczystością albo po edycji maski).

Z Asystenta PIM: Kadruj (auto), Ramka / Paski 5 px, Min. 500 px, Inspekcja tła (`I`), Start nr,
menu kontekstowe listy (otwórz plik/folder, GIMP, ZIP, PDF, wczytaj ponownie, usuń z dysku do
kosza lub trwale, usuń z listy / wszystkie z listy).

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
| `I` | inspekcja tła |
| `Ctrl++` / `Ctrl+−` | skala interfejsu |
| `Alt+P/U/W/M`, `F10`, `←` `→`, `Esc` | menu Plik / Ustawienia / Widok / Pomoc |
| `Ctrl+S` / `Ctrl+O` / `Ctrl+Q` | Zapisz z zakładek / Dodaj zdjęcia / Zakończ |

## Budowanie

```sh
cargo run --release -p asone            # aplikacja okienkowa
cargo run --release -p asone -- *.jpg   # od razu z plikami w zakładkach
cargo build --release -p asone-cli      # CLI + MCP
cargo test --workspace                  # testy (bez GPU)
packaging/linux/package.sh              # dist/AsOne-<wersja>-x86_64.AppImage + .flatpak
```

### Pakiety Linuksa

`packaging/linux/package.sh [appimage|flatpak|all]` buduje z tych samych binarek release:

- **AppImage** – jeden plik, `AsOne.AppImage --cli …` uruchamia `asone-cli` (np. dla MCP);
  przy pierwszym starcie dodaje skrót i ikonę do menu (`ASONE_NO_DESKTOP_INTEGRATION=1` wyłącza).
- **Flatpak** – `io.github.lucasterg.AsOne`, runtime `org.freedesktop.Platform 26.08`; potrzebne jest
  tylko polecenie `flatpak` (bez flatpak-buildera i SDK). Instalacja:
  `flatpak install --user dist/AsOne-<wersja>-x86_64.flatpak`. Uprawnienia: Wayland/X11, GPU, sieć
  (pobieranie modelu AI), foldery Obrazy / Pobrane / Dokumenty / Pulpit – bez wychodzenia poza
  piaskownicę (dlatego „Edytuj w GIMP” w Flatpaku tylko podpowiada, jak otworzyć plik).

Binarki zbudowane w kontenerze Debian 13 wymagają glibc ≥ 2.39 (GUI) / 2.34 (CLI).

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

Lokalnie, przez ONNX Runtime (`ort`, wkompilowany statycznie) – zdjęcia nie są nigdzie wysyłane.
Ustawienia › Preferencje › **Usuwanie tła (AI)**: wybór modelu, ścieżka pliku („Wybierz…”),
„Pobierz model” z paskiem postępu.

| Model | Plik | Dostęp |
|---|---|---|
| BRIA RMBG-1.4 | [model.onnx](https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx) (~176 MB) | publiczny |
| BRIA RMBG-2.0 | [model.onnx](https://huggingface.co/briaai/RMBG-2.0/resolve/main/onnx/model.onnx) (~1 GB) | token Hugging Face (darmowy po rejestracji, <https://huggingface.co/settings/tokens>) + akceptacja licencji na stronie modelu |

Domyślne miejsce: `~/.local/share/asone/models/`. Modele BRIA są darmowe do użytku
niekomercyjnego. AI działa w wersjach desktopowych (Linux, Windows, macOS); w przeglądarce,
na Androidzie i FreeBSD przycisk zgłasza, że funkcja jest niedostępna.
Test na prawdziwym modelu: `cargo test -p asone rmbg14 -- --ignored`.

## Ograniczenia

- Profile ICC (np. Display P3 z iPhone'a) nie są konwertowane do sRGB – są usuwane, więc kolory
  takich zdjęć mogą się lekko przesunąć.
- Każde otwarte zdjęcie trzyma teksturę z mipmapami i maskę w VRAM/RAM (~130 MB dla 24 Mpx).
- Brak Undo/Redo.
