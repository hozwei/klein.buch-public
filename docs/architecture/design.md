# Designleitlinie

> Erster Stand aus U0 (Lauf 2026.11), U4 schärft ihn. Grundlage: `src/lib/styles/tokens.css`,
> Bausteine in `src/lib/`, Musterseite `/design`. **[Ist]** = im Code, **[Ziel]** = Soll für U1–U4.

## 1. Zweck und Geltung

- **Design ändert sich nur über Tokens und Bausteine** (`tokens.css`, `src/lib/*.svelte`),
  nie per Einzelkorrektur je Seite. Fehlt ein Baustein, wird er angelegt.
- **Vorrang:** GoBD-/§19-Hardlines → Barrierefreiheit (orientiert sich an WCAG 2.2 AA)
  → deutsche Standards (DIN 5008) → Konsistenz mit dem Design-System → Geschmack.
- Keine Konformitätsaussagen (Plan §4.1); erlaubt: „orientiert sich an WCAG 2.2 AA, geprüft mit axe-core und manuell“.

## 2. Charakter und Prinzipien

Ein ruhiges, präzises Werkzeug für Belege und Zahlen — ein **sorgfältig geführtes Kontorbuch**, kein Marketing-Dashboard.

1. **Zahlen sind das Gestaltungselement.** Tabellarische Ziffern, Beträge
   rechtsbündig, saubere Fluchten; Belegnummern fallen als Monospace auf.
2. **Klare Typo-Hierarchie.** Ein Seitentitel (`h1` in der PageBar), Abschnitte
   als `h2` in Karten, sonst Fließtext. Keine Zwischenstufen ohne Grund.
3. **Wenig Farbe, bewusst eingesetzt.** Petrol für Marke und Primäraktion,
   Statusfarben nur für Status. Status immer mit Farbe **und** Text.
4. **Weißraum mit Zweck.** Genug zum Lesen, nie Leere ohne Grund; Desktop dichter als Mobil.
5. **Ruhe statt Effekt.** Bewegung nur zur Orientierung (≤ 200 ms), `prefers-reduced-motion` gilt.
6. **Mobil ist kein verkleinerter Desktop** (Abschnitt 5).

## 3. Tokens

### 3.1 Ist-Bestand (`tokens.css`)

| Gruppe | Ist |
|---|---|
| Primär (Petrol) | `--c-primary-50 … -800`; Default `-600` `#176b87`, dunkelste Stufe `-800` `#0d4150` (Theme-Farbe, Sidebar, Mobilleiste) |
| Akzent | `--c-accent-500` `#10aeb8`, `-600` `#0d949d` (kaum genutzt) |
| Status | `--c-{success,warning,danger,info}-{50,500,700}`; `info` = Petrol |
| Neutral | `--c-bg` `#f4f7f9`, `--c-surface` `#fff`, `--c-surface-2`, `--c-border` `#e3e9ed`, `--c-border-strong` `#cdd6dc`, `--c-text` `#16242c`, `--c-text-muted` `#5a6b75`, `--c-text-subtle` `#8696a0` |
| Typo | `--fs-xs 12` · `sm 13` · `base 14` · `md 16` · `lg 18` · `xl 22` · `2xl 28` · `3xl 34` (px); System-Font-Stacks `--font-sans`/`-display`/`-mono`, kein Web-Font |
| Abstände | keine Skala; nur `--main-pad` (2 rem, ≤ 900 px 1 rem), sonst Einzelwerte in px/rem |
| Radien | `--r-sm 8` · `md 12` · `lg 16` · `xl 20` · `pill 999` |
| Schatten | `--sh-sm … --sh-xl`, zweilagig |
| Bewegung | `--ease-apple`, `--t-fast 140 ms`, `--t-base 200 ms`, `--t-slow 280 ms` |
| Breakpoints | keine Tokens; Media Queries mit festen Werten 600 / 820 / 900 px und `(pointer: coarse)` |
| Fokus | global `:focus-visible` 2 px `--c-primary-500`, Abstand 2 px; `.kb-input` mit Halo statt Outline |

Globale `!important`-Klassen (`.btn-*`, `.link*`, `.back`, `.status-*`, `th`/`td`): Übergang, kein Vorbild.

### 3.2 Nachgerechnete Kontraste [Ist]

WCAG-Formel (relative Leuchtdichte), gerechnet mit den Hex-Werten oben:

| Paar | Verhältnis | Bewertung |
|---|---|---|
| `--c-text` auf Fläche / Hintergrund | 15,87 / 14,76 | ok |
| `--c-text-muted` auf Fläche / Hintergrund | 5,53 / 5,14 | ok |
| `--c-text-subtle` auf Fläche / Hintergrund | **3,05 / 2,84** | unter 4,5:1 — betrifft `th`, Hinweise, Platzhalter, `data-label` |
| Link `--c-primary-600` auf Fläche / Hintergrund | 6,02 / 5,59 | ok |
| Weiß auf `--c-primary-600` (Primär-Button) | 6,02 | ok |
| Weiß auf `--c-danger-500` (Bestätigen „gefährlich“ im Dialog) | **4,35** | unter 4,5:1 |
| Fokusring `--c-primary-500` auf Fläche / Hintergrund | 4,66 / 4,33 | ≥ 3:1 ok |
| `--c-border-strong` (Feldrahmen, Checkbox, Toggle „aus“) auf Weiß | **1,47** | unter 3:1 (1.4.11) |
| `--c-border` auf Weiß | 1,22 | nur Trennlinie, keine Bedienelement-Grenze |
| Badges (neutral/primary/info/success/warning/danger) | 5,66 / 8,60 / 7,48 / 4,80 / 5,09 / 6,41 | ok |
| `--c-success-500` / `--c-warning-500` / `--c-accent-500` auf Weiß | 3,16 / 2,69 / 2,70 | nie als Textfarbe |

### 3.3 Soll-Struktur [Ziel U1]

Bausteine nutzen **nur** semantische Rollen, die Palette ist Rohmaterial. Ein späterer Dark Mode
ist dann ein zweiter Rollensatz, kein Umbau (Dark Mode ist **nicht** Ziel von 2026.11).

| Rolle | Tokens (Vorschlag) |
|---|---|
| Fläche | `--surface-page`, `--surface-card`, `--surface-sunken`, `--surface-inverse` (Sidebar) |
| Text | `--text`, `--text-muted` (≥ 4,5:1 auf allen Flächen), `--text-on-accent`, `--text-inverse`; „subtle“ nur noch dekorativ |
| Linie | `--line` (Lineatur), `--line-control` (Bedienelement-Grenze, ≥ 3:1) |
| Akzent | `--accent`, `--accent-hover`, `--accent-soft` |
| Status | `--status-{success,warning,danger,info}-{bg,fg,line}`; `fg` auf `bg` ≥ 4,5:1 |
| Fokus | `--focus-ring` (≥ 3:1 gegen alle Flächen), `--focus-width 2px`, `--focus-offset 2px` |
| Typo | Skala `--fs-*` bleibt; Ziel Grundgröße 16 px im Fließtext auf Mobil, Zeilenhöhe 1,5; `--font-num` = Sans mit `tabular-nums` |
| Abstände | 4-px-Raster: `--sp-1 4` · `2 8` · `3 12` · `4 16` · `5 24` · `6 32` · `7 48` |
| Größen | `--target-min 24px`, `--target-touch 44px` (auf `(pointer: coarse)` und ≤ 900 px) |
| Breakpoints | dokumentiert als 820 / 900 px (CSS-Variablen gehen in Media Queries nicht; Werte nur hier und in `tokens.css` pflegen) |

**Mindestgrößen:** Trefferfläche 44×44 CSS-px auf Touch-Profilen (Projektstandard), sonst
≥ 24×24 CSS-px (2.5.8). **Kontrast:** Text 4,5:1 (groß 3:1), Nicht-Text 3:1 (1.4.11).

## 4. Komponenten-Regeln

**Buttons.** Varianten `primary`, `secondary`, `ghost`, `danger`; eine Größe für alle
(Manuel-Vorgabe 2026-05-26, `size="sm"` bleibt wirkungslos). **Höchstens eine Primäraktion je
Bereich** (PageBar, Dialog, Formular). Gefährliche Aktionen („Stornieren“) als `danger`, nie als
Primär-Optik. Beschriftung = Verb des Ergebnisses („Festschreiben“, nicht „OK“). Deaktivierte
Buttons erklären per sichtbarem Hinweis, warum.

**Links vs. Buttons.** Navigation = `<a href>` (auch im Button-Look über `Button href=…`),
Aktion = `<button>`. Kein `<div onclick>`. Text-Links im Fließtext sind unterstrichen oder
anders als nur über Farbe erkennbar (1.4.1).

**Formulare.** Immer sichtbares Label über `FormField`; Platzhalter ersetzen kein Label. Hinweis
unter dem Feld, Fehler am Feld: `FormField` liefert `inputId`, `describedBy`, `invalid` — der
Aufrufer setzt `id`, `aria-describedby`, `aria-invalid`, `aria-required`. Fehlertext sagt, was
zu tun ist (3.3.3). Einspaltig auf Mobil, zweispaltig ab Tablet. Autofill nach Plan §4.2 (eigene
Daten mit Tokens, Daten Dritter und IBAN/BIC/USt-IdNr. mit `autocomplete="off"`).

**Betragsfelder.** Kein `type="number"`; `type="text"` mit `inputmode="decimal"`. Umrechnung in
Integer-Cent in **einem** geprüften Helfer (`src/lib/money.ts`, [Ziel]). Leseregel: kommen `,`
und `.` vor, ist das letzte das Dezimalzeichen; nur `,` → Dezimalkomma; nur `.` → genau ein
Punkt mit 1–2 Ziffern danach ist Dezimalpunkt (`12.5`), sonst Tausenderpunkt (`1.234`). Mehr als
2 Nachkommastellen → Fehler am Feld, nie still runden. Beim Verlassen erscheint der erkannte
Wert im DIN-Format. Felder mit erlaubten negativen Werten bekommen eine Vorzeichen-Auswahl (die
iOS-Dezimaltastatur hat kein Minus).

**Tabellen.** Feine Lineatur (`--line`, 1 px je Zeile, keine Zebrastreifen, keine senkrechten
Linien), Kopfzeile klein in Versalien. Beträge rechtsbündig mit `tabular-nums` (Klasse `.kb-num`
bzw. `.num`), Summenzeile mit doppelter Linie oben (Kontorbuch-Detail, [Ziel]). Unter 820 px
werden Listen über `<table class="cards">` und `data-label` an jeder `<td>` zu Karten. Breite
Tabellen scrollen in sich, nie die ganze Seite (1.4.10).

**Status-Badges.** `Badge` mit `tone` und **immer Text** („Bezahlt“, „Überfällig“); Konvention:
neutral = Entwurf, primary = Festgeschrieben, info = Versendet, success = Bezahlt, warning =
Teilbezahlt, danger = Überfällig, neutral + `strike` = Storniert. Farbe allein trägt keine
Bedeutung.

**Dialoge.** Ein Bestätigungs-Dialog (`ConfirmDialog`, `role="dialog"`, `aria-modal`, Titel per
`aria-labelledby`); Escape bricht ab, Fokus geht beim Schließen zum Auslöser zurück. [Ziel]
Fokusfalle innerhalb des Dialogs und auf kleinen Geräten (≤ 600 px) Vollbild-Blatt mit `100dvh`
und Safe-Area-Abständen. Bei gefährlichen Aktionen liegt der Startfokus auf „Abbrechen“ [Ziel;
Ist: auf Bestätigen].

**Toasts.** Nur über `flash()`; Live-Region (`role="status"` für Erfolg, `role="alert"` für
Fehler), schließbar. Erfolg verschwindet nach Ablauf, **Fehler und Toasts mit Aktion schließen
nicht automatisch** bzw. pausieren bei Fokus oder Hover (2.2.1) [Ziel; Ist: Fehler nach 7 s].
Toasts verdecken keine fokussierten Elemente (2.4.11). Dauerhafte Zustände sind `Banner`, kein
Toast.

**PageBar.** Je Seite genau eine: Zurück, Titel (`h1`), Aktionen rechts, die Primäraktion
zuletzt. Klebt ab 601 px oben; der Inhalt darunter muss beim Tabben sichtbar bleiben
(`scroll-padding-top`, 2.4.11) [Ziel].

**Zustände.** Leer: ein Satz, was erscheint, plus die eine Aktion — keine Illustration. Laden:
„Lädt …“ mit `aria-busy`, kein Vollbild-Spinner. Fehler: `Banner kind="error"` mit Ursache und
nächstem Schritt.

## 5. Responsive

| Band | Verhalten |
|---|---|
| ≤ 820 px | Tabellen als Karten, Formulare einspaltig, Dialoge als Blatt, Toasts unten über die volle Breite (≤ 600 px) |
| 821–900 px | Tabellen als Tabellen, Navigation noch als Drawer |
| > 900 px | feste Sidebar, dichtere Tabellen, Tastaturkürzel (F1) |

- **Touch bedeutet immer Drawer** [Ziel]: `(pointer: coarse)` zeigt die
  Navigation unabhängig von der Breite als Drawer (Ist: nur ≤ 900 px).
- Hauptaktion mit dem Daumen erreichbar; Tablet: zweispaltige Formulare, Liste + Detail.
- `dvh` statt `vh` (Ist: Sidebar > 900 px noch `100vh`); wegen `viewport-fit=cover`
  bekommen klebende Leisten und Vollbild-Dialoge `env(safe-area-inset-*)`.
- Keine Hover-Abhängigkeit: reine Hover-Effekte in `@media (hover: hover)`,
  jede Information auch per Tippen/Fokus erreichbar.
- Eingaben ≤ 900 px mit 16 px Schrift (kein Auto-Zoom auf iOS).

## 6. Deutsche Formate

| Was | Format | Helfer |
|---|---|---|
| Betrag | `1.234,56 €`, geschütztes Leerzeichen (U+00A0) vor `€` | `euro()` in `src/lib/format.ts` |
| Prozent | `19 %` (geschütztes Leerzeichen) | [Ziel] `percent()` |
| Datum | `TT.MM.JJJJ`, nie `JJJJ-MM-TT` in Leseansichten | `date()` |
| Datum mit Zeit | `TT.MM.JJJJ, HH:MM` (Protokolle mit Sekunden) | `dateTime()` |
| IBAN | Vierergruppen `DE89 3704 0044 0532 0130 00` | [Ziel] `iban()` |
| Zeitzone | Europe/Berlin überall | `src/lib/date.ts` |
| Sortierung | `Intl.Collator('de')` | — |

Seiten formatieren nie selbst (`toFixed`, `toLocaleString` ohne Zeitzone,
`toISOString().slice(0, 10)` sind verboten).

## 7. Anti-Muster und eigene Details

**Vermeiden** (Plan §5, auf Klein.Buch bezogen):

- Farbverläufe als Dekoration; violett-blaue Standardpaletten — Petrol bleibt.
- Glassmorphism/Weichzeichner (Ist: PageBar und Dialog-Hintergrund mit
  `backdrop-filter` — in U4 durch blickdichte Flächen ersetzen).
- Überall gleiche, weich gerundete Karten mit Schlagschatten (Ist: `.card` mit
  `--r-xl` 20 px und `--sh-md`); Karten nur, wo sie gruppieren.
- Emoji als Icons, generische Illustrationen, „Hero“-Flächen auf der Startseite.
- Übergroße Abstände ohne Informationsgewinn; Animation um ihrer selbst willen.
- Floskeln wie „Willkommen zurück!“ mit Emoji; stattdessen Sachstand („3 Rechnungen überfällig“).

**Wiedererkennbare Details:**

- **Lineatur:** feine Zeilenlinien wie im Kontorbuch, doppelte Linie über Summen.
- **Belegnummern** (`RE-2026-0042`, `ST-2026-0003`) in `--font-mono`, leicht
  gesperrt, nicht umbrechend (`white-space: nowrap`).
- **Status-Badges mit Text** und Punkt in Statusfarbe.

## 8. Beispiele

```svelte
<!-- Eine Primäraktion, Rest sekundär -->
<PageBar title="Rechnung RE-2026-0042" back="/invoices">
  {#snippet actions()}
    <Button variant="secondary" href="/invoices/0042/pdf">Als PDF</Button>
    <Button variant="danger" onclick={storno}>Stornieren</Button>
    <Button onclick={lock}>Festschreiben</Button>
  {/snippet}
</PageBar>
```

```svelte
<!-- Betragsfeld -->
<FormField label="Betrag" hint="z. B. 1.234,56" error={amountError}>
  {#snippet children({ inputId, describedBy, invalid })}
    <input id={inputId} class="kb-input kb-num" type="text" inputmode="decimal"
      autocomplete="off" aria-describedby={describedBy} aria-invalid={invalid}
      bind:value={amountText} onblur={normalizeAmount} />
  {/snippet}
</FormField>
```

```svelte
<!-- Tabellenzeile, unter 820 px als Karte -->
<table class="cards">
  <thead><tr><th>Nummer</th><th>Kunde</th><th class="num">Betrag</th><th>Status</th></tr></thead>
  <tbody>
    <tr>
      <td data-label="Nummer"><span class="doc-no">RE-2026-0042</span></td>
      <td data-label="Kunde">Muster GmbH</td>
      <td data-label="Betrag" class="kb-num">{euro(123456)}</td>
      <td data-label="Status"><Badge tone="neutral" strike>Storniert</Badge></td>
    </tr>
  </tbody>
</table>
```

Beispieladressen in Doku und Tests nur mit `example.org`.

## 9. Offene Punkte für U4

1. `--c-text-subtle` auf ≥ 4,5:1 anheben oder auf Dekoration begrenzen.
2. Bedienelement-Grenze `--line-control` ≥ 3:1 für Feldrahmen, Checkbox, Toggle.
3. Gefahr-Bestätigung im Dialog: Weiß auf `--c-danger-500` (4,35) → `-700`.
4. Feste Hex-Werte der Bausteine in Rollen überführen (`Badge`, `Banner`, `Button`, `ToastHost`, `ConfirmDialog`).
5. `!important`-Klassen abbauen, sobald die Seiten auf Bausteine umgestellt sind.
6. Abstands-Skala `--sp-*` einführen und Einzelwerte ersetzen.
7. Dialog: Fokusfalle, Vollbild-Blatt ≤ 600 px, Startfokus bei Gefahr.
8. Toasts: Fehler ohne Auto-Schließen, Pause bei Fokus/Hover, Abstand zur PageBar.
9. Touch-Drawer über `(pointer: coarse)`; `100vh` → `100dvh`; Hover-Medienabfrage.
10. `euro()` mit geschütztem Leerzeichen; `percent()`, `iban()`, `money.ts`.
11. Glas-Effekte, Radien, Schatten zurücknehmen; Web-App-Manifest (ohne Service
    Worker) entscheiden; Musterseite `/design` um alle Bausteine und Zustände erweitern.
