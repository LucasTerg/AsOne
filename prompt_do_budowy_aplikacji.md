# Prompt: Budowa desktopowego edytora zdjęć produktowych w Rust (wgpu + egui_dock)

Wciel się w rolę starszego inżyniera Rust specjalizującego się w grafice komputerowej (`wgpu`), aplikacjach desktopowych (`egui` / `eframe`) oraz przetwarzaniu obrazów.

Zaprojektuj i zaimplementuj od podstaw lekką, natywną aplikację desktopową w stylu **PhotoCraft / Adobe Photoshop** (ciemny interfejs) służącą do szybkiej, wsadowej obróbki zdjęć produktowych przeznaczonych do publikacji w internecie.

---

## 1. Stos technologiczny (Crates)
Użyj wyłącznie nowoczesnych, sprawdzonych bibliotek Rust:
- **GUI i dokowanie zakładek:** `eframe` (z backendem `wgpu`), `egui`, `egui_dock`, `egui_extras` (do ikon SVG).
- **Renderowanie GPU:** `wgpu` (własny *RenderCallback* i shader WGSL dla płótna roboczego oraz renderowania *offscreen* przy eksporcie).
- **Wczytywanie obrazów i wielowątkowość:** `image`, `rayon`, `crossbeam-channel`.
- **Skalowanie (Resize):** `fast_image_resize` (filtr `Lanczos3` z akceleracją SIMD).
- **Kompresja pod internet (silnik ze Squoosh):** `mozjpeg` (zawsze **Progressive JPEG**, `optimize_scans(true)`, brak jakichkolwiek metadanych EXIF/XMP/ICC).
- **Usuwanie tła (lokalne AI – przygotuj architekturę / moduł):** `ort` (ONNX Runtime dla modelu np. `rmbg-1.4.onnx` uruchamianego w wątku w tle).

---

## 2. Układ interfejsu (Styl Photoshop / PhotoCraft)
Zastosuj ciemną, neutralną paletę kolorów:
- Tło płótna (Canvas): `#1E1E1E`
- Panele boczne i górne: `#262626` / `#2B2B2B`
- Pola i przyciski: `#383838`, zaokrąglenia `2.0–3.0 px`
- Akcent aktywnego narzędzia: `#2680EB`

Podziel okno na 4 główne strefy:
1. **Górny pasek (`TopBottomPanel::top`):**
   - Pole tekstowe na bazową nazwę paczki (np. `123-marka model`).
   - Ustawienia eksportu: jakość MozJPEG (domyślnie `90%`, zakres do `100%`), opcjonalne zmniejszanie rozmiaru (np. checkbox „Max dłuższy bok” + pole w pikselach, np. `1200 px`).
   - Główny przycisk **„Zapisz z zakładek (N)”** (gdzie `N` to liczba otwartych zakładek ze zdjęciami).
2. **Lewy pasek narzędzi (`SidePanel::left`, szerokość ~46 px):**
   - Pionowy pasek ikon: Przesuwanie (`H`), Kadrowanie (`C`), Gumka / Przywróć maskę (`E`), Wycinanie obszaru prostokątem (`M`).
3. **Prawy panel inspektora (`SidePanel::right`, szerokość ~270 px):**
   - Edycja aktywnego zdjęcia: przycisk **Usuń tło (AI)**, checkbox **Białe tło**, 4 pola na marginesy (Góra, Dół, Lewo, Prawo) + kłódka **„Spnij wszystkie boki”** + przycisk **„Dopasuj do kwadratu 1:1”**.
   - Przycisk **„Zastosuj ustawienia do wszystkich otwartych”** (kopiuje `EditParams` z aktywnego zdjęcia do pozostałych).
4. **Centrum (`CentralPanel` z `egui_dock::DockArea`):**
   - Użyj enuma `AppTab`:
     - `AppTab::FileList` – stała, niezamykalna zakładka („Lista plików”) wyświetlająca wszystkie wrzucone przez Drag & Drop zdjęcia jako listę/siatkę miniatur. Zdjęcia otwarte w osobnych zakładkach mają widoczne oznaczenie (np. `#1`, `#2`).
     - `AppTab::Photo(DocId)` – osobne zakładki dla pojedynczych zdjęć otwierane kliknięciem/dwuklikiem z `FileList`. Zamknięcie zakładki `×` nie usuwa pliku z listy, a jedynie zamyka zakładkę edycji.

---

## 3. Logika nazw plików i kolejności zakładek
- Napisz funkcję `build_filename(raw_input: &str, index: usize, ext: &str) -> String`, która:
  - zamienia polskie znaki diakrytyczne na ASCII (`ą→a`, `ł→l`, itd.),
  - zamienia spacje i znaki specjalne na pojedynczy myślnik `-`,
  - dokleja numer porządkowy (`-1`, `-2`, `-3`...) oraz rozszerzenie `.jpg`.
  - Przykład: `123-marka model` → `123-marka-model-1.jpg`, `123-marka-model-2.jpg`.
- **Kluczowa zasada:** Kolejność numeracji zależy wyłącznie od aktualnej kolejności otwartych zakładek typu `AppTab::Photo(DocId)` w `DockState` (z pominięciem `AppTab::FileList`). Przeciągnięcie zakładki w inne miejsce natychmiast aktualizuje tytuły zakładek i kolejność zapisu.

---

## 4. Architektura niedestrukcyjna i potok GPU (`wgpu`)
- Trzymaj dokumenty w `HashMap<DocId, Document>`. Każdy `Document` zawiera:
  - `source_texture: wgpu::Texture` (oryginał w VRAM),
  - `mask_texture: wgpu::Texture` (`R8Unorm`, domyślnie wypełniona `255`),
  - `mask_cpu: Vec<u8>` (kopia maski w RAM do szybkiej edycji pędzlem/gumką),
  - `edits: EditParams` (`crop: [f32; 4]`, `margins: [u32; 4]`, `bg_color: Option<[f32; 4]>`, `use_bg_removal: bool`).
- **Shader WGSL (jeden wspólny dla podglądu i eksportu):**
  - Obsługuje kamerę 2D (`zoom`, `offset_x`, `offset_y` – płynne przybliżanie kółkiem myszy w punkt kursora i przesuwanie środkowym przyciskiem / `Spacja + LPM`).
  - Wylicza obszar kadrowania oraz dodane marginesy (wypełniane kolorem tła lub szachownicą w podglądzie).
  - Łączy `source_texture` z `mask_texture` i opcjonalnym białym tłem (`mix(bg_color.rgb, pixel.rgb, mask_alpha)`).
- **Ręczne wycinanie / Gumka:**
  - LPM maluje `0` w `mask_cpu` (usuwa), PPM / `Alt+LPM` maluje `255` (przywraca).
  - Aktualizuj teksturę na GPU wyłącznie w zmienionym prostokącie (*dirty rect*) przez `queue.write_texture`.
  - Narzędzie prostokąta pozwala zaznaczyć obszar i wyzerować go w masce klawiszem `Delete`.

---

## 5. Potok eksportu („Zapisz z zakładek”)
Po kliknięciu „Zapisz z zakładek”:
1. Pobierz listę `DocId` z otwartych zakładek `AppTab::Photo` w ich aktualnej kolejności.
2. Dla każdego dokumentu wyrenderuj wynik na GPU do tekstury *offscreen* o dokładnym rozmiarze wynikowym (`crop + margins`) i skopiuj czyste piksele RGBA do RAM (automatycznie usuwa to 100% metadanych EXIF/GPS/ICC).
3. W puli wątków `rayon`:
   - Jeśli włączony jest limit rozdzielczości, przeskaluj obraz przez `fast_image_resize` (`Lanczos3`).
   - Skonwertuj RGBA → RGB i zakoduj przez `mozjpeg` jako **Progressive JPEG** (`set_progressive_mode()`, `set_optimize_scans(true)`).
   - Zapisz na dysk pod wygenerowaną nazwą (`123-marka-model-1.jpg`, itd.).

---

## 6. Oczekiwany wynik (Etapy implementacji)
Zacznij od wygenerowania kompletnej struktury projektu (`Cargo.toml` oraz podziału na moduły: `main.rs`, `app.rs`, `document.rs`, `renderer.rs`, `shader.wgsl`, `export.rs`, `naming.rs`) i zaimplementuj działający **Etap 1 + 2**:
- okno w ciemnym stylu z `egui_dock` (`FileList` + otwierane zakładki `Photo`),
- Drag & Drop zdjęć,
- działające nazewnictwo i zmiana kolejności zakładek,
- shader `wgpu` z Zoom/Pan, kadrowaniem, marginesami i białym tłem,
- działający eksport przez `fast_image_resize` i `mozjpeg`.
Moduł AI (`ort`) przygotuj jako czysty interfejs/stub gotowy do podpięcia pliku `.onnx`.