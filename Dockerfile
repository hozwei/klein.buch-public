# syntax=docker/dockerfile:1.7
#
# Klein.Buch — Server-Image (Plan W11, ADR 0039).
# Build-Kontext: Wurzel des öffentlichen Repos (hozwei/klein.buch-public),
# im Entwicklungsordner also `klein-buch/`.
#
#   docker build -t klein-buch:dev .
#
# Stufen: SPA (Node) → Server (Rust) → Java-Laufzeit (jlink) → Laufzeit
# (debian:trixie-slim). Basis-Images mit Digest; Versionen und Quellen auch in
# src-tauri/sidecar/versions.json.

ARG NODE_IMAGE=node:22-trixie-slim@sha256:8eb619e402eaede1748a3c8173bffab391592812e85136452f781741c7242ddf
ARG RUST_IMAGE=rust:1.99.1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc
ARG JDK_IMAGE=eclipse-temurin:21.0.12_8-jdk@sha256:92a2a4d7a928d057e7bd999c418d66c26a34eb9a0442f3ab67721c3f88110b2d
ARG RUNTIME_IMAGE=debian:trixie-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad

# ---------------------------------------------------------------------------
# SPA
# ---------------------------------------------------------------------------
FROM ${NODE_IMAGE} AS web
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
RUN pnpm build

# ---------------------------------------------------------------------------
# Server
# ---------------------------------------------------------------------------
FROM ${RUST_IMAGE} AS server
RUN apt-get update \
 && apt-get install -y --no-install-recommends pkg-config liblzma-dev perl make \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /src/src-tauri
COPY src-tauri/rust-toolchain.toml ./
RUN rustup show active-toolchain && cargo install cargo-auditable --locked --version 0.7.7
COPY src-tauri/Cargo.toml src-tauri/Cargo.lock ./
COPY src-tauri/migrations ./migrations
COPY src-tauri/src ./src
# Quell-Angaben für /api/public/info (AGPL §13) spät, damit der Cache greift.
ARG KLEIN_BUCH_COMMIT=unbekannt
ENV KLEIN_BUCH_COMMIT=${KLEIN_BUCH_COMMIT}
RUN cargo auditable build --release --locked --bin klein-buch-server \
 && cp target/release/klein-buch-server /usr/local/bin/klein-buch-server

# ---------------------------------------------------------------------------
# Java-Laufzeit (jlink) für KoSIT-Validator und Mustang
# ---------------------------------------------------------------------------
FROM ${JDK_IMAGE} AS jre
COPY src-tauri/sidecar/versions.json /tmp/versions.json
RUN MODULES=$(sed -n 's/.*"jlink_modules": *"\([^"]*\)".*/\1/p' /tmp/versions.json) \
 && test -n "$MODULES" \
 && jlink --add-modules "$MODULES" --strip-debug --no-man-pages --no-header-files \
      --compress=zip-6 --output /opt/jre

# ---------------------------------------------------------------------------
# Laufzeit
# ---------------------------------------------------------------------------
FROM ${RUNTIME_IMAGE}
RUN apt-get update \
 && apt-get install -y --no-install-recommends tzdata fontconfig fonts-dejavu-core libfreetype6 \
 && rm -rf /var/lib/apt/lists/*

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
COPY LICENSE COPYRIGHT THIRD-PARTY-NOTICES.md /opt/klein-buch/legal/
RUN chmod 0755 /opt/klein-buch/sidecar/klein-buch-java.sh \
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
