# Designleitlinie

> Stand U4 (Lauf 2026.11, 26.10.2026, Korrekturrunde X und Design-Runde V3). Grundlage:
> `src/lib/styles/tokens.css`, die Bausteine in `src/lib/`, die Musterseite `/design` (zeigt
> jeden Baustein dieser Leitlinie, §9). Alles hier ist **Ist** (im Code), außer wo
> ausdrücklich „Übernahme durch die Seiten“ steht. Entscheidungen mit Begründung: ADR 0041.

## 1. Zweck und Geltung

- **Design ändert sich nur über Tokens und Bausteine** (`tokens.css`, `src/lib/*.svelte`), nie per
  Einzelkorrektur je Seite. Fehlt ein Baustein, wird er angelegt; Seiten setzen keine eigenen
  Farben (Hex-Werte), Schriftgrößen in px oder Rahmen für Felder und Knöpfe.
- **Vorrang:** GoBD-/§19-Hardlines → Barrierefreiheit (orientiert sich an WCAG 2.2 AA) → deutsche
  Standards (DIN 5008) → Konsistenz mit dem Design-System → Geschmack.
- Keine Konformitätsaussagen in öffentlichen Texten; erlaubt: „orientiert sich an WCAG 2.2 AA,
  geprüft mit axe-core und manuell“.

## 2. Charakter und Prinzipien

Ein ruhiges, präzises Werkzeug für Belege und Zahlen — ein **sorgfältig geführtes Kontorbuch**,
kein Marketing-Dashboard.

1. **Zahlen sind das Gestaltungselement.** Tabellarische Ziffern, Beträge rechtsbündig, saubere
   Fluchten; Belegnummern fallen als Monospace auf und brechen nie um.
2. **Klare Typo-Hierarchie.** Ein Seitentitel (`h1` in der PageBar), Abschnitte als `h2` in
   Karten, sonst Fließtext. Sechs Rollen genügen (§3.2).
3. **Wenig Farbe, bewusst eingesetzt.** Petrol für Marke, Primäraktion und Links; Statusfarben
   nur für Status. Status immer mit Farbe **und** Text oder Symbol.
4. **Lineatur statt Kästen.** Feine Linien trennen Zeilen; Karten nur, wo sie gruppieren;
   Schatten nur für schwebende Ebenen (Menü, Toast, Dialog).
5. **Eine Höhe, eine Kante.** Knöpfe, Text-, Datums- und Auswahlfelder sind gleich hoch
   (40 px, Touch 44 px) und haben dieselbe Kante (8 px Radius, Rahmen ≥ 3:1).
6. **Weißraum mit Zweck.** Abstände aus der 4-px-Skala; Desktop dichter als Mobil.
7. **Ruhe statt Effekt.** Bewegung nur zur Orientierung (≤ 200 ms), `prefers-reduced-motion`
   schaltet sie ab.
8. **Mobil ist kein verkleinerter Desktop** (§6): Hauptaktion mit dem Daumen erreichbar,
   Nebenaktionen im Menü „Mehr“.

## 3. Tokens

`tokens.css` hat drei Ebenen: **Palette** (`--c-*`, Rohmaterial, für Seiten noch gültig),
**Rollen** (Bausteine nutzen nur diese) und **Skalen**. Ein späterer Dark Mode ist ein zweiter
Rollensatz (Flächen, Text, Linien, Akzent, Status, Fokus) plus das Pfeil-Bild der Auswahlfelder
(Daten-URL mit fester Farbe) — kein Umbau. Dark Mode ist **nicht** Ziel von 2026.11.

### 3.1 Rollen (Farben)

| Gruppe | Rollen | Wert |
|---|---|---|
| Fläche | `--surface-page` · `-card` · `-sunken` · `-hover` · `-row-hover` | `#f4f7f9` · `#fff` · `#fbfcfd` · `#eaf3f6` · `#edf4f7` |
| Fläche invers | `--surface-inverse` (Seitenleiste, Kopfleiste) · `-inverse-hover` · `-inverse-active` | `#0d4150` · Weiß 7 % · Weiß 13 % |
| Abdunklung | `--overlay` (hinter Dialogen und Drawer, ohne Weichzeichner) | `rgba(16,36,44,.55)` |
| Text | `--text` · `--text-muted` · `--text-subtle` · `--text-on-accent` · `--text-inverse` · `--text-inverse-muted` | `#16242c` · `#5a6b75` · `#5f717c` · Weiß · Weiß · `#bcd4dc` |
| Linie | `--line` (Lineatur, dekorativ) · `--line-control` (Bedienelement-Grenze) · `--line-hover` · `--line-inverse` · `--line-inverse-control` | `#e3e9ed` · `#788a94` · `#6fa9bb` · Weiß 12 % · Weiß 45 % |
| Linie (Kurzform) | `--rule` · `--rule-control` · `--rule-sum` (doppelte Linie über Summen) | `1px solid` · `1px solid` · `3px double` |
| Akzent | `--accent` · `--accent-hover` · `--accent-soft` · `--accent-on-soft` | `#176b87` · `#115468` · `#eaf3f6` · `#115468` |
| Status | `--status-{success,warning,danger,info,neutral,primary}-{bg,fg,line}` | s. 3.3 |
| Gefahr voll | `--danger-solid` · `--danger-solid-hover` | `#a02832` · `#861f28` |
| Fokus | `--focus-ring` · `--focus-ring-inverse` · `--focus-width` | `#1f7e9a` · Weiß · 2 px |
| Fokus-Abstand | `--focus-offset` · `--focus-offset-tight` · `--focus-offset-inset` | 2 px (Standard) · 1 px (Felder, Seitenleiste) · −2 px (Menüeinträge, Editor-Textfeld, Scrollbereich im Dialog) — keine anderen Werte (U4, V4-13) |

### 3.2 Typo, Abstände, Größen, Radien, Schatten, Bewegung

| Gruppe | Tokens | Wert / Verwendung |
|---|---|---|
| Typo-Rollen | `--type-display` | 24 → 34 px (fließend bis ~720 px): derzeit ungenutzt — keine Hero-Flächen; das Datum der Startseite steht in `--type-heading` (V3-03) |
| | `--type-figure` | 22 → 28 px: Kennzahlen (`.kb-metric .val`, höchstens 13 % der Kachelbreite, §4.4) |
| | `--type-title` | 20 → 22 px: Seitentitel (`h1` der PageBar) |
| | `--type-heading` | 18 px: `h2` in Karten, Dialogtitel |
| | `--type-body` | 14 px: Fließtext, Tabellen, Felder am Desktop |
| | `--type-small` | 13 px: Labels, Hinweise, Metazeilen |
| | `--type-caption` | 12 px: Spaltenköpfe (Versalien), Badges, Karten-Labels |
| Zeilenhöhe | `--lh-tight` · `--lh-snug` · `--lh-body` | 1,2 (Knöpfe, Kennzahlen) · 1,3 (Überschriften, Labels) · 1,55 (Fließtext) |
| Kompatibilität | `--fs-xs … --fs-3xl` | zeigen auf die Rollen (`--fs-md` 16 px = Eingaben im Drawer-Band) |
| Schrift | `--font-sans` · `--font-display` · `--font-mono` | System-Schriften, kein Web-Font (local-first) |
| Abstände | `--sp-1 … --sp-7` | 4 · 8 · 12 · 16 · 24 · 32 · 48 px |
| Trefferfläche | `--target-min` · `--target-touch` · `--target` | 24 · 44 px · gilt: 24 px, im Drawer-Band 44 px |
| Bedienhöhe | `--control-h` | 40 px (2,5rem), im Drawer-Band 44 px: Knöpfe, Text-, Datums-, Auswahlfelder |
| Feldbreiten | `--w-short` · `-date` · `-amount` · `-medium` · `-long` | 8 · 11 · 12 · 20 · 32 rem (Höchstbreiten, §4.8) |
| Radien | `--radius-control` · `--radius-card` · `--radius-pill` | 8 px (Knöpfe, Felder, Banner, Menü) · 12 px (Karten, Tabellen, Dialoge) · rund (Badges) |
| Radien (Skala) | `--r-sm` · `-md` · `-lg` · `-xl` | 6 · 8 · 12 · 16 px (U4: vorher 8/12/16/20) |
| Schatten | `--shadow-card` · `--shadow-overlay` · `--shadow-modal` | Haarlinie (Karten) · Menü, Toast, Sprunglink · Dialog, Drawer. Die Rohwerte `--sh-sm/-lg/-xl` stehen nur in `tokens.css` (`--sh-md` entfernt, V3-20) |
| Bewegung | `--t-fast` · `--t-base` · `--ease-apple` | 140 ms · 200 ms; nichts darüber (`--t-slow` mit 280 ms entfernt, V3-06), keine Endlos-Animation |
| Layout | `--main-pad` · `--sticky-top` · `--pagebar-h` · `--toast-h` · `--content-max` · `--form-max` | 2rem / 1rem · Kopfleiste · per ResizeObserver · per ResizeObserver · 100rem · 72rem |

Breakpoints sind keine Tokens (CSS-Variablen gehen in Media Queries nicht); die Werte stehen in §6
und nur dort sowie in `tokens.css`/`+layout.svelte`.

### 3.3 Kontraste (nachgerechnet, WCAG-Formel)

| Paar | Verhältnis | Anforderung |
|---|---|---|
| `--text` auf page / card / sunken / row-hover | 14,76 / 15,87 / 15,45 / 14,28 | 4,5 ✓ |
| `--text-muted` auf page / card / sunken / row-hover / accent-soft | 5,14 / 5,53 / 5,39 / 4,98 / 4,91 | 4,5 ✓ |
| `--text-subtle` auf page / card / sunken / row-hover | 4,72 / 5,07 / 4,94 / 4,56 | 4,5 ✓ |
| Text auf Status-Flächen (U4, V4-11): `--text-muted` auf danger / warning / success / info / neutral | 4,81 / 5,03 / 4,96 / 4,91 / 4,88 | 4,5 ✓ |
| — nicht textfähig: `--text-subtle` auf danger / neutral / primary-bg · `--text-muted` auf primary-bg | 4,40 / 4,47 / 3,92 · 4,28 | ✗ — dort `--text` oder das passende `-fg` |
| Link `--accent` auf page / card / row-hover | 5,59 / 6,02 / 5,41 | 4,5 ✓ |
| `--accent-on-soft` (Sekundärknopf) auf card / page / accent-soft | 8,43 / 7,83 / 7,48 | 4,5 ✓ |
| Weiß auf `--accent` / `--accent-hover` (Primärknopf, Hinweis-Zähler) | 6,02 / 8,43 | 4,5 ✓ |
| Weiß auf `--danger-solid` / `-hover` (Gefahr bestätigen, Zähler mit dringendem Hinweis) | 7,38 / 9,37 | 4,5 ✓ |
| Status fg auf bg: danger · warning · success · info · neutral · primary | 6,41 · 5,09 · 4,80 · 7,48 · 5,66 · 8,60 | 4,5 ✓ |
| Status fg auf card: danger · warning · success | 7,38 · 5,60 · 5,35 | 4,5 ✓ |
| `--line-control` auf card / sunken / page | 3,58 / 3,49 / 3,33 | 3 ✓ (1.4.11) |
| Fokusring auf card / page / sunken / accent-soft / row-hover / danger-bg / warning-bg | 4,66 / 4,33 / 4,53 / 4,14 / 4,19 / 4,04 / 4,23 | 3 ✓ |
| Weiß auf `--surface-inverse` / inverse-hover / inverse-active | 11,12 / 9,13 / 7,59 | 4,5 ✓ |
| Aktuelle Seite in der Seitenleiste: Balken `--text-inverse` gegen Leiste / aktive Fläche (U4, V4-12) | 11,12 / 7,59 | 3 ✓ (1.4.11; die Fläche allein hob sich nur 1,46:1 ab) |
| `--text-inverse-muted` auf inverse / inverse-hover | 7,19 / 5,91 | 4,5 ✓ |
| `--line-inverse-control` (Knopfkante „Menü schließen“) auf inverse | 3,58 | 3 ✓ |
| Pfeil der Auswahlfelder (`#5a6b75`) auf card | 5,53 | 3 ✓ |
| `--line` auf card | 1,22 | nur Lineatur, nie Bedienelement-Grenze |
| Statuskanten (`-line`): danger · success · warning · info | 4,35 · 3,16 · 2,69 · 2,60 | dekorativ; Text trägt den Status |
| `--c-success-500` / `--c-warning-500` / `--c-accent-500` auf Weiß | 3,16 / 2,69 / 2,70 | nie als Textfarbe |

Gesperrte Bedienelemente (`opacity: .5`) sind nach 1.4.3 ausgenommen; ihr Grund steht als Text.

**Regel Text auf Status-Flächen** (Zeilen fehlgeschlagener Backups/Versände, Banner, Toasts): nur
`--text`, `--text-muted` (nicht auf `--status-primary-bg`) oder das passende `--status-*-fg`; nie
`--text-subtle`. Die Karten-Labels der Tabellen (`td[data-label]::before`) nutzen deshalb
`--text-muted`.

## 4. Komponenten-Regeln

### 4.1 Knöpfe (`Button`, `.btn-*`)

- **Eine Größe für alle** (Manuel-Vorgabe 2026-05-26): Höhe `--control-h`, Innenabstand 8/16 px,
  Schrift 600. `size="sm"`/`.btn-sm` bleiben wirkungslos. Rohes `<button class="btn-primary">`
  und `Button` sehen gleich aus und verhalten sich gleich (Übergang `--t-base` mit
  `--ease-apple`, kein Eindrücken beim Klick, V3-21). Auch Toast-Aktionen und Textvorschläge
  („Bezahlt per …“) sind normale `secondary`-Knöpfe — keine Pillen, keine 32-px-Knöpfe.
- **Höchstens eine `primary` je Bereich** (PageBar, Dialog, Formular, Karte). Sie steht zuletzt
  (rechts) und heißt wie das Ergebnis („Rechnung ausstellen“, nicht „OK“).
- **`secondary`** für jede weitere Aktion mit eigenem Ergebnis (Öffnen, Senden, PDF, Bearbeiten).
  Standard für Aktionen in Karten und Tabellenzeilen.
- **`ghost`** nur für leichte Nebenaktionen **neben** einer sichtbaren `primary`/`secondary`
  derselben Gruppe (Abhaken, Zuklappen, „Mehr“ in Tabellenzeilen); nie allein, nie in der
  PageBar, nie für Navigation (U1-068). „Alle abhaken“ in der PageBar ist `secondary`.
- **`danger`** für Gefahraktionen (Stornieren, Verwerfen, Archivieren, Anonymisieren); nie in
  Primär-Optik, in der PageBar im Menü „Mehr“ unten (§4.3). Bestätigen im Gefahr-Dialog ist
  `--danger-solid`.
- **Gesperrt** (`disabled`) nur mit sichtbarem Grund in der Nähe (`ariaDescribedby`). Während eines
  Ladevorgangs `ariaDisabled` statt `disabled`, damit der Fokus bleibt (ADR 0041 Nr. 33).
- **Aktion läuft:** `<Button busy={saving}>` setzt `aria-disabled` und `aria-busy`; Klick und Absenden
  sind wirkungslos, der Knopf behält den Fokus, der Handler prüft trotzdem `if (saving) return`.
  Rohe Knöpfe: `aria-disabled={saving || undefined} aria-busy={saving || undefined}`. Felder während
  der Anfrage `readonly` statt `disabled`. Nach einer Ablehnung geht der Fokus aufs erste betroffene
  Feld bzw. auf das Banner mit den Gründen, nie auf `<body>` (V2-06, V4-03).
- **Gruppen** mit `.kb-actions` (§4.8): gleicher Abstand, umbrechend; `.kb-stack` für
  untereinander stehende Aktionen in gleicher Breite (U3-036).

### 4.2 PageBar

- Je Seite genau eine: Zurück (Liste bzw. übergeordnete Seite), Titel (`h1`), rechts die Aktionen.
  Klebt ab 601 px Breite und 501 px Höhe; meldet ihre Höhe als `--pagebar-h` (2.4.11).
- **Seitentitel** (2.4.2, U4): Der `title` geht zugleich als „<Titel> – Klein.Buch“ in
  `document.title` (`pageTitle.svelte.ts`, Root-Layout). Ohne `title` (Detailseite lädt) zeigen
  `h1` und Seitentitel den Ersatztitel der Route („Rechnung“), nie keine Überschrift (V4-08).
  Seiten ohne PageBar: Handbuch (Kapiteltitel), Anmelde-Maske (Schritt), Fehlerseite.
- Gesperrte Nebenaktionen (`secondary` mit `disabled`) bleiben fokussierbar (`aria-disabled`).
- Belegnummern im Titel erscheinen automatisch als `.kb-docno` (Monospace, ohne Umbruch).
- **Aktionen getrennt nach Gewicht** (U1-059):
  - `actions` (Snippet): die Hauptaktion (eine `primary`), rechts außen.
  - `secondary` (Daten): Nebenaktionen; ab 601 px als `secondary`-Knöpfe, bis 600 px im Menü.
  - `more` (Daten): Seltenes und Gefahraktionen, immer im Menü „Mehr“.
  - Bis 600 px stehen damit höchstens **Hauptaktion + „Mehr“** in der Leiste.
- Lange Formulare wiederholen die Hauptaktion am Ende (`.kb-form-end`, nur sichtbar, wenn die
  PageBar nicht klebt).

### 4.3 Menü „Mehr“ (`ActionMenu`)

- Muster „Menu Button“ (WAI-ARIA APG): Knopf mit `aria-haspopup="menu"`, `aria-expanded`,
  `aria-controls`; Enter/Leertaste/Pfeil runter öffnen mit Fokus auf dem ersten Eintrag, Pfeil
  hoch auf dem letzten; im Menü Pfeiltasten (rundum), Pos1/Ende, Anfangsbuchstaben; Escape
  schließt und gibt den Fokus an den Knopf zurück; Tab schließt und geht vom Knopf aus weiter;
  Klick daneben schließt.
- Einträge: `{ label, onSelect | href, danger?, disabled?, hint?, download? }`. Gefahraktionen
  stehen unten hinter einer Trennlinie in Gefahrfarbe; gesperrte Einträge bleiben erreichbar
  (`aria-disabled`) und zeigen den Grund als Zusatzzeile (`hint`).
- Trefferflächen: Knopf `--control-h`, Einträge ≥ 36 px (Drawer-Band 44 px).
- Lage per Popover-API in der obersten Ebene (kein Abschneiden in Tabellen-Scrollbereichen),
  unter dem Knopf oder darüber, nie aus dem Fenster; schließt, wenn der Knopf aus dem Bild scrollt.
- In Tabellenzeilen: `variant="ghost"` neben einer sichtbaren Aktion, `ariaLabel` mit Bezug
  („Mehr zu RE-2026-0042“, beginnt mit dem sichtbaren Text).

### 4.4 Karten (`Card`, `.card`)

- Weiße Fläche, Haarlinie `--rule`, Radius 12 px, Schatten `--shadow-card` (Haarlinie), Innenabstand
  24 px (≤ 900 px 16 px). Erstes Kind `h2` = Abschnittstitel (`--type-heading`).
- **Nur wo sie gruppieren**: ein Abschnitt mit Überschrift, ein Formularteil. Keine Karte in der
  Karte; Listen stehen im `Table`-Baustein (eigene Karte), nicht zusätzlich in einer Card.
- `danger`: Abschnitt mit Gefahraktionen (rötliche Fläche, rote Kante).
- Kennzahlen: `.kb-metric` (Label `--type-small`, Wert `--type-figure`), keine eigenen Kacheln mit
  großem Radius und weichem Schatten. Der Wert bricht nie um (Betrag und € bleiben zusammen); die
  Kachel ist ein Container, der Wert höchstens 13 % ihrer Breite groß (`min(--type-figure, 13cqi)`),
  darum steht sie in einem Raster mit festen Spalten (V3-01). Texte statt Zahlen („Noch keine
  Kopie“) setzen auf der Seite eine eigene Größe und dürfen umbrechen. Startseite: bis 36rem
  Inhaltsbreite zwei Spalten, Backup über die volle Breite; ab 36rem drei, ab 66rem fünf (V3-02).
- **Linkliste** statt Einzelkarten (`.kb-linklist`, §4.8): Ziele mit Titel und Unterzeile in einer
  Karte, durch Linien getrennt — Schnellzugriff der Startseite, Einstellungen-Hub (V3-04). Listen
  von Einträgen mit Aktionen (Hinweise) stehen ebenso in einer Karte mit Lineatur, nicht als
  Kartenstapel (V3-05).

### 4.5 Tabellen (`Table`)

- Feine Lineatur (`--rule` je Zeile, keine Zebrastreifen, keine senkrechten Linien), Kopf
  `--type-caption` in Versalien, `--text-subtle`, linksbündig; Beträge `.kb-num` (rechtsbündig,
  tabellarische Ziffern) — an `th` und `td`, der Kopf steht dann über den Werten (V3-10).
  Summenzeile mit Linie oben (`tfoot`); die Gesamtsumme (Brutto, Summe der EÜR-Zeilen) als
  `<tr class="kb-sum">` mit doppelter Linie (`--rule-sum`, eine Regel in `tokens.css`, im
  Tabellenmodus und Druck; V3-13). Aufstellungen ohne Tabelle: `.kb-ledger` (§4.8).
- Gruppen einer Liste (Pakete je Kategorie) teilen dieselben festen Spaltenbreiten
  (`table-layout: fixed`, Breiten am `th`), damit die Fluchten über alle Gruppen gehen (V3-11).
- Listen stehen **immer** im Baustein (weiße Karte), nie frei auf dem Seitenhintergrund (U3-030).
- Bis 820 px Karten (`cards`, jede `<td>` mit `data-label`), 601–820 px zweispaltig mit
  `cardGrid` (nur Listen mit Detailseite). Breite Tabellen scrollen in sich (Name per `caption`).
- Nebenspalten mit `.kb-col-opt1..3` (Spaltenpriorität nach Inhaltsbreite, ADR 0041 Nr. 42/53).
- Aktionsspalte: Kopf nur für Screenreader („Aktion“), höchstens eine sichtbare Aktion
  (`secondary`) plus `ActionMenu` (`ghost`) — nicht drei Knöpfe untereinander.

### 4.6 Belegnummern (`DocNo`, `.kb-docno`)

- Monospace (`--font-mono`, 0,95 em, Gewicht 500, tabellarische Ziffern), **nie umbrechend**.
- Als Link **immer unterstrichen** (feine Linie in `--line-hover`, beim Zeigen 2 px in Linkfarbe),
  in jeder Liste gleich (U3-030); Trefferfläche ≥ `--target`, im Fließtext `inline`.
- `muted` für stornierte/abgegangene Belege (Status trägt das Badge daneben); ohne Nummer
  (Entwurf) `placeholder`.
- `splitDocNumbers(text)` markiert Nummern in beliebigem Text (Titel, Hinweise).

### 4.7 Formulare (`FormField`, `MoneyInput`, `QuantityInput`, `ContactSelect`, `Toggle`)

- **Immer sichtbares Label** über dem Feld; Platzhalter ersetzen kein Label. Pflicht: `*` hinter
  dem Label (`aria-hidden`), `required`/`aria-required`, Legende „Felder mit * sind Pflicht“.
- Hinweis unter dem Feld, Fehler am Feld (`aria-describedby`, `aria-invalid`), außerhalb des
  Labels; erst nach dem ersten Absenden bzw. beim Verlassen; Fokus danach aufs erste fehlerhafte
  Feld (ADR 0041 Nr. 22). Fehlertext sagt, was zu tun ist. **Der Hinweis bleibt bei einem Fehler
  stehen** (er ist die Korrekturhilfe, 3.3.3): Fehler darüber, `aria-describedby` nennt Fehler und
  Hinweis — so in `FormField`, `MoneyInput`, `QuantityInput`, `ContactSelect` (U4, V4-14).
- **Pflicht-Legende** über dem Formular nur über `<RequiredLegend />`: „Felder mit * sind Pflicht.“,
  vorgelesen „Felder mit Stern sind Pflicht.“ (U4, V2-18; ersetzt „* Pflichtfeld“ und die
  Seitenklassen `req-legend`, `legend-req`, `legend` — Übernahme durch die Seiten).
- **Gleiche Höhe**: alle einzeiligen Felder `--control-h` (U3-033), Auswahlfelder mit eigenem
  Pfeil und Kante (keine Browser-Optik, U1-062), lange Optionstexte enden mit „…“ (U3-032) —
  besser: Feld so breit wie der längste Text oder volle Zeile (`.kb-span-all`).
- **Breite nach Inhalt** (`.kb-w-*`, §4.8): Datum `.kb-w-date`, Betrag `.kb-w-amount`, PLZ/Menge/km
  `.kb-w-short` (U3-031) — kein Datumsfeld über 1.000 px. An `MoneyInput`/`QuantityInput` über die
  Prop **`fieldClass`** (`<MoneyInput fieldClass="kb-w-amount" …>`): Sie begrenzt nur die Feld-Box;
  Label, Hinweis und Fehler behalten die Breite des Bausteins und brechen nicht in einer 12rem-Spalte
  um. `class` setzt dagegen den ganzen Baustein (Rasterplatz, `kb-span-all`).
- **Einheiten der Positionen**: Auswahl im Klartext (`unitOptions()`). Steht ein Code nicht in
  `UNIT_CODES` (Freitext älterer Pakete wie „Pauschal“, den die KoSIT-Prüfung beim Ausstellen
  ablehnt), bleibt er als Option sichtbar, das Feld meldet `unitCodeError()` mit Vorschlag
  (`suggestUnit()`: „z. B. Pauschale“) sofort — der Wert stammt aus den Daten, nicht aus einer
  Eingabe —, und Speichern springt dorthin. Eine noch an ein Paket
  gebundene Position bekommt dafür ein Auswahlfeld; die Wahl löst die Bindung (sauberer Bruch wie
  „Paket anpassen“), der Fokus bleibt auf dem Feld. Gilt in allen vier Positions-Editoren.
- Raster `.kb-form-grid` (höchstens zwei Spalten ab 37rem eigener Breite), Textfelder und Hinweise
  `.kb-span-all`; zusammengehörige Felder nebeneinander (PLZ und Ort).
- Beträge nur mit `MoneyInput`, Mengen mit `QuantityInput` (DIN-Leseregel, Integer-Cent,
  ADR 0041 Nr. 15/16). Schalter (`Toggle`, `role="switch"`) für Ein/Aus mit sofortiger Wirkung,
  Checkbox für Formularwerte.
- **Kontaktauswahl** `ContactSelect`: natives Auswahlfeld, ab 8 Kontakten mit Suchfeld darüber
  (Name oder Ort, Umlaute egal, Trefferzahl wird angesagt; Enter übernimmt einen einzigen Treffer).
  Deutsche Sortierung. Der gewählte Kontakt bleibt sichtbar, auch wenn die Suche ihn ausblendet.
- Autofill nach Plan §4.2 (eigene Daten mit Tokens, Daten Dritter und IBAN/BIC/USt-IdNr. `off`).

### 4.8 Hilfsklassen (global, `tokens.css`)

| Klasse | Wirkung |
|---|---|
| `.kb-actions` (+ `.kb-start`, `.kb-stack`) | Aktionsgruppe: nebeneinander rechts, links, oder untereinander gleich breit |
| `.kb-w-short/-date/-amount/-medium/-long` | Höchstbreite eines Felds; an MoneyInput/QuantityInput über `fieldClass` (nur die Feld-Box, §4.7) |
| `.kb-form-grid`, `.kb-span-all`, `.kb-sections` | Formular-Raster (ADR 0041 Nr. 41) |
| `.kb-form-end` | Wiederholung der Hauptaktion am Formularende (≤ 600 px Breite, ≤ 500 px Höhe) |
| `.kb-col-opt1..3`, `.kb-card-grid` | Spaltenpriorität, Kartenraster (über `Table cardGrid`) |
| `.kb-docno`, `.kb-num`, `.kb-nowrap` | Belegnummer, Betrag (auch Spaltenkopf), kein Umbruch („z. B.“, Zahl + Einheit, U3-037) |
| `.kb-link` | einzige Link-Klasse: unterstrichen (ADR 0041 Nr. 31), auch auf `<button>` für Aktionen im Text; `.txt-link`/`.inline-link` sind ersetzt (V2-21, V3-18) |
| `.kb-linklist` (+ `.kb-ll-title`, `.kb-ll-sub`) | Linkliste: `<ul>` mit Links aus Titel und Unterzeile in einer Karte, 1-px-Linien in `--line`, Spalten ab 18rem (Seiten dürfen feste Spaltenzahlen setzen), Fokusring innen (V3-04) |
| `.kb-ledger` (+ `.kb-sum`) | Aufstellung als `<dl>`: je `<div>` Posten (`dt`) und Betrag (`dd`, rechtsbündig, tabellarisch), Linie je Zeile, `<div class="kb-sum">` mit doppelter Linie darüber (EÜR, Abgang einer Anschaffung; V3-13) |
| `tr.kb-sum` | Summenzeile einer Tabelle mit doppelter Linie (`--rule-sum`, V3-13) |
| `.kb-loading` | Ladetext „Wird geladen …“ (mit `role="status"`; Baustein `LoadingState`) |
| `.kb-req-legend` | Pflicht-Legende (Baustein `RequiredLegend`) |
| `.kb-target` | einzeln stehender Link mit Mindest-Trefferfläche |
| `.kb-visually-hidden` | nur für Screenreader |
| `.kb-input` | Feld-Optik für Seiten ohne Baustein (Rahmen, Radius, Fläche) |

### 4.9 Status (`Badge`)

`Badge` mit **immer Text**, in zwei Arten (U4, V3-07/V3-08):

- **Status** (`kind="status"`, Standard): Zustand eines Belegs oder Objekts, Pille mit Punkt in
  Statusfarbe davor, `tone` nach der Tabelle. Das Wort beginnt groß und kommt aus dem
  Status-Helfer des Bereichs, damit derselbe Zustand überall gleich heißt: `invoiceStatusView`
  (`routes/invoices/invoiceStatus.ts`, auch Startseite), `quoteStatusView`, `expenseStatusView`
  (`routes/expenses/expenseStatus.ts`), `assetStatusView` (`routes/assets/assetStatus.ts`).
- **Merkmal oder Art** (`kind="tag"`): kein Status — immer neutral, **ohne Punkt**, kleine Ecke
  (`--r-sm`), lange Texte brechen um; `tone` gilt nicht. Der Punkt bedeutet damit allein Status
  (§2 Nr. 3). Beispiele: Belegart „Storno-Beleg“ und „RE/ST/K“ (Startseite), „Abo“, „§13b“, Modi
  („Automatisch“, „Erinnerung“, „Nur Entwurf“, „Automatisch ausstellen“), Bewegungsart,
  „80 % betrieblich“, „Sofort“, „Standard“ (Konto, Postfach), „Mitgeliefert“/„Eigene Vorlage“,
  „Diese Sitzung“, „Aus Paket: … (V2)“, „Abgang …“. Ausnahme mit Absicht: der Abo-Modus
  „Ausstellen + senden“ bleibt eine Warnung (Status-Badge `warning`, ADR 0041 Nr. 177).
- Statusmarken nur über `Badge`: keine eigenen Klassen (`task-badge`, `pkg-badge`, `.tag`,
  `recent-status` sind entfernt), keine Zeichen wie „✓/✗“ im Text (Versandstatus: Badge
  „Versendet“/„Fehlgeschlagen“ wie im E-Mail-Protokoll, V3-17). Ja/Nein-Werte sind Text.

| Zustand | tone |
|---|---|
| Entwurf, Noch keine AfA, Inaktiv, Archiviert, Abgeschrieben, Veräußert | `neutral` |
| Festgeschrieben / Ausgestellt / Fertiggestellt | `primary` |
| Versendet | `info` |
| Bezahlt, Angenommen, Aktiv, In Abschreibung | `success` |
| Teilbezahlt, Unbezahlt (Kosten), Pausiert mit Rückstand, Heute fällig | `warning` |
| Überfällig, Abgelehnt, Fehlgeschlagen | `danger` |
| Storniert, Verworfen | `neutral` + `strike` |

Kosten werden sofort festgeschrieben (ADR 0019); ihr Status sagt darum, ob bezahlt ist
(„Bezahlt“/„Unbezahlt“), nicht mehr „erfasst“ (V3-09). Statuspillen als Klassen (`.status-*`)
gibt es nicht mehr (U4 entfernt, ungenutzt).

**Hinweis-Zähler** in der Seitenleiste und am Menüknopf: Weiß auf `--accent`; nur wenn mindestens
ein dringender Hinweis offen ist, Weiß auf `--danger-solid` (Screenreader: „…, dringend“; V3-16).

### 4.10 Hinweise: Banner und Toast

- **Banner** (`Banner kind="error|warning|info|success"`, optional `title`) für Zustände, die
  bleiben, bis sie behoben sind: nicht gefunden, Firmendaten fehlen, Pause der Abo-Läufe,
  Server-Ablehnung mit mehreren Gründen. `error` = `role="alert"`, sonst `role="status"`.
  Einziger Baustein für Hinweisflächen (U4, V2-21): `href` + `linkLabel` für den nächsten Schritt als
  Link („Link-Banner“, z. B. fällige Kosten-Abos), Snippet `actions` für Knöpfe, `id` für
  `aria-describedby`. Links im Banner sind unterstrichen und tragen die Statusfarbe. Belegnummern
  im Text und in der fetten Kurzaussage über `DocNo` (die Kurzaussage dann als `<strong>` im
  Inhalt statt `title`, V3-23). Seiteneigene Flächen (`.info`, `.edit-hint`, `.due-banner`) und
  die Altklassen `.info-banner`, `.caveat`, `.danger-banner` ersetzen die Seiten durch `Banner`
  (Übernahme durch die Seiten; übernommen: Einrichtungs- und Anmeldemaske `BackupGate`, Vorschlag
  in „Neue Anschaffung“; offen: rund 25 Stellen in 14 Seiten, danach entfallen die Altklassen).
- **Toast** nur über `flash(text, kind, opts?)`: `ok` (Erfolg, schließt nach 5 s, Pause bei
  Zeiger/Fokus), `info` (neutral, z. B. „Abgebrochen – nichts geändert.“, schließt wie Erfolg),
  `warning` (gelungen, aber etwas braucht Aufmerksamkeit, z. B. „E-Mail-Protokoll unvollständig“;
  bleibt stehen, höflich angesagt), `error` (bleibt stehen, sofort angesagt). Toasts mit Aktion
  bleiben stehen: `flash("Kontakt archiviert.", "ok", { action: { label: "Rückgängig", run } })`.
  Jede Art trägt ein Symbol (Haken, „i“, Warndreieck, Kreuz) — Farbe ist nie allein (1.4.1); der
  Schließen-Knopf nennt die Art („Fehlermeldung schließen“). Die beiden Live-Regionen sind auch
  leer im Accessibility-Tree (V4-02). Toasts liegen unten und verdecken keine fokussierten Felder
  (`--toast-h`).

### 4.11 Dialoge

- `confirmDialog({ title, body, confirmLabel, danger })` für folgenreiche Aktionen; eigene Dialoge
  mit `.kb-dialog` + `use:focusTrap` (+ `portal` bei seiteneigenen Dialogen).
- Titel als Frage („Rechnung stornieren?“), Text nennt die Folge, Knopf = Verb. Gefahr-Dialoge:
  `alertdialog`, Startfokus „Abbrechen“, kein Schließen über den Hintergrund.
- Fuß: ab 601 px „Abbrechen“ zuerst, Hauptaktion rechts, Beschriftungen ohne Umbruch (reicht die
  Breite nicht, rückt ein ganzer Knopf in die nächste Zeile); bis 600 px untereinander in voller
  Breite, Hauptaktion oben, „Abbrechen“ unten. Die DOM-Reihenfolge folgt jeweils der sichtbaren
  (Tab- und Lesereihenfolge = Sichtreihenfolge, 1.3.2/2.4.3) — `ConfirmDialog` ordnet die Knöpfe je
  Breite neu statt `column-reverse`; der Fokus bleibt beim Wechsel auf seinem Knopf.
  Bis 600 px Vollbild-Blatt (`dvh`, Safe-Area), bis 420 px Höhe scrollt der ganze Dialog.
- Ansichts-Dialoge (PDF-Vorschau, Roh-XML): „Schließen“ ist `secondary` und steht zuerst wie
  „Abbrechen“; `primary` nur für ein echtes Ergebnis („XML kopieren“). In der PDF-Vorschau ist die
  Vorschau selbst das Ergebnis — im Fuß nur `secondary`; bis 600 px ist „PDF öffnen“ die einzige
  `primary`, „In neuem Tab öffnen“ entfällt dort (V3-15).
- Drei Aktionen (`chooseDialog` mit `altLabel`, z. B. „Verpasste Termine“): `--dialog-w: 36rem`,
  damit „Abbrechen“, „Ab heute fortsetzen“ und „Nachholen“ in eine Zeile passen (U2-019).

- **Re-Auth** (geschützte Aktionen, Plan W6): Der Dialog nennt die Aktion („Aktion bestätigen: Backup
  wiederherstellen“; Namen in `labels.ts`, `reauthActionLabel`, abgeglichen mit der Registry).
  Abbrechen ist kein Fehler: Der Transport wirft `REAUTH_CANCELED`, `flash` zeigt daraus
  „Abgebrochen – nichts geändert.“ als `info`, auch wenn die Seite `flash(String(e), "error")`
  aufruft (U4, V1-09).

### 4.12 `!important` (ADR 0041 Nr. 7, U4)

Abgebaut: Fokusring und Rahmenfarbe der Felder, alle globalen Knopfklassen (`.btn-*`, `.link`,
`.back`), PageBar-Knöpfe und Zurück-Knopf, Fokusring im Markdown-Editor. `tokens.css`: vorher
94 Deklarationen mit `!important` außerhalb von Druck und reduzierter Bewegung, jetzt 33 (davon 7 neu
für die Auswahlfelder); `PageBar.svelte` 24 → 0, `MarkdownEditor.svelte` 2 → 0. Bleibt, mit Grund:

| Regel | Grund |
|---|---|
| `.kb-visually-hidden` | Hilfsklasse, muss jede Element- und Seitenregel schlagen |
| Fehlerkante `[aria-invalid="true"]` | Zustand muss gegen seitenlokale `border`-Kurzschreibweisen gleicher Spezifität gewinnen |
| Pfeil und Pfeil-Abstand der Auswahlfelder | Seiten setzen `background:`/`padding:` als Kurzschreibweise; ohne `!important` verschwände der Pfeil bzw. Text liefe darunter |
| `th`/`td` (Lineatur, Kopf in Versalien, Innenabstand; Desktop-Dichte; Kartenansicht) | Seiten haben noch eigene `th`/`td`-Regeln (Fläche, Schriftgröße, Abstände; vor U4 24 Seiten, nach Phase 2 noch 22 Dateien, teils nur Ausrichtung wie `td.num`). Abbau, sobald sie entfernt sind |
| Schrift 16 px in Feldern im Drawer-Band | Seiten setzen Feldschriftgrößen (z. B. 14 px); unter 16 px zoomt iOS beim Fokus |
| Druck, `prefers-reduced-motion` | müssen alles überstimmen |

## 5. Muster

**Seitentitel und Begriffe.** Titel im Glossar-Begriff des Handbuchs: Listen im Plural
(„Rechnungen“), Detailseiten „Art + Nummer“ („Rechnung RE-2026-0042“), Editoren „Neue Rechnung“ /
„Entwurf RE-2026-0001 bearbeiten“. Der Seitentitel im Browser ist „<PageBar-Titel> – Klein.Buch“
(automatisch, §4.2). Du-Form mit kleinem „du“ wie in der Oberfläche, auch in Meldungen des
Servers; keine Floskeln.

| Glossar / Handbuch | Navigation | Seitentitel | Zurück |
|---|---|---|---|
| Rechnung, Storno-Beleg | Rechnungen | Rechnungen · Rechnung RE-… · Storno-Beleg ST-… | Rechnungen |
| Abo-Rechnung (Abo-Vorlage) | — (über Rechnungen) | Abo-Rechnungen · Abo-Vorlage | Abo-Rechnungen |
| Kosten, Kosten-Abo | Kosten | Kosten · Kosten KO-… · Kosten-Abos | Kosten |
| Anlage (Steuerbegriff) | Anschaffungen | Anschaffungen · Anschaffung AV-… | Anschaffungen |
| Privatbewegung | Privatbewegungen | Privatbewegungen · Privatbewegung PV-… | Privatbewegungen |
| Hinweise (offen / abgehakt) | Hinweise (Zähler „offen“) | Hinweise · Erinnerungen einstellen | Hinweise |
| Verkäuferprofil | — (Einstellungen) | Meine Firmendaten (in Meldungen „Firmendaten“) | Einstellungen |
| E-Mail-Protokoll | — (Einstellungen) | E-Mail-Protokoll | Einstellungen |

Stand U4: Navigation, Seitentitel-Ersatz (`pageTitle.svelte.ts`) und Server-Meldungen folgen der
Tabelle; Seitentitel und Zurück-Beschriftungen der einzelnen Seiten übernehmen die Seiten.

**Meldungen des Servers** (Toasts, Banner, Prüfliste): Begriffe der Oberfläche („Firmendaten“,
„Kosten“, „verworfen“ für Entwürfe, „storniert“ für festgeschriebene Belege, „E-Mail-Protokoll“),
Daten als TT.MM.JJJJ, keine Variantennamen und keine Regel-IDs der XRechnung im Text (die stehen im
Prüfbericht); gesetzliche Fundstellen („§14 Abs. 4 UStG“) bleiben. Prüf-Ablehnungen beginnen mit
„Die Rechnung kann nicht ausgestellt werden: “ (Storno: „Der Storno-Beleg …“, Angebot: „Das Angebot
kann nicht fertiggestellt werden: “), Entwurfs-Ablehnungen mit „Der Entwurf kann nicht gespeichert
werden: “; die Gründe sind mit „; “ getrennt (höchstens fünf, dann „… und N weitere Gründe“).

**Listen.** PageBar mit Hauptaktion „+ Neue …“ (`primary`), Nebenziele als `secondary`
(„Wiederkehrende Rechnungen“). Filterleiste darunter (Felder und Knöpfe gleich hoch, „Filtern“
`secondary`, unten bündig). Das Jahr ist eine Auswahl („Alle“ plus die Jahre der geladenen Belege
und das laufende Jahr, `yearChoices()` in `date.ts`), kein Zahlenfeld (U1-062); der erste Aufruf ist
ungefiltert, ein gefilterter lässt keine Jahre verschwinden. Tabelle im `Table`-Baustein: Nummer als
`DocNo`-Link, Partner, Beträge `.kb-num`, Status als Badge, eine Zeilenaktion + `ActionMenu`.

**Menü „Mehr“.** Je Bereich höchstens eine sichtbare Hauptaktion; was selten oder gefährlich ist,
steht im Menü (`ActionMenu`, §4.3).
- Detailseite: PageBar `actions` = nächster Schritt, `secondary` = häufige Nebenaktionen (bis 600 px
  im Menü), `more` = Seltenes und Gefahraktionen (immer im Menü, Gefahr unten).
- Listenzeile: die Aktion, die den Zustand der Zeile auflöst, als `secondary` („Rückstand …“ vor
  „Jetzt erstellen“/„Buchen“ vor „Bearbeiten“; pausiert „Fortsetzen“), alles Übrige im Menü
  (`variant="ghost"`, Name „Mehr zu „…““). So in Abo-Rechnungen, Kosten-Abos, Paketen, Postfächern
  und Konten.
- Gesperrte Einträge bleiben im Menü erreichbar (`disabled` + `hint` mit Grund); während eines
  Laufs ist die sichtbare Zeilenaktion `ariaDisabled`, damit der Fokus bleibt.
- Prüf-Harness und E2E öffnen zuerst `.kb-menu-btn` und wählen dann `menuitem`.

**Seiten mit Editor-Baustein im Routenordner.** Teilen sich zwei Routen eines Bereichs ein Formular
(„Neue Rechnung“ `/invoices/new` und „Entwurf bearbeiten“ `/invoices/[id]/edit`), liegt der
Baustein im Routenordner (`routes/invoices/InvoiceEditor.svelte`, keine `+`-Datei, also keine
Route), nicht in `src/lib` — er gehört nur diesem Bereich. Die Routen sind dünn (Prop `editId`;
`{#key id}`, damit ein Wechsel des Entwurfs neu lädt). Alte Adressen leiten per `+page.ts` mit
`redirect()` weiter; in der SPA (`ssr = false`) läuft `load` im Browser, SvelteKit ersetzt den
Verlaufseintrag (beim Start wie bei einer Navigation). Achtung: `no-side-effects-on-open.test.ts`
prüft nur `+page`/`+layout`-Dateien — das `onMount` eines Editor-Bausteins ruft ebenso nur lesende
Commands auf.

**Sortieren mit Auf/Ab statt Ziehen** (2.5.7, Paket-Kategorien). Je Eintrag „Nach oben“ und „Nach
unten“ (`secondary`, Pfeil `aria-hidden`, Name mit dem Eintrag: „Nach oben: Fotografie“). Am Rand
sind die Knöpfe nur `ariaDisabled` (fokussierbar, Klick ohne Wirkung), damit der Fokus nicht
verloren geht. Nach dem Verschieben bleibt der Fokus auf demselben Knopf des verschobenen Eintrags
(der Knoten wandert im DOM, darum nach `tick()` neu setzen); eine Statuszeile (`role="status"`,
nur für Screenreader) sagt „„X“ steht jetzt an Stelle 2 von 5.“ an. Scheitert das Speichern, gilt
wieder die alte Reihenfolge und ein Fehler-Toast erscheint. Kein Ziehen, keine eigene Ziehlogik.

**Wiederverwenden einer Revision** (Pakete, append-only). Ältere Versionen stehen als Tabelle,
neueste zuerst; die aktive trägt das Badge „Aktiv“, jede andere „Wiederverwenden“ (`secondary`). Der
Knopf fragt nach (kein Gefahr-Dialog: Titel „Version 2 wiederverwenden?“, Knopf „Als neue Version
übernehmen“) und legt eine **neue** Version mit dem alten Inhalt an; nichts wird überschrieben,
bestehende Belege ändern sich nicht. Wort „Wiederverwenden“ statt „zurücksetzen“, weil
„zurücksetzen“ nach Überschreiben klingt (ADR 0041 Nr. 132); das Audit-Label bleibt.

**Detailseiten.** Zurück zur Liste, Titel mit Nummer, Hauptaktion = nächster Schritt des Belegs
(„Rechnung ausstellen“, „+ Zahlung“), häufige Nebenaktionen als `secondary`, Seltenes und Gefahr in
`more`. Darunter eine Metazeile (Datum · Badge; Trennpunkt nie am Zeilenanfang, U1-065), dann Karten
je Gruppe (Verkäufer, Empfänger, Daten, Positionen, Historie). `dl`-Labels in eigener Spalte, nicht
unter 9rem; Werte mit Belegnummer über `DocNo`.

**Editoren (Neu/Bearbeiten).** PageBar: „Abbrechen“ (`secondary`) + Hauptaktion („Als Entwurf
speichern“). Formular im Raster, Pflicht-Legende oben, Abschnitte als Karten, Hauptaktion am Ende
wiederholt (`.kb-form-end`). Gesperrte Felder (festgeschriebene Belege, §19) zeigen den Grund.

**Speichern und Abbrechen.** Hauptaktion rechts/zuletzt, „Abbrechen“ links davon und ohne
Rückfrage, solange nichts verloren geht. Erfolg: Toast „Gespeichert.“ und Wechsel zur Detailseite.
Fehler: am Feld, bei mehreren Server-Gründen ein Banner oben, bei technischem Fehler ein
Fehler-Toast.

**Bestätigungen.** Nur für folgenreiche oder nicht umkehrbare Aktionen (Ausstellen, Stornieren,
Wiederherstellen, Sperren). Umkehrbares (Archivieren mit Reaktivieren, Konto deaktivieren, Pausieren)
ohne Rückfrage, dafür Toast mit „Rückgängig“ (`flash(text, "ok", { action })`, §4.10). Abhaken von
Hinweisen läuft ohne Rückfrage und ohne „Rückgängig“ (Toast, Fokus auf den nächsten Eintrag; „Alle
abhaken“ fragt nur bei dringenden Hinweisen nach; ADR 0041 Nr. 184). GoBD: Stornieren und Verwerfen
verlangen einen Grund.

**Leer, Laden, Fehler.** Leer: ein Satz, was hier erscheint, plus die eine Aktion („Noch keine
Rechnungen. + Neue Rechnung“) — keine Illustration; leerer Filter: „Keine Treffer für diese
Filter.“ + „Filter zurücksetzen“. Laden (U4, V2-02/V4-07): `<LoadingState />` — „Wird geladen …“
als `role="status"` (ohne `aria-busy` am Text, das unterdrückte die Ansage); `aria-busy` nur an
einem Bereich, der mit Inhalt nachlädt (`<Table busy>`). Platzhalter-Balken nur, wo das Layout sonst
springt (Start), und **statisch** (Fläche `--surface-page`, keine Puls- oder Endlos-Animation —
die Ansage trägt die Statuszeile; V3-06). Fehler: `Banner kind="error"` mit Ursache und nächstem Schritt; nicht gefunden:
Banner + Zurück in der PageBar, `h1` bleibt („Rechnung nicht gefunden“). Unbekannte Adresse: die
Fehlerseite der App (`routes/+error.svelte`, PageBar „Seite nicht gefunden“, Zurück zu Start).

**Status.** Badge (§4.9) in Listen und in der Metazeile; derselbe Zustand heißt überall gleich
(„Verwerfen“ für Entwürfe, „Stornieren“ für festgeschriebene Belege, ADR 0041 Nr. 27).

## 6. Responsive

| Band | Verhalten |
|---|---|
| ≤ 600 px (Handy) | PageBar klebt nicht, höchstens Hauptaktion + „Mehr“; `.kb-form-end`; Dialoge als Vollbild-Blatt; Toasts unten über die volle Breite; Formulare einspaltig |
| 601–820 px (Tablet hoch) | Tabellen als Karten, mit `cardGrid` zwei Kartenspalten; Formular-Raster zweispaltig ab 37rem |
| 821–900 px | Tabellen als Tabellen (scrollen in sich), Navigation als Drawer |
| > 900 px mit Maus | feste Seitenleiste (208 px), dichtere Zellen, Spaltenauswahl nach Inhaltsbreite, F1 = Hilfe |
| Touch (`pointer: coarse`) jeder Breite | Drawer, Trefferflächen 44 px, `--control-h` 44 px |
| Höhe ≤ 500 px | PageBar klebt nicht, `.kb-form-end` |
| Höhe ≤ 420 px | Dialog scrollt als Ganzes |

- `<main>` ist der Container `kb-main`; Seiten fragen die **Inhaltsbreite** ab (Container
  Queries), nicht den Viewport (ADR 0041 Nr. 40).
- Inhaltsbreiten: Listen/Details ≤ 100rem, Formularseiten ≤ 72rem, Fließtext ≤ 72ch.
- `dvh` statt `vh`; klebende Leisten und Vollbild-Dialoge mit `env(safe-area-inset-*)`.
- Keine Information nur per Hover: reine Hover-Effekte in `@media (hover: hover)`.
- Eingaben im Drawer-Band mit 16 px Schrift (kein Auto-Zoom auf iOS).
- Seitenleiste: Bereiche oben, „Über“, „Datenbank sperren“, „Abmelden“ als eigene Gruppe unten
  (Liste „Sitzung“), Version darunter (U3-040). Aktuelle Seite: Fläche, halbfette Schrift und ein
  3-px-Balken links (`aria-current="page"`, V4-12). F1 öffnet das Handbuch-Kapitel zur Seite
  (`routeHelp.ts`, sonst `/help`).

**Web-App-Manifest** (U4): `static/manifest.webmanifest` (Name „Klein.Buch“, `display:
standalone`, `theme_color` `#0d4150`, Hintergrund `#f4f7f9`, Symbole: `favicon.svg`, `favicon.png`,
`app-icon-1024.png` auch als `maskable`), verlinkt in `app.html` samt `apple-touch-icon`. **Kein
Service Worker**, kein Offline-Cache. Die CSP der SPA (`default-src 'self'`) erlaubt das Manifest
gleicher Herkunft; statische Dateien liefert der Server ohne Sitzung aus.

## 7. Deutsche Formate

| Was | Format | Helfer |
|---|---|---|
| Betrag | `1.234,56 €` (U+00A0 vor `€`); negativ mit echtem Minus `−450,00 €` (U+2212, so breit wie eine Ziffer; auch im Eingabefeld, der Parser liest `−` und `-`; V3-22) | `euro()`, `formatCents()` |
| Prozent | `19 %` (U+00A0) | `percent()` |
| Datum | `TT.MM.JJJJ`, nie `JJJJ-MM-TT` in Leseansichten | `date()` |
| Datum mit Zeit | Listen `TT.MM.JJJJ, HH:MM`, Protokolle mit Sekunden | `dateTimeShort()`, `dateTime()` |
| IBAN | Vierergruppen | `iban()` |
| Einheit | Klartext („Stunde“), Code nur im XML; Code außerhalb der Liste wird am Feld gemeldet (§4.7) | `unitLabel()`, `unitShort()`, `unitOptions()`, `unitCodeError()` (`labels.ts`) |
| PDF-Vorlage | „Standard“, Option „Wie in den Einstellungen (Standard)“ | `pdfTemplateLabel()`, `pdfTemplateDefaultOptionLabel()` |
| Zeitzone | Europe/Berlin überall | `src/lib/date.ts` |
| Sortierung | `Intl.Collator('de')` | `sortContacts()` (ContactSelect) |
| Abkürzungen | „z. B.“, „u. a.“ ohne Umbruch | `.kb-nowrap` oder U+00A0 |

Seiten formatieren nie selbst (`toFixed`, `toLocaleString` ohne Zeitzone,
`toISOString().slice(0, 10)` sind verboten).

## 8. Anti-Muster und eigene Details

**Vermeiden:** Farbverläufe als Dekoration; Glassmorphism/Weichzeichner; überall gleiche, weich
gerundete Karten mit Schlagschatten; Emoji als Icons; generische Illustrationen und Hero-Flächen;
violett-blaue Standardpaletten; übergroße Abstände ohne Informationsgewinn; Animation um ihrer
selbst willen; Floskeln („Willkommen zurück!“); Information nur per Farbe oder nur per Tooltip;
mehrere gleichrangige Primärknöpfe; Knöpfe unterschiedlicher Höhe in einer Zeile.

**Wiedererkennbare Details:** Lineatur wie im Kontorbuch (doppelte Linie über Summen), Belegnummern
als Monospace, Status-Badges mit Text und Punkt, Petrol-Leiste mit Linien-Symbolen.

### 8.1 Anti-Muster-Check (U4, gegen die Kontaktbögen `u3b`, `u3b-m`, `u3b-t`)

Geprüft: Start, Rechnungen/Angebote, Einstellungen (Desktop 1280), Rechnung, Start (iPhone 320)
und die Messwerte des Inventars (Feldhöhen) nach dem Stand vor U4. Spalte „Maßnahme“ mit dem Stand
nach U4 Phase 2 (Seiten) und den Restkorrekturen; „übernommen“ heißt im Code der Seiten umgesetzt,
die Messung mit dem Harness steht noch aus. Einträge „V3“: zweiter Anti-Muster-Check der
Design-Runde gegen die Bögen `u4a-d`, `u4a-m`, `u4a-t` (Befunde V3-01 … V3-23).

| Muster | Ergebnis | Maßnahme |
|---|---|---|
| Farbverläufe | kein Befund (Hero seit U1 flach) | — |
| Glas/Weichzeichner | kein Befund (PageBar deckend, Abdunklung ohne `blur`) | — |
| Gleiche weiche Karten mit Schatten | **Befund Start**: Kennzahl-Kacheln (Radius 20 px, `--sh-md`, Anheben beim Zeigen) und vier gleiche Schnellaktions-Karten | zentral: Radien 12/8 px, `--shadow-card` als Haarlinie; Seite übernommen: Kennzahlen als `.kb-metric` ohne Anheben, Schnellzugriff als Liste mit Lineatur; Einstellungen-Hub ohne Anheben; V3: Einstellungen-Hub und Schnellzugriff als `.kb-linklist` (eine Karte je Gruppe), Hinweise als eine Liste mit Lineatur statt Kartenstapel (V3-04/05) |
| Emoji als Icons | kein Befund (Inline-SVG) | —; V3: „✓/✗“ als Versandstatus durch Badge ersetzt (V3-17) |
| Hero-Fläche, Floskel | **Befund Start**: „Willkommen, Beispiel“ in Display-Größe belegt auf 320 px rund ein Drittel (U1-064) | zentral: `--type-display` 24 px auf 320 px; Seite übernommen: Sachstand (Datum, Geschäftsjahr, Firma, §19-Zeile) statt Begrüßung; V3: Datum der Startseite in `--type-heading` statt `--type-display`, nicht größer als der Seitentitel (V3-03) |
| Violett-blau | kein Befund (Petrol) | — |
| Übergroße Abstände | Teilbefund: Positionen als lange Kartenliste auf dem Tablet (U2-020) | übernommen: Positionstabellen messen ihre Kartenbreite (Tabelle ab 560/720 px, ADR 0041 Nr. 95); offen: Prop `cardsBelow` im `Table` statt Messung je Seite; V3: Start-Kennzahlen auf dem Handy zweispaltig, Backup über die volle Breite (V3-02); offen: Kartenansicht der Listen ohne Hierarchie (V3-12) |
| Animation um ihrer selbst willen | Teilbefund: Anheben der Kennzahl-Kacheln (Start) | übernommen: kein Anheben mehr (Start, Einstellungen-Hub); sonst nur Seitenwechsel 200 ms und Toast-Einblenden, beides aus bei reduzierter Bewegung; V3: Lade-Platzhalter statisch statt pulsierend, `--t-slow` entfernt (V3-06) |
| Status nur über Farbe | kein Befund bei Badges; Toasts nur Farbe + Gruppenname | zentral: Symbol je Toast-Art; V3: Statusfarbe nur für Status — Merkmale und Arten als `Badge kind="tag"` ohne Punkt, Status-Wörter aus den Helfern, Hinweis-Zähler nur bei dringenden Hinweisen rot (V3-07/08/09/16) |
| Information nur per Tooltip | **Befund** Markdown-Werkzeugleiste „H1/H2/F/K“ (U2-018) | zentral: Klartext-Beschriftung; Seiten: `title`-Tooltips in Abo-Zeilen und auf der Kontakt-Detailseite entfernt, Gründe stehen als Text (gesperrte Menüeinträge mit `hint`) |
| Mehrere gleichrangige Aktionen | **Befund** PageBar mit bis zu 6 Knöpfen (U1-059); Zeilenaktionen untereinander, verschieden breit (U3-036); ghost neben secondary ohne Regel (U1-068) | zentral: PageBar `secondary`/`more` + `ActionMenu`, `.kb-actions`, Regel §4.1; übernommen auf Rechnung, Angebot, Kontakt, Anlage (PageBar) und in den Zeilen von Abo-Rechnungen, Kosten-Abos, Paketen, Postfächern, Konten (eine Aktion + Menü, §5 „Menü „Mehr““); Dialog mit drei Aktionen 36rem breit, ohne Umbruch (U2-019); V3: PDF-Vorschau ohne zweite primary, „Schließen“ in PDF- und XML-Dialog `secondary` (V3-15) |
| Knöpfe/Felder verschieden hoch | **Befund**: Felder 32–44 px, Knöpfe 37 px (U3-033), Auswahlfelder in WebKit grau mit Doppelrahmen (U1-062) | zentral: `--control-h`, eigene Auswahlfeld-Optik; Feldbreiten nach Inhalt (`.kb-w-*`, an Betrag/Menge über `fieldClass`); Jahresfilter als Auswahl statt Zahlenfeld (U1-062); V3: Toast-Aktion und Textvorschläge als `.btn-secondary` (Höhe `--control-h`), `Button` und `.btn-*` mit gleichem Übergang (V3-21) |
| Belegnummern uneinheitlich | **Befund**: in Rechnungen/Angeboten ohne, sonst mit Unterstreichung; in Titeln trennbar (U1-061, U3-030) | zentral: `.kb-docno`, PageBar-Titel; übernommen: `DocNo` in allen Listen, Detail-, Storno-, Zahlungs- und Versandseiten, Backup und E-Mail-Protokoll; V3: Nummern im Fließtext und in Banner-Kurzaussagen über `DocNo` bzw. `splitDocNumbers` (V3-23) |
| Seitenleiste | **Befund** „Datenbank sperren“ zweizeilig (U3-040) | zentral erledigt: einzeilig, Sitzungs-Gruppe unten |
| Zahlen ohne Flucht | **Befund V3**: Kennzahlen ab fünf Stellen umbrochen (Betrag und € getrennt), Bindestrich statt Minus, Spaltenköpfe von `.kb-num` links, Teiltabellen der Pakete mit eigenen Spalten, drei Fassungen der Summenlinie | zentral: `.kb-metric .val` ohne Umbruch und an die Kachelbreite gekoppelt (V3-01), „−“ (U+2212) in `formatCents`/`euro()` (V3-22), `.kb-num` auch im Kopf (V3-10), `tr.kb-sum`/`.kb-ledger` mit `--rule-sum` (V3-13); Seite: Pakete mit festen Spaltenbreiten (V3-11) |

## 9. Beispiele

Die Musterseite `/design` rendert die Bausteine dieser Leitlinie mit echten Komponenten: Knöpfe
(Formularende, Nebenaktionen, Gefahr, gesperrt mit Grund), Badges (Status und Merkmal), Felder mit
`.kb-w-*`, Betrag/Menge, Banner, Toast mit Aktion, Dialoge, Kennzahlen, Linkliste, Aufstellung
mit Summe und eine Tabelle mit `DocNo` und Summenzeile (V3-19).

```svelte
<!-- Detailseite: eine Hauptaktion, Nebenaktionen, Gefahr im Menü -->
<PageBar
  back="/invoices" backLabel="Rechnungen" title={`Rechnung ${inv.docNumber}`}
  secondary={[
    { label: "PDF öffnen", onSelect: openPdf },
    { label: "Senden", href: `/invoices/${inv.id}/send` },
  ]}
  more={[
    { label: "PDF herunterladen", href: pdfUrl, download: true },
    { label: "Stornieren", href: `/invoices/${inv.id}/cancel`, danger: true },
  ]}
>
  {#snippet actions()}<Button href={`/invoices/${inv.id}/payment`}>+ Zahlung</Button>{/snippet}
</PageBar>
```

```svelte
<!-- Listenzeile: Nummer, Betrag, Status (Wort aus dem Status-Helfer), Belegart als Merkmal,
     eine Aktion + Menü -->
{#each rows as r (r.id)}
{@const st = invoiceStatusView(r, todayIso)}
<tr>
  <td data-label="Nummer">
    <DocNo value={r.docNumber} href={`/invoices/${r.id}`} muted={st.strike} />
    {#if r.isStornoFor}<Badge kind="tag">Storno-Beleg</Badge>{/if}
  </td>
  <td data-label="Kunde">{r.contactName}</td>
  <td data-label="Brutto" class="kb-num">{euro(r.totalCents)}</td>
  <td data-label="Status"><Badge tone={st.tone} strike={st.strike}>{st.label}</Badge></td>
  <td>
    <div class="kb-actions">
      <Button variant="secondary" href={`/invoices/${r.id}`}>Öffnen</Button>
      <ActionMenu variant="ghost" ariaLabel={`Mehr zu ${r.docNumber}`} items={[{ label: "Senden", href: `/invoices/${r.id}/send` }]} />
    </div>
  </td>
</tr>
{/each}
```

```svelte
<!-- Formular: Raster, Breiten nach Inhalt, Kontaktauswahl, Betrag -->
<div class="kb-form-grid">
  <ContactSelect bind:this={contactEl} label="Empfänger" required contacts={contacts}
    bind:value={contactId} showErrors={submitted} />
  <FormField label="Rechnungsdatum" required>
    {#snippet children({ inputId })}
      <input id={inputId} type="date" class="kb-input kb-w-date" bind:value={issueDate} required />
    {/snippet}
  </FormField>
  <MoneyInput label="Betrag" fieldClass="kb-w-amount" bind:cents required />
  <FormField label="Notiz" class="kb-span-all">
    {#snippet children({ inputId })}<textarea id={inputId} class="kb-input" rows="3" bind:value={note}></textarea>{/snippet}
  </FormField>
</div>
```

```svelte
<!-- Linkliste (Einstellungen-Hub, Schnellzugriff) und Aufstellung mit Summe -->
<ul class="kb-linklist">
  <li><a href="/settings/seller"><span class="kb-ll-title">Meine Firmendaten</span><span class="kb-ll-sub">Name, Adresse, Steuernummer</span></a></li>
</ul>
<dl class="kb-ledger">
  <div><dt>Bezahlte Rechnungen</dt><dd>{euro(r.invoiceIncomeCents)}</dd></div>
  <div class="kb-sum"><dt>Summe Einnahmen</dt><dd>{euro(r.totalIncomeCents)}</dd></div>
</dl>
```

```ts
// Warnung statt Fehler, wenn die Aktion gelungen ist
flash("Rechnung versendet. Das E-Mail-Protokoll ist unvollständig.", "warning");

// Umkehrbares ohne Rückfrage, mit „Rückgängig“ (bleibt stehen, bis geschlossen)
flash("Paket archiviert.", "ok", { action: { label: "Rückgängig", run: reactivate } });
```

Beispieladressen in Doku und Tests nur mit Dokumentations-Adressen (`example.org`, 192.0.2.0/24).
