# Handbook Resource Skeleton

> Build-zeitliche Ablage des **User-Handbuchs** (G2-DOC.2). Dieses
> Verzeichnis ist eine **Tauri-Resource**: alle Files landen über
> `bundle.resources` im NSIS/MSI-Paket und sind zur Laufzeit per
> `tauri::path::resource_dir()` (Rust) bzw. das asset-Protokoll
> (Frontend, in G2-DOC.3) lesbar.
>
> Die Tech-Doku (G2-DOC.1) liegt in `klein-buch/docs/` und ist **nicht**
> Bestandteil dieses Bundles.

## Verzeichnis-Layout

```
src-tauri/resources/handbook/
├── README.md          ← dieses Konventionsdokument (NICHT als Handbuch-Seite ausgespielt)
└── <slug>.md          ← jede Handbuch-Seite, flach
```

Das Handbuch ist **reine Textform** (Manuel-Entscheidung 2026-05-26).
Keine Screenshots, kein `img/`-Verzeichnis. Die UI wird in Worten
beschrieben statt gezeigt — Bilder würden bei jeder UI-Änderung veralten.

**Flach**, keine Unterordner pro Kategorie. Die Kategorie steht im
Front-Matter, nicht im Pfad. So bleibt das Umverteilen ohne Datei-Renames
möglich, und der Renderer (G2-DOC.3) baut den Sidebar-Baum aus den
Front-Matter-Daten.

## Front-Matter (Pflicht)

Jede `<slug>.md`-Datei beginnt mit einem YAML-Front-Matter-Block:

```yaml
---
slug: erste-schritte-passphrase
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

Genau diese sechs Werte, sonst schlägt der Verify-Test fehl:

```
erste-schritte         → Schritt-für-Schritt (G2-DOC.2.2)
bedienen               → Referenz je UI-Bereich (G2-DOC.2.3)
recht-und-steuern      → §-Erklärungen, GoBD, AfA, EÜR (G2-DOC.2.4)
faq                    → Häufige Fragen (G2-DOC.2.5)
troubleshooting        → SmartScreen, Sidecar, OAuth, Hash-Mismatch (G2-DOC.2.6)
glossar                → Fachbegriffe (G2-DOC.2.7) — Wahrheits-Single-Source
```

## Datei-Naming

- Datei `==` `<slug>.md`. Wenn der Slug `passphrase-einrichten` ist,
  heißt die Datei `passphrase-einrichten.md`.
- Slugs sind dauerhaft stabil. Sie werden in G2-DOC.4 für
  `<HelpAnchor slug="…">`-Deeplinks aus dem UI referenziert; eine
  Slug-Umbenennung bricht alle Anker.

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
  für Definitionen, Kapitel-Erklärungen müssen damit konsistent sein.
- §-Bezüge bleiben drin, immer mit einem Halbsatz übersetzt.
- Disclaimer wörtlich: "Klein.Buch ist ein Werkzeug, kein Steuerberater."

Beispiele und vollständige Liste der Anti-Muster: siehe
`docs/RELEASE-1.0-GUIDE.md`.

## Versionierungs-Footer (G2-DOC.2.9 vorbereitend)

Jede Handbuch-Seite endet mit einem Footer-Block der Form:

```markdown
---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
```

Datum im **deutschen Format** (TT.MM.JJJJ, Manuel-Entscheidung
2026-05-27 — die englische ISO-Schreibweise verwirrt deutsche
Endnutzer). App-Version aus `tauri.conf.json/version`. Der Renderer
(G2-DOC.3) zeigt zusätzlich die zur Laufzeit gelesene App-Version,
der hartkodierte Wert hier ist der dokumentierte Stand zur
Aktualisierung des Kapitels.

## Hinzufügen einer neuen Seite — Checkliste

1. Slug festlegen (kebab-case, eindeutig).
2. Datei `<slug>.md` in diesem Verzeichnis anlegen mit vollständigem
   Front-Matter (siehe oben).
3. Inhalt nach Ton-Hardline schreiben (Textform, keine Screenshots).
4. Versionierungs-Footer eintragen.
5. `cargo test -p klein-buch-app --test handbook_resources` laufen lassen
   (Front-Matter-Verify, Slug-Eindeutigkeit, Kategorien-Whitelist).

## Was hier nicht hingehört

- **Tech-Doku.** Liegt in `klein-buch/docs/`, ist Engineer-Ton, nicht
  Teil dieses Bundles.
- **ADRs.** `klein-buch/docs/adr/`.
- **PRD und QA-Handbuch.** Repo-Root.
- **inputs/-Pflege.** Das `inputs/`-Tabu greift nicht für diesen Pfad
  (Resource-Bundle), aber der Inhalt hier ist Benutzer-Doku, keine
  Domänen-Eingabe.
