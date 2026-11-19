# egui_dock 0.21.1 – łatka AsOne

Kopia `egui_dock` 0.21.1 z crates.io (licencja MIT, plik `LICENSE`), podpięta w głównym
`Cargo.toml` przez `[patch.crates-io]`.

Zmiana: pasek zakładek może być pod treścią liścia zamiast nad nią.

- `TabViewer::tab_bar_at_bottom(&self, tabs) -> bool` (domyślnie `false`) – decyzja dla każdego liścia;
- `show_leaf` układa liść od dołu (`Layout::bottom_up_justified`), więc pasek trafia na dół,
  a treść zakładki nad niego;
- zakładki mają wtedy zaokrąglone dolne rogi, linia oddzielająca i „łącznik” aktywnej zakładki
  są rysowane u góry paska, a ukryta krawędź ramki treści jest na dole.

Miejsca zmian oznaczone komentarzem `[AsOne]` oraz polem `DockArea::tab_bar_bottom`.
Przy aktualizacji egui_dock: przenieść te kilka zmian albo usunąć łatkę, jeśli upstream doda
tę opcję.
