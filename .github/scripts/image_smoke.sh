#!/usr/bin/env bash
# Smoke-Test eines gebauten Klein.Buch-Images (Plan W11/W12), gemeinsam für den
# Trockenlauf und den Release-Lauf in .github/workflows/release-image.yml.
# Dieselben Schritte wie im Image-Workflow (image.yml), dazu die Lizenz- und
# Quell-Angaben aus W12 (/usr/share/doc/klein-buch, /legal/, /api/public/info).
#
# Aufruf aus der Wurzel des öffentlichen Repos:
#   EXPECT_COMMIT=<sha> bash .github/scripts/image_smoke.sh klein-buch:ci
#
# Braucht: docker, curl, jq, python3, sudo (Bind-Mounts für die fremde UID).
set -euo pipefail

IMAGE=${1:?Aufruf: image_smoke.sh <image>}
EXPECT_COMMIT=${EXPECT_COMMIT:-}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
NAME=kb-smoke
PORT=${SMOKE_PORT:-8080}
URL="http://localhost:${PORT}"
WORK=$(mktemp -d "${RUNNER_TEMP:-/tmp}/kb-smoke.XXXXXX")
DOC=/usr/share/doc/klein-buch
JRE=/opt/klein-buch/sidecar/jre

step() { printf '\n== %s\n' "$*"; }
fail() { printf 'SMOKE FEHLER: %s\n' "$*" >&2; exit 1; }

cleanup() {
  rc=$?
  if docker container inspect "$NAME" >/dev/null 2>&1; then
    step "Container-Log"
    docker logs "$NAME" 2>&1 | grep -v '^Setup-Token' | tail -200 || true
    docker rm -f "$NAME" >/dev/null 2>&1 || true
  fi
  sudo rm -rf "$WORK" || true
  exit "$rc"
}
trap cleanup EXIT

step "Image-Größe < 400 MB"
size=$(docker image inspect -f '{{.Size}}' "$IMAGE")
echo "Image: $((size / 1024 / 1024)) MiB"
test "$size" -lt $((400 * 1024 * 1024)) || fail "Image zu groß"

step "Mock-Schalter im Release-Build verweigert den Start"
set +e
out=$(docker run --rm -e KLEIN_BUCH_SIDECAR_MOCK=1 -e KLEIN_BUCH_PUBLIC_URL="$URL" \
  -e KLEIN_BUCH_ALLOW_UNMOUNTED=1 "$IMAGE" 2>&1)
rc=$?
set -e
echo "$out"
test "$rc" -ne 0 || fail "Start mit KLEIN_BUCH_SIDECAR_MOCK=1 wurde nicht verweigert"
echo "$out" | grep -q "KLEIN_BUCH_SIDECAR_MOCK" || fail "Meldung nennt KLEIN_BUCH_SIDECAR_MOCK nicht"

step "Lizenz- und Quell-Dateien im Image"
docker run --rm --entrypoint /bin/sh -e DOC="$DOC" -e JRE="$JRE" "$IMAGE" -c '
  set -eu
  for f in "$DOC/LICENSE" "$DOC/SOURCES.md" "$DOC/debian-pakete.tsv"; do
    test -s "$f" || { echo "fehlt oder leer: $f"; exit 1; }
  done
  n=0
  for f in /usr/share/doc/*/copyright; do
    test -s "$f" || { echo "leer: $f"; exit 1; }
    n=$((n + 1))
  done
  test "$n" -gt 0 || { echo "keine /usr/share/doc/*/copyright"; exit 1; }
  test -n "$(ls -A "$JRE/legal" 2>/dev/null)" || { echo "$JRE/legal leer"; exit 1; }
  if grep -rqs "Dev-Build-Platzhalter" /opt/klein-buch/legal "$DOC"; then
    echo "Dev-Build-Platzhalter im Image"; exit 1
  fi
  echo "copyright-Dateien: $n"
' || fail "Lizenz-/Quell-Dateien unvollständig"

step "Container starten (gehärtet, fremde UID)"
mkdir -p "$WORK/data" "$WORK/inputs"
sudo chown -R 568:568 "$WORK/data" "$WORK/inputs"
docker run -d --name "$NAME" --restart unless-stopped \
  --read-only --tmpfs /tmp:size=512m,mode=1777,noexec,nosuid,nodev \
  --cap-drop ALL --security-opt no-new-privileges --user 568:568 \
  -v "$WORK/data:/data" -v "$WORK/inputs:/inputs" \
  -e KLEIN_BUCH_PUBLIC_URL="$URL" -e KLEIN_BUCH_INSECURE_HTTP=1 \
  -p "127.0.0.1:${PORT}:8080" "$IMAGE" >/dev/null
for _ in $(seq 1 60); do
  if docker exec "$NAME" /usr/local/bin/klein-buch-server healthcheck; then break; fi
  sleep 2
done
docker exec "$NAME" /usr/local/bin/klein-buch-server healthcheck || fail "Healthcheck"

step "Laufzeit-Eigenschaften"
test "$(docker exec "$NAME" id -u)" = "568" || fail "UID"
tz=$(docker exec "$NAME" date +%Z); echo "Zeitzone im Container: $tz"
case "$tz" in CET|CEST) ;; *) fail "Zeitzone $tz" ;; esac
if docker exec "$NAME" sh -c 'touch /schreibtest' 2>/dev/null; then fail "/ ist beschreibbar"; fi
docker exec "$NAME" grep -E '^CapEff:\s+0+$' /proc/1/status || fail "CapEff"
docker exec "$NAME" test -f /inputs/.seed-manifest.json || fail "Seed-Manifest"
docker exec "$NAME" test -f /inputs/pdf-templates/default.typ || fail "Seed default.typ"

step "Lizenzseite und Quell-Angaben ohne Sitzung (/legal/, /api/public/info)"
# Erst in eine Datei laden: `curl | grep -q` kann unter pipefail mit Exit 23 scheitern.
curl -fsS -o "$WORK/license.txt" "$URL/legal/LICENSE.txt" \
  && grep -q "GNU AFFERO GENERAL PUBLIC LICENSE" "$WORK/license.txt" || fail "/legal/LICENSE.txt"
curl -fsS -o "$WORK/tpl.html" "$URL/legal/third-party-licenses.html" || fail "/legal/third-party-licenses.html"
for marker in "SIL OPEN FONT LICENSE" "GUST Font License" "Zetetic" "BY-SA 3.0"; do
  grep -qi -- "$marker" "$WORK/tpl.html" || fail "Lizenzseite ohne „$marker“"
done
# Genau die JS-Pakete im SPA-Bundle (build/legal/js-licenses.json; seit Kit 3 mit
# devalue, ohne esm-env). Der Frontend-Job in ci.yml vergleicht diese Zeile mit der
# Liste aus dem Build (Lauf F7a).
for js in svelte @sveltejs/kit devalue clsx marked minisearch; do
  grep -q -- "$js" "$WORK/tpl.html" || fail "Lizenzseite ohne JS-Paket $js"
done
if grep -q "Dev-Build-Platzhalter" "$WORK/tpl.html"; then fail "Lizenzseite ist der Dev-Build-Platzhalter"; fi
curl -fsS -o /dev/null "$URL/legal/quellen.html" || fail "/legal/quellen.html"
curl -fsS -o "$WORK/debian-pakete.tsv" "$URL/legal/debian-pakete.tsv" \
  && test -s "$WORK/debian-pakete.tsv" || fail "/legal/debian-pakete.tsv"
info=$(curl -fsS "$URL/api/public/info") || fail "/api/public/info"
echo "$info"
echo "$info" | jq -e '.version and .commit and .sourceUrl and .licenseSpdx' >/dev/null \
  || fail "/api/public/info unvollständig"
if [ -n "$EXPECT_COMMIT" ]; then
  test "$(echo "$info" | jq -r .commit)" = "$EXPECT_COMMIT" || fail "Commit in /api/public/info ≠ $EXPECT_COMMIT"
fi

step "SPA vorkomprimiert nach Accept-Encoding, API nie (ADR 0041 Nr. 212)"
curl -fsS -H 'Accept-Encoding: br' -D "$WORK/spa-br.h" -o /dev/null "$URL/" || fail "/ mit br"
grep -qi '^content-encoding: br' "$WORK/spa-br.h" || fail "/ ohne Content-Encoding: br"
grep -qi '^vary: .*accept-encoding' "$WORK/spa-br.h" || fail "/ ohne Vary: Accept-Encoding"
curl -fsS -H 'Accept-Encoding: gzip' -D "$WORK/spa-gz.h" -o "$WORK/index.html.gz" "$URL/invoices" \
  || fail "/invoices mit gzip"
grep -qi '^content-encoding: gzip' "$WORK/spa-gz.h" || fail "/invoices ohne Content-Encoding: gzip"
gzip -dc "$WORK/index.html.gz" > "$WORK/index.html" && grep -q '<html' "$WORK/index.html" \
  || fail "index.html.gz nicht lesbar"
curl -fsS -H 'Accept-Encoding: gzip, br' -D "$WORK/api.h" -o /dev/null "$URL/api/public/info" \
  || fail "/api/public/info mit gzip, br"
if grep -qi '^content-encoding' "$WORK/api.h"; then fail "API-Antwort komprimiert"; fi

step "Smoke über HTTP (Einrichtung, Festschreiben, Rechnungseingang)"
token=$(docker logs "$NAME" 2>&1 | sed -n 's/^Setup-Token: \([A-Z0-9]*\).*/\1/p' | head -1 || true)
test -n "$token" || fail "Setup-Token nicht im Log"
SMOKE_URL="$URL" python3 "$HERE/image_smoke.py" "$token"

step "Smoke grün"
