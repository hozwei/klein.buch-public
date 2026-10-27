---
slug: pflichtangaben-rechnung
title: Pflichtangaben auf Rechnungen
category: recht-und-steuern
order: 310
keywords: [§14, pflichtangaben, rechnung, ust, steuernummer, leistungsdatum, fälligkeit, telefon, leitweg-id]
---

# Pflichtangaben auf Rechnungen (§14 UStG)

§14 UStG listet die Felder auf, die jede Rechnung enthalten muss.
Wenn Pflichtangaben fehlen, ist die Rechnung formal mangelhaft.
Der Empfänger kann sie zurückweisen, das Finanzamt kann sie bei
einer Prüfung beanstanden.

## Die vollständige Liste

Jede Rechnung enthält:

1. Vollständiger Name und Anschrift des Rechnungs-Erstellers
   (also Du)
2. Vollständiger Name und Anschrift des Empfängers
3. Steuernummer oder Umsatzsteuer-Identifikations-Nummer des
   Erstellers
4. Rechnungs-Datum
5. Eindeutige fortlaufende Rechnungsnummer
6. Menge und handelsübliche Bezeichnung der Leistung
7. Zeitpunkt der Lieferung oder Leistungs-Erbringung
8. Entgelt (Netto) und der darauf entfallende Steuerbetrag
9. Bei Steuerbefreiung: Hinweis auf den Grund
10. Bei Kleinunternehmern: Der §19-Hinweis

Klein.Buch prüft die ersten 9 Punkte vor jeder Festschreibung
automatisch. Der §19-Hinweis (Punkt 10) wird automatisch
eingesetzt, solange Du Kleinunternehmer bist.

## Was Klein.Buch beim Ausstellen prüft

Schon beim Öffnen eines Rechnungs-Entwurfs und noch einmal beim
Ausstellen prüft Klein.Buch automatisch:

1. Dein Verkäuferprofil (Einstellungen → Meine Firmendaten) hat Name
   und Anschrift.
2. Der gewählte Kunde hat Name und Anschrift.
3. Die Rechnung hat ein Rechnungsdatum.
4. Es ist mindestens eine Position eingetragen.
5. Im Verkäuferprofil ist eine Steuernummer oder USt-IdNr. hinterlegt,
   auch bei Kleinbetragsrechnungen (siehe unten).
6. Im Verkäuferprofil ist eine Telefonnummer hinterlegt.
7. Ist ein Betrag offen, hat die Rechnung ein Fälligkeitsdatum. Im
   Rechnungs-Formular ist die Fälligkeit deshalb immer Pflicht.

Die Punkte 6 und 7 verlangt nicht §14 UStG, sondern die XRechnung,
die Klein.Buch zu jeder Rechnung erzeugt. Ohne sie würde der
KoSIT-Validator (das amtliche Prüfprogramm für XRechnungen) die
Rechnung ablehnen.

In den Firmendaten trägt das Feld "Telefon" deshalb den Zusatz
"(Pflicht für die E-Rechnung)". Fehlen Telefon oder Steuernummer bzw.
USt-IdNr., speichert Klein.Buch die Firmendaten trotzdem, weist aber
am Feld und beim Speichern darauf hin: Eine Rechnung ausstellen kannst
Du ohne sie nicht.

Was fehlt, nennt die Karte "Ausstellen" unter den Positionen des
Entwurfs schon vor dem Klick, jeweils mit einem Link zur Korrektur
("Firmendaten ergänzen", "Kontakt ergänzen" oder "Entwurf
bearbeiten"). Lehnt Klein.Buch das Ausstellen ab, steht dort der
Hinweis "Die Rechnung wurde nicht ausgestellt." mit den Gründen.
Korrigiere und versuche erneut.

Die Rechnungsnummer vergibt Klein.Buch automatisch fortlaufend
pro Geschäftsjahr; eine Eindeutigkeits-Verletzung kann gar nicht
entstehen. Das Rechnungsdatum ist über das Datenbank-Schema Pflicht,
es kann nicht leer sein.

## Kleinbetragsrechnung (bis 250 € Brutto)

Für Rechnungen mit einem Gesamtbetrag bis 250 € brutto erlaubt
§33 UStDV vereinfachte Pflichtangaben. Es reichen:

1. Vollständiger Name und Anschrift des leistenden Unternehmers
2. Ausstellungs-Datum
3. Menge und Art der gelieferten Gegenstände oder Umfang und Art
   der sonstigen Leistung
4. Das Entgelt und der darauf entfallende Steuerbetrag in einer
   Summe (Brutto) sowie der anzuwendende Steuersatz oder im Fall
   einer Steuerbefreiung ein Hinweis auf die Steuerbefreiung

Bei Kleinunternehmern kommt der §19-Hinweis dazu. Auch in der
Kleinbetragsrechnung muss er stehen, weil Du auch hier keine
Umsatzsteuer ausweist.

Klein.Buch erkennt die 250 €-Grenze automatisch und nutzt das
passende Pflichtangaben-Schema. Du musst Dich nicht aktiv darum
kümmern. In einem Punkt ist Klein.Buch strenger als das Gesetz: Auch eine
Kleinbetragsrechnung bekommt eine XRechnung, und die verlangt eine
Steuernummer oder USt-IdNr. Ohne sie stellt Klein.Buch auch eine
Kleinbetragsrechnung nicht aus.

## Leistungsdatum genauer

Das Leistungsdatum ist der Zeitpunkt, an dem Du die Leistung
tatsächlich erbracht hast. Es ist Pflichtangabe nach §14 UStG.
Bei einer einmaligen Leistung (zum Beispiel ein Reparatur-Termin)
ist es ein konkretes Datum. Bei laufenden Leistungen (zum Beispiel
"Beratung im Monat April") ist es ein Zeitraum. Das Formular hat
dafür ein einzelnes Datumsfeld; den Zeitraum nennst Du zusätzlich in
der Beschreibung der Position. Bei Abo-Rechnungen schreibt Klein.Buch
den Leistungszeitraum auf Wunsch selbst auf die Rechnung (siehe
Kapitel "Abo-Rechnungen").

Im Rechnungs-Formular ist das Leistungsdatum ein Pflichtfeld.
Klein.Buch belegt es mit dem Rechnungsdatum vor und zieht es mit, bis
Du es selbst änderst; es darf nicht nach dem Rechnungsdatum liegen.
Auf der fertigen Rechnung steht damit in jedem Fall ein
Leistungsdatum.

## Steuernummer und USt-IdNr

Auf der Rechnung brauchst Du eine der beiden Nummern. Als
§19-Kleinunternehmer mit deutschen Kunden reicht die Steuernummer.
Eine USt-IdNr brauchst Du nur, wenn Du auf §19 verzichtet hast und
Kunden im EU-Ausland belieferst. Klein.Buch zeigt Dir an, welche
Nummer eingetragen ist.

Internationale Geschäfte (innergemeinschaftliche Lieferungen,
Reverse Charge, Drittland) sind in Klein.Buch ohnehin nicht
voll abgebildet — siehe Disclaimer-Kapitel.

## Leitweg-ID bei Behörden

Rechnungen an öffentliche Auftraggeber (Behörden) brauchen in der
Regel eine Leitweg-ID, die Kennung, unter der die Behörde
E-Rechnungen entgegennimmt. Du trägst sie unmittelbar vor dem
Ausstellen in der Karte "Ausstellen" in das Feld "Leitweg-ID
(optional)" ein; Klein.Buch schreibt sie als Käuferreferenz in die
E-Rechnung. Der Entwurf speichert sie nicht. Für alle anderen Kunden
bleibt das Feld leer.

## Bewährter Ablauf

1. Verkäuferprofil sauber pflegen (einmalig).
2. Kontakte mit kompletter Anschrift anlegen.
3. Beim Rechnungs-Schreiben Rechnungsdatum, Leistungsdatum und
   Fälligkeit bewusst setzen.
4. Im Entwurf alle Eingaben und die Prüfung in der Karte "Ausstellen"
   aufmerksam ansehen, weil nach dem Ausstellen nur noch ein Storno
   möglich ist.

Wenn Du diese Reihenfolge einhältst, sind Deine Rechnungen formal
vollständig nach §14 UStG.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
