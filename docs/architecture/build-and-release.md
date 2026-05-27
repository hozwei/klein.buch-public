# Build- und Release-Prozess

> Vertiefung zu „Build/Release" in `../ARCHITECTURE.md` §7 und §8. Wie
> Klein.Buch von Source-Code zum auslieferbaren Installer wird, was die CI
> grün macht, wie ein Release-Tag das Auslieferungs-Bundle erzeugt.

**v1.0 ist Windows-only** (Scope-Wechsel 2026-05-26). macOS und Linux sind
explizit aus v1.0 raus; Detail siehe `docs/RELEASE-1.0-GUIDE.md`. Der
Sidecar-Build und das Tauri-Bundle existieren entsprechend nur für die
Windows-Target-Triplets.

---

## 1. Werkzeug-Kette

| Werkzeug | Rolle | Version (Stand) |
|---|---|---|
| **Rust stable** | Tauri-Backend kompilieren | rust-toolchain stable (CI: dtolnay/rust-toolchain@stable) |
| **Node + pnpm 10** | Frontend kompilieren | Node 20, pnpm 10 (lock-file ist `klein-buch/pnpm-lock.yaml`) |
| **Tauri 2** | Bundler (App + NSIS + MSI auf Windows) | aus `Cargo.toml`/`package.json` |
| **SQLCipher** | DB-Verschlüsselung (G1-ENC) | gepinnt über `libsqlite3-sys = "=0.30.1"` mit `bundled-sqlcipher-vendored-openssl` |
| **Perl + NASM (Windows-MSVC)** | OpenSSL-vendored bauen | Host-Pflicht — siehe Cargo.toml-Kommentar |
| **Java 21+ JDK + `jlink`** | Sidecar-Build (KoSIT + Mustang) | `scripts/build-sidecar.ps1` |

**Host-Build vs. CI.** Tests + `cargo fmt`/`cargo clippy` laufen in der CI
(`.github/workflows/ci.yml`, Ubuntu-Runner). Der **Release-Build** läuft
auf einem **Windows-Runner** (`.github/workflows/release.yml`,
`windows-latest`) — sonst kann das NSIS+MSI-Bundle nicht erzeugt werden.

---

## 2. Sidecar-Build

Der Java-Sidecar (KoSIT-Validator + Mustang) ist **eine** ausführbare
Einheit, einmal pro Target-OS. Aktuell existiert er nur für
`x86_64-pc-windows-msvc`.

### 2.1 Script

`scripts/build-sidecar.ps1` am Repo-Root (`Buchhaltung/scripts/`). Output:
`sidecar-build/klein-buch-java-x86_64-pc-windows-msvc/`.

**Pinned versions** (im Skript-Header):

- **KoSIT-Validator** `v1.6.2` (GitHub-Release Asset
  `*standalone*.jar`).
- **XRechnung-Konfiguration** `v2026-01-31` (oder fallback `release-2026-01-31`
  / `2026-01-31`).
- **Mustang** `2.23.0` (direkt von `mustangproject.org`, kein GitHub-Release).

### 2.2 Schritte

1. **KoSIT-Validator-JAR** runterladen (`*standalone*.jar` aus GitHub-Release).
2. **XRechnung-Konfiguration-ZIP** runterladen und nach
   `xrechnung-config/` entpacken.
3. **Mustang-CLI-JAR** direkt von `mustangproject.org`.
4. **jlink-JRE** bauen — Minimal-Java-Runtime mit nur den benötigten Modulen
   (`java.base`, `java.xml`, `java.naming`, `java.management`, …). Größe
   typischerweise ~80 MB statt ~200 MB für eine volle JRE.
5. **Launcher-Skript** `klein-buch-java.bat` erzeugen, das die JRE und die
   JARs zusammenfügt und Mustang oder KoSIT als CLI-Tool aufruft.
6. **Versions-Manifest** `versions.json` — KoSIT-Tag, XRechnung-Tag,
   Mustang-Version, JDK-Version, Build-Zeitpunkt. Wird zur Laufzeit von der
   App gelesen (Settings → „Über") und im EÜR-Export beigelegt.

### 2.3 Committed Artifacts

Der gebaute Sidecar liegt **committed** unter
`klein-buch/src-tauri/binaries/klein-buch-java-x86_64-pc-windows-msvc/`.
Das ist Absicht: der Release-Workflow soll **kein** Sidecar-Build laufen
lassen (sonst wären KoSIT + Mustang + JDK + Mustang.org-Download
Build-Zeit-Abhängigkeiten und ein Server-Ausfall würde das Release blocken).
Das committen wird durch `.gitignore` so geregelt, dass nur die Sidecar-
Binary-Artefakte ihren Weg ins Repo finden, nichts aus `target/` oder
`work/`.

**Updaten** des Sidecars passiert **manuell**: Manuel führt
`scripts/build-sidecar.ps1` aus, prüft mit den Sidecar-Smokes
(`sidecar-build/smokes/`), committet das neue Binary-Verzeichnis mit dem
Prefix `block-…: sidecar update kosit X / xrechnung Y / mustang Z`.

---

## 3. Frontend-Build

`klein-buch/package.json` definiert vier Skripte:

- `pnpm dev` — Vite-Dev-Server, gestartet von `tauri dev`
- `pnpm build` — Vite-Production-Build (Output: `klein-buch/build/`)
- `pnpm check` / `pnpm lint` — `svelte-kit sync` + `svelte-check`
- `pnpm tauri` — Wrapper auf `@tauri-apps/cli`

`tauri.conf.json` zeigt `frontendDist` auf `../build`. Der Tauri-Build ruft
`pnpm build` als `beforeBuildCommand`.

---

## 4. Tauri-Bundle

`klein-buch/src-tauri/tauri.conf.json`:

- `productName = "Klein.Buch"`, `identifier = "de.wildbach.kleinbuch"`.
- `bundle.targets = "all"` — auf Windows ergibt das NSIS-Installer (.exe)
  + MSI-Paket (.msi).
- `bundle.resources` packt den Windows-Sidecar (`binaries/klein-buch-
  java-x86_64-pc-windows-msvc/*`) und die `inputs/`-Default-Assets ins
  Bundle.
- `bundle.category = "Finance"`.
- Icons in `klein-buch/src-tauri/icons/` (`32x32`, `128x128`, `128x128@2x`,
  `.ico`, `.icns`).

**App-Lokales Datenverzeichnis** zur Laufzeit: `%APPDATA%\
de.wildbach.kleinbuch\`. Dort liegen die SQLCipher-DB, das Archiv, der
lokale Backup-Floor, Branding-Logos, Exporte und der Restore-Staging-
Ordner. **`inputs/`** liegt **außerhalb** dieses Verzeichnisses (im Repo
für Dev, im Bundle für die installierte App).

---

## 5. CI-Workflow (`ci.yml`)

Trigger: `push` auf `main` und PR-Open gegen `main`.

**Job 1 — `rust`**:
1. Ubuntu-Runner.
2. Tauri-Linux-Deps installieren (`libwebkit2gtk-4.1-dev`,
   `libappindicator3-dev`, `librsvg2-dev`, `libssl-dev`, `patchelf`).
3. Free disk space (`/usr/local/lib/android` und ähnliches wegräumen — Tauri
   + Typst lassen den Runner sonst volllaufen).
4. Rust stable + `rustfmt` + `clippy`.
5. `Swatinem/rust-cache@v2` mit `workspaces: klein-buch/src-tauri`.
6. `cargo fmt --all -- --check`.
7. `cargo clippy --all-targets -- -D warnings`.
8. `cargo test --all-features`.

**Services in Job 1**: MailHog (SMTP 1025 + HTTP-API 8025) für den
Block-5-E2E-SMTP-Test. Der Test erkennt selbst, ob MailHog erreichbar ist
und skippt sich notfalls.

**Job 2 — `frontend`**:
1. Ubuntu-Runner.
2. pnpm + Node 20 (Cache auf `klein-buch/pnpm-lock.yaml`).
3. `pnpm install --frozen-lockfile`.
4. `pnpm lint` (= `svelte-kit sync && svelte-check`).

**Beide Jobs grün** ⇒ CI grün. Die Sidecar-/KoSIT-Tests laufen in CI im
Mock-Mode (`MOCK_KOSIT=1` / `MOCK_MUSTANG=1` über Test-Helpers), weil der
Sidecar im Runner nicht vorhanden ist (Linux, kein Windows-Binary).

---

## 6. Release-Workflow (`release.yml`)

Trigger: Tag-Push `v*` oder manueller Workflow-Dispatch (mit `tag`-Input).

**Job — `windows`**:
1. `windows-latest`-Runner.
2. Checkout.
3. Rust stable + `Swatinem/rust-cache`.
4. pnpm 10 + Node 20.
5. `pnpm install --frozen-lockfile`.
6. **`pnpm tauri build`** — Frontend (über `beforeBuildCommand: pnpm build`)
   + Rust-Release-Build + NSIS-Installer + MSI-Paket.
7. **SHA-256-Checksums** (PowerShell): findet alle `.exe` und `.msi` unter
   `src-tauri/target/release/bundle/`, schreibt `SHA256SUMS.txt`.
8. **Draft-Release** mit `softprops/action-gh-release@v2`:
   - `draft: true` (Manuel reviewt vor Publish).
   - `name`/`tag_name` aus dem Workflow-Input oder dem gepushten Tag.
   - Release-Notes-Body enthält SmartScreen-Hinweis und SHA-256-Anleitung
     (siehe §8).
   - Artefakte: `*.exe`, `*.msi`, `SHA256SUMS.txt`.

**Kein Code-Signing.** Bewusst unsignierte Installer (Apple-Developer-ID-
Programm wäre 99 €/Jahr, EV-Certificate für Windows-SmartScreen-Bypass
~300 €/Jahr; beides für v1.0 gespart). Konsequenz: Windows SmartScreen
warnt beim ersten Start.

**Signing-Hintergrund**: `klein-buch/docs/release-signing-guide.md` — was es
kosten würde, was sich ändert, wann es relevant wird.

---

## 7. Tag-Konvention

- **Feature-/Phasen-Stand-Tags**: `v0.1.0`, `v0.1.0-phase2c`, `v0.1.0-phase4`
  etc. Vor v1.0 als „milestones" gesetzt, lösen Release-Workflow aus, Result
  ist Draft-Release.
- **v1.0-Release-Tag**: `v1.0` (oder `v1.0.0`, Workflow akzeptiert beides
  über `v*`-Pattern). Tag-Push triggert `release.yml`.
- **Pre-Releases**: `v1.0.0-rc.1` etc. — werden gleich behandelt wie
  Release-Tags.

---

## 8. SmartScreen + erster Start

Ein unsigned Windows-Installer löst SmartScreen aus:

> Windows hat Ihren PC geschützt. Microsoft Defender SmartScreen hat den
> Start einer unbekannten App verhindert.

Nutzer-Pfad: **„Weitere Informationen" → „Trotzdem ausführen"**. Das wird
in den Release-Notes erklärt:

> **Unsignierte Windows-Installer.** Windows SmartScreen warnt beim ersten
> Start („Unbekannter Herausgeber") → „Weitere Informationen" → „Trotzdem
> ausführen". Wer das nicht möchte, baut aus dem Quellcode selbst (siehe
> README).
>
> **Integrität prüfen:** SHA-256-Summen in `SHA256SUMS.txt`. Unter Windows:
> `Get-FileHash <datei> -Algorithm SHA256` muss übereinstimmen.

Das gleiche steht auch im User-Handbuch (G2-DOC.2.6 Troubleshooting:
„SmartScreen-Warnung").

---

## 9. Self-Build aus dem Quellcode

Für Nutzer, die SmartScreen nicht akzeptieren wollen:

1. Repo klonen.
2. Pflicht-Werkzeug installieren: Rust stable, Node 20, pnpm 10, Java JDK 21,
   Perl, NASM (auf Windows-MSVC).
3. **Sidecar bauen**: `pwsh scripts/build-sidecar.ps1` (im Repo-Root).
4. Sidecar nach `klein-buch/src-tauri/binaries/` verschieben (im Skript
   beschrieben).
5. **Frontend bauen**: `cd klein-buch && pnpm install && pnpm build`.
6. **Tauri bauen**: `pnpm tauri build`. Output: `klein-buch/src-tauri/target/
   release/bundle/`.

`README.md` am Repo-Root listet das in einer Self-Build-Sektion.

---

## 10. Pre-Release-Smokes

Vor einem v1.0-Tag empfiehlt sich:

1. **Lokal `cargo fmt --check` + `cargo clippy -D warnings` + `cargo test`**
   grün (alle drei in `klein-buch/src-tauri/`).
2. **`pnpm check`** grün.
3. **`pnpm tauri dev`** öffnet die App, Login mit Test-Passphrase, eine
   Beispiel-Rechnung End-to-End: Draft → Lock & Issue → PDF in `archive/`,
   XML in `archive/` → Versand (SMTP zum Test-Account oder Mailhog).
4. **Backup-Roundtrip**: Backup erstellen, App schließen, Datei manuell
   umbenennen (DB unlesbar machen), App starten, Restore vom Backup wählen,
   Restart, verifizieren dass die Rechnung wieder da ist.
5. **`scripts/build-sidecar.ps1` re-run** wenn KoSIT, XRechnung-Config oder
   Mustang seit dem letzten Build aktualisiert wurden.

Diese Liste wird im `RELEASE-1.0-GUIDE.md` als Teil von G4 final
formalisiert.

---

## Letzte Verifikation

Stand: 2026-05-26. Quelle: `.github/workflows/ci.yml`,
`.github/workflows/release.yml`, `klein-buch/src-tauri/tauri.conf.json`,
`klein-buch/src-tauri/Cargo.toml`, `klein-buch/package.json`,
`scripts/build-sidecar.ps1`. Bei jedem CI/Release-Workflow-Update diese
Datei mitziehen.
