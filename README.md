# AsOne – zdjęcia produktowe do internetu

Lekki edytor (Rust + `wgpu` + `egui_dock`) do jednego zadania: szybko przygotować paczkę ~10 zdjęć
produktu do publikacji w sieci. Kadr, marginesy, białe tło, kwadrat 1:1, usuwanie tła (lokalny
model AI, gumka, prostokąt), nazwy przyjazne dla URL i eksport do Progressive JPEG bez metadanych
z limitami pilnowanymi przy każdym zapisie.

## Pobieranie

Gotowe pliki są w [Releases](https://github.com/LucasTerg/AsOne/releases): Linux (AppImage,
Flatpak, tar.gz; x86_64 i aarch64), Windows (x86_64, ARM64), macOS (Apple Silicon, Intel),
FreeBSD, Android (APK) i wersja web. Buduje je GitHub Actions (`.github/workflows/release.yml`)
po wypchnięciu tagu: `git tag v0.1.1 && git push origin v0.1.1`.

Android: własne repozytorium F-Droid na GitHub Pages – <https://lucasterg.github.io/AsOne/>
(w aplikacji F-Droid: Ustawienia › Repozytoria › + › zeskanuj kod QR).

<img src="packaging/fdroid/qr.svg" width="200" alt="Kod QR repozytorium F-Droid AsOne">

`https://lucasterg.github.io/AsOne/fdroid/repo?fingerprint=74B23E2A2688B820B1CB70B47D702F16BC7CFB1F129AF377D7FD11A92BF33E3E`

Workflow dokłada do niego APK przy każdym
wydaniu (`packaging/fdroid/update-repo.sh`, gałąź `gh-pages`), jeśli są ustawione sekrety
`ANDROID_KEYSTORE_BASE64` i `ANDROID_KEYSTORE_PASSWORD` (stały klucz podpisu – inaczej aktualizacje
się nie zainstalują). Bez F-Droid: [Obtainium](https://github.com/ImranR98/Obtainium) z adresem
tego repozytorium pobiera APK prosto z Releases.

APK lokalnie (Podman, Android SDK/NDK, Java z Android Studio – jak w word-apk):
`packaging/android/build-local.sh` → `dist/AsOne-<wersja>-android.apk`, instalacja `adb install -r …`.

## Struktura (workspace jak w programach Craft)

| Ścieżka | Co to jest |
|---|---|
| `crates/asone-core` | Rdzeń bez GUI: `naming` (slug + numeracja), `edits` (kadr, marginesy, tło, maska), `imageio` (dekodowanie z orientacją EXIF, Lanczos3), `compose` (kompozycja CPU), `jpeg` (MozJPEG / `jpeg-encoder`), `finish` (limity 3000×3600 / 3 MB), `autocrop`, `archive` (ZIP, PDF), `batch` (eksport wsadowy z ochroną oryginałów). |
| `apps/asone` | Aplikacja okienkowa: `app/` (stan, panele, inspektor, pliki), `document.rs`, `renderer.rs` + `shader.wgsl`, `canvas.rs`, `tabs.rs`, `export.rs`, `loader.rs`, `ai.rs` (RMBG przez ONNX Runtime), `titlebar.rs`, `prefs.rs`, `settings.rs`, `theme.rs`, `platform.rs`, `web.rs`; wersja web: `index.html`, `web/loader.mjs`. |
| `packaging/linux` | AppImage i Flatpak: `package.sh`, `AppRun`, `.desktop`, metadane AppStream, ikony. |
| `apps/asone-cli` | `asone-cli`: `export`, `names`, `inspect`, `mcp` (serwer MCP na `rmcp`). |

## Przepływ pracy

1. Przeciągnij zdjęcia (JPG, PNG, WebP, AVIF, TIFF, BMP; albo folder) na okno → **Lista plików**.
   Widok: *Miniatury* (suwak rozmiaru, Ctrl + kółko, nazwy pod miniaturą albo bez), *Lista*
   (miniatura i pełna, zawijana nazwa) lub *Szczegóły* (tabela: nazwa, wymiary, rozmiar, typ).
2. Kliknij miniaturę → zdjęcie otwiera się w osobnej zakładce `#1`, `#2`, … (ponowny klik zamyka
   zakładkę). „Odznacz” zamyka wszystkie zakładki, „Wyczyść listę” usuwa zdjęcia z listy.
3. Wklej nazwę (przycisk „Wklej”), np. `123-marka model` → `123-marka-model-1.jpg`, `-2.jpg`, …
   („Start nr” zmienia pierwszy numer). Menu kontekstowe zdjęcia › **Dopisz wersję**: `-ver2`
   (zwykle), `-ver3`, `-ver4`, `-ver5` → `123-marka-2-ver2.jpg`.
4. Przeciągnij miniatury na liście albo zakładki, żeby zmienić kolejność – numeracja przelicza się od razu.
5. Kadruj (C), dodaj marginesy, białe tło, „Dopasuj do kwadratu 1:1”, usuń tło gumką (E) lub prostokątem (M).
6. **Zapisz z zakładek (N)** albo `Ctrl+S` – eksport tylko otwartych zakładek, w ich kolejności.
   Albo **Zmień tylko nazwę** – bez nowych JPG: pliki otwartych zakładek dostają na dysku nowe
   nazwy (w swoich folderach, rozszerzenie zostaje), po podglądzie zmian (jak w Asystencie PIM).

Domyślny folder zapisu to `web/` obok zdjęć (zmiana: przycisk „Folder” w górnym pasku).
Eksport nigdy nie modyfikuje ani nie nadpisuje oryginałów; „Zmień tylko nazwę” zmienia wyłącznie
ich nazwy i nie nadpisuje istniejących plików.

**Ustawienia › Preferencje** (jak w PhotoCrafcie): *Interfejs* (16 motywów, w tym Sepia, kolor
płótna, skala interfejsu − / +, zakładki zdjęć na górze lub na dole, na zakładce pełna nazwa albo
tylko numer `#1`, systemowy pasek tytułu, „Ukryj”), *Eksport i limity* (maks. 3000 ×
3600 px i 3 MB, pilnowane przy każdym zapisie – najpierw spada jakość, potem wymiary; min. bok
500 px – mniejsze zdjęcia są dopełniane białym tłem, nie da się ustawić mniej),
*Usuwanie tła (AI)*. **Ustawienia › Ukryj** chowa: Edytuj w GIMP, Usuń z dysku, Inspekcja tła,
Maska aktywna (ta i tak pokazuje się tylko dla PNG z przezroczystością albo po edycji maski).

**Prawy panel › Metadane**: skąd jest plik (adres pobrania zapisany przez przeglądarkę), prawa
autorskie i licencja (w tym rozpoznanie banków zdjęć), oznaczenia AI (IPTC, C2PA, program, parametry
PNG), GPS i dane osobowe, a pod spodem pełne EXIF / XMP / IPTC / teksty PNG. „Zapisz z zakładek”
usuwa wszystkie metadane.

**Edycja › Cofnij / Ponów** (też przyciski na pasku narzędzi): osobna historia każdego zdjęcia –
kadr, marginesy, tło, gumka, wycinanie prostokątem, usunięcie tła AI. Jeden krok = jedna czynność
(np. całe pociągnięcie gumką); z maski zapamiętywany jest tylko zmieniony fragment.

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
| `Ctrl+Z` / `Ctrl+Shift+Z` (`Ctrl+Y`) | Cofnij / Ponów |
| `Ctrl++` / `Ctrl+−` | skala interfejsu |
| `Alt+P/E/U/W/M`, `F10`, `←` `→`, `Esc` | menu Plik / Edycja / Ustawienia / Widok / Pomoc |
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
| **Android** | `android/` (Gradle, powłoka Kotlin na GameActivity) + `libasone_app.so` z `cargo ndk`; lokalnie `packaging/android/build-local.sh` (Podman) | Zdjęcia z systemowego okna wyboru (Galeria, Pliki), zapis do Galerii (`Pictures/AsOne`), nazwa przez okno z klawiaturą systemową, „Wklej” ze schowka; interfejs omija zegar, pasek gestów i wycięcie aparatu. Usuwanie tła AI – niedostępne. |
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

| Model | Plik | Dostęp | RAM (szczyt) · czas na CPU |
|---|---|---|---|
| BRIA RMBG-1.4 | [model.onnx](https://huggingface.co/briaai/RMBG-1.4/resolve/main/onnx/model.onnx) (~176 MB) | publiczny | ~1 GB · ~1 s |
| BRIA RMBG-2.0 | [model.onnx](https://huggingface.co/briaai/RMBG-2.0/resolve/main/onnx/model.onnx) (~1 GB) | token Hugging Face (darmowy po rejestracji, <https://huggingface.co/settings/tokens>) + akceptacja licencji na stronie modelu | **~7,6 GB** · 15–20 s |

RMBG-2.0 (BiRefNet) jest bardzo pamięciożerny: przed każdym przebiegiem program sprawdza wolną
pamięć (RAM + swap) i zamiast dać się zabić systemowi (OOM) pokazuje komunikat z propozycją
RMBG-1.4. Model jest zwalniany z pamięci po 2 minutach bezczynności.

Domyślne miejsce: `~/.local/share/asone/models/`. Modele BRIA są darmowe do użytku
niekomercyjnego. AI działa w wersjach desktopowych (Linux, Windows, macOS); w przeglądarce,
na Androidzie i FreeBSD przycisk zgłasza, że funkcja jest niedostępna.
Test na prawdziwym modelu: `cargo test -p asone rmbg14 -- --ignored`; pomiar pamięci:
`ASONE_RMBG_MODEL=…/rmbg-2.0.onnx ASONE_RMBG_KIND=20 cargo test -p asone --release rmbg_pamiec -- --ignored --nocapture`.

## Ograniczenia

- Profile ICC (np. Display P3 z iPhone'a) nie są konwertowane do sRGB – są usuwane, więc kolory
  takich zdjęć mogą się lekko przesunąć.
- Każde otwarte zdjęcie trzyma teksturę z mipmapami i maskę w VRAM/RAM (~130 MB dla 24 Mpx).
- Brak Undo/Redo.
