#!/usr/bin/env bash
# Pakiety AsOne dla Linuksa z jednych binarek release:
#
#   dist/AsOne-<wersja>-<arch>.AppImage   każda dystrybucja z glibc ≥ 2.39 (jak system budujący)
#   dist/AsOne-<wersja>-<arch>.flatpak    pojedynczy plik Flatpak (runtime org.freedesktop.Platform 26.08)
#
# Użycie (z dowolnego katalogu):  packaging/linux/package.sh [appimage|flatpak|all] [--no-build] [--install]
#   --no-build   użyj istniejących target/release/asone{,-cli}
#   --install    po zbudowaniu zainstaluj Flatpak dla użytkownika i sprawdź `asone-cli --version`
#
# Potrzebne: cargo (np. wrapper anything-rust), curl; dla Flatpaka – samo polecenie `flatpak`
# (flatpak-builder i SDK NIE są potrzebne: binarki są gotowe, pakowane przez build-init/build-finish).
# MozJPEG z SIMD: FEATURES="--features nasm" (wymaga nasm w środowisku budowania).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/packaging/linux"
APP_ID=io.github.lucasterg.AsOne
RUNTIME_VERSION=26.08
APPIMAGETOOL_VERSION=1.9.1
FLATHUB=https://dl.flathub.org/repo/flathub.flatpakrepo

WHAT=all
BUILD=1
INSTALL=0
for arg in "$@"; do
  case "$arg" in
    appimage | flatpak | all) WHAT="$arg" ;;
    --no-build) BUILD=0 ;;
    --install) INSTALL=1 ;;
    -h | --help) sed -n '2,13p' "$0"; exit 0 ;;
    *) echo "nieznany argument: $arg" >&2; exit 2 ;;
  esac
done

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
ARCH="$(uname -m)"
DATE="$(date -u +%Y-%m-%d)"
DIST="$ROOT/dist"
WORK="$ROOT/target/linux-package"
mkdir -p "$DIST"

if [[ "$BUILD" = 1 ]]; then
  echo "==> cargo build --release (asone, asone-cli)"
  (cd "$ROOT" && cargo build --release -p asone -p asone-cli ${FEATURES:-})
fi
for bin in asone asone-cli; do
  [[ -x "$ROOT/target/release/$bin" ]] || { echo "błąd: brak target/release/$bin" >&2; exit 1; }
done

# ---- Wspólny katalog (układ /usr) ---------------------------------------------------------------
STAGE="$WORK/stage"
rm -rf "$STAGE"
mkdir -p "$STAGE/bin" "$STAGE/share/applications" "$STAGE/share/metainfo" "$STAGE/share/doc/asone"
install -m755 "$ROOT/target/release/asone" "$ROOT/target/release/asone-cli" "$STAGE/bin/"
install -m644 "$HERE/$APP_ID.desktop" "$STAGE/share/applications/"
sed -e "s/@VERSION@/$VERSION/" -e "s/@DATE@/$DATE/" "$HERE/$APP_ID.metainfo.xml.in" > "$STAGE/share/metainfo/$APP_ID.metainfo.xml"
mkdir -p "$STAGE/share/icons"
cp -R "$HERE/icons/hicolor" "$STAGE/share/icons/"
install -m644 "$ROOT/README.md" "$STAGE/share/doc/asone/"

# ---- AppImage ------------------------------------------------------------------------------------
build_appimage() {
  echo "==> AppImage $VERSION ($ARCH)"
  local appdir="$WORK/AsOne.AppDir"
  rm -rf "$appdir"
  mkdir -p "$appdir/usr"
  cp -R "$STAGE/." "$appdir/usr/"
  install -m755 "$HERE/AppRun" "$appdir/AppRun"
  cp "$HERE/$APP_ID.desktop" "$appdir/"
  cp "$HERE/icons/hicolor/256x256/apps/$APP_ID.png" "$appdir/$APP_ID.png"
  ln -sf "$APP_ID.png" "$appdir/.DirIcon"

  local tool="$WORK/appimagetool-$APPIMAGETOOL_VERSION-$ARCH.AppImage"
  if [[ ! -x "$tool" ]]; then
    curl -fsSL -o "$tool.part" "https://github.com/AppImage/appimagetool/releases/download/$APPIMAGETOOL_VERSION/appimagetool-$ARCH.AppImage"
    chmod +x "$tool.part" && mv "$tool.part" "$tool"
  fi
  local out="$DIST/AsOne-$VERSION-$ARCH.AppImage"
  # Bez FUSE (kontenery, SteamOS): appimagetool rozpakowuje się sam.
  # Nowy plik obok + podmiana nazwy: działająca kopia AppImage czyta ze starego pliku (FUSE)
  # i wysypałaby się, gdyby nadpisać go w miejscu.
  APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "$tool" --no-appstream "$appdir" "$out.new" >/dev/null
  mv -f "$out.new" "$out"
  echo "    zapisano $out"
}

# ---- Flatpak (bez flatpak-buildera) --------------------------------------------------------------
build_flatpak() {
  echo "==> Flatpak $VERSION ($ARCH), runtime org.freedesktop.Platform $RUNTIME_VERSION"
  local app="$WORK/flatpak-app" repo="$WORK/flatpak-repo"
  rm -rf "$app" "$repo"
  flatpak build-init "$app" "$APP_ID" org.freedesktop.Sdk org.freedesktop.Platform "$RUNTIME_VERSION" </dev/null
  cp -R "$STAGE/." "$app/files/"
  flatpak build-finish "$app" \
    --command=asone \
    --socket=wayland --socket=fallback-x11 --share=ipc \
    --device=dri \
    --share=network \
    --filesystem=xdg-pictures --filesystem=xdg-download --filesystem=xdg-documents --filesystem=xdg-desktop \
    </dev/null
  flatpak build-export "$repo" "$app" stable </dev/null >/dev/null
  local out="$DIST/AsOne-$VERSION-$ARCH.flatpak"
  flatpak build-bundle --runtime-repo="$FLATHUB" "$repo" "$out.new" "$APP_ID" stable </dev/null
  mv -f "$out.new" "$out"
  echo "    zapisano $out"
  if [[ "$INSTALL" = 1 ]]; then
    flatpak install --user -y --noninteractive --reinstall "$out"
    flatpak run --command=asone-cli "$APP_ID" --version
  fi
}

case "$WHAT" in
  appimage) build_appimage ;;
  flatpak) build_flatpak ;;
  all) build_appimage; build_flatpak ;;
esac
ls -lh "$DIST"/AsOne-"$VERSION"-* 2>/dev/null || true
