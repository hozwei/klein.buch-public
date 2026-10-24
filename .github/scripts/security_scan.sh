#!/usr/bin/env bash
# Sicherheits-Scan eines gebauten Klein.Buch-Images (Plan W14), aufgerufen aus
# .github/workflows/security.yml:
#   bash .github/scripts/security_scan.sh klein-buch:scan
#
# 1. Container mit der Härtung aus compose.example.yaml starten, Härtung per
#    `docker inspect`/`docker exec` prüfen (UID ≠ 0, Root-FS read-only, CapEff 0).
# 2. Einrichtung + Testdaten + Abnahme-Ablauf über image_smoke.py (Sitzungs-Cookie
#    für ZAP in eine Datei).
# 3. OWASP ZAP: Baseline (Spider + passiv) und API-Scan gegen alle Commands aus der
#    Registry (OpenAPI aus openapi_from_registry.py), beide mit Sitzung.
# 4. Gate: zap_gate.py (Medium/High nur mit Begründung in .github/zap-ausnahmen.tsv),
#    Server danach erreichbar, keine Geheimnisse im Container-Log.
set -euo pipefail

IMAGE=${1:?Aufruf: security_scan.sh <image>}
HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
ZAP_IMAGE="ghcr.io/zaproxy/zaproxy:2.17.0@sha256:5ccfff9a411743ab2615136a2edd7416bec9ce41572fa94e935df38c14adb2b6"
NAME=kb-scan
URL="http://localhost:8080"
WORK=$(mktemp -d "${RUNNER_TEMP:-/tmp}/kb-scan.XXXXXX")
ZAPDIR="$WORK/zap"
PASS_MARKER="smoke-pferd-batterie-klammer-2026"   # Passphrase aus image_smoke.py

step() { printf '\n== %s\n' "$*"; }
fail() { printf 'SCAN FEHLER: %s\n' "$*" >&2; exit 1; }

cleanup() {
  rc=$?
  if docker container inspect "$NAME" >/dev/null 2>&1; then
    step "Container-Log (gekürzt)"
    docker logs "$NAME" 2>&1 | grep -v '^Setup-Token' | tail -150 || true
    docker rm -f "$NAME" >/dev/null 2>&1 || true
  fi
  sudo rm -rf "$WORK/data" "$WORK/inputs" || true
  exit "$rc"
}
trap cleanup EXIT

step "Container starten (Härtung wie compose.example.yaml)"
mkdir -p "$WORK/data" "$WORK/inputs" "$ZAPDIR"
sudo chown -R 568:568 "$WORK/data" "$WORK/inputs"
chmod 0777 "$ZAPDIR"
# Berichte auch bei Fehlschlag als Artefakt (Workflow-Schritt „ZAP-Berichte“).
if [ -n "${GITHUB_ENV:-}" ]; then echo "ZAP_REPORTS=$ZAPDIR" >> "$GITHUB_ENV"; fi
docker run -d --name "$NAME" --restart unless-stopped \
  --read-only --tmpfs /tmp:size=512m,mode=1777,noexec,nosuid,nodev \
  --cap-drop ALL --security-opt no-new-privileges --user 568:568 \
  --pids-limit 256 --memory 2g --ulimit core=0 \
  -v "$WORK/data:/data" -v "$WORK/inputs:/inputs" \
  -e KLEIN_BUCH_PUBLIC_URL="$URL" -e KLEIN_BUCH_INSECURE_HTTP=1 \
  -p 127.0.0.1:8080:8080 "$IMAGE" >/dev/null
for _ in $(seq 1 60); do
  if docker exec "$NAME" /usr/local/bin/klein-buch-server healthcheck >/dev/null 2>&1; then break; fi
  sleep 2
done
docker exec "$NAME" /usr/local/bin/klein-buch-server healthcheck >/dev/null || fail "Healthcheck"

step "Härtung (docker inspect / exec)"
test "$(docker image inspect -f '{{.Config.User}}' "$IMAGE")" = "65532:65532" || fail "Image-USER"
test "$(docker inspect -f '{{.HostConfig.ReadonlyRootfs}}' "$NAME")" = "true" || fail "Root-FS nicht read-only"
docker inspect -f '{{json .HostConfig.CapDrop}}' "$NAME" | grep -q '"ALL"' || fail "CapDrop ALL fehlt"
docker inspect -f '{{json .HostConfig.SecurityOpt}}' "$NAME" | grep -q 'no-new-privileges' || fail "no-new-privileges fehlt"
test "$(docker exec "$NAME" id -u)" != "0" || fail "läuft als root"
docker exec "$NAME" grep -qE '^CapEff:\s+0+$' /proc/1/status || fail "CapEff ≠ 0"
if docker exec "$NAME" sh -c 'touch /schreibtest' 2>/dev/null; then fail "/ beschreibbar"; fi
if docker exec "$NAME" sh -c 'test -w /opt/klein-buch/web' 2>/dev/null; then fail "/opt/klein-buch beschreibbar"; fi
echo "Härtung ok"

step "Einrichtung, Testdaten, Abnahme-Ablauf (image_smoke.py)"
token=$(docker logs "$NAME" 2>&1 | sed -n 's/^Setup-Token: \([A-Z0-9]*\).*/\1/p' | head -1 || true)
test -n "$token" || fail "Setup-Token nicht im Log"
SMOKE_URL="$URL" SMOKE_COOKIE_FILE="$WORK/cookie" python3 "$HERE/image_smoke.py" "$token"
cookie=$(cat "$WORK/cookie")
test -n "$cookie" || fail "kein Sitzungs-Cookie"

# Sitzung und Pflicht-Header für alle ZAP-Anfragen (Replacer).
ZAP_OPTS="-config replacer.full_list(0).description=session \
-config replacer.full_list(0).enabled=true \
-config replacer.full_list(0).matchtype=REQ_HEADER \
-config replacer.full_list(0).matchstr=Cookie \
-config replacer.full_list(0).regex=false \
-config replacer.full_list(0).replacement=__Host-kb_session=${cookie} \
-config replacer.full_list(1).description=csrf \
-config replacer.full_list(1).enabled=true \
-config replacer.full_list(1).matchtype=REQ_HEADER \
-config replacer.full_list(1).matchstr=X-KleinBuch-Request \
-config replacer.full_list(1).regex=false \
-config replacer.full_list(1).replacement=1"

step "OWASP ZAP Baseline"
docker run --rm --network host -v "$ZAPDIR:/zap/wrk:rw" "$ZAP_IMAGE" \
  zap-baseline.py -t "$URL" -m 3 -I -J baseline.json -r baseline.html -z "$ZAP_OPTS" \
  || fail "ZAP Baseline lief nicht durch"

step "OpenAPI aus der Registry"
python3 -I "$HERE/openapi_from_registry.py" "$URL" > "$ZAPDIR/openapi.json"

step "OWASP ZAP API-Scan (alle Commands, mit Sitzung, höchstens 25 min)"
docker run --rm --network host -v "$ZAPDIR:/zap/wrk:rw" "$ZAP_IMAGE" \
  zap-api-scan.py -t /zap/wrk/openapi.json -f openapi -T 25 -I -J api.json -r api.html -z "$ZAP_OPTS" \
  || fail "ZAP API-Scan lief nicht durch"

step "Gate"
python3 -I "$HERE/zap_gate.py" "$ROOT/.github/zap-ausnahmen.tsv" "$ZAPDIR/baseline.json" "$ZAPDIR/api.json"
docker exec "$NAME" /usr/local/bin/klein-buch-server healthcheck >/dev/null || fail "Server nach dem Scan nicht erreichbar"
logs=$(docker logs "$NAME" 2>&1 || true)
if grep -qF "$PASS_MARKER" <<<"$logs"; then fail "Passphrase im Container-Log"; fi
if grep -qF "$cookie" <<<"$logs"; then fail "Sitzungs-Cookie im Container-Log"; fi
echo "keine Geheimnisse im Log"

step "Scan grün"
