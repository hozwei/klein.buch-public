# syntax=docker/dockerfile:1.7
#
# Klein.Buch — Server-Image (Plan W11, ADR 0039).
# Build-Kontext: Wurzel des öffentlichen Repos (hozwei/klein.buch-public),
# im Entwicklungsordner also `klein-buch/`.
#
#   docker build -t klein-buch:dev .
#
# Stufen: SPA (Node) → Server (Rust) → Java-Laufzeit (jlink) → Lizenz- und
# Quellangaben (`/legal/`, Plan W12) → Laufzeit (debian:trixie-slim).
# Basis-Images mit Digest; Versionen und Quellen auch in
# src-tauri/sidecar/versions.json.
#
# Quell-Angaben (AGPL §13, `/api/public/info`, `/legal/quellen.html`) kommen als
# Build-Argumente; der Release-Workflow setzt alle drei:
#
#   docker build --build-arg KLEINBUCH_BUILD_COMMIT=$(git rev-parse HEAD) \
#     --build-arg KLEINBUCH_BUILD_TAG=v2026.10.0 -t klein-buch:dev .
#
# Ohne Commit zeigt die Oberfläche „Entwicklungs-Build, Quellstand unbekannt“.
#
# Basis-Images stehen mit Digest direkt in den FROM-Zeilen (Dependabot erkennt
# nur diese Form). .github/scripts/build_sources_archive.sh liest die Stufen
# `jre` und `runtime-base`; ci.yml vergleicht alle mit versions.json.

# ---------------------------------------------------------------------------
# SPA
# ---------------------------------------------------------------------------
FROM node:22-trixie-slim@sha256:8eb619e402eaede1748a3c8173bffab391592812e85136452f781741c7242ddf AS web
WORKDIR /src
ENV CI=true \
    COREPACK_ENABLE_DOWNLOAD_PROMPT=0
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN corepack enable && pnpm install --frozen-lockfile
COPY svelte.config.js vite.config.js tsconfig.json ./
COPY static ./static
COPY src ./src
# Das Handbuch bindet Vite beim Build ein (import.meta.glob).
COPY src-tauri/resources/handbook ./src-tauri/resources/handbook
# Lizenzliste der JS-Pakete im Bundle (Vite-Plugin, Plan W12) aus dem
# Web-Wurzelordner nehmen: Sie geht in die Lizenzseite unter `/legal/`.
RUN pnpm build \
 && mkdir -p /out \
 && mv build/legal /out/legal-js \
 && test -s /out/legal-js/js-licenses.json

# ---------------------------------------------------------------------------
# Server
# ---------------------------------------------------------------------------
FROM rust:1.99.1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc AS server
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config perl make \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src/src-tauri
COPY src-tauri/rust-toolchain.toml ./
RUN rustup show active-toolchain && cargo install cargo-auditable --locked --version 0.7.7
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./
COPY src-tauri/migrations ./migrations
COPY src-tauri/src ./src
# Quell-Angaben für /api/public/info (AGPL §13) spät, damit der Cache greift.
ARG KLEINBUCH_SOURCE_URL=https://github.com/hozwei/klein.buch-public
ARG KLEINBUCH_BUILD_COMMIT=
ARG KLEINBUCH_BUILD_TAG=
ENV KLEINBUCH_SOURCE_URL=${KLEINBUCH_SOURCE_URL} \
    KLEINBUCH_BUILD_COMMIT=${KLEINBUCH_BUILD_COMMIT} \
    KLEINBUCH_BUILD_TAG=${KLEINBUCH_BUILD_TAG}
RUN cargo auditable build --release --locked --bin klein-buch-server \
 && cp target/release/klein-buch-server /usr/local/bin/klein-buch-server

# ---------------------------------------------------------------------------
# Java-Laufzeit (jlink) für KoSIT-Validator und Mustang
# ---------------------------------------------------------------------------
FROM eclipse-temurin:21.0.12_8-jdk@sha256:92a2a4d7a928d057e7bd999c418d66c26a34eb9a0442f3ab67721c3f88110b2d AS jre
COPY src-tauri/sidecar/versions.json /tmp/versions.json
RUN MODULES=$(sed -n 's/.*"jlink_modules": *"\([^"]*\)".*/\1/p' /tmp/versions.json) \
 && test -n "$MODULES" \
 && jlink --add-modules "$MODULES" --strip-debug --no-man-pages --no-header-files \
      --compress=zip-6 --output /opt/jre

# ---------------------------------------------------------------------------
# Werkzeug für die Lizenzseite (parallel zum Server-Build)
# ---------------------------------------------------------------------------
FROM rust:1.99.1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc AS legal-tools
RUN cargo install cargo-about --locked --version 0.9.2 --features cli

# ---------------------------------------------------------------------------
# Laufzeit-Basis: Pakete + Paketliste. Danach kein apt-get install mehr,
# sonst stimmt debian-pakete.tsv nicht.
# ---------------------------------------------------------------------------
FROM debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad AS runtime-base
# `upgrade`: Sicherheitsupdates seit dem Basis-Digest (sonst meldet der Scan sie
# als behoben); das Quellarchiv holt die Quellen zum Build-Zeitpunkt.
RUN apt-get update \
 && DEBIAN_FRONTEND=noninteractive apt-get upgrade -y --no-install-recommends \
 && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
      tzdata fontconfig fonts-dejavu-core libfreetype6 \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /usr/share/doc/klein-buch \
 && dpkg-query -W -f='${Package}\t${Version}\t${source:Package}\t${source:Version}\n' \
    | sort > /usr/share/doc/klein-buch/debian-pakete.tsv

# ---------------------------------------------------------------------------
# Lizenz- und Quellangaben (`/legal/`, Plan W12): Rust-Teil per cargo-about aus
# dem Registry-Stand des Server-Builds, NOTICE-Dateien, Schriften, Java- und
# JS-Teil. build_legal.py bricht ab, wenn ein Pflichtteil fehlt oder der
# Dev-Build-Platzhalter auf der Seite steht.
# ---------------------------------------------------------------------------
FROM server AS legal
RUN apt-get update \
 && apt-get install -y --no-install-recommends python3 \
 && rm -rf /var/lib/apt/lists/*
COPY --from=legal-tools /usr/local/cargo/bin/cargo-about /usr/local/cargo/bin/cargo-about
COPY src-tauri/about.toml src-tauri/about.hbs ./
COPY legal/build_legal.py legal/legal.css /src/legal/
COPY LICENSE COPYRIGHT THIRD-PARTY-NOTICES.md /src/
COPY src-tauri/sidecar/THIRD-PARTY-JAVA.md src-tauri/sidecar/versions.json /src/src-tauri/sidecar/
COPY src-tauri/sidecar/notices /src/src-tauri/sidecar/notices
COPY --from=web /out/legal-js/js-licenses.json /tmp/legal/js-licenses.json
# Mit Netz: cargo-about löst den Graphen für alle Plattformen auf (gefiltert wird
# danach per about.toml), der Server-Build hat nur die Linux-Crates geladen.
RUN cargo metadata --locked --format-version 1 --filter-platform x86_64-unknown-linux-gnu \
      > /tmp/legal/cargo-metadata.json \
 && cargo about generate --locked --fail -c about.toml -o /tmp/legal/rust.html about.hbs \
 && python3 -I /src/legal/build_legal.py licenses --out /out/legal \
      --rust-html /tmp/legal/rust.html --cargo-metadata /tmp/legal/cargo-metadata.json \
      --java-inventory /src/src-tauri/sidecar/THIRD-PARTY-JAVA.md \
      --java-notices /src/src-tauri/sidecar/notices \
      --js-licenses /tmp/legal/js-licenses.json --license /src/LICENSE --css /src/legal/legal.css \
      --versions /src/src-tauri/sidecar/versions.json \
      --extra-notice /src/THIRD-PARTY-NOTICES.md --extra-notice /src/COPYRIGHT

FROM legal AS legal-sources
COPY --from=runtime-base /usr/share/doc/klein-buch/debian-pakete.tsv /tmp/legal/debian-pakete.tsv
RUN python3 -I /src/legal/build_legal.py sources --out /out/legal \
      --versions /src/src-tauri/sidecar/versions.json \
      --debian-packages /tmp/legal/debian-pakete.tsv \
      --source-url "$KLEINBUCH_SOURCE_URL" --commit "$KLEINBUCH_BUILD_COMMIT" \
      --tag "$KLEINBUCH_BUILD_TAG" \
      --version "$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)" \
 && ! grep -rl "Dev-Build-Platzhalter" /out/legal

# ---------------------------------------------------------------------------
# Laufzeit
# ---------------------------------------------------------------------------
FROM runtime-base

ENV TZ=Europe/Berlin \
    HOME=/tmp \
    TMPDIR=/data/tmp \
    JAVA_HOME=/opt/klein-buch/sidecar/jre \
    KLEIN_BUCH_DATA_DIR=/data \
    KLEIN_BUCH_INPUTS_DIR=/inputs \
    KLEIN_BUCH_RESOURCE_DIR=/opt/klein-buch \
    KLEIN_BUCH_WEB_DIR=/opt/klein-buch/web \
    KLEIN_BUCH_LISTEN=0.0.0.0:8080

COPY --from=server /usr/local/bin/klein-buch-server /usr/local/bin/klein-buch-server
COPY --from=web /src/build /opt/klein-buch/web
COPY inputs /opt/klein-buch/inputs
COPY src-tauri/sidecar/lib /opt/klein-buch/sidecar/lib
COPY src-tauri/sidecar/xrechnung-config /opt/klein-buch/sidecar/xrechnung-config
COPY src-tauri/sidecar/versions.json src-tauri/sidecar/klein-buch-java.sh /opt/klein-buch/sidecar/
COPY --from=jre /opt/jre /opt/klein-buch/sidecar/jre
# Lizenzen, Hinweise und Quellangaben: ohne Sitzung unter /legal/ und im
# üblichen Debian-Ort.
COPY --from=legal-sources /out/legal /opt/klein-buch/legal
COPY --from=legal-sources /out/legal /usr/share/doc/klein-buch
RUN cp /usr/share/doc/klein-buch/LICENSE.txt /usr/share/doc/klein-buch/LICENSE \
 && chmod 0755 /opt/klein-buch/sidecar/klein-buch-java.sh \
 && /opt/klein-buch/sidecar/jre/bin/java -version

# Keine Dateien, die der Laufzeit-UID gehören müssen: jede UID per `user:`.
USER 65532:65532
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD ["/usr/local/bin/klein-buch-server", "healthcheck"]
ENTRYPOINT ["/usr/local/bin/klein-buch-server"]
CMD ["serve"]

LABEL org.opencontainers.image.title="Klein.Buch" \
      org.opencontainers.image.description="EÜR-Buchhaltung für §19-Kleinunternehmer (selbst gehostet)" \
      org.opencontainers.image.source="https://github.com/hozwei/klein.buch-public" \
      org.opencontainers.image.licenses="AGPL-3.0-or-later"
