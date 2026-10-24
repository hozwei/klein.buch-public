# Handbook Resource Skeleton

> Build-zeitliche Ablage des **User-Handbuchs** (G2-DOC.2). Der Ordnername
> `src-tauri/resources/` ist historisch (Desktop-Fassung); seit Phase W
> (ADR 0039) gibt es keine Tauri-Resource mehr. Eingebunden wird das
> Handbuch beim Build der SPA: `src/lib/handbook/index.ts` lädt alle
> `*.md`-Dateien dieses Ordners per Vite-Glob (`import.meta.glob`, `?raw`,
> eager) in das Frontend-Bundle, `src/lib/handbook/render.ts` löst Bilder
> aus `img/` ebenso auf. Der Server liefert das Handbuch damit als Teil der
> SPA unter `/help/<slug>` aus; zur Laufzeit liest niemand diesen Ordner.
> Das Dockerfile kopiert ihn deshalb nur in die Node-Stufe (SPA-Build).
>
> Eingebunden werden nur `*.md` (außer dieser README) und Bilder unter
> `img/`. Andere Dateien in diesem Ordner liefert die SPA nicht aus.
>
> Die Tech-Doku (G2-DOC.1) liegt in `klein-buch/docs/` und ist **nicht**
> Bestandteil des Handbuchs.

## Verzeichnis-Layout

```
src-tauri/resources/handbook/
├── README.md          ← dieses Konventionsdokument (NICHT als Handbuch-Seite ausgespielt)
├── img/               ← muss existieren (Test), bleibt leer (reine Textform)
└── <slug>.md          ← jede Handbuch-Seite, flach
```

Das Handbuch ist **reine Textform** (Manuel-Entscheidung 2026-05-26).
Keine Screenshots. Das Verzeichnis `img/` bleibt nur als leerer Ordner
bestehen, weil `handbook_resources_test` es verlangt und der Renderer
`![…](img/<name>)` auflösen könnte. Die UI wird in Worten beschrieben
statt gezeigt, Bilder würden bei jeder UI-Änderung veralten.

**Flach**, keine Unterordner pro Kategorie. Die Kategorie steht im
Front-Matter, nicht im Pfad. So bleibt das Umverteilen ohne Datei-Renames
möglich, und der Renderer (G2-DOC.3) baut den Sidebar-Baum aus den
Front-Matter-Daten.

## Front-Matter (Pflicht)

Jede `<slug>.md`-Datei beginnt mit einem YAML-Front-Matter-Block:

```yaml
---
slug: passphrase-einrichten
title: Passphrase einrichten
category: erste-schritte
order: 20
keywords: [passphrase, login, verschlüsselung, sqlcipher, backup]
---
```

| Feld       | Pflicht | Form                                                    |
|------------|---------|---------------------------------------------------------|
| `slug`     | ja      | kebab-case, eindeutig im gesamten Verzeichnis. Dateiname `==` `<slug>.md`. |
| `title`    | ja      | deutscher Titel, wird als Seiten-Headline + Sidebar-Label benutzt. |
| `category` | ja      | exakt einer der erlaubten Werte (siehe unten).         |
| `order`    | ja      | Integer; Sortierschlüssel innerhalb der Kategorie (kleinere Werte zuerst). 10er-Schritte empfohlen, damit später ohne Renumerieren eingefügt werden kann. |
| `keywords` | ja      | Liste deutscher Begriffe für die Volltextsuche (G2-DOC.3.3). Mindestens 3 Einträge. |

Zusätzliche Felder sind erlaubt (zum Beispiel ein späteres
`anchor_targets: [...]` für G2-DOC.4), werden aber vom Front-Matter-
Verify-Test nur durchgewinkt, nicht erzwungen.

## Erlaubte Kategorien

Genau diese sieben Werte, sonst schlägt der Verify-Test fehl. Die
Reihenfolge ist die Anzeigereihenfolge im Kapitelbaum; Liste und Labels
stehen in `src/lib/handbook/categories.ts`, die Whitelist des Tests in
`src-tauri/tests/handbook_resources_test.rs` (beide synchron halten,
ebenso die Objekte je Kategorie in `src/routes/help/+layout.svelte`):

```
erste-schritte         → Schritt-für-Schritt (G2-DOC.2.2)
bedienen               → Referenz je UI-Bereich (G2-DOC.2.3)
recht-und-steuern      → §-Erklärungen, GoBD, AfA, EÜR (G2-DOC.2.4)
faq                    → Häufige Fragen (G2-DOC.2.5)
troubleshooting        → Fehlerbehebung in der Bedienung: Sidecar, Backup,
                         Wiederherstellen, Hash-Mismatch, Rechnungs-Eingang (G2-DOC.2.6)
glossar                → Fachbegriffe (G2-DOC.2.7) — Wahrheits-Single-Source
betrieb                → für Betreiber der Docker-Instanz: Installation, Reverse-Proxy,
                         Volumes, TOTP-Wiederherstellung, Updates, Fehlersuche im
                         Container, Lizenz (W13, ADR 0039)
```

Kapitel der Kategorie `betrieb` dürfen technischer sein (Befehle,
Umgebungsvariablen, Compose-Ausschnitte), bleiben aber in der Ton-Hardline:
Fachwörter beim ersten Auftreten erklären, Fließtext statt Bullet-Wüste.
Beispieladressen nur `example.org` und die Dokumentationsnetze
`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`; jede genannte
Variable und Option muss es im Code geben.

## Datei-Naming

- Datei `==` `<slug>.md`. Wenn der Slug `passphrase-einrichten` ist,
  heißt die Datei `passphrase-einrichten.md`.
- Slugs sind dauerhaft stabil. Sie werden in G2-DOC.4 für
  `<HelpAnchor slug="…">`-Deeplinks aus dem UI referenziert; eine
  Slug-Umbenennung bricht alle Anker (`handbook_anchors_test` prüft das).

## Inhalt — Ton

Der **Ton-Hardline-Block** in `docs/RELEASE-1.0-GUIDE.md` (§G2-DOC.2,
ab "Zielgruppe sind Menschen ohne steuerliches und ohne IT-Vorwissen")
ist verbindlich für jede Seite. Kurzfassung der harten Regeln:

- Anrede: **Du** (großgeschrieben in direkter Anrede).
- Keine em-dashes als Stilmittel.
- Keine Bullet-Wüsten, keine Fett-Inflation, keine Emojis,
  keine Floskeln, keine Deko-Zwischenüberschriften.
- Jede Abkürzung und jedes Fachwort beim ersten Auftreten im Kapitel
  ausschreiben und erklären. Das Glossar ist die Single-Source-of-Truth
  für Definitionen, Kapitel-Erklärungen müssen damit übereinstimmen.
- §-Bezüge bleiben drin, immer mit einem Halbsatz übersetzt.
- Disclaimer wörtlich: "Klein.Buch ist ein Werkzeug, kein Steuerberater."
- Keine Begriffe der früheren Desktop-Fassung (Begriffsliste im Done-Check
  von Block W13 in `docs/WEB-DOCKER-PLAN.md`; der grep über diesen Ordner
  bleibt leer, auch über diese README).

Beispiele und vollständige Liste der Anti-Muster: siehe
`docs/RELEASE-1.0-GUIDE.md`.

## Versionierungs-Footer (G2-DOC.2.9 vorbereitend)

Jede Handbuch-Seite endet mit einem Footer-Block der Form:

```markdown
---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.10*
```

Datum im **deutschen Format** (TT.MM.JJJJ, Manuel-Entscheidung
2026-05-27 — die englische ISO-Schreibweise verwirrt deutsche
Endnutzer). Die Version ist die CalVer-Veröffentlichung (`JJJJ.M`, ohne
Patch-Stelle), für die das Kapitel zuletzt überarbeitet wurde; die
laufende Version steht in `version` von `src-tauri/Cargo.toml`.
`handbook_footer_test` prüft nur das Format (`MAJOR.MINOR[.PATCH]`).

## Hinzufügen einer neuen Seite — Checkliste

1. Slug festlegen (kebab-case, eindeutig).
2. Datei `<slug>.md` in diesem Verzeichnis anlegen mit vollständigem
   Front-Matter (siehe oben).
3. Inhalt nach Ton-Hardline schreiben (Textform, keine Screenshots).
4. Versionierungs-Footer eintragen.
5. Aus `src-tauri/` die Handbuch-Tests laufen lassen (im Container nur in
   der Arbeitskopie, nie im Projektordner):
   `cargo test --test handbook_resources_test --test handbook_footer_test --test handbook_anchors_test`
   (Front-Matter-Verify, Slug-Eindeutigkeit, Kategorien-Whitelist, Footer,
   HelpAnchor-Slugs).

## Was hier nicht hingehört

- **Tech-Doku.** Liegt in `klein-buch/docs/`, ist Engineer-Ton, nicht
  Teil des Handbuchs.
- **ADRs.** `klein-buch/docs/adr/`.
- **PRD und QA-Handbuch.** Repo-Root.
- **inputs/-Pflege.** Das `inputs/`-Tabu greift nicht für diesen Pfad,
  aber der Inhalt hier ist Benutzer-Doku, keine Domänen-Eingabe.
