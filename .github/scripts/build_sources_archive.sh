#!/usr/bin/env bash
# Quellarchiv „Quellen Dritter“ zu einem gebauten Klein.Buch-Image (Plan W12, ADR 0039).
#
# Inhalt von klein-buch-<Version>-quellen-dritter.tar.zst:
#   openjdk/   Quelltext des exakten Temurin-Builds (GitHub-Release adoptium/temurinNN-binaries,
#              Asset *jdk-sources*, SHA-256 geprüft), jlink-Befehl, `java -version` und
#              `release` der Laufzeit aus dem Image
#   debian/    Quellpakete aller im Image installierten Debian-Pakete (`dpkg-query` im Image,
#              `apt-get source --download-only` in einem Container aus dem Basis-Image mit
#              Digest, deb-src gegen snapshot.debian.org zum Zeitpunkt des Basis-Images und des
#              Builds sowie deb.debian.org; Rest über die snapshot-API, SHA-1 geprüft)
#   copyleft/  -sources.jar (Maven Central, SHA-1 geprüft) und weitere Quell-Archive laut
#              src-tauri/sidecar/copyleft-sources.txt
#   README.md, SHA256SUMS
#
# Aufruf aus der Wurzel des öffentlichen Repos (im Entwicklungsordner aus Buchhaltung/):
#   bash .github/scripts/build_sources_archive.sh --image klein-buch:ci --version 2026.11.0 \
#     --out "$RUNNER_TEMP/dist" [--commit <sha>] [--tag <tag>] [--source-url <url>]
#
# Läuft im Workflow (Docker nötig); braucht docker, curl, jq, zstd, sha1sum, sha256sum.
# Optional: GITHUB_TOKEN (nur lesend, für die GitHub-API), GITHUB_OUTPUT (setzt `archive=`).
set -euo pipefail

die() { printf 'Quellarchiv: %s\n' "$*" >&2; exit 1; }
log() { printf '== %s\n' "$*"; }

IMAGE=""
VERSION=""
OUT=""
COMMIT=${KLEINBUCH_BUILD_COMMIT:-${GITHUB_SHA:-}}
TAG=${KLEINBUCH_BUILD_TAG:-}
SOURCE_URL=${KLEINBUCH_SOURCE_URL:-}

while [ $# -gt 0 ]; do
  case "$1" in
    --image) IMAGE=${2:?}; shift 2 ;;
    --version) VERSION=${2:?}; shift 2 ;;
    --out) OUT=${2:?}; shift 2 ;;
    --commit) COMMIT=${2:?}; shift 2 ;;
    --tag) TAG=${2-}; shift 2 ;;
    --source-url) SOURCE_URL=${2:?}; shift 2 ;;
    *) die "unbekannte Option: $1" ;;
  esac
done
[ -n "$IMAGE" ] || die "--image fehlt"
[ -n "$OUT" ] || die "--out fehlt"
[[ "$VERSION" =~ ^[0-9A-Za-z][0-9A-Za-z.+-]*$ ]] || die "ungültige --version: '$VERSION'"

for tool in docker curl jq zstd sha1sum sha256sum; do
  command -v "$tool" >/dev/null || die "$tool fehlt"
done

# Wurzel des Produkts: öffentliches Repo (flach) oder Entwicklungsordner (klein-buch/).
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
if [ ! -f "$ROOT/src-tauri/sidecar/versions.json" ] && [ -f "$ROOT/klein-buch/src-tauri/sidecar/versions.json" ]; then
  ROOT="$ROOT/klein-buch"
fi
VERSIONS="$ROOT/src-tauri/sidecar/versions.json"
DOCKERFILE="$ROOT/Dockerfile"
COPYLEFT="$ROOT/src-tauri/sidecar/copyleft-sources.txt"
for f in "$VERSIONS" "$DOCKERFILE" "$COPYLEFT"; do
  [ -f "$f" ] || die "fehlt: $f"
done

docker image inspect "$IMAGE" >/dev/null 2>&1 || die "Image $IMAGE nicht vorhanden (vorher laden)"

NAME="klein-buch-${VERSION}-quellen-dritter"
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
TMP=$(mktemp -d "${RUNNER_TEMP:-/tmp}/quellen.XXXXXX")
trap 'rm -rf "$TMP" 2>/dev/null || true' EXIT
STAGE="$TMP/$NAME"
mkdir -p "$STAGE/openjdk" "$STAGE/debian/quellen" "$STAGE/copyleft"

CURL=(curl -fsSL --retry 5 --retry-delay 5 --connect-timeout 30)
GH_AUTH=()
if [ -n "${GITHUB_TOKEN:-}" ]; then GH_AUTH=(-H "Authorization: Bearer ${GITHUB_TOKEN}"); fi

# Erste Spalte einer Prüfsummen-Datei ("<hash>" oder "<hash>  <name>").
first_field() { tr -d '\r' < "$1" | awk 'NF { print $1; exit }'; }

# ---------------------------------------------------------------------------
# OpenJDK (Temurin)
# ---------------------------------------------------------------------------
JDK_VERSION=$(jq -r '.jdk.version // empty' "$VERSIONS")
[ -n "$JDK_VERSION" ] || die "jdk.version fehlt in versions.json"
JDK_MAJOR=${JDK_VERSION%%.*}
JDK_TAG="jdk-${JDK_VERSION}"
JDK_TAG_URL=${JDK_TAG//+/%2B}
JDK_REPO="adoptium/temurin${JDK_MAJOR}-binaries"
# Basis-Images stehen mit Digest direkt in den FROM-Zeilen der Stufen (W12).
from_image() { sed -n "s/^FROM \([^ ]*\) AS $1[[:space:]]*\$/\1/p" "$DOCKERFILE" | head -1; }
JDK_IMAGE=$(from_image jre)
log "OpenJDK ${JDK_VERSION} (${JDK_REPO})"

# Die Laufzeit im Image muss genau dieser Build sein, sonst passt der Quelltext nicht.
java_version=$(docker run --rm --entrypoint /opt/klein-buch/sidecar/jre/bin/java "$IMAGE" -version 2>&1) \
  || die "java -version im Image fehlgeschlagen: $java_version"
printf '%s\n' "$java_version" > "$STAGE/openjdk/java-version.txt"
grep -qF "$JDK_VERSION" "$STAGE/openjdk/java-version.txt" \
  || die "Laufzeit im Image ist nicht ${JDK_VERSION}: $java_version"
docker run --rm --entrypoint /bin/cat "$IMAGE" /opt/klein-buch/sidecar/jre/release \
  > "$STAGE/openjdk/jre-release.txt" 2>/dev/null || rm -f "$STAGE/openjdk/jre-release.txt"

asset_url=$("${CURL[@]}" ${GH_AUTH[@]+"${GH_AUTH[@]}"} \
  "https://api.github.com/repos/${JDK_REPO}/releases/tags/${JDK_TAG_URL}" \
  | jq -r '[.assets[] | select(.name | test("jdk-sources.*\\.tar\\.gz$"))][0].browser_download_url // empty') \
  || asset_url=""
if [ -z "$asset_url" ]; then
  asset_url="https://github.com/${JDK_REPO}/releases/download/${JDK_TAG_URL}/OpenJDK${JDK_MAJOR}U-jdk-sources_${JDK_VERSION//+/_}.tar.gz"
fi
jdk_src_name=$(basename "$asset_url")
"${CURL[@]}" -o "$STAGE/openjdk/$jdk_src_name" "$asset_url"
"${CURL[@]}" -o "$TMP/jdk-src.sha256" "${asset_url}.sha256.txt"
expected=$(first_field "$TMP/jdk-src.sha256")
echo "${expected}  $STAGE/openjdk/$jdk_src_name" | sha256sum -c - >/dev/null \
  || die "SHA-256 von $jdk_src_name stimmt nicht"

# jlink-Befehl aus der Stufe "jre" des Dockerfiles (Fortsetzungszeilen zusammengezogen),
# Modulliste aus versions.json eingesetzt.
MODULES=$(jq -r '.jlink_modules // empty' "$VERSIONS")
[ -n "$MODULES" ] || die "jlink_modules fehlt in versions.json"
awk '/^FROM .* AS jre[[:space:]]*$/ { p = 1; print; next } p && /^FROM / { exit } p' "$DOCKERFILE" \
  > "$STAGE/openjdk/dockerfile-stufe-jre.txt"
[ -s "$STAGE/openjdk/dockerfile-stufe-jre.txt" ] || die "Stufe \"jre\" im Dockerfile nicht gefunden"
jlink_cmd=$(sed -e ':a' -e '/\\$/N; s/\\\n//; ta' "$STAGE/openjdk/dockerfile-stufe-jre.txt" \
  | grep -o 'jlink --add-modules.*--output [^ ]*' | head -1 | tr -s ' \t' ' ')
[ -n "$jlink_cmd" ] || die "jlink-Aufruf im Dockerfile nicht gefunden"
jlink_cmd=${jlink_cmd//\"\$MODULES\"/$MODULES}
{
  echo "# jlink-Aufruf der Java-Laufzeit im Klein.Buch-Image (Dockerfile, Stufe \"jre\")."
  echo "# JDK: ${JDK_IMAGE:-unbekannt} (Eclipse Temurin ${JDK_VERSION})"
  echo "# Die Laufzeit liegt im Image unter /opt/klein-buch/sidecar/jre."
  echo "$jlink_cmd"
} > "$STAGE/openjdk/jlink-befehl.txt"

# ---------------------------------------------------------------------------
# Debian-Quellpakete
# ---------------------------------------------------------------------------
RUNTIME_IMAGE=$(from_image runtime-base)
case "$RUNTIME_IMAGE" in
  *@sha256:*) ;;
  *) die "Basis-Image ohne Digest im Dockerfile: '$RUNTIME_IMAGE'" ;;
esac
log "Debian-Quellpakete (Basis ${RUNTIME_IMAGE})"
docker pull -q "$RUNTIME_IMAGE" >/dev/null
base_created=$(docker image inspect -f '{{.Created}}' "$RUNTIME_IMAGE")
BASE_TS=$(date -u -d "$base_created" +%Y%m%dT%H%M%SZ)
BUILD_TS=${KLEINBUCH_SNAPSHOT_TS:-$(date -u +%Y%m%dT%H%M%SZ)}

# Installierte Pakete des fertigen Images (inkl. der im Dockerfile installierten).
{
  printf 'paket\tversion\tquellpaket\tquellversion\n'
  docker run --rm --entrypoint dpkg-query "$IMAGE" -W \
    -f '${db:Status-Abbrev}\t${binary:Package}\t${Version}\t${source:Package}\t${source:Version}\n' \
    | awk -F'\t' '$1 ~ /^ii/ { print $2 "\t" $3 "\t" $4 "\t" $5 }' | sort
} > "$STAGE/debian/pakete.tsv"
tail -n +2 "$STAGE/debian/pakete.tsv" | awk -F'\t' '{ print $3 "=" $4 }' | sort -u > "$TMP/quellen.txt"
[ -s "$TMP/quellen.txt" ] || die "keine Debian-Pakete im Image gefunden"
echo "$(wc -l < "$TMP/quellen.txt") Quellpakete"
cp "$TMP/quellen.txt" "$STAGE/debian/quellen-liste.txt"

# apt-get source im Basis-Image (als root im Wegwerf-Container, Ausgabe in den Stage-Ordner).
docker run --rm -i --user 0 --entrypoint /bin/bash \
  -e BASE_TS="$BASE_TS" -e BUILD_TS="$BUILD_TS" -e HOST_IDS="$(id -u):$(id -g)" \
  -v "$STAGE/debian/quellen:/out" -v "$TMP/quellen.txt:/quellen.txt:ro" \
  "$RUNTIME_IMAGE" -s <<'INNER'
set -euo pipefail
signed=$(sed -n 's/^Signed-By: *//p' /etc/apt/sources.list.d/debian.sources 2>/dev/null | head -1)
rm -f /etc/apt/sources.list.d/*.sources /etc/apt/sources.list
stanza() {
  printf 'Types: deb-src\nURIs: %s\nSuites: %s\nComponents: main\n' "$1" "$2"
  if [ -n "$signed" ]; then printf 'Signed-By: %s\n' "$signed"; fi
  printf '\n'
}
{
  stanza "http://deb.debian.org/debian" "trixie trixie-updates"
  stanza "http://deb.debian.org/debian-security" "trixie-security"
  for ts in "$BASE_TS" "$BUILD_TS"; do
    stanza "http://snapshot.debian.org/archive/debian/$ts" "trixie trixie-updates"
    stanza "http://snapshot.debian.org/archive/debian-security/$ts" "trixie-security"
  done
} > /etc/apt/sources.list.d/kb-quellen.sources
APT=(-o Acquire::Retries=5 -o Acquire::http::Timeout=60 -o Acquire::Check-Valid-Until=false -q)
# Fällt eine Quelle aus (snapshot.debian.org ist oft langsam), holt der Rest die API.
apt-get "${APT[@]}" update >/dev/null || echo "apt-get update mit Fehlern; fehlende Pakete über die snapshot-API" >&2
cd /out
: > /out/.fehlend
while IFS= read -r spec; do
  apt-get "${APT[@]}" source --download-only --only-source "$spec" >/dev/null 2>>/out/.apt-source.log \
    || echo "$spec" >> /out/.fehlend
done < /quellen.txt
chown -R "$HOST_IDS" /out
INNER

# Was apt nicht fand: über die snapshot-API (/mr/…/srcfiles) mit SHA-1-Prüfung.
while IFS= read -r spec; do
  [ -n "$spec" ] || continue
  src=${spec%%=*}
  ver=${spec#*=}
  log "snapshot-API: $src $ver"
  json=$("${CURL[@]}" "https://snapshot.debian.org/mr/package/${src}/${ver}/srcfiles?fileinfo=1") \
    || die "Quellpaket $src $ver weder per apt noch per snapshot-API gefunden"
  while IFS=$'\t' read -r hash fname; do
    [[ "$fname" =~ ^[A-Za-z0-9][A-Za-z0-9.+~_-]*$ ]] || die "unerwarteter Dateiname von snapshot: $fname"
    "${CURL[@]}" -o "$STAGE/debian/quellen/$fname" "https://snapshot.debian.org/file/${hash}"
    echo "${hash}  $STAGE/debian/quellen/$fname" | sha1sum -c - >/dev/null || die "SHA-1 von $fname stimmt nicht"
  done < <(echo "$json" | jq -r '.result[].hash as $h | [$h, .fileinfo[$h][0].name] | @tsv')
done < "$STAGE/debian/quellen/.fehlend"

# Jedes Quellpaket muss mit seiner .dsc vorliegen (ohne Epoche im Dateinamen).
while IFS= read -r spec; do
  src=${spec%%=*}
  ver=${spec#*=}
  ver=${ver#*:}
  [ -s "$STAGE/debian/quellen/${src}_${ver}.dsc" ] || die "Quellpaket fehlt: ${src}_${ver}.dsc"
done < "$TMP/quellen.txt"
mv "$STAGE/debian/quellen/.apt-source.log" "$TMP/" 2>/dev/null || true
rm -f "$STAGE/debian/quellen/.fehlend"

# ---------------------------------------------------------------------------
# Copyleft-Teile des Java-Sidecars u. a. (copyleft-sources.txt)
# ---------------------------------------------------------------------------
log "Copyleft-Quellen ($(basename "$COPYLEFT"))"
printf 'datei\tquelle\tsha256\n' > "$STAGE/copyleft/quellen.tsv"
n_copyleft=0
while read -r kind arg _rest; do
  case "$kind" in
    "" | "#"*) continue ;;
    maven)
      IFS=: read -r group artifact version extra <<< "$arg"
      [ -n "$group" ] && [ -n "$artifact" ] && [ -n "$version" ] && [ -z "${extra:-}" ] \
        || die "ungültige Maven-Koordinate: $arg"
      for part in "$group" "$artifact" "$version"; do
        [[ "$part" =~ ^[A-Za-z0-9._-]+$ ]] || die "ungültige Maven-Koordinate: $arg"
      done
      fname="${artifact}-${version}-sources.jar"
      url="https://repo1.maven.org/maven2/${group//.//}/${artifact}/${version}/${fname}"
      "${CURL[@]}" -o "$STAGE/copyleft/$fname" "$url"
      "${CURL[@]}" -o "$TMP/maven.sha1" "${url}.sha1"
      echo "$(first_field "$TMP/maven.sha1")  $STAGE/copyleft/$fname" | sha1sum -c - >/dev/null \
        || die "SHA-1 von $fname stimmt nicht"
      ;;
    url)
      url=$arg
      [[ "$url" =~ ^https://[A-Za-z0-9.-]+/[^[:space:]]+$ ]] || die "nur https-URLs erlaubt: $url"
      path=${url#https://*/}
      path=${path%%[?#]*}
      fname=$(printf '%s' "$path" | tr '/' '_' | tr -c 'A-Za-z0-9._-' '_')
      [ -n "$fname" ] || die "kein Dateiname aus $url"
      [ ! -e "$STAGE/copyleft/$fname" ] || die "doppelter Eintrag: $url"
      "${CURL[@]}" -o "$STAGE/copyleft/$fname" "$url"
      ;;
    *) die "unbekannte Zeile in copyleft-sources.txt: $kind $arg" ;;
  esac
  printf '%s\t%s\t%s\n' "$fname" "${url}" "$(sha256sum "$STAGE/copyleft/$fname" | cut -d' ' -f1)" \
    >> "$STAGE/copyleft/quellen.tsv"
  n_copyleft=$((n_copyleft + 1))
done < <(tr -d '\r' < "$COPYLEFT")
[ "$n_copyleft" -gt 0 ] || die "copyleft-sources.txt enthält keine Einträge"

# ---------------------------------------------------------------------------
# README, Prüfsummen, Archiv
# ---------------------------------------------------------------------------
jdk_source_url=$(jq -r '.jdk.source_url // empty' "$VERSIONS")
n_debian=$(wc -l < "$TMP/quellen.txt")
{
  echo "# Klein.Buch ${VERSION} – Quellen Dritter"
  echo
  echo "Dieses Archiv enthält den Quelltext der Komponenten Dritter, die im Docker-Image"
  echo "von Klein.Buch ${VERSION} mitgeliefert werden und deren Lizenzen die Weitergabe des"
  echo "Quelltexts verlangen (GPL/LGPL der Debian-Pakete, GPL-2.0 mit Classpath-Exception"
  echo "des OpenJDK, MPL-2.0/EPL-1.0/EUPL-1.2 von Teilen des Java-Sidecars)."
  echo
  echo "## Klein.Buch selbst (AGPL-3.0-or-later)"
  echo
  echo "- Quelltext: ${SOURCE_URL:-unbekannt}"
  echo "- Commit: ${COMMIT:-unbekannt}"
  if [ -n "$TAG" ]; then echo "- Tag: ${TAG}"; fi
  if [ -n "$SOURCE_URL" ] && [ -n "$COMMIT" ]; then echo "- Stand: ${SOURCE_URL}/tree/${COMMIT}"; fi
  echo
  echo "## Inhalt"
  echo
  echo "- \`openjdk/\`: Quelltext von Eclipse Temurin ${JDK_VERSION} (\`${jdk_src_name}\`, aus"
  echo "  dem GitHub-Release \`${JDK_TAG}\` von ${JDK_REPO}, SHA-256 geprüft; Quell-Tag"
  echo "  ${jdk_source_url:-–}). Die Laufzeit im Image entsteht per jlink aus"
  echo "  \`${JDK_IMAGE:-unbekannt}\`; der Aufruf steht in \`jlink-befehl.txt\`, die Stufe des"
  echo "  Dockerfiles in \`dockerfile-stufe-jre.txt\`, die Versionsausgabe der Laufzeit im Image"
  echo "  in \`java-version.txt\`."
  echo "- \`debian/\`: ${n_debian} Debian-Quellpakete (\`quellen/\`, je \`.dsc\` mit Archiven) zu"
  echo "  allen im Image installierten Paketen (\`pakete.tsv\`: Paket, Version, Quellpaket,"
  echo "  Quellversion). Basis-Image \`${RUNTIME_IMAGE}\` (erstellt ${base_created});"
  echo "  geladen mit \`apt-get source --download-only\` gegen deb.debian.org und"
  echo "  snapshot.debian.org (${BASE_TS} und ${BUILD_TS}), sonst über die snapshot-API."
  echo "  Entpacken z. B. mit \`dpkg-source -x <paket>_<version>.dsc\`."
  echo "- \`copyleft/\`: Quell-Archive (\`-sources.jar\` von Maven Central, Tag-Archive) der"
  echo "  Copyleft-Teile; Liste mit Herkunft und SHA-256 in \`quellen.tsv\`."
  echo "- \`SHA256SUMS\`: Prüfsummen aller Dateien (\`sha256sum -c SHA256SUMS\`)."
  echo
  echo "Lizenzen und Hinweise aller Komponenten zeigt die laufende Instanz unter \`/legal/\`;"
  echo "im Image liegen sie unter \`/usr/share/doc/klein-buch\` und \`/usr/share/doc/*/copyright\`."
} > "$STAGE/README.md"

(cd "$STAGE" && find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS)

ARCHIVE="$OUT/${NAME}.tar.zst"
log "Archiv $ARCHIVE"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime="@${SOURCE_DATE_EPOCH:-$(date +%s)}" \
  -C "$TMP" -cf - "$NAME" | zstd -q -T0 -6 -f -o "$ARCHIVE"
zstd -q -t "$ARCHIVE"
(cd "$OUT" && sha256sum "${NAME}.tar.zst" > "${NAME}.tar.zst.sha256")
ls -l "$ARCHIVE"
if [ -n "${GITHUB_OUTPUT:-}" ]; then echo "archive=$ARCHIVE" >> "$GITHUB_OUTPUT"; fi
