# ADR 0042 — Fachliche Korrekturen 2026.11.1

**Status:** In Arbeit · 2026-10-27 · Lauf 2026.11.1. Migrationen 0033–0035 (Schema 35).
Ergänzt ADR 0019, 0021, 0023, 0027, 0033, 0039 und 0041 (Nr. 21, 91, 220, 251); PRD-Revision 11.

## Kontext

Die Abschlussprüfung von Lauf 2026.11 und der Doku-Abgleich U6 fanden fachliche Lücken, die
2026.11.0 offen ließ: Die Rückkehr zur Kleinunternehmerregelung nach einem gesetzlichen Wegfall war
fünf Jahre gesperrt, einen Wechsel mitten im Jahr kannte Klein.Buch nicht, eine falsch gebuchte
Zahlung ließ sich nicht korrigieren, ein Leistungszeitraum fehlte, Angebots-Entwürfe ließen sich
nicht bearbeiten, und der DATEV-Buchungsstapel wich in drei Punkten von der Formatbeschreibung ab.
Bei der Kartierung kam dazu: Die Zahlungshistorie einer festgeschriebenen Rechnung lag als
JSON-Spalte vor, die kein Trigger schützte.

Maßstab sind Gesetz, Verwaltungsanweisungen und Standards, nicht Vorlieben. Jede Entscheidung
nennt ihre Fundstelle. Festgeschriebene Belege bleiben unverändert (GoBD-Hardline), die
§19-Hardline gilt weiter: Solange ein Beleg unter § 19 UStG fällt, weist er keine Umsatzsteuer aus.

## Entscheidungen

### Steuerstatus nach § 19 UStG (Migration 0033)

1. **Statushistorie statt Schalter.** Der Status steht in einer append-only Tabelle
   `tax_status_changes` (Wirkungsdatum, Status, Grund, Bindungsende, auslösender Beleg, Notiz).
   Kein Löschen, kein Ändern; ein irrtümlicher Eintrag wird mit Grund widerrufen (`revoked_at`), wie
   beim Backup- und Mail-Protokoll. `seller_profile.is_kleinunternehmer` und
   `waived_paragraph_19_since` bleiben nur als Spiegel für ältere Leser stehen und sind nicht mehr
   maßgeblich. Fundstelle: § 146 Abs. 4 AO, GoBD Rz. 58 ff. (Nachvollziehbarkeit von Änderungen).
2. **Gründe und Wirkung** (§ 19 Abs. 1 und 3 UStG in der Fassung ab 2025; BMF-Schreiben vom
   18.03.2025, Abschn. 19.1 UStAE):
   - *Verzicht* (§ 19 Abs. 3): wirkt ab dem 1. Januar des Jahres, für das er gilt, und bindet fünf
     Kalenderjahre einschließlich des Wirkungsjahres. Ein Verzicht ab 2026 erlaubt die Rückkehr
     frühestens zum 01.01.2031. Die bisherige Rechnung (Wirkungsjahr + 6) war ein Jahr zu lang.
   - *Wegfall wegen des Vorjahres* (Gesamtumsatz des Vorjahres über 25.000 €): ab 1. Januar, ohne
     Bindung.
   - *Wegfall im laufenden Jahr* (über 100.000 €) und *im Gründungsjahr* (über 25.000 €): ab dem
     Umsatz, mit dem die Grenze überschritten wird; dieser Umsatz ist schon steuerpflichtig, die
     vorherigen bleiben steuerfrei. Ohne Bindung.
   - *Rückkehr*: zum 1. Januar eines Jahres; nach einem Wegfall ohne Sperre, nach einem Verzicht
     frühestens nach Ablauf der Bindung (Widerruf nur mit Wirkung vom Beginn eines Kalenderjahres).
3. **Maßgebliches Datum je Beleg.** Ausgangsbelege: Leistungsdatum, bei einem Leistungszeitraum
   dessen Ende, ersatzweise das Rechnungsdatum (§ 13 Abs. 1 Nr. 1 Buchst. a UStG). Kosten: das
   Belegdatum. Beim Festschreiben wird der Status je Beleg wie bisher eingefroren (Trigger
   unverändert); Entwürfe zeigen den Status ihres Datums.
4. **Rückwirkende Einträge** sind nur in offenen Geschäftsjahren erlaubt (der Verzicht ist bis Ende
   Februar des übernächsten Jahres möglich, § 19 Abs. 3 Satz 1). Festgeschriebene Rechnungen werden
   nicht umgeschrieben; betroffene Belege meldet Klein.Buch als Hinweis zur Prüfung (Berichtigung
   nach § 31 Abs. 5 UStDV bzw. Storno und Neuausstellung, Steuerberatung).
5. **Gesamtumsatz als Hilfe.** Klein.Buch zeigt den Gesamtumsatz nach vereinnahmten Entgelten
   (§ 19 Abs. 2, ohne Umsätze mit Anlagevermögen) für Vorjahr und laufendes Jahr und meldet als
   Hinweis, wenn eine Grenze erreicht oder überschritten ist, mit dem auslösenden Beleg als
   Vorschlag für das Wirkungsdatum. Den Wechsel trägt der Nutzer ein; ein automatischer Wechsel
   würde ohne sein Wissen Rechnungen mit Umsatzsteuer erzeugen.
6. **Übernahme des Bestands.** Die Migration bildet den bisherigen Stand so ab, dass für jedes Jahr
   dasselbe Ergebnis entsteht wie bisher (Wirkungsdatum 1. Januar), damit abgeschlossene Jahre und
   ihre Snapshots unverändert bleiben; das Bindungsende folgt Nr. 2.

### Zahlungen (Migration 0034)

7. **Zahlungsbuch.** Zahlungen auf Ausgangsrechnungen stehen in der append-only Tabelle
   `invoice_payments` (Zahlung oder Gegenbuchung, Betrag, Datum, Konto, Grund, Verweis auf die
   stornierte Zahlung). `paid_amount_cents`, `paid_at`, `status` und die bisherige JSON-Historie
   sind daraus abgeleitet und per Trigger an das Buch gebunden. Die Migration übernimmt die
   vorhandene Historie und bricht bei Abweichungen mit einer benannten Meldung ab (ADR 0039
   Nr. 124). Fundstelle: § 146 Abs. 4 AO, § 239 Abs. 3 HGB, GoBD Rz. 58–60, 64.
8. **Zahlung stornieren.** Eine falsch erfasste Zahlung wird durch eine Gegenbuchung neutralisiert:
   gleicher Betrag mit umgekehrtem Vorzeichen, Datum und Konto der Originalbuchung, Grund Pflicht,
   Re-Auth, Audit in derselben Transaktion. Danach wird richtig neu gebucht. Die Gegenbuchung wirkt
   am ursprünglichen Zahldatum, weil eine nie geflossene Zahlung kein Zufluss nach § 11 EStG war.
   Nur im offenen Geschäftsjahr des Originaldatums; im abgeschlossenen Jahr lehnt Klein.Buch ab und
   verweist auf die Berichtigung der Erklärung (§ 153 AO). Echte Rückzahlungen bleiben Storno mit
   Erstattung (am Tag der Rückzahlung). Nicht bei Storno-Belegen, stornierten Rechnungen oder
   offenem Storno-Entwurf.
9. **Auswertungen.** EÜR, ELSTER-Ausfüllhilfe, DATEV-Stapel und Steuerberater-Paket berücksichtigen
   Gegenbuchungen; im DATEV-Stapel als Buchung mit umgekehrtem Soll/Haben und Verweis im Belegfeld.

### Leistungszeitpunkt und Leitweg-ID (Migration 0035)

10. **Leistungszeitraum.** Rechnungen (auch Entwürfe und Abo-Vorlagen) können statt eines
    Leistungsdatums einen Zeitraum von–bis tragen. Fundstelle: § 14 Abs. 4 Satz 1 Nr. 6 UStG,
    § 31 Abs. 4 UStDV (Kalendermonat genügt), EN 16931 BG-14 (BT-73/BT-74). Ist ein Zeitraum
    gesetzt, entfällt BT-72 und die PDF zeigt „Leistungszeitraum“. Ohne Datum und Zeitraum gilt
    der bisherige Rückfall auf das Rechnungsdatum. Abo-Rechnungen setzen den Abrechnungszeitraum
    (auch quartalsweise, halbjährlich, jährlich). Ein Storno übernimmt den Zeitraum des Originals.
    Zeitraum und Leistungsdatum sind nach dem Festschreiben unveränderlich (Trigger erweitert).
11. **Vorausrechnung.** Das Leistungsdatum darf nach dem Rechnungsdatum liegen (Rechnung vor
    Ausführung der Leistung, § 14 Abs. 4 Satz 1 Nr. 6 UStG; bei Kleinunternehmern verlangt
    § 34a UStDV den Leistungszeitpunkt nicht). Die bisherige Regel „Leistungsdatum nicht nach dem
    Rechnungsdatum“ war strenger als das Gesetz.
12. **Leitweg-ID am Entwurf.** Die Leitweg-ID (BT-10) wird am Entwurf gespeichert und beim
    Ausstellen übernommen (revidiert ADR 0041 Nr. 91); nach dem Festschreiben unveränderlich. Der
    Storno liest sie weiter aus der archivierten E-Rechnung (ADR 0041 Nr. 250).

### Angebote

13. **Entwürfe bearbeiten.** Ein Angebot, das noch Entwurf ist, lässt sich bearbeiten. Es ist weder
    Handelsbrief noch Buchungsbeleg; die Unveränderbarkeit beginnt mit dem Fertigstellen
    (GoBD Rz. 58 ff., Trigger unverändert). Ein nach einem Restore vorgemerkter Entwurf behält seine
    Sperre (ADR 0040 Nr. 13).

### Vorsteuerberichtigung

14. **Übersicht nach § 15a UStG.** Bei einem Statuswechsel zeigt Klein.Buch für Anlagen mit bekannter
    Vorsteuer den Berichtigungszeitraum (fünf Jahre ab erstmaliger Verwendung, § 15a Abs. 1;
    ersatzweise das Anschaffungsdatum), den jährlichen Anteil (§ 15a Abs. 5) und die
    Vereinfachungen nach § 44 UStDV (keine Berichtigung bei Vorsteuer bis 1.000 €). Nur eine
    Übersicht für die Erklärung, keine Buchung; Hinweis auf die Steuerberatung.

### DATEV-Buchungsstapel

15. **Format nach der Formatbeschreibung** (DATEV Developer Portal, „DATEV-Format“, Header und
    Buchungsstapel): Dateiname mit dem Präfix `EXTF_`; Berater- und Mandantennummer sind Pflicht
    (Feld 11 `^(\d{4,6}|\d{7})$`, Feld 12 `^\d{1,5}$`, kein Leerfeld); leere Textfelder im Kopf als
    `""`. Formatversion 12 und einstellige BU-Schlüssel bleiben, solange Prüfprogramm oder Import
    nichts anderes zeigen (Version 12 ist weiter zulässig; Fremdprogramme wie Lexware lesen nur bis
    12). Ein Test prüft Kopf und Buchungsfelder gegen die Regeln der Formatbeschreibung. Die Aussage
    „vom Steuerberater bestätigt“ in der Doku war unbelegt und entfällt; der Import gilt als
    ungeprüft, bis ein Prüfprotokoll vorliegt.

### Übrige Punkte

16. Kleinere Punkte aus Lauf 2026.11 (Rückstand-Vorschau der Abo-Läufe, Restore-Hinweis vor dem
    Speichern, Karten-Hierarchie, Hinweisflächen auf `Banner`, Klassen-Aufräumen, Handbuch aus dem
    Start-Bundle, Pool-Zeitlimit in Tests) folgen ADR 0041; Einzelheiten in der Befundliste.

### Umsetzung V1 (Ergänzungen zu Nr. 1–6)

17. **Abschnitt „Steuerstatus“** unter „Meine Firmendaten“ löst den Schalter und den
    §19-Wechseldialog ab (ADR 0041 Nr. 21, 220, 251): Historie, aktueller Status, Gesamtumsatz
    mit Grenzhinweis, Dialog „Statuswechsel eintragen“ mit Gründen nach Nr. 2, Re-Auth.
    Vorschlag für das Wirkungsdatum eines Verzichts ist der nächste 1. Januar; eine Rückwirkung
    trägt der Nutzer bewusst von Hand ein.
18. **Warnen statt sperren**, wo Klein.Buch die Tatsachen nicht sicher kennt: ein Verzicht nach
    Ablauf der Erklärungsfrist (Ende Februar des übernächsten Jahres) und eine Rückkehr bei einem
    in Klein.Buch erfassten Vorjahresumsatz über 25.000 € (Umsätze außerhalb von Klein.Buch fehlen).
19. **Abgeschlossene Jahre** schützt die Prüfung auch über die Vorsteuer aktivierter Kosten, die im
    abgeschlossenen Jahr gezahlt wurden (Snapshot); das Festschreiben einer Rechnung und das
    Fertigstellen eines Angebots prüfen den Status in der Schreibtransaktion erneut.
20. **Übernommene Wechsel** (`legacy`, aus dem bisherigen Schalter) binden wie ein Verzicht, weil
    Klein.Buch den Grund nicht kennt. War es ein gesetzlicher Wegfall, widerruft der Nutzer den
    Eintrag mit Grund und trägt ihn richtig ein; das Audit hält beides fest. Die übernommenen
    Einträge tragen feste IDs im UUIDv7-Format (Zeitstempel 0), damit die Migration reines SQL
    bleibt und ein Restore dieselben IDs ergibt.
21. **Paket-Revisionen** behalten den Status von heute (Katalogpreis); beim Einfügen in einen Beleg
    gilt der Status am Belegdatum.

### Umsetzung V3 (Ergänzungen zu Nr. 10–12)

22. **Abo-Vorlagen** tragen keinen eigenen Zeitraum: Jede Abo-Rechnung erhält den Abrechnungszeitraum
    vom Fälligkeitstag bis zum Tag vor der nächsten Fälligkeit (lückenlos auch am Monatsende);
    der Klartext in Position 1 bleibt („Juni 2026“ bzw. „01.04.2026 – 30.06.2026“). Status-Tag für
    § 19 ist das Ende des Zeitraums.
23. **Eigene PDF-Vorlagen** ohne Leistungszeitraum: Für Rechnungen mit Zeitraum nimmt Klein.Buch
    die eingebaute Vorlage und meldet das in den Hinweisen (Muster wie beim Storno).
24. **Rechnungen vor Migration 0035** zeigen ihre Leitweg-ID nicht in der Detailansicht; maßgeblich
    bleibt die archivierte E-Rechnung, aus der auch der Storno liest (ADR 0041 Nr. 250). Ein
    Rückschreiben aus dem Archiv in festgeschriebene Rechnungen unterbleibt (GoBD).

### Umsetzung V4 (Ergänzungen zu Nr. 13 und 16)

25. **Vorgemerkte Angebots-Entwürfe** nach einem Restore lassen sich bearbeiten wie Rechnungs-Entwürfe;
    gesperrt bleibt das Fertigstellen, bis der Abgleich bestätigt ist, und jede spätere Änderung merkt
    den Entwurf wieder vor (ADR 0040 Nr. 13). Die Editoren zeigen den Zustand vor dem Speichern
    (Lesefeld `restoreConflict`).
26. **Angebots-Editor bei Regelbesteuerung** bietet USt-Satz und USt-Art wie der Rechnungs-Editor an;
    der Server prüft wie bisher, unter § 19 bleiben E/0 erzwungen.
27. **Rückstand-Vorschau** rechnet aus derselben Funktion wie der Lauf (Anzahl, Daten bis zur Grenze
    von 24); oberhalb der Grenze ist „Ab heute fortsetzen“ ein Gefahr-Dialog mit Fokus auf
    „Abbrechen“, weil das Überspringen nicht umkehrbar ist.

## Konsequenzen

- Schema 35. Ein Image 2026.11.0 kann eine Datenbank oder ein Backup mit Schema 35 nicht öffnen;
  vor der Migration legt Klein.Buch wie immer ein Pflicht-Backup an (ADR 0039 E15).
- Die Rückkehr-Sperre gilt nur noch nach einem Verzicht; nach einem Wegfall ist die Rückkehr
  jederzeit zum 1. Januar möglich, sobald die Voraussetzungen wieder vorliegen.
- Zahlungen sind wie Belege unveränderlich; Korrekturen sind sichtbar.
- Der DATEV-Export verlangt Berater- und Mandantennummer.

## Alternativen

| Option | Contra |
|---|---|
| Grund des §19-Wechsels nur als Spalte in `seller_profile` | keine Historie, Wechsel mitten im Jahr und Rückwirkung nicht abbildbar |
| Statuswechsel automatisch bei Überschreiten der Grenze | erzeugt ohne Wissen des Nutzers Rechnungen mit Umsatzsteuer; der Gesamtumsatz ist nur aus Klein.Buch nicht sicher vollständig |
| Zahlung löschen oder ändern | widerspricht § 146 Abs. 4 AO |
| Gegenbuchung am Tag der Korrektur | verschiebt einen nie geflossenen Betrag in ein anderes Jahr (§ 11 EStG) |
| DATEV-Formatversion 13 | Fremdprogramme lesen nur bis 12; kein belegter Vorteil für den Buchungsstapel ohne Feld 125 |
