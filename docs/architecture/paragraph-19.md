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

**Klein.Buch ist primär ein §19-Tool.** Ohne Eintrag gilt §19. Seit Migration
0033 (ADR 0042) steht der Status in der append-only Historie
`tax_status_changes`: ab einem Wirkungsdatum gilt ein Status, mit Grund
(Verzicht, Wegfall wegen des Vorjahres, im laufenden Jahr oder im
Gründungsjahr, Rückkehr). Maßgeblich ist der Status am Datum des Belegs
(Leistungsdatum, sonst Rechnungs- bzw. Angebotsdatum; bei Kosten das
Belegdatum). `seller_profile.is_kleinunternehmer` und
`waived_paragraph_19_since` sind nur noch Spiegel, die kein Code liest.

---

## 2. UI-Sperren

Gilt am **maßgeblichen Datum des Belegs** §19 (ADR 0042 Nr. 3: Leistungsdatum,
sonst Rechnungs- bzw. Angebotsdatum; Abo-Vorlage: nächster Termin), sperrt das
Frontend alle USt-relevanten Eingabefelder. Den Status je Datum holen die
Editoren über `tax_status_at` (`src/lib/taxStatusAt.svelte.ts`, je Datum
zwischengespeichert); `applyTaxRules` (`src/lib/tax.ts`) läuft nur mit dem
Status genau dieses Datums, damit ein veralteter Wert die Sätze eines geladenen
Entwurfs nicht umschreibt. Weicht der Status vom heutigen ab, zeigt
`TaxStatusNotice` einen Hinweis („Am Leistungsdatum gilt die Regelbesteuerung.“).

- **Rechnungs- und Angebots-Items.** Tax-Rate-Feld ist ausgeblendet,
  Tax-Category ist auf `E` (Exempt) fest verdrahtet, kein Switch zwischen
  19/7/0. Ändert sich das Datum über einen Wechsel, stellt das Formular sofort
  um (§19 → `E`/0; Regel → `S`/19 % für `E`-Positionen).
- **Umwandlung Angebot → Rechnung.** Status am Leistungsdatum der Rechnung;
  weicht er vom eingefrorenen Status des Angebots ab, Hinweis „Anderer
  Steuerstatus als im Angebot.“ (`quoteConversionHint`).
- **Pakete.** `PackageItemAdder` übergibt das maßgebliche Datum an
  `package_materialize_item` (`documentDate`).
- **Settings/PDF-Vorlagen.** Templates ohne §19-Klausel sind gesperrt, solange
  §19 heute oder ab einem eingetragenen künftigen Wechsel gilt
  (`kleinTodayOrLater`, wie der Server).
- **Recurring-Invoice-Templates.** Beim Speichern prüft der Server die
  Positionen gegen den Status am ersten Termin, im Lauf je Termin (KB-0053,
  ADR 0042 Nr. 3).

Der Status selbst ändert sich nur im Abschnitt „Steuerstatus“ von „Meine
Firmendaten“ (`routes/settings/seller/TaxStatusSection.svelte`): Verlauf,
Status heute, Gesamtumsatz mit Grenzhinweisen, Gefahr-Dialog „Statuswechsel
eintragen?“ (`alertdialog`, Grund mit Erklärung, Wirkungsdatum mit Vorschlag
und 1.-Januar-Prüfung, Bindungshinweis beim Verzicht, Vorschau der Folgen über
`tax_status_change_preview`, § 15a UStG, Re-Auth) und „Eintrag widerrufen?“ für
den jüngsten Eintrag. Er löst den Schalter und den §19-Wechseldialog ab
(ADR 0041 Nr. 21/220/251).

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
   `tax_rate_per_mille = 0` und `tax_amount_cents = 0` gilt, **wenn** am
   Leistungs- bzw. Rechnungsdatum §19 gilt (Statushistorie, ADR 0042 Nr. 3). Verstoß ⇒ Lock schlägt fehl.
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

## 6. Statushistorie, Gründe und Bindung (ADR 0042 Nr. 1–6)

Der Status ist kein Schalter mehr, sondern eine Folge von Einträgen in
`tax_status_changes` (Migration 0033): Wirkungsdatum, Status, Grund,
Bindungsende, auslösender Beleg, Notiz. Kein Löschen, kein Ändern; ein
irrtümlicher Eintrag wird mit Grund widerrufen (nur der jüngste gültige,
Trigger `trg_tax_status_changes_revoke_only`).

| Grund | Status | Wirkungsdatum | Bindung |
|---|---|---|---|
| Verzicht (§ 19 Abs. 3 UStG) | Regel | 1. Januar | fünf Kalenderjahre einschließlich des Wirkungsjahres: Verzicht ab 2026 → Rückkehr frühestens 01.01.2031 |
| Wegfall wegen des Vorjahres (> 25.000 €) | Regel | 1. Januar | keine |
| Wegfall im laufenden Jahr (> 100.000 €) bzw. im Gründungsjahr (> 25.000 €) | Regel | Tag des Umsatzes, der die Grenze überschreitet | keine |
| Rückkehr | §19 | 1. Januar | nach einem Verzicht erst ab Bindungsende |

**Prüfung** (`domain::tax_status::validate_change`, rein): Grund passt zum
Status und zum Wirkungsdatum; ein neuer Eintrag wirkt nach dem jüngsten
gültigen (keine Überschneidung) und ändert den Status; ein Wegfall liegt nicht
in der Zukunft; die Rückkehr verletzt keine Bindung; kein abgeschlossenes
Geschäftsjahr ändert seinen Status (der Jahres-Snapshot hängt über die
Vorsteuer aktivierter Kosten davon ab). Die Commands prüfen über
`validate_change_with` bzw. `validate_revoke_with` zusätzlich aktivierte
Kosten, die in einem abgeschlossenen Jahr gezahlt wurden und deren Belegdatum
danach liegt (`CapitalizedInputTax`): Ändert sich ihr Status, verschöbe sich
Vorsteuer ins abgeschlossene Jahr (`ClosedYearInputTax`). Warnungen, die
nicht sperren (`change_warnings`, Feld `warnings` der Vorschau): Verzicht nach
Ablauf der Erklärungsfrist (Ende Februar des übernächsten Jahres, § 19 Abs. 3
Satz 1), Rückkehr bei einem Vorjahresumsatz über 25.000 €.

**Commands** (`commands::tax_status`): `tax_status_overview`, `tax_status_at`,
`tax_status_change_preview`, `tax_status_change_add`,
`tax_status_change_revoke`. Eintrag und Widerruf verlangen eine frische
Re-Auth (Registry) und schreiben Eintrag, Audit (`tax_status.changed` bzw.
`tax_status.revoked`) und Spiegel in einer Transaktion unter
`locks::SELLER_PROFILE`; ein Jahresabschluss wartet so lange. Danach meldet die
Inbox festgeschriebene Rechnungen, deren eingefrorener Status nicht mehr passt
(nicht geändert; Berichtigung nach § 31 Abs. 5 UStDV bzw. Storno und
Neuausstellung mit der Steuerberatung), Entwürfe und Abo-Vorlagen mit
unpassender Steuerart, betroffene Anlagen aus Kosten und rückwirkend geänderte
Auswertungen offener Jahre. Die Dedup-Schlüssel der Hinweise nennen die
Aktion (`tax_status_affected:add:<id>` bzw. `…:revoke:<id>`), damit der
Widerruf eigene Belege meldet. `seller_profile_upsert` ändert den Status nicht;
ein abweichender `isKleinunternehmer` im Input wird abgelehnt.

**Festschreiben** (Rechnung, Angebot): Der Status wird am maßgeblichen Datum
bestimmt und in der Schreibtransaktion des Locks gegen die Historie erneut
geprüft (`LockUpdate::tax_status_date`); ein Statuseintrag dazwischen bricht
das Festschreiben ab, statt den alten Status einzufrieren. Detailseiten von
Entwürfen zeigen den Status ihres Datums wie die Editoren, festgeschriebene den
eingefrorenen Wert.

**Gesamtumsatz und Grenzen** (ADR 0042 Nr. 5): Gesamtumsatz nach
vereinnahmten Entgelten, netto, ohne Anlagenverkäufe
(`domain::tax_status::gesamtumsatz`); Grenzprüfung für Vorjahr (25.000 €),
laufendes Jahr (100.000 €) und vermutetes Gründungsjahr (25.000 €, wenn
Klein.Buch keinen früheren Umsatz kennt) mit dem auslösenden Beleg als
Vorschlag. Hinweis über die Regel `rule_tax_threshold` (täglich und nach
jedem Eintrag); den Wechsel trägt der Nutzer selbst ein.

**Übernahme des Bestands** (Nr. 6): Die Migration bildet den alten Schalter so
ab, dass jedes Jahr dasselbe Ergebnis behält: Anfangsstand §19 bzw. Regel
„seit Beginn“, ein Verzicht als Eintrag `legacy` zum 1. Januar seines Jahres,
Bindung nach Nr. 2.

---

## 7. Was passiert mit Belegen aus der §19-Zeit nach dem Verzicht

GoBD-Frage: was ist mit den alten §19-Rechnungen, wenn der Nutzer ab Datum X
verzichtet? Antwort: **nichts ändert sich an den alten Belegen.** Sie sind
festgeschrieben und bleiben so, wie sie waren — mit BT-22-Klausel, ohne USt-
Ausweis. Das ist auch korrekt, denn zum Zeitpunkt ihrer Erzeugung war der
Aussteller Kleinunternehmer.

Neue Belege ab dem Wirkungsdatum laufen unter Regelbesteuerung. Im
Übergangsjahr zeigen EÜR-Report und Ausfüllhilfe den Jahresstatus „Wechsel im
Jahr“ (`YearStatus::Wechsel`) mit den Abschnitten; Einnahmen folgen dem
eingefrorenen Status jeder Rechnung, Kosten dem Status am Belegdatum (vor dem
Stichtag brutto, danach netto mit Vorsteuer, auch im DATEV-Stapel).

---

## 8. Edge-Cases

### 8.1 Kleinbetragsrechnung (≤ 250 € Brutto)

§33 UStDV erlaubt vereinfachte Pflichtangaben. `domain::kleinbetragsrechnung::
is_small_amount(total_cents)` prüft das. Die PDF-Templates haben einen
optionalen Block, der Kleinbetrags-Layouts erlaubt. Für §19-Belege ändert
sich am Klausel-Erfordernis nichts.

### 8.2 Abo-Rechnungen mit §19 (Phase 4)

Jeder Termin trägt den Status seines Leistungsdatums (ADR 0042 Nr. 3). Ein
nachgeholter Termin vor einem Wechsel bleibt §19; passt die Steuerart der
Vorlage nicht zum Status am Termin, entsteht dort keine Rechnung (Hinweis,
keine Nummer), bis die Vorlage umgestellt ist.

### 8.3 E-Rechnung-Empfang von Lieferanten mit USt

Eingangsseitig (`expenses`) macht §19 **keinen Unterschied** an der
Verarbeitung. Ein Lieferant darf USt ausweisen; der Kleinunternehmer kann sie
nur nicht als Vorsteuer ziehen. Eingangsbelege landen brutto in der EÜR
(ADR 0019). Der Empfangs-Parser akzeptiert XRechnungen und ZUGFeRD-PDFs mit
allen Tax-Categories, der Klein.Buch-Nutzer ist „nur Empfänger".

---

## 9. Was im UI explizit kommuniziert wird

- In „Meine Firmendaten“, Abschnitt „Steuerstatus“: Status heute, Bindung und
  frühestmögliche Rückkehr, eingetragener künftiger Wechsel, Gesamtumsatz
  Vorjahr/laufendes Jahr mit Grenzhinweisen (auslösender Beleg, Vorschlag für
  das Wirkungsdatum, „Klein.Buch wechselt nie selbst“) und der Verlauf mit
  Wirkungsdatum, Status, Grund, Bindung und Widerruf; Link ins
  Handbuch-Kapitel „Kleinunternehmer-Regelung“.
- In den Editoren: Status am Datum des Belegs, mit Hinweis bei Abweichung
  von heute.
- Bei jedem PDF-Render einer §19-Rechnung wird die Klausel sichtbar
  ausgegeben, in der Regel im Fuß des Beleg-Bereichs.
- Im EÜR-Report wird der Jahresstatus ausgewiesen (`yearStatus`, bei einem
  Wechsel im Jahr mit den Zeiträumen aus `statusChange`), damit der
  Steuerberater es beim Drüberlesen sofort sieht; die ELSTER-Ausfüllhilfe
  nennt den Wechsel über der Tabelle.

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
- `domain::tax_status::tests`: Status am Stichtag (inklusiv), 1.-Januar-Regeln
  je Grund, Bindung bis Wirkungsjahr + 5, Rückkehr nach Wegfall ohne Sperre,
  mehrfacher Wechsel, Widerruf, abgeschlossene Jahre, Grenzprüfung.
- `tests/migration_0033_test.rs`: Append-only, CHECKs, Übernahme des Bestands
  (2000–2040 wie bisher), Spiegel.
- `tests/tax_status_flow_test.rs`: Wegfall am 15.08. (Rechnung 14.08. §19,
  15.08. Regel), Verzicht 2026 → Rückkehr 2031, Wegfall Vorjahr → Rückkehr im
  Folgejahr, Rückwirkung im abgeschlossenen Jahr, Widerruf, Mischjahr in
  EÜR/ELSTER/DATEV, Grenzhinweis, Paket und Angebot → Rechnung nach Datum;
  Hinweis nach dem Widerruf, Vorsteuer aktivierter Kosten im abgeschlossenen
  Jahr, erneute Statusprüfung im Lock, Warnungen der Vorschau.
- `db::repo::tax_status::tests::mirror_columns_are_written_only_here`: keine
  Lesestelle der Spiegelspalten.
- `commands::recurring_invoice::tests` (KB-0053): Speichern einer Abo-
  Vorlage mit `tax_category_code != 'E'` und §19-Seller wird abgelehnt.
- `src/lib/taxStatus.test.ts` (vitest): maßgebliches Datum, Status am Stichtag,
  wählbare Gründe nach dem jüngsten Eintrag, Vorschläge für das Wirkungsdatum,
  1.-Januar- und Zukunftsprüfung, Bindungsende, Texte (Grenzhinweis, Folgen,
  Editor-Hinweis, Umwandlung), `kleinTodayOrLater`, Zwischenspeicher je Datum.

---

## Letzte Verifikation

Stand: 2026-05-26, ADR 0005; 2026-10-27 (Lauf 2026.11.1, V1): §1, §5, §6, §7,
§8.2, §10 auf die Statushistorie (ADR 0042) umgestellt; §2 und §9 (Oberfläche) gegen
`src/lib/taxStatus.ts`, `src/lib/taxStatusAt.svelte.ts`,
`routes/settings/seller/TaxStatusSection.svelte` und die Editoren geprüft. Quelle:
`domain::tax_status`, `db::repo::tax_status`, `commands::tax_status`,
`domain::kleinunternehmer`,
`domain::invoice::validate_for_issue`, `einvoice::generator::build_cii`,
`pdf::klausel_check`, `commands::settings::seller_profile_upsert`.
