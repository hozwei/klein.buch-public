# DATEV-Buchungsstapel (EXTF) — Format + Konten-Mapping (Block 14b)

Referenz für den DATEV-Export (`euer::datev_csv`). Zielgruppe: die **Übergabe an
den Steuerberater**, der den Buchungsstapel in DATEV Rechnungswesen über die
Stapelverarbeitung importiert.

> **Steuerberater-Caveat:** Alle Konten sind ein **Vorschlag** und vor der
> Verbuchung zu prüfen/anzupassen. Klein.Buch ist ein Werkzeug, kein
> Steuerberater. Der Buchungsstapel sollte vom Steuerberater einmal **testweise
> importiert** werden (DATEV ist die maßgebliche Validierung).

## Format

- **Vorlauf (Kopfzeile):** `"EXTF";700;21;"Buchungsstapel";12;<Erzeugt am>;…`
  - Kennzeichen `EXTF` (Export aus Fremdsoftware), Versionsnummer `700`,
    Datenkategorie `21` (Buchungsstapel), Formatname `Buchungsstapel`,
    Formatversion `12`.
  - WJ-Beginn = `{Jahr}0101`, Datum von/bis = `{Jahr}0101`/`{Jahr}1231`,
    Sachkontenlänge `4`, WKZ `EUR`, Festschreibung `0`.
  - **Berater-/Mandantennummer bleiben leer** — der Steuerberater setzt bzw.
    überschreibt sie beim Import.
- **Spaltenüberschrift + Buchungszeilen:** die führenden Standard-Spalten des
  Buchungsstapels (positionsbasiert): Umsatz (ohne S/H-Kz), Soll/Haben-Kennzeichen,
  WKZ Umsatz, Kurs, Basis-Umsatz, WKZ Basis-Umsatz, Konto, Gegenkonto, BU-Schlüssel,
  Belegdatum, Belegfeld 1, Belegfeld 2, Skonto, Buchungstext.
- **Kodierung:** Windows-1252 (CP1252), CRLF-Zeilenenden, Dezimal-**Komma**, kein
  Tausendertrenner. Belegdatum als **TTMM**, Umsatz immer positiv (Richtung über
  Konto/Gegenkonto + S/H).

## Buchungslogik (Cash-Basis, „Konto im Soll")

| Vorgang | Konto (Soll) | Gegenkonto (Haben) |
|---|---|---|
| Einnahme (Zahlungseingang) | Geldkonto | §19-Erlöse |
| Ausgabe (bezahlte Kosten) | Aufwandskonto (je Kategorie) | Geldkonto |
| Storno-Erstattung | §19-Erlöse | Geldkonto |
| Anlagen-Verkauf | Geldkonto | Erlöse Anlagenverkauf |
| AfA (31.12.) | Abschreibungsaufwand | Anlagekonto (direkte AfA) |

## Konten (SKR03 Default / SKR04)

| Zweck | SKR03 | SKR04 |
|---|---|---|
| §19-Erlöse | 8195 | 4185 |
| Erlöse Anlagenverkauf | 8820 | 4845 |
| Bank (Geldkonto) | 1200 | 1800 |
| Kasse | 1000 | 1600 |
| AfA Abschreibungsaufwand | 4830 | 6220 |
| GWG-Sofortabschreibung | 4855 | 6260 |
| Anlagekonto (AfA-Gegenkonto) | 0420 | 0650 |
| GWG-Anlagekonto | 0480 | 0670 |

### Kosten je Kategorie → Aufwandskonto

| Kategorie | SKR03 | SKR04 |
|---|---|---|
| goods (Wareneinkauf) | 3200 | 5200 |
| services (Fremdleistungen) | 3100 | 5900 |
| office (Bürobedarf) | 4930 | 6815 |
| travel (Reisekosten) | 4670 | 6670 |
| communications (Telefon/Internet) | 4920 | 6805 |
| vehicle (Kfz) | 4530 | 6530 |
| rent (Miete/Raumkosten) | 4210 | 6310 |
| insurance (Versicherungen) | 4360 | 6400 |
| training (Fortbildung) | 4945 | 6821 |
| fees (Gebühren/Bankspesen) | 4970 | 6855 |
| marketing (Werbung) | 4600 | 6600 |
| hardware / software / other | 4980 | 6800 |

## Bewusste Vereinfachungen (vom Steuerberater zu prüfen)

- **Geldkonto = Standard-Bank** (1200/1800). Das konkret hinterlegte Zahlungskonto
  (Bank vs. Kasse vs. PayPal/Stripe) wird noch nicht je Buchung aufgelöst — das ist
  ein Folge-Schritt zusammen mit der Verdrahtung von
  `invoices.paid_to_account_id` / `expenses.paid_from_account_id` (offener
  Follow-up aus Block 9).
- **Anlage-/AfA-Gegenkonto** = generische Betriebs- und Geschäftsausstattung
  (0420/0650), da Klein.Buch keine anlagenspezifischen Bilanzkonten führt
  (direkte Abschreibung).
- **hardware/software/other** laufen auf „Sonstige betriebliche Aufwendungen".
- Die elektronische Übermittlung an den Steuerberater erfolgt nicht automatisch —
  die Datei wird gespeichert und übergeben (z. B. DATEV Unternehmen Online ist
  v0.3+).

## Quellen

- DATEV-Format, Header + Buchungsstapel:
  <https://developer.datev.de/de/file-format/details/datev-format/format-description/header>
- Konten-Recherche SKR03/SKR04 (Stand 2026-05): u. a.
  <https://epago.de/konten/6815-buerobedarf/>,
  <https://lewo-media.de/en/daten/buchhaltung/skr03-buchungskonten>
