#!/usr/bin/env bash
# APK AsOne lokalnie, w kontenerach Podman (jak w word-apk): cargo ndk → android/app/src/main/jniLibs,
# potem Gradle → dist/AsOne-<wersja>-android.apk.
#
#   packaging/android/build-local.sh
#
# Domyślne ścieżki odpowiadają układowi na SteamOS; każdą można nadpisać zmienną:
#   ANDROID_SDK (~/Android/Sdk), NDK ($ANDROID_SDK/ndk/27.2.12479018),
#   JBR (Java z Android Studio z Flatpaka), BUILDER_IMAGE (localhost/wordcraft-builder-image:latest),
#   RUSTUP_DIR (~/.cache/wordcraft-rustup – Rust z celami Androida), CARGO_BIN (~/.cache/wordcraft-cargo-bin – cargo-ndk),
#   TARGET_DIR (~/.cache/asone-android-target), SIGNING_DIR (~/.local/share/asone-signing).
# Gdy w SIGNING_DIR jest android-release.p12 (+ ANDROID_KEYSTORE_PASSWORD.txt), APK ma ten sam podpis co
# wydania z GitHub Actions i F-Droid (instaluje się jako aktualizacja); bez niego – klucz debug.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SDK="${ANDROID_SDK:-$HOME/Android/Sdk}"
NDK="${NDK:-$SDK/ndk/27.2.12479018}"
JBR="${JBR:-/var/lib/flatpak/app/com.google.AndroidStudio/current/active/files/extra/jbr}"
IMAGE="${BUILDER_IMAGE:-localhost/wordcraft-builder-image:latest}"
RUSTUP_DIR="${RUSTUP_DIR:-$HOME/.cache/wordcraft-rustup}"
CARGO_BIN="${CARGO_BIN:-$HOME/.cache/wordcraft-cargo-bin}"
TARGET_DIR="${TARGET_DIR:-$HOME/.cache/asone-android-target}"
SIGNING_DIR="${SIGNING_DIR:-$HOME/.local/share/asone-signing}"
mkdir -p "$TARGET_DIR"

echo "==> cargo ndk (arm64-v8a, x86_64)"
podman run --rm \
  -v "$ROOT:/work" -v "$TARGET_DIR:/target" \
  -v "$HOME/.cache/cargo/registry:/usr/local/cargo/registry" -v "$HOME/.cache/cargo/git:/usr/local/cargo/git" \
  -v "$RUSTUP_DIR:/rustup:ro" -v "$CARGO_BIN:/cargo-bin:ro" -v "$NDK:/ndk:ro" \
  -w /work -e CARGO_TARGET_DIR=/target -e RUSTUP_HOME=/rustup -e ANDROID_NDK_HOME=/ndk \
  -e RUSTFLAGS='-C link-arg=-Wl,-z,max-page-size=16384' -e CARGO_PROFILE_RELEASE_STRIP=symbols \
  "$IMAGE" bash -c 'set -euo pipefail; export PATH="/cargo-bin/bin:$PATH"
    cargo ndk -t arm64-v8a -t x86_64 -o android/app/src/main/jniLibs build --release -p asone --lib'

echo "==> Gradle assembleRelease"
SIGN=()
if [ -f "$SIGNING_DIR/android-release.p12" ]; then
  SIGN=(-v "$SIGNING_DIR:/signing:ro" -e ASONE_KEYSTORE=/signing/android-release.p12)
else
  echo "    (brak $SIGNING_DIR/android-release.p12 – podpis kluczem debug)"
fi
podman run --rm \
  -v "$ROOT:/work" -v "$SDK:/android-sdk" -v "$HOME/.gradle:/root/.gradle" -v "$HOME/.android:/root/.android" \
  -v "$JBR:/jbr:ro" "${SIGN[@]}" \
  -w /work/android -e JAVA_HOME=/jbr -e ANDROID_HOME=/android-sdk -e ANDROID_SDK_ROOT=/android-sdk \
  "$IMAGE" bash -c 'set -euo pipefail
    if [ -n "${ASONE_KEYSTORE:-}" ]; then export ASONE_KEYSTORE_PASSWORD="$(cat /signing/ANDROID_KEYSTORE_PASSWORD.txt)"; fi
    ./gradlew assembleRelease --no-daemon --console=plain -q'

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
mkdir -p "$ROOT/dist"
OUT="$ROOT/dist/AsOne-$VERSION-android.apk"
cp "$ROOT/android/app/build/outputs/apk/release/app-release.apk" "$OUT"
echo "zapisano $OUT"
echo "instalacja przez USB: adb install -r $OUT"
