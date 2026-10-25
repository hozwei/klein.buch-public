# Build- und Release-Prozess

> Vertiefung zu „Phasen und Auslieferung" in `../ARCHITECTURE.md` §8. Wie aus dem
> Quellstand das signierte Docker-Image wird, was die CI prüft und wie ein
> Release entsteht. Grundlage: ADR 0039 (Web-/Docker-Betrieb, löst ADR 0002 und
> 0038 ab), Nr. 99–114.

**Ein Artefakt:** das Image `ghcr.io/hozwei/klein-buch:<CalVer>`, nur
`linux/amd64`, gebaut in GitHub Actions aus dem öffentlichen Repository
`hozwei/klein.buch-public`. Installer, Desktop-Bundles und der
Windows-Release-Workflow sind entfallen (W5, W11).

**Pfade** in diesem Dokument gelten relativ zur Wurzel des öffentlichen
Repositorys (= Inhalt des Produktordners `klein-buch/` plus `.github/`).

---

## 1. Werkzeuge und Versionen

| Werkzeug | Rolle | Version / Quelle |
|---|---|---|
| Rust | Server `klein-buch-server` | `src-tauri/rust-toolchain.toml` (1.99.1); im Image `rust:1.99.1-trixie` mit Digest |
| cargo-auditable | Abhängigkeitsliste im Binary (für Scanner) | 0.7.7 |
| SQLCipher + OpenSSL | DB-Verschlüsselung | `libsqlite3-sys = "=0.30.1"` mit `bundled-sqlcipher-vendored-openssl`; braucht `perl`, `make` |
| Node + pnpm | SPA (SvelteKit 3, Vite 8, `adapter-static`; Konfiguration in `vite.config.js`) | Node 24 LTS (`node:24-trixie-slim` mit Digest), pnpm aus `packageManager` in `package.json` |
| Eclipse Temurin JDK + `jlink` | Java-Laufzeit für den Sidecar | `eclipse-temurin:21.0.12_8-jdk` mit Digest; Module aus `versions.json` (`jlink_modules`) |
| cargo-about | Rust-Teil der Lizenzseite `/legal/` | 0.9.2 (`--features cli`) |
| Debian | Laufzeit-Basis | `debian:trixie-slim` mit Digest |
| syft, trivy, cosign, skopeo | SBOM, Scan, Signatur, OCI-Transport | syft 1.55.0, trivy 0.76.1, cosign 3.1.4 (per SHA-256 geprüft); skopeo vom Runner |
| cargo-deny, cargo-audit, gitleaks | Gates in der CI | 0.20.2, 0.22.2, 8.30.1 (per SHA-256 geprüft) |

Alle Basis-Images stehen mit Digest direkt in den `FROM`-Zeilen des
`Dockerfile` (Dependabot erkennt nur diese Form); `legal/check_base_images.py`
prüft in der CI, dass sie zu `src-tauri/sidecar/versions.json` passen. Alle
GitHub Actions sind per Commit-SHA festgelegt.

---

## 2. Java-Sidecar aus geprüften Quellen

`src-tauri/sidecar/` enthält:

- `lib/kosit-validator.jar` (KoSIT-Validator 1.6.2, `validator-1.6.2-standalone.jar`
  von GitHub) und `lib/mustang.jar` (Mustang 2.23.0, `Mustang-CLI-2.23.0.jar` von
  Maven Central);
- `xrechnung-config/` (XRechnung-Konfiguration 2026-01-31, Release-ZIP entpackt);
- `versions.json` (Schema 2: je Komponente Quelle + SHA-256, jlink-Module,
  JDK-Hersteller/-Version/-Image, Basis-Images, Angaben der früheren
  Windows-Releases unter `desktop_releases_2026_5`);
- `klein-buch-java.sh` (POSIX-sh-Launcher; der Server startet ihn über
  `/bin/sh`, weil das Ausführbar-Bit nicht überall erhalten bleibt; JVM mit
  `-Xmx512m`, Temp unter `$TMPDIR`);
- `THIRD-PARTY-JAVA.md`, `notices/`, `copyleft-sources.txt` (Lizenzinventar,
  NOTICE-Texte, Quellen der Copyleft-Teile für das Quellarchiv).

`.gitattributes` schließt `lib/` und `xrechnung-config/` von der
Zeilenend-Umwandlung aus. Der Workflow `image.yml` lädt die Originale und
vergleicht SHA-256 bzw. den entpackten Baum (§5). Ein Update der Komponenten
heißt: neue Originale nach `lib/` bzw. `xrechnung-config/`, `versions.json`
nachziehen, Inventar (`THIRD-PARTY-JAVA.md`) und Hinweise prüfen
(`legal/check_java_inventory.py`).

---

## 3. Image-Build (`Dockerfile`)

Build-Kontext ist die Wurzel des öffentlichen Repositorys (`.dockerignore` hält
ihn klein). Stufen:

| Stufe | Inhalt |
|---|---|
| `web` | `pnpm install --frozen-lockfile`, `pnpm build` (SPA samt Handbuch aus `src-tauri/resources/handbook`); die JS-Lizenzliste des Bundles (`js-licenses.json`, eigenes Vite-Plugin) geht in die Lizenzseite |
| `server` | `cargo auditable build --release --locked --bin klein-buch-server`; Build-Argumente `KLEINBUCH_SOURCE_URL`, `KLEINBUCH_BUILD_COMMIT`, `KLEINBUCH_BUILD_TAG` spät (Cache) |
| `jre` | `jlink` mit den Modulen aus `versions.json` → `/opt/jre` |
| `legal-tools` | cargo-about (parallel zum Server-Build) |
| `runtime-base` | `debian:trixie-slim` + `apt-get upgrade` + `tzdata`, `fontconfig`, `fonts-dejavu-core`, `libfreetype6`; Paketliste `debian-pakete.tsv` per `dpkg-query` (danach kein `apt-get install` mehr) |
| `legal` | `cargo metadata`, `cargo about generate --fail`, `legal/build_legal.py licenses` → `third-party-licenses.html`, `LICENSE.txt`, `legal.css`. Braucht Netz (cargo-about löst den Graphen für alle Plattformen auf). |
| `legal-sources` | `build_legal.py sources` → `quellen.html`, `SOURCES.md`, `debian-pakete.tsv`; Abbruch bei „Dev-Build-Platzhalter“ |
| Laufzeit | Binary, SPA (`/opt/klein-buch/web`), Seed-Quelle `inputs/` (`/opt/klein-buch/inputs`), Sidecar + JRE (`/opt/klein-buch/sidecar`), Lizenzseite (`/opt/klein-buch/legal` und `/usr/share/doc/klein-buch`); `ENV` mit `KLEIN_BUCH_*`-Pfaden, `TZ=Europe/Berlin`, `HOME=/tmp`, `TMPDIR=/data/tmp`; `USER 65532:65532`, `EXPOSE 8080`, `HEALTHCHECK` über `klein-buch-server healthcheck`; kein `VOLUME` |

`build_legal.py` bricht ab, wenn ein Pflichtteil der Lizenzseite fehlt (u. a.
„SIL OPEN FONT LICENSE“, „GUST Font License“, „Zetetic“, „BY-SA 3.0“).

Lokal:

```sh
docker build -t klein-buch:dev .
docker build --build-arg KLEINBUCH_BUILD_COMMIT=$(git rev-parse HEAD) \
  --build-arg KLEINBUCH_BUILD_TAG=v<Version> -t klein-buch:dev .
```

Ohne Commit zeigt die Oberfläche „Entwicklungs-Build, Quellstand unbekannt“
(`/api/public/info` liefert dann `sourceLink: null`).

---

## 4. CI (`.github/workflows/ci.yml`)

Trigger: Push auf `main` und `phase-w`, Pull Requests gegen `main`. Nur
`contents: read`, `concurrency` je Zweig (ein neuer Push bricht den laufenden
Lauf ab), Runner fest `ubuntu-24.04`, Zeitlimits je Job. Alle Jobs laufen nur im
Repository `hozwei/klein.buch-public`.

| Job | Schritte |
|---|---|
| `rust` | Plattenplatz freigeben; Toolchain aus `rust-toolchain.toml`; `Swatinem/rust-cache`; `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-features`. Dienst Mailpit (`axllent/mailpit` per Digest, SMTP 1025, API 8025; bis Lauf F7a MailHog) für den SMTP-E2E-Test `tests/e2e_test.rs`. Keine System-Pakete (seit W5 ohne WebKit, seit W8 ohne libdbus, seit W12 ohne lzma). |
| `frontend` | pnpm + Node 24; `pnpm install --frozen-lockfile`; `pnpm lint` (svelte-check); `pnpm test` (vitest, `TZ=UTC`); `pnpm build` mit Prüfung von `build/index.html` und `build/legal/js-licenses.json` (wie die Stufe `web`) und Vergleich der Paketliste mit der Prüfliste der Lizenzseite in `image_smoke.sh`; `openapi_from_registry.py` (ZAP-Scan); Prüfsumme von `LICENSE` (AGPL-3.0-Volltext), `COPYRIGHT` vorhanden |
| `rust-supply-chain` | `cargo deny --locked check advisories licenses` (`src-tauri/deny.toml`); `cargo audit` über das ganze `Cargo.lock` mit denselben begründeten Ausnahmen |
| `js-audit` | `pnpm audit --prod --audit-level high` und `pnpm audit --audit-level high` (mit Entwicklungsabhängigkeiten, blockierend; Lauf F7a, ADR 0039 Nr. 142 (5)); Ausnahmen nur über `auditConfig.ignoreGhsas` in `pnpm-workspace.yaml` mit Begründung und Ablaufdatum |
| `secrets` | `gitleaks dir` über den Arbeitsbaum (Funde geschwärzt) |
| `java-inventory` | `legal/check_java_inventory.py` (Java-Inventar ↔ JARs), `legal/check_base_images.py` (Basis-Images ↔ `versions.json`; dazu jedes Image der Workflows und CI-Skripte mit Digest, Lauf F7a) |

Die Rust-Tests laufen mit dem Sidecar-Mock (`KLEIN_BUCH_SIDECAR_MOCK`, nur in
Debug-Builds wirksam). Die echten Validator-Tests (`tests/sidecar_real_test.rs`,
`#[ignore]`) laufen lokal gegen eine jlink-JRE; im Image decken die
HTTP-Schritte des Smoke-Tests dieselben Wege ab.

Abhängigkeiten übersetzt Cargo im Dev-/Test-Profil mit `opt-level = 2`
(`[profile.dev.package."*"]` in `Cargo.toml`, Lauf F3c; der eigene Crate bleibt
bei 0, Debug-Prüfungen bleiben an, das Release-Profil ist unberührt). Ohne das
liefen Argon2id, SQLite/SQLCipher, Deflate und Typst in den Tests so langsam, dass
Server-Tests unter Volllast die 30-s-Grenze der Anfragen rissen. Der Cache von
`Swatinem/rust-cache` hängt an `Cargo.toml` und `Cargo.lock`; nach einer Änderung
übersetzt der erste Lauf die Abhängigkeiten einmal neu (lokal etwa 2–3 Minuten
mehr), `cargo clippy` erzeugt keinen Code und ist davon kaum betroffen.

---

## 5. Image-Workflows

**`.github/workflows/image.yml`** (Push auf `phase-w`, manuell; nur
`contents: read`):

- `sidecar-sources`: KoSIT-Validator, Mustang und XRechnung-Konfiguration aus den
  Quellen in `versions.json` laden, SHA-256 bzw. entpackten Baum vergleichen.
- `compose-example`: `docker compose -f compose.example.yaml config --quiet` und
  Prüfung, dass das Beispiel ein Image `ghcr.io/hozwei/klein-buch:…` nennt.

**`.github/workflows/release-image.yml`**, Job `dry-run` (Trockenlauf auf
`phase-w`, nur `contents: read`, kein Push, keine Signatur):

1. Werkzeuge laden (syft, trivy per SHA-256), Version
   `<Cargo-Version>-dev.<Kurz-SHA>`.
2. Build als OCI-Layout mit SBOM- und Provenance-Attestierung
   (`sbom: true`, `provenance: mode=max`), GHA-Cache.
3. Attestierungen im Index prüfen, per skopeo in Docker laden, Schicht-IDs mit
   dem Layout vergleichen.
4. Smoke-Test `.github/scripts/image_smoke.sh` (mit `image_smoke.py`): Größe
   < 400 MB, Mock-Schalter verweigert den Start, Container read-only ohne
   Capabilities mit fremder UID, Healthcheck, Zeitzone, `CapEff` 0,
   Seed-Manifest; über HTTP Einrichtung mit Setup-Token und TOTP, Festschreiben
   mit echtem KoSIT + Mustang (PDF/A-3), ZUGFeRD-Extraktion im Rechnungseingang,
   Ablehnung einer manipulierten XRechnung; Lizenz- und Quelldateien,
   `/legal/` (Prüfliste der JS-Pakete), `/api/public/info` mit dem Build-Commit;
   das Rechnungs-PDF geht nach `SMOKE_PDF_OUT`.
5. PDF/A-3b mit veraPDF (R3-018, Lauf F7a): veraPDF 1.30.3 (Installer per SHA-256,
   Java 21 des Runners) prüft das Rechnungs-PDF mit `--flavour 3b`; nicht konform
   bricht ab, der Bericht kommt ins Artefakt. Nur im Trockenlauf.
6. SBOM mit syft (SPDX-JSON).
7. trivy: CRITICAL/HIGH mit verfügbarem Fix brechen ab; Ausnahmen begründet und
   befristet in `.trivyignore`.
8. Quellarchiv „Quellen Dritter“ (`.github/scripts/build_sources_archive.sh`).
9. SBOM, veraPDF-Bericht und Quellarchiv als Workflow-Artefakt (14 Tage).

---

## 6. Release (`release-image.yml`, Jobs `release` und `publish`)

**Auslöser:** ein Tag `v*` im öffentlichen Repository. Zwei Jobs (Lauf F): `release` baut und
prüft **nur lesend** (`contents: read`) und übergibt OCI-Layout, Quellarchiv und SBOM als
Artefakt; `publish` (`contents: write`, `packages: write`, `id-token: write` für cosign keyless)
holt genau diesen Build, prüft dessen Digest gegen den geprüften Index und veröffentlicht.
„Re-run failed jobs“ wiederholt nur `publish` mit demselben Build. Schritte 1–4 laufen in
`release`, ab 5 in `publish`.

1. **Tag-Commit liegt auf `main`**, sonst Abbruch: Ein Tag auf einem
   Arbeitszweig baut kein Image.
2. **Version:** Tag im Format `vJJJJ.M.P` (Regex `^v[0-9]{4}\.[0-9]{1,2}\.[0-9]+$`)
   und gleich `version` in `src-tauri/Cargo.toml` (die zeigt der Server an).
   Tags mit Zusatz (z. B. `-rc.1`) ergeben kein Release.
3. Build **ohne Cache** mit `pull: true`, SBOM- und Provenance-Attestierung,
   Build-Argumente mit Commit und Tag.
4. Laden, Smoke-Test, SBOM, trivy, Quellarchiv — wie im Trockenlauf (ohne veraPDF:
   der Stand von `main` ist vorher als `phase-w` geprüft).
5. **Release zuerst** (Lauf E, ADR 0039 Nr. 123): `gh release create --verify-tag
   --latest` — veröffentlicht, kein Entwurf — mit Notizen (Einleitung des
   CHANGELOG-Abschnitts, Tag und Digest, Prüfbefehl, Quellen) und den Assets
   `klein-buch-<Version>-image.txt`, `klein-buch-<Version>-sbom.spdx.json`,
   `klein-buch-<Version>-quellen-dritter.tar.zst` (+ `.sha256`). Der Digest steht
   vor dem Push fest (gebauter Index). Bei einer Wiederholung werden nur fehlende
   Assets hochgeladen, vorhandene nie ersetzt. Ist das Release danach noch ein
   Entwurf, bricht der Lauf vor dem Push ab.
6. **Push nur mit Quellarchiv:** `skopeo copy --all --preserve-digests` nach
   `ghcr.io/<owner>/klein-buch:<Version>`; der gepushte Digest muss dem
   gebauten Index entsprechen. Gibt es das Tag schon mit anderem Inhalt, bricht
   der Lauf ab; mit gleichem Inhalt wird nicht erneut gepusht. Das Image trägt
   die direkte Adresse seines Quellarchivs im Label `de.klein-buch.source-archive`.
7. **Signatur:** `cosign sign` auf den Digest, danach `cosign verify` mit der
   genauen Workflow-Identität.

Feste Regeln (E15): nur volle Versionen als Tag, kein `:latest`, kein gleitender
Minor-Tag; Images und Tags `v*` werden nie gelöscht oder verschoben — sie sind
Quellbezug älterer Images und der Rückfall nach einem gescheiterten Update.

**Quellangebot (Lauf E, ADR 0039 Nr. 123):** Der Quelltext der Komponenten Dritter liegt je
Version als Release-Asset bereit, **bevor** das Image öffentlich wird, und bleibt so lange,
wie ein zugehöriges Image abrufbar ist (GPLv2 §3 „from the same place“, GPLv3 §6(d)). Deshalb:
Release-Assets nie löschen oder ersetzen; wenn überhaupt, erst Image-Versionen in GHCR
löschen, dann frühestens das Archiv. Das Repository bleibt öffentlich. In die Registry gehen
nur Tag-Builds von `main` (`phase-w` pusht nie). Wird das Image auf einem Datenträger oder
vorinstalliert weitergegeben, gehört der Quelltext mit dazu.

Beim ersten Release: Existiert das Paket `ghcr.io/hozwei/klein-buch` schon, ohne
mit dem Repository verknüpft zu sein, braucht der Workflow Schreibrechte unter
„Package settings → Manage Actions access“.

Prüfen durch Nutzer: `SECURITY.md` (Signatur, SBOM, Quellarchiv).

---

## 7. Veröffentlichen ins öffentliche Repository

Entwickelt wird in einem privaten Entwicklungsordner. In das öffentliche
Repository kommt der Stand nur über das Skript
`tools/veroeffentlichen/Veroeffentlichen.ps1` (im Entwicklungsordner, nicht Teil
dieses Repositorys). Es spiegelt `klein-buch/` als Wurzel plus `.github/`, lässt
Interna weg (z. B. `CLAUDE.md`, Memory, Review- und Planungsunterlagen, Daten,
Build-Ordner), bricht bei verbotenen Begriffen oder fremden Commits auf GitHub ab
und passt die Pfade in `.github/workflows/*.yml` an das flache Repository an.

| Aufruf | Wirkung |
|---|---|
| `… -NurAnzeigen` | nur zeigen, was sich ändern würde (immer zuerst) |
| `… -Branch phase-w -Ja` | Arbeitsstand auf einen Arbeitszweig; löst CI, `image.yml` und den Trockenlauf aus |
| `… -Tag v<Version>` | Release: Standardzweig `main`, Tag nur zusammen mit `main`; prüft, dass `src-tauri/Cargo.toml` dieselbe Version und `CHANGELOG.md` einen Abschnitt `## [<Version>]` hat |
| `… -MainOhneTag` | bewusst auf `main` ohne Tag (sonst gesperrt) |

Auf GitHub wird nichts gemergt; Pull Requests (auch von Dependabot,
`.github/dependabot.yml`) sind nur Hinweise, übernommen wird im
Entwicklungsordner (`CONTRIBUTING.md`).

---

## 8. Vor einem Release

1. Lokal grün: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
   `TZ=UTC cargo test` (in `src-tauri/`), `pnpm check`, `pnpm test`.
2. Version in `src-tauri/Cargo.toml` setzen, `CHANGELOG.md` mit Abschnitt
   `## [<Version>]`.
3. Auf `phase-w` veröffentlichen; CI, `image.yml` und Trockenlauf grün (Smoke,
   Scan, Quellarchiv).
4. Stand nach `main` bringen und mit `-Tag v<Version>` veröffentlichen.
5. Release-Lauf grün; Release-Seite (Notes, vier Assets, Digest in
   `image.txt`) und Signatur mit dem Befehl aus `SECURITY.md` prüfen. Das Release
   ist schon veröffentlicht (vor dem Image, ADR 0039 Nr. 123).

---

## Letzte Verifikation

Stand: 2026-10-24 (Phase W, Block W13). Quellen: `Dockerfile`,
`.dockerignore`, `.gitattributes`, `.github/workflows/{ci.yml,image.yml,release-image.yml}`,
`.github/scripts/{image_smoke.sh,build_sources_archive.sh}`,
`.github/dependabot.yml`, `src-tauri/rust-toolchain.toml`,
`src-tauri/Cargo.toml`, `src-tauri/sidecar/versions.json`, `package.json`,
`tools/veroeffentlichen/LIESMICH.md` (Entwicklungsordner), ADR 0039 Nr. 99–114.
Bei jeder Änderung an Dockerfile oder Workflows diese Datei mitziehen.
