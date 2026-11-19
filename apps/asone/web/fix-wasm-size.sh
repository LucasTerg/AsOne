#!/bin/sh
# Trunk wpisuje do inicjalizatora rozmiar .wasm sprzed wasm-opt; karta startowa liczy postęp
# w bajtach po dekompresji, więc podmieniamy go na rzeczywisty rozmiar pliku wynikowego.
set -eu
dir="${TRUNK_STAGING_DIR:?brak TRUNK_STAGING_DIR}"
wasm=$(ls "$dir"/*_bg.wasm | head -n 1)
size=$(wc -c < "$wasm" | tr -d ' ')
sed -i -E "s/(__trunkInitializer\(init, '[^']*', )[0-9]+/\1${size}/" "$dir/index.html"
echo "rozmiar wasm w karcie startowej: ${size} B"
