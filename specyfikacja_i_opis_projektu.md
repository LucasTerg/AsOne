# Specyfikacja projektu: Natywny edytor zdjęć produktowych do internetu (Rust + wgpu)

## 1. Cel i założenia projektu
Aplikacja to lekki, szybki, natywny edytor graficzny (inspirowany ergonomią **Adobe Photoshop** oraz architekturą pakietu **ArtCraft / PhotoCraft**), zoptymalizowany pod jeden konkretny przepływ pracy (*workflow*): **szybkie przygotowanie paczki zdjęć (ok. 10 plików) do publikacji w internecie**.

Główne założenia:
- **Szybkość i lokalność:** brak chmury, natychmiastowy start, akceleracja GPU (`wgpu`) i wielowątkowy eksport (`rayon`).
- **Architektura niedestrukcyjna:** oryginały zdjęć nie są modyfikowane w trakcie pracy; wszystkie operacje (kadrowanie, marginesy, maska tła, białe tło) są nakładane w czasie rzeczywistym przez shader WGSL.
- **Gotowość pod internet (standard Squoosh):** każdy wyeksportowany plik jest automatycznie pozbawiony metadanych (EXIF, GPS, aparat), opcjonalnie zmniejszony wysokiej jakości filtrem (`Lanczos3`) i skompresowany silnikiem **MozJPEG** jako **Progressive JPEG**.

---

## 2. Przepływ pracy (Workflow) z zakładkami (`egui_dock`)
Interfejs łączy stałe panele narzędziowe znane z Photoshopa z elastycznym systemem zakładek w centrum okna:

1. **Wrzucenie paczki:** Przeciągasz pliki (Drag & Drop) na okno aplikacji. Wszystkie trafiają do stałej, niezamykalnej zakładki **Lista plików** (`AppTab::FileList`).
2. **Selekcja do zakładek:** Klikasz wybrane zdjęcia na liście – każde otwiera się w osobnej zakładce roboczej (`AppTab::Photo(DocId)`). Możesz otworzyć np. 5 najlepszych ujęć z 15 wrzuconych.
3. **Nazewnictwo zbiorcze:** W górnym pasku wklejasz bazową nazwę, np. `123-marka model`. Program automatycznie czyści ją do formatu przyjaznego dla URL (`slugify`) i przypisuje numery zgodnie z kolejnością otwartych zakładek ze zdjęciami:
   - 1. zakładka ze zdjęciem → `123-marka-model-1.jpg`
   - 2. zakładka ze zdjęciem → `123-marka-model-2.jpg`
   - 3. zakładka ze zdjęciem → `123-marka-model-3.jpg`
4. **Zmiana kolejności:** Przeciągnięcie zakładki w lewo lub w prawo natychmiast przelicza numerację.
5. **Obróbka i zapis:** Wchodzisz w zakładkę, przybliżasz detal, usuwasz tło (AI lub ręcznie gumką/wycinaniem), ustawiasz kadrowanie, białe tło i marginesy (ze wszystkich stron lub z pojedynczego boku). Klikasz **„Zapisz z zakładek”** – program eksportuje wyłącznie otwarte zakładki ze zdjęciami w ich dokładnej kolejności.

---

## 3. Funkcjonalności programu

| Moduł | Opis działania |
|---|---|
| **Podgląd GPU (Zoom / Pan)** | Płynne przybliżanie kółkiem myszy w punkt kursora i przesuwanie (`Spacja + LPM` lub środkowy przycisk) w 60–120 FPS. |
| **Kadrowanie i marginesy** | Interaktywna ramka kadrowania + 4 niezależne pola marginesów (Góra, Dół, Lewo, Prawo), opcja spięcia wszystkich boków kłódką oraz szybkie „Dopasuj do kwadratu 1:1”. |
| **Usuwanie tła (AI)** | Lokalny model ONNX (np. RMBG-1.4 / BiRefNet przez crate `ort`) działający w wątku w tle, generujący maskę kanału alfa. |
| **Ręczne odcinanie / Gumka** | Edycja maski (`mask_cpu` + częściowa aktualizacja tekstur GPU *dirty rect*). LPM usuwa fragment, PPM przywraca. Dodatkowo narzędzie wycinania prostokątem (`Delete`). |
| **Białe tło** | Wtapianie czystego białego tła (`#FFFFFF`) bezpośrednio w shaderze pod usunięte obszary i dodane marginesy. |
| **Kopiowanie ustawień** | Możliwość skopiowania parametrów `EditParams` (np. marginesy + białe tło) na wszystkie otwarte zdjęcia jednym kliknięciem. |
| **Eksport Web (Squoosh)** | Renderowanie *offscreen* w `wgpu` → brak metadanych EXIF → opcjonalny resize (`fast_image_resize`, Lanczos3) → kompresja `mozjpeg` (Progressive JPEG, jakość do 100%). |

---

## 4. Stos technologiczny (Rust)

- **GUI i układ okien:** `eframe`, `egui`, `egui_dock`, `egui_extras` (ikony SVG)
- **Grafika i shadery:** `wgpu` (WGSL)
- **Dekodowanie i wielowątkowość:** `image`, `rayon`, `crossbeam-channel`
- **Skalowanie obrazów:** `fast_image_resize` (SIMD + Lanczos3)
- **Kompresja JPG:** `mozjpeg` (natywny silnik ze Squoosh)
- **Lokalne AI:** `ort` (ONNX Runtime)

---

## 5. Plan rozwoju (Etapy)
1. **Etap 1 (Szkielet UI i zakładki):** Ciemny motyw, `egui_dock` z `FileList` i `Photo(DocId)`, Drag & Drop, generator nazw `slugify` zsynchronizowany z kolejnością zakładek.
2. **Etap 2 (Silnik `wgpu`):** Wczytywanie tekstur do VRAM, kamera 2D (Zoom/Pan), shader WGSL obsługujący kadrowanie, marginesy i białe tło.
3. **Etap 3 (Eksport pod internet):** Renderowanie *offscreen* z `wgpu`, integracja `fast_image_resize` i `mozjpeg` (Progressive, bez EXIF), wielowątkowy zapis z zakładek.
4. **Etap 4 (Edycja maski):** Narzędzia Gumka/Przywróć oraz wycinanie prostokątem z aktualizacją *dirty rect* i historią Undo/Redo.
5. **Etap 5 (Usuwanie tła AI):** Podpięcie `ort` + modelu `rmbg-1.4.onnx` w wątku roboczym z paskiem postępu.