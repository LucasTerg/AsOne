#!/usr/bin/env bash
# Strona AsOne na GitHub Pages (gałąź gh-pages): repozytorium F-Droid (fdroid/repo), wersja
# przeglądarkowa (app/) i strona główna. Dokłada podane APK, przelicza i podpisuje indeks F-Droid.
# W repozytorium zostają tylko najnowsze wersje APK, a gh-pages to zawsze jeden commit bez historii –
# strona i repo nie rosną z każdym wydaniem (GitHub Pages: limit 1 GB).
#
#   KEYSTORE_B64=… KEYSTORE_PASSWORD=… [WEB_DIR=dist/web] packaging/fdroid/update-repo.sh [plik.apk…]
#
# KEYSTORE_B64 – magazyn kluczy PKCS12 (base64) do podpisu indeksu repozytorium,
# KEYSTORE_PASSWORD – jego hasło (to samo dla klucza). Konfiguracja z hasłem powstaje w katalogu
# tymczasowym i nigdy nie trafia na gh-pages. Zmienne opcjonalne:
#   WEB_DIR    zbudowana wersja web (trunk build) → app/; bez niej app/ zostaje bez zmian
#   REPO_URL   (domyślnie https://<domena z packaging/pages/CNAME>/fdroid/repo)
#   KEEP_APKS  ile najnowszych wersji APK zostaje w repozytorium F-Droid (domyślnie 3)
#   NO_PUSH=1  (bez publikacji – do testów; wynik zostaje w $SITE)
#   FORCE_CNAME=1  nowa domena bez sprawdzania DNS (gdy lokalny resolver pamięta jeszcze jej brak)
# Bez plików APK tylko odświeża indeks i stronę.
# Potrzebne: fdroidserver ≥ 2.4 (pip; starszy androguard nie czyta APK z AGP 9), JDK 21 (keytool, jarsigner – na Ubuntu 24.04 `openjdk-21-jdk-headless`,
# bo fdroid używa /usr/lib/jvm/default-java), git, qrencode (kod QR na stronie).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/packaging/fdroid"
: "${KEYSTORE_B64:?brak KEYSTORE_B64}" "${KEYSTORE_PASSWORD:?brak KEYSTORE_PASSWORD}"
if [ -n "${WEB_DIR:-}" ] && [ ! -f "$WEB_DIR/index.html" ]; then
  echo "WEB_DIR=$WEB_DIR: brak index.html" >&2
  exit 2
fi

WORK="$(mktemp -d)"
SITE="${SITE:-$(mktemp -d)/site}"
trap 'rm -rf "$WORK"' EXIT

# 1. Dotychczasowa zawartość gh-pages (poprzednie wersje APK – do KEEP_APKS najnowszych).
cd "$ROOT"
if git ls-remote --exit-code --heads origin gh-pages >/dev/null 2>&1; then
  git fetch -q origin gh-pages
  git worktree add -q --detach "$SITE" FETCH_HEAD
else
  git worktree add -q --detach "$SITE"
  (cd "$SITE" && git checkout -q --orphan gh-pages && git rm -rqf . && git clean -fdq)
fi

# Własna domena (packaging/pages/CNAME). Plik CNAME na gh-pages przekierowuje lucasterg.github.io/AsOne
# na tę domenę, więc nowa domena trafia tam dopiero, gdy istnieje w DNS – inaczej strona i repozytorium
# F-Droid przestałyby działać. Domena, która już jest na gh-pages, zostaje (bez względu na lokalny DNS).
DOMAIN="$(tr -d '[:space:]' < "$ROOT/packaging/pages/CNAME" 2>/dev/null || true)"
LIVE="$(tr -d '[:space:]' < "$SITE/CNAME" 2>/dev/null || true)"
if [ -n "$DOMAIN" ] && { [ "$DOMAIN" = "$LIVE" ] || [ "${FORCE_CNAME:-0}" = 1 ] || getent hosts "$DOMAIN" >/dev/null 2>&1; }; then
  PAGES_URL="https://$DOMAIN"
else
  [ -z "$DOMAIN" ] || echo "uwaga: $DOMAIN nie istnieje jeszcze w DNS – strona zostaje pod lucasterg.github.io/AsOne" >&2
  DOMAIN=""
  PAGES_URL="https://lucasterg.github.io/AsOne"
fi
REPO_URL="${REPO_URL:-$PAGES_URL/fdroid/repo}"

# 2. Katalog roboczy fdroid (z hasłem – poza gh-pages).
mkdir -p "$WORK/repo/icons" "$WORK/metadata"
if [ -d "$SITE/fdroid/repo" ]; then
  cp -R "$SITE/fdroid/repo/." "$WORK/repo/"
fi
cp "$HERE"/metadata/*.yml "$WORK/metadata/"
# Ikona aplikacji w katalogu F-Droid – fdroidserver 2.4 nie wyciąga jej z naszych APK.
mkdir -p "$WORK/metadata/io.github.lucasterg.asone/en-US"
cp "$ROOT/packaging/linux/icons/hicolor/512x512/apps/io.github.lucasterg.AsOne.png" \
  "$WORK/metadata/io.github.lucasterg.asone/en-US/icon.png"
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

# Tylko KEEP_APKS najnowszych wersji (po versionCode z nazw plików nadanych przez --rename-apks);
# starsze APK i ich ikony wypadają, indeks liczony ponownie.
KEEP_APKS="${KEEP_APKS:-3}"
mapfile -t PRUNED < <(find "$WORK/repo" -maxdepth 1 -name '*_*.apk' -printf '%f\n' \
  | sed -n 's/.*_\([0-9][0-9]*\)\.apk$/\1/p' | sort -un | head -n -"$KEEP_APKS")
if [ "${#PRUNED[@]}" -gt 0 ]; then
  for vc in "${PRUNED[@]}"; do
    find "$WORK/repo" \( -name "*_$vc.apk*" -o -name "*.$vc.png" \) -delete
  done
  echo "starsze wersje APK usunięte z repozytorium (versionCode): ${PRUNED[*]}"
  (cd "$WORK" && fdroid update --use-date-from-apk --pretty)
fi

# 4. Strona: fdroid/repo + instrukcja na stronie głównej.
rm -rf "$SITE/fdroid/repo"
mkdir -p "$SITE/fdroid"
cp -R "$WORK/repo" "$SITE/fdroid/repo"
HOST_PATH="${REPO_URL#https://}"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
sed -e "s|@REPO_URL@|$REPO_URL|g" -e "s|@HOST_PATH@|$HOST_PATH|g" -e "s|@FINGERPRINT@|$FINGERPRINT|g" -e "s|@VERSION@|$VERSION|g" \
  "$HERE/index.html.in" > "$SITE/index.html"
touch "$SITE/.nojekyll"
if [ -n "$DOMAIN" ]; then
  printf '%s\n' "$DOMAIN" > "$SITE/CNAME"
else
  rm -f "$SITE/CNAME"
fi

# Kod QR z adresem repozytorium i odciskiem klucza (skaner F-Droid albo aparat telefonu).
if command -v qrencode >/dev/null; then
  qrencode -t SVG -l M -m 2 -o "$SITE/qr.svg" "$REPO_URL?fingerprint=$FINGERPRINT"
else
  echo "uwaga: brak qrencode – strona bez kodu QR" >&2
fi

# 5. Wersja przeglądarkowa.
if [ -n "${WEB_DIR:-}" ]; then
  rm -rf "$SITE/app"
  cp -R "$WEB_DIR" "$SITE/app"
  rm -f "$SITE/app/.trunk-lock" "$SITE/app/_headers"
fi

echo "Repozytorium: $REPO_URL?fingerprint=$FINGERPRINT"
ls -la "$SITE/fdroid/repo" | grep -E '\.apk|index' || true

if [ "${NO_PUSH:-0}" = 1 ]; then
  echo "NO_PUSH=1 – wynik w $SITE"
  exit 0
fi
cd "$SITE"
git add -A
TREE="$(git write-tree)"
PREV="$(git rev-parse -q --verify HEAD || true)"
if [ -n "$PREV" ] && [ "$TREE" = "$(git rev-parse "$PREV^{tree}")" ]; then
  echo "bez zmian"
else
  VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)"
  MSG="Strona: AsOne $VERSION – F-Droid"
  [ -z "${WEB_DIR:-}" ] || MSG="$MSG, wersja web"
  [ -z "$DOMAIN" ] || MSG="$MSG ($DOMAIN)"
  # W GitHub Actions jako bot, lokalnie – bieżąca tożsamość git.
  GIT_ID=()
  if [ -n "${GITHUB_ACTIONS:-}" ]; then
    GIT_ID=(-c user.name="github-actions[bot]" -c user.email="41898282+github-actions[bot]@users.noreply.github.com")
  fi
  # Jeden commit bez rodzica zastępuje gh-pages: stare APK i wersje web nie zostają w historii.
  # --force-with-lease: tylko gdy gh-pages to nadal pobrany wyżej commit.
  NEW="$(git "${GIT_ID[@]}" commit-tree "$TREE" -m "$MSG")"
  if [ -n "$PREV" ]; then
    git push -q --force-with-lease="refs/heads/gh-pages:$PREV" origin "$NEW:refs/heads/gh-pages"
  else
    git push -q origin "$NEW:refs/heads/gh-pages"
  fi
  echo "opublikowano na gh-pages"
fi
cd "$ROOT"
git worktree remove --force "$SITE"
