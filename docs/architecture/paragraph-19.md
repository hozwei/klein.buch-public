# §19-Logik

> Vertiefung zu „§19 Kleinunternehmer" in `../ARCHITECTURE.md` §7. Wie die
> Kleinunternehmer-Regelung (§19 UStG) durch das ganze System gewahrt wird:
> UI-Sperren, BT-22-Pfad in der XRechnung, PDF-Klausel-Check, Verzicht auf
> §19 mit 5-Jahres-Bindung, §14c-Schutz im Backend.

ADR-Basis: 0005 (§19 als Hardline-Default). Querverweise: 0007 (CII/BT-22),
0019 (Eingangsseitige USt-Behandlung), 0030 (PDF-Templates), 0033 (Abo-Rechnungen).

---

## 1. Was §19 UStG für die Software bedeutet

§19 UStG erlaubt Kleinunternehmern, **keine** Umsatzsteuer auszuweisen, dafür
aber auch keine Vorsteuer zu ziehen. Eine Rechnung von einem Kleinunternehmer
hat keinen USt-Block; im XML steht stattdessen die Tax-Category `E` (exempt)
und eine textuelle Klausel.

**Klein.Buch ist primär ein §19-Tool.** Default: `seller_profile.is_kleinunternehmer
= true`. Wer auf §19 verzichten will (Option zur Regelbesteuerung nach §19
Abs. 2 UStG), kann das per Settings-Toggle aktivieren — mit Warnhinweis auf
die **5-Jahres-Bindung**. Datum landet in
`seller_profile.waived_paragraph_19_since`.

---

## 2. UI-Sperren

Wenn `is_kleinunternehmer = true`, sperrt das Frontend alle USt-relevanten
Eingabefelder:

- **Rechnungs- und Angebots-Items.** Tax-Rate-Feld ist disabled, Tax-Category
  ist auf `E` (Exempt) fest verdrahtet, kein Switch zwischen 19/7/0.
- **Settings/PDF-Vorlagen.** Templates, die einen USt-Block voraussetzen,
  werden im Picker durchgestrichen oder ganz ausgeblendet (siehe Klausel-Check
  in §4).
- **Recurring-Invoice-Templates.** Beim Speichern wird `tax_category_code = 'E'`
  serverseitig erzwungen (KB-0053). Wer eine andere Kategorie schickt, kriegt
  einen Validierungs-Fehler.

Wenn der Toggle umgelegt wird (Verzicht), schaltet das UI die Felder frei und
gibt einen Hinweis aus: „Sie haben auf §19 verzichtet. Die Bindung beträgt
5 Jahre."

**Beim Lesen alter Belege** bleiben die UI-Sperren *aus*, weil der Beleg-
Snapshot festgeschriebene `tax_*`-Felder mitbringt, die nicht mehr veränderbar
sind. Die Sperren betreffen also nur neue Drafts.

---

## 3. BT-22 in der XRechnung

XRechnung CII (UN/CEFACT) trägt textuelle Hinweise über das Element
**`IncludedNote`** mit Subject-Code `REG`. In der semantischen Spec heißt das
**BT-22** („Invoice note"). Klein.Buch setzt für §19-Rechnungen genau eine
Note:

> *Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen.*

**Wortgleich.** Der Text kommt aus `domain::kleinunternehmer::paragraph_19_clause`
und ist eine `const &'static str`. Wer den Wortlaut ändert (z. B. um „§ 19
UStG" mit Leerzeichen vor 19 zu schreiben), muss die Konstante ändern und
die Tests laufen lassen.

Zusätzlich wird im XML auf **Item-Ebene** die Tax-Category-Code-Spalte
(`BT-151`) auf **`E`** und der Tax-Rate (`BT-152`) auf `0.0` gesetzt. Auf
Document-Total-Ebene wird die VAT-Breakdown (`BG-23`) mit Category `E` und
Tax-Amount `0` aufgeführt.

Validierungs-Quelle: KoSIT-Sidecar bestätigt das. Ein BT-22-fehlender §19-
Beleg wird als formaler Fehler gemeldet.

---

## 4. PDF-Klausel-Check (`pdf::klausel_check`)

Ein Typst-Template kann den §19-Hinweis im PDF-Body unterbringen oder nicht.
Klein.Buch erlaubt **nicht**, eine §19-Rechnung mit einem Template zu rendern,
das die Klausel **nicht** ausweist. Der Check funktioniert so:

1. Jedes für §19 zugelassene Template enthält im Source einen
   **Marker-Kommentar**:
   ```
   // §19-KLAUSEL-BLOCK: REQUIRED
   ```
   und benutzt die Variable, die den Klausel-Text einsetzt
   (`seller.paragraph_19_clause`).
2. `pdf::klausel_check::verify(template_source, is_kleinunternehmer=true)`
   sucht beides — Marker **und** Variable-Referenz.
3. Fehlt eines, wird der Render-Aufruf **abgebrochen** (kein still-broken
   Beleg). Im UI erscheint ein Fehler: „Das gewählte Template enthält keinen
   §19-Klausel-Block. Bitte ein §19-fähiges Template wählen."

**Konsequenz:** Wer eigene Templates baut (`inputs/pdf-templates/*.typ`) und
sie für §19-Rechnungen benutzen will, muss den Marker und die Variable
einsetzen. Wer sie für eine reine Regelbesteuerungs-Rechnung baut, kann den
Marker weglassen — dann ist das Template halt nur für Nicht-§19 zugelassen.

Manuel hat eine stehende Erlaubnis, alle PDF-Vorlagen direkt zu editieren —
die `inputs/`-Tabu-Regel hat genau diese eine Ausnahme (siehe CLAUDE.md).
Wer den Klausel-Block aus Versehen entfernt, fängt sich beim nächsten
§19-Render einen klaren Fehler. Das ist die Idee.

---

## 5. §14c-Schutz im Backend

§14c UStG sagt: Wer USt **ausweist** ohne berechtigt zu sein, schuldet sie
auch (selbst wenn er Kleinunternehmer ist und sie eigentlich nicht hätte
ausweisen dürfen). Das wäre ein teurer Bug. Klein.Buch verhindert das
mehrfach:

1. **UI-Sperren** (siehe §2) — der Nutzer kann die Felder gar nicht ausfüllen,
   solange `is_kleinunternehmer = true`.
2. **Domain-Validierung.** `domain::invoice::validate_for_issue` prüft beim
   Lock, dass für jedes Item `tax_category_code = 'E'` und
   `tax_rate_per_mille = 0` und `tax_amount_cents = 0` gilt, **wenn** der
   `seller_profile.is_kleinunternehmer = true` ist. Verstoß ⇒ Lock schlägt fehl.
3. **Generator-Layer.** `einvoice::generator::build_cii` setzt für §19
   automatisch BT-22, Tax-Category `E`, Tax-Amount 0 — selbst wenn ein Item
   versehentlich anderes mitbringen würde, wird es im XML überschrieben (mit
   Audit-Warnung, weil das eigentlich nicht passieren dürfte).
4. **Klausel-Check** (siehe §4) — keine §19-Rechnung ohne Klausel-Hinweis im
   PDF.
5. **Storno-Symmetrie.** Wird eine §19-Rechnung storniert, ist der Storno
   auch §19. `domain::storno::make_storno` übernimmt `is_kleinunternehmer`
   und Klausel-Lage aus dem Original.

**Verzicht auf §19** schaltet das alles um: `is_kleinunternehmer = false`
bewirkt, dass `validate_for_issue` einen USt-Block **verlangt** (sonst
Fehler), dass der Generator BT-22 weglässt und Tax-Categories nach den
tatsächlichen Item-Werten setzt, und dass der Klausel-Check für §19-Marker
keinen Eintritt fordert.

---

## 6. Verzicht auf §19 und 5-Jahres-Bindung

§19 Abs. 2 UStG erlaubt den **Verzicht** auf die Kleinunternehmer-Regelung —
sinnvoll z. B., wenn überwiegend B2B-Kunden Vorsteuer ziehen wollen. Wer
verzichtet, ist **5 Jahre an die Regelbesteuerung gebunden**.

**Im UI** ist das ein Toggle in den Verkäuferprofil-Einstellungen mit
**ausführlichem** Warn-Dialog, der die 5-Jahres-Bindung erklärt und nach
einer expliziten Bestätigung verlangt. Bei Bestätigung wird
`seller_profile.waived_paragraph_19_since = TODAY` gesetzt.

**Im Code** prüft `domain::kleinunternehmer::waiver_window(seller, today)`,
ob der 5-Jahres-Zeitraum noch läuft (`waived_paragraph_19_since` plus 5 Jahre,
Kalenderjahre, nicht 1825 Tage). Während der Bindung ist das Zurückschalten
auf `is_kleinunternehmer = true` über das UI **gesperrt**. Wer das trotzdem
versucht, bekommt einen Hinweis: „Bindung läuft noch bis $YEAR. Verzicht
ist bindend."

**Nach Ablauf der Bindung** kann der Toggle wieder gesetzt werden — das ist
eine fachliche Entscheidung des Nutzers; `domain::kleinunternehmer::
reactivate_check` prüft nur die Bindung, nicht die Berechtigung (die hängt
am Vorjahres-Umsatz, den Klein.Buch zwar berechnen könnte, aber nicht
verbindlich vor-prüft — das macht der Steuerberater).

---

## 7. Was passiert mit Belegen aus der §19-Zeit nach dem Verzicht

GoBD-Frage: was ist mit den alten §19-Rechnungen, wenn der Nutzer ab Datum X
verzichtet? Antwort: **nichts ändert sich an den alten Belegen.** Sie sind
festgeschrieben und bleiben so, wie sie waren — mit BT-22-Klausel, ohne USt-
Ausweis. Das ist auch korrekt, denn zum Zeitpunkt ihrer Erzeugung war der
Aussteller Kleinunternehmer.

Neue Belege ab Verzichts-Datum laufen unter Regelbesteuerung. Der EÜR-Report
ist mit Datum-Filter pro Geschäftsjahr — wer das Übergangsjahr ansieht, sieht
beides nebeneinander.

---

## 8. Edge-Cases

### 8.1 Kleinbetragsrechnung (≤ 250 € Brutto)

§33 UStDV erlaubt vereinfachte Pflichtangaben. `domain::kleinbetragsrechnung::
is_small_amount(total_cents)` prüft das. Die PDF-Templates haben einen
optionalen Block, der Kleinbetrags-Layouts erlaubt. Für §19-Belege ändert
sich am Klausel-Erfordernis nichts.

### 8.2 Abo-Rechnungen mit §19 (Phase 4)

Wenn die Vorlage `is_kleinunternehmer = true` hat (aus dem aktuellen
`seller_profile`), wird das in jeden materialisierten Beleg übernommen.
Wechselt der Nutzer mitten in einer Abo-Periode die §19-Lage (Verzicht),
gilt für die *nächste* materialisierte Rechnung die neue Lage — Klein.Buch
warnt nicht aktiv, weil das eine bewusste Entscheidung des Nutzers ist.
Audit-Eintrag des Verzichts ist im Log; wer es nachvollziehen will, findet es.

### 8.3 E-Rechnung-Empfang von Lieferanten mit USt

Eingangsseitig (`expenses`) macht §19 **keinen Unterschied** an der
Verarbeitung. Ein Lieferant darf USt ausweisen; der Kleinunternehmer kann sie
nur nicht als Vorsteuer ziehen. Eingangsbelege landen brutto in der EÜR
(ADR 0019). Der Empfangs-Parser akzeptiert XRechnungen und ZUGFeRD-PDFs mit
allen Tax-Categories, der Klein.Buch-Nutzer ist „nur Empfänger".

---

## 9. Was im UI explizit kommuniziert wird

- Im Verkäuferprofil: „Kleinunternehmer nach §19 UStG (Default — siehe
  Hilfe)" mit Link ins Handbuch-Kapitel „Recht & Steuern" (G2-DOC.2.4).
- Bei jedem PDF-Render einer §19-Rechnung wird die Klausel sichtbar
  ausgegeben, in der Regel im Fuß des Beleg-Bereichs.
- Im EÜR-Report wird `is_kleinunternehmer` ausgewiesen, damit der
  Steuerberater es beim Drüberlesen sofort sieht.

---

## 10. Tests

- `domain::kleinunternehmer::tests`: Klausel-Text ist wortgleich; 5-Jahres-
  Fenster greift bei Tag 0, Tag-1-vor-Ende, Tag-genau-Ende, Tag-nach-Ende.
- `domain::invoice::tests`: Validate schlägt fehl bei §19 + Tax-Rate > 0; OK
  bei §19 + alle Items `E`/0; OK bei Verzicht + USt-Block.
- `einvoice::generator::tests`: BT-22 erscheint, Item-Tax-Category `E`,
  BG-23 mit Total-Amount 0.
- `pdf::klausel_check::tests`: Marker + Variable beide nötig; Variable allein
  reicht nicht; nur Marker reicht nicht; korrekt zugelassenes Template
  passiert.
- `commands::settings::tests`: Verzicht-Toggle schreibt
  `waived_paragraph_19_since`; Reaktivierungs-Versuch während der Bindung
  schlägt fehl; nach Ablauf der Bindung geht es.
- `commands::recurring_invoice::tests` (KB-0053): Speichern einer Abo-
  Vorlage mit `tax_category_code != 'E'` und §19-Seller wird abgelehnt.

---

## Letzte Verifikation

Stand: 2026-05-26, ADR 0005. Quelle: `domain::kleinunternehmer`,
`domain::invoice::validate_for_issue`, `einvoice::generator::build_cii`,
`pdf::klausel_check`, `commands::settings::seller_profile_upsert`.
