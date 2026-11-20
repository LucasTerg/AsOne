#!/usr/bin/env bash
# Repozytorium F-Droid na GitHub Pages (gałąź gh-pages, katalog fdroid/repo):
# dokłada podane APK, przelicza i podpisuje indeks, publikuje stronę z instrukcją.
#
#   KEYSTORE_B64=… KEYSTORE_PASSWORD=… packaging/fdroid/update-repo.sh plik.apk [plik2.apk…]
#
# KEYSTORE_B64 – magazyn kluczy PKCS12 (base64) do podpisu indeksu repozytorium,
# KEYSTORE_PASSWORD – jego hasło (to samo dla klucza). Konfiguracja z hasłem powstaje w katalogu
# tymczasowym i nigdy nie trafia na gh-pages. Zmienne opcjonalne:
#   REPO_URL   (domyślnie https://lucasterg.github.io/AsOne/fdroid/repo)
#   NO_PUSH=1  (bez publikacji – do testów; wynik zostaje w $SITE)
# Potrzebne: fdroidserver, JDK 21 (keytool, jarsigner – na Ubuntu 24.04 `openjdk-21-jdk-headless`,
# bo fdroid używa /usr/lib/jvm/default-java), git, qrencode (kod QR na stronie).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/packaging/fdroid"
REPO_URL="${REPO_URL:-https://lucasterg.github.io/AsOne/fdroid/repo}"
: "${KEYSTORE_B64:?brak KEYSTORE_B64}" "${KEYSTORE_PASSWORD:?brak KEYSTORE_PASSWORD}"
[ "$#" -ge 1 ] || { echo "podaj co najmniej jeden plik APK" >&2; exit 2; }

WORK="$(mktemp -d)"
SITE="${SITE:-$(mktemp -d)/site}"
trap 'rm -rf "$WORK"' EXIT

# 1. Dotychczasowa zawartość gh-pages (stare wersje APK zostają w repozytorium).
cd "$ROOT"
if git ls-remote --exit-code --heads origin gh-pages >/dev/null 2>&1; then
  git fetch -q origin gh-pages
  git worktree add -q --detach "$SITE" FETCH_HEAD
else
  git worktree add -q --detach "$SITE"
  (cd "$SITE" && git checkout -q --orphan gh-pages && git rm -rqf . && git clean -fdq)
fi

# 2. Katalog roboczy fdroid (z hasłem – poza gh-pages).
mkdir -p "$WORK/repo/icons" "$WORK/metadata"
if [ -d "$SITE/fdroid/repo" ]; then
  cp -R "$SITE/fdroid/repo/." "$WORK/repo/"
fi
cp "$HERE"/metadata/*.yml "$WORK/metadata/"
# repo_icon – ścieżka względem katalogu roboczego; fdroid kopiuje ją do repo/icons/.
cp "$ROOT/packaging/linux/icons/hicolor/256x256/apps/io.github.lucasterg.AsOne.png" "$WORK/icon.png"
umask 077
printf '%s' "$KEYSTORE_B64" | base64 -d > "$WORK/keystore.p12"
ALIAS="$(keytool -list -keystore "$WORK/keystore.p12" -storetype PKCS12 -storepass "$KEYSTORE_PASSWORD" 2>/dev/null \
  | awk -F, '/PrivateKeyEntry/ { print $1; exit }')"
[ -n "$ALIAS" ] || { echo "nie znaleziono klucza w magazynie" >&2; exit 1; }
FINGERPRINT="$(keytool -list -v -keystore "$WORK/keystore.p12" -storetype PKCS12 -storepass "$KEYSTORE_PASSWORD" -alias "$ALIAS" 2>/dev/null \
  | awk '/SHA256:/ { print $2; exit }' | tr -d ':' | tr 'a-f' 'A-F')"
cat > "$WORK/config.yml" <<EOF
repo_url: $REPO_URL
repo_name: AsOne
repo_icon: icon.png
repo_description: >-
  Wersje AsOne na Androida budowane przez GitHub Actions z kodu
  https://github.com/LucasTerg/AsOne – edytor zdjęć produktowych.
archive_older: 0
keystore: keystore.p12
keystorepass: "$KEYSTORE_PASSWORD"
keypass: "$KEYSTORE_PASSWORD"
repo_keyalias: "$ALIAS"
EOF
chmod 600 "$WORK/config.yml"
umask 022
for apk in "$@"; do
  cp "$apk" "$WORK/repo/"
done

# 3. Indeks (wersje APK, ikony, podpis).
(cd "$WORK" && fdroid update --rename-apks --use-date-from-apk --pretty)

# 4. Strona: fdroid/repo + instrukcja na stronie głównej.
rm -rf "$SITE/fdroid/repo"
mkdir -p "$SITE/fdroid"
cp -R "$WORK/repo" "$SITE/fdroid/repo"
HOST_PATH="${REPO_URL#https://}"
sed -e "s|@REPO_URL@|$REPO_URL|g" -e "s|@HOST_PATH@|$HOST_PATH|g" -e "s|@FINGERPRINT@|$FINGERPRINT|g" \
  "$HERE/index.html.in" > "$SITE/index.html"
touch "$SITE/.nojekyll"
# Kod QR z adresem repozytorium i odciskiem klucza (skaner F-Droid albo aparat telefonu).
if command -v qrencode >/dev/null; then
  qrencode -t SVG -l M -m 2 -o "$SITE/qr.svg" "$REPO_URL?fingerprint=$FINGERPRINT"
else
  echo "uwaga: brak qrencode – strona bez kodu QR" >&2
fi
echo "Repozytorium: $REPO_URL?fingerprint=$FINGERPRINT"
ls -la "$SITE/fdroid/repo" | grep -E '\.apk|index' || true

if [ "${NO_PUSH:-0}" = 1 ]; then
  echo "NO_PUSH=1 – wynik w $SITE"
  exit 0
fi
cd "$SITE"
git add -A
if git diff --cached --quiet; then
  echo "bez zmian"
else
  git -c user.name="github-actions[bot]" -c user.email="41898282+github-actions[bot]@users.noreply.github.com" \
    commit -q -m "F-Droid: $(cd "$WORK/repo" && ls -1 ./*.apk | sed 's|^\./||' | tr '\n' ' ')"
  git push -q origin HEAD:gh-pages
  echo "opublikowano na gh-pages"
fi
cd "$ROOT"
git worktree remove --force "$SITE"
