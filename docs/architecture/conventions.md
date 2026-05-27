# Doku-Konventionen

> Wie alle Doku-Dateien im Klein.Buch-Repo gepflegt werden. Diese Datei
> bündelt die Konventionen, die in der Tech-Doku (`docs/`, `docs/architecture/`,
> `docs/adr/`, `docs/reference/`) gelten. Für das User-Handbuch
> (`klein-buch/src-tauri/resources/handbook/` oder wo immer G2-DOC.3 das
> ablegt) gelten **andere** Regeln — siehe G2-DOC.2 Ton-Hardline in
> `docs/RELEASE-1.0-GUIDE.md`.

---

## 1. Sprache

- **Doku-Sprache: Deutsch.** Alle Markdown-Files in `docs/` sind auf
  Deutsch. Englische Schnipsel sind erlaubt, wo das Original-Vokabular
  englisch ist (z. B. CLI-Optionen, API-Routen, Code-Identifier).
- **Code-Identifier: Englisch.** Variablen, Funktionen, Module, Trigger,
  Spalten, Tabellen, Commits, Branches — alles `snake_case` für Rust,
  `camelCase`/`PascalCase` für TS, `kebab-case` für Ordner. Die Doku zitiert
  diese Identifier wörtlich (in Backticks), übersetzt sie aber nicht.
- **Konversation (Chat zwischen Manuel und Claude/Agent): Deutsch.** Manuel
  switcht selbst, wenn er es will.
- **Commits: Englisch.** Block-Prefix-Format `block-<name>: <was geändert>`.

---

## 2. Markdown-Stil

- **Headings.** `#` für Datei-Titel, `##` für Hauptsektionen, `###` für
  Unter-Sektionen. Nur dort tiefer, wo wirklich nötig.
- **Anchor-IDs.** Markdown-Standard generiert sie aus den Heading-Texten.
  Wer eine stabile ID braucht, gibt sie explizit (`{#section-id}` an
  Heading-Ende, je nach Renderer); in der Klein.Buch-Doku noch nicht nötig.
- **Tabellen.** Pipe-Syntax, mit Header-Trennzeile. Tabellen, die in
  Roh-Markdown unangenehm zu lesen wären (>6 Spalten, sehr lange Zellen),
  besser als Definitionsliste oder als nummerierte Liste umsetzen.
- **Code-Blöcke.** Triple-Backtick mit Sprach-Hint (` ```rust `, ` ```sql `,
  ` ```powershell `, ` ```toml `). Bei Pseudo-Code: ` ```text `.
- **Backticks für Inline-Code.** Modul-Pfade, Funktionsnamen, Spalten,
  Dateinamen, Konstanten. Alles, was im Code so auftauchen würde.
- **Querverweise als Markdown-Links.** Innerhalb von `docs/architecture/`
  als relative Links (`[Sicherheits-Modell](security.md)`). Zurück auf
  ARCHITECTURE.md oder andere `docs/`-Files mit `../` (z. B.
  `[Übersicht](../ARCHITECTURE.md#bootstrap-reihenfolge)`).
- **Em-dashes erlaubt** in der Tech-Doku. Im Handbuch verboten (G2-DOC.2),
  das ist die einzige Trennlinie.
- **Bullets** sparsam, nur wo es wirklich Aufzählung ist. Lange Bullet-
  Listen mit einem Wort pro Zeile sind ein Code-Smell — entweder Tabelle
  oder Fließtext.
- **Fett**markierungen nur dort, wo eine Information aus einem Absatz
  herausspringen muss (z. B. eine Hardline oder ein Default-Wert). Nicht
  inflationär.

---

## 3. Aufbau einer Modul-/Themen-Datei

Konvention für alle `docs/architecture/<thema>.md`:

1. **Datei-Titel** (`# <Thema>`) — knapp, ohne Marketing.
2. **Blockquote oben** als Pitch: „Vertiefung zu X in `../ARCHITECTURE.md` Y.
   Diese Datei beschreibt …"  Ein bis drei Sätze.
3. **ADR-Querverweise** in einem Satz direkt nach dem Pitch, wenn ein oder
   mehrere ADRs der Hauptbezug sind.
4. **Hauptsektionen** (`## 1.`, `## 2.`, …) nummeriert. Nummerierung erleichtert
   den Verweis aus Chat und anderen Doku-Files.
5. **Letzte Verifikation** als letzte Sektion: Datum, Schema-Stand,
   Quelle(n) (welche Code-Dateien, ADRs, externe Spec). Das ist die Anker-
   Aussage, was diese Doku im damaligen Stand spiegelt.

Beispiel-Schluss:

```markdown
## Letzte Verifikation

Stand: 2026-05-26, Schema v27, ADRs 0006/0027/0036. Quelle:
`klein-buch/src-tauri/migrations/*.sql` + `src/archive/`,
`src/fiscal_year/`, `src/backup/factory_reset.rs`.
```

---

## 4. ADR-Konvention

ADRs liegen in `klein-buch/docs/adr/NNNN-<kebab-title>.md`. Header:

```markdown
# ADR NNNN — <Titel>

**Status:** akzeptiert (Datum YYYY-MM-DD)  
**Bezug:** <Block oder Sub-Block> + <ADR-Verweise>

## Kontext

…

## Entscheidung

…

## Konsequenzen

…

## Alternativen, die wir nicht gewählt haben

…
```

**Amendments** werden in derselben ADR-Datei angehängt mit einer
`## Amendment YYYY-MM-DD`-Sektion. Die Status-Zeile bleibt „akzeptiert"
(wir reden über den Amendment-Zeitraum mit), die Datums-Liste wächst.

**Nummerierung** fortlaufend, kein Springen. Reversal einer ADR ⇒ neue ADR
mit höherer Nummer, die das alte explizit ersetzt; alte ADR bekommt eine
Status-Notiz „ersetzt durch NNNN" am Anfang.

---

## 5. Code-Kommentare und Doc-Comments

- **Modul-Header** in Rust (`//!`) erklären die Verantwortung des Moduls in
  ein bis drei Sätzen.
- **Funktions-Doc-Comments** (`///`) für **alle** `pub`-Funktionen. Inhalt:
  was tut sie, was sind die Parameter, was sind die Fehler-Pfade. Beispiele
  sind optional, aber sehr gerne gesehen, wenn eine Funktion mehrere
  Modi/Branches hat.
- **Inline-Kommentare** (`//`) zur Begründung, wenn der Code allein nicht
  selbsterklärend ist. „Was" steht im Code; „Warum" gehört in den Kommentar.
- **Cargo-Dependency-Kommentare** sind sehr willkommen, besonders bei
  exotisch gepinnten Versionen (siehe `libsqlite3-sys`-Kommentar in
  `Cargo.toml` — die Begründung gehört genau dorthin, damit ein Update-
  Versuch nicht versehentlich den Pin entfernt).

---

## 6. Cross-Reference-Stil

- Innerhalb dieser `docs/architecture/`-Familie: **relative Markdown-Links**
  (`security.md`, `modules.md#6-scheduler`).
- Auf die Top-Level-Übersicht: **`../ARCHITECTURE.md`** + Anchor (`#bootstrap-
  reihenfolge`).
- Auf einen ADR: **`../adr/NNNN-<kebab>.md`** oder kurz **„ADR NNNN"** im
  Fließtext.
- Auf den PRD: **`../../PRD-klein-buch.md`** (PRD liegt am Repo-Root,
  außerhalb von `klein-buch/`).
- Auf den Release-Guide: **`../RELEASE-1.0-GUIDE.md`**.
- Auf eine Code-Datei: **`src/<pfad>`**, ohne Repo-Root-Präfix
  (`src/backup/encrypt.rs`, nicht `klein-buch/src-tauri/src/...`). Wer
  Klarheit über den Anker braucht, fügt `klein-buch/src-tauri/` davor.

---

## 7. Pflege-Disziplin

- **Bei einem Code-Change** wird **immer** geprüft, ob ein Doku-File
  betroffen ist. Wer ein neues Modul anlegt, ergänzt `modules.md`. Wer eine
  Migration schreibt, ergänzt `data-model.md` Sektion 1 + die Tabellen-
  Sektion. Wer eine Hardline ändert, prüft `gobd.md` oder `paragraph-19.md`.
- **„Letzte Verifikation"-Datum** wird in der betroffenen Doku-Datei
  aktualisiert, wenn der Inhalt überarbeitet wird. Wer eine Doku nur lesen
  geht, lässt das Datum stehen.
- **Schema-Version-Updates** schlagen in `data-model.md` Sektion 1 (Migrations-
  Liste) + Top-Header (Schema-Stand) + relevante Tabellen-Sektion.
- **CI-/Release-Workflow-Änderungen** schlagen in `build-and-release.md`.
- **Neue Hardline** (selten, nur über ADR) schlägt in `gobd.md` oder
  `paragraph-19.md` und in `../ARCHITECTURE.md` §7.

---

## 8. Was diese Konvention NICHT ist

- **Kein** Style-Guide für `src/`-Code (das macht `cargo fmt` und `clippy`
  + die Rust-Standard-Konventionen). Code-Style ist nicht hier definiert.
- **Kein** Style-Guide für das User-Handbuch. Das hat eigene, strengere
  Regeln in `docs/RELEASE-1.0-GUIDE.md` G2-DOC.2.
- **Kein** Prosa-Manifest. Doku-Qualität misst sich daran, ob ein neuer
  Maintainer mit ihr arbeiten kann. Wenn ja, ist sie gut genug. Wenn nicht,
  wird sie ergänzt.

---

## Letzte Verifikation

Stand: 2026-05-26. Diese Konvention wurde bei der Konsolidierung der
Tech-Doku in G2-DOC.1 angelegt; sie spiegelt den Stil, der bereits in
ARCHITECTURE.md und den ADRs gelebt wird, und macht ihn explizit.
