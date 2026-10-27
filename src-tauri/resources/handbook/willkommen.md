---
slug: willkommen
title: Willkommen in Klein.Buch
category: erste-schritte
order: 10
keywords: [willkommen, einstieg, überblick, kleinunternehmer, einzelunternehmer, tastatur, f1, bedienung, betrag, menü mehr, rückgängig, startseite]
---

# Willkommen in Klein.Buch

Klein.Buch ist Deine eigene Buchhaltung. Sie läuft auf einem Server,
den Du selbst betreibst, in einem sogenannten Container (einem
abgeschlossenen Programm-Paket, das der Server startet). Du bedienst
Klein.Buch im Browser, am Schreibtisch genauso wie auf dem Tablet oder
dem Handy. Alle Daten, alle Rechnungen und alle Anhänge liegen auf
Deiner Instanz, also Deiner eigenen Klein.Buch-Installation auf diesem
Server, und nicht bei einem Anbieter. Es gibt kein Konto bei einem
Hersteller und keinen Telemetrie-Kanal, der Daten über Dich
verschickt.

Diese App ist für **Kleinunternehmer nach §19 UStG** gebaut. §19
UStG ist der Paragraph im Umsatzsteuergesetz, der Dich von der
Umsatzsteuer befreit, solange Dein Umsatz unter den gesetzlichen
Grenzen bleibt. Konkret heißt das: Auf Deinen Rechnungen weist Du
keine Umsatzsteuer aus. Sobald Du eine Rechnung ausstellst, muss
der Hinweis "Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
darauf stehen. Klein.Buch setzt ihn automatisch.

## Was Du mit Klein.Buch tun kannst

Rechnungen schreiben, auch als XRechnung. XRechnung ist das amtliche
Rechnungsformat in Deutschland. Behörden und größere Kunden verlangen
es seit 2025. Klein.Buch erzeugt Dir beides in einem Schritt: ein
PDF zum Anschauen und die XRechnung-Datei zum Verschicken.

Angebote, Pakete und Anfahrt verwalten. Du kannst Deine Leistungen
einmal als Paket anlegen und dann mit einem Klick in jedes Angebot
oder jede Rechnung übernehmen. Wenn Du zum Kunden fährst, rechnet
Klein.Buch die Anfahrt automatisch nach Kilometer und Satz aus.

Kosten und Anlagen erfassen (Bereiche "Kosten" und "Anschaffungen").
Eine Anlage ist ein teurer Gegenstand, den Du über mehrere Jahre
abschreibst (zum Beispiel ein Notebook für
1.500 €). Diese Verteilung des Wertverlusts heißt AfA (Absetzung für
Abnutzung). Klein.Buch führt die AfA-Tabelle automatisch und übernimmt
die Beträge in Deine Steuer-Aufstellung.

EÜR machen und exportieren. EÜR (Einnahmen-Überschuss-Rechnung) ist
die einfache Steuer-Aufstellung: Einnahmen minus Ausgaben. Du
brauchst sie einmal im Jahr fürs Finanzamt. Klein.Buch erstellt sie
auf Knopfdruck und liefert eine Ausfüllhilfe für ELSTER (das
Online-Portal des Finanzamts) sowie einen Export für DATEV (das
Format, das Dein Steuerberater versteht).

## Was Du wissen musst, bevor Du loslegst

Klein.Buch ist ein Werkzeug, kein Steuerberater. Das Programm hilft
Dir, Deine Buchführung sauber zu führen und die gesetzlichen Pflichten
einzuhalten. Es ersetzt aber keine steuerliche Beratung. Wenn Du
unsicher bist, ob eine Ausgabe absetzbar ist oder ob ein Kunde aus
dem Ausland besondere Regeln auslöst, frag einen Steuerberater.

Du musst eine Passphrase einrichten, bevor Du die erste Rechnung
schreiben kannst. In der Oberfläche heißt sie "Daten-Passwort". Diese
Passphrase verschlüsselt Deine Datenbank und Deine Backups. Wenn Du
sie verlierst, sind Deine Daten unwiederbringlich weg. Es gibt
absichtlich keine Hintertür. Notiere die Passphrase an einem sicheren
Ort, am besten in einem Passwort-Manager. Zusätzlich verbindest Du bei
der Einrichtung eine Authenticator-App auf Deinem Handy. Sie liefert
den App-Code, den Du bei jeder Anmeldung zusammen mit dem
Daten-Passwort eingibst.

## Wie es jetzt weitergeht

Läuft Klein.Buch noch nicht auf Deinem Server, beginnst Du mit dem
Kapitel "Installation mit Docker Compose". Danach liest Du das
Kapitel "Erste Schritte" der Reihe nach durch. Nach der Einrichtung
im Browser landest Du auf der Startseite ("Start").
Geh dann auf "Einstellungen → Meine Firmendaten" (Name, Adresse,
Steuernummer, E-Mail, Telefon), trag unter "Einstellungen → Konten"
Dein Bankkonto ein, richte unter "Einstellungen → Backups" ein
Off-Site-Ziel ein und schreib die erste Rechnung. Das dauert ungefähr
20 Minuten. Danach kennst Du den Ablauf.

Wenn Du irgendwo im Programm ein Fragezeichen-Symbol siehst, kommst
Du mit einem Klick direkt zur passenden Erklärung in diesem Handbuch.
Die Taste F1 öffnet das Kapitel zu der Seite, auf der Du gerade bist.

## Klein.Buch bedienen

Die Startseite zeigt das heutige Datum, das Geschäftsjahr, Deine
Firma und, solange Du Kleinunternehmer bist, eine Zeile zu §19.
Darunter stehen Kennzahlen (Einnahmen, Ausgaben, Saldo, offene
Rechnungen, Backup), "Anstehende Aufgaben" und "Letzte Belege".
Fehlen Firmendaten, ohne die Du keine Rechnung ausstellen kannst,
steht oben ein Hinweis mit einem Link dorthin. Die Navigation führt
zu allen Bereichen; unten in ihr stehen "Über", "Datenbank sperren"
und "Abmelden". "Abmelden" beendet nur die Sitzung in diesem Browser,
"Datenbank sperren" schließt die Datenbank und meldet alle Geräte ab.
Auf dem Handy, auf Tablets und in schmalen Fenstern öffnest Du die
Navigation mit dem Knopf mit den drei Strichen oben links; eine Zahl
daran zeigt, wie viele Hinweise offen sind.

Die meisten Seiten haben oben eine Hauptaktion, meist den nächsten
Schritt. Seltene und folgenschwere Aktionen wie "Stornieren" oder
"Entwurf verwerfen" stehen im Menü "Mehr", auf dem Handy auch die übrigen
Nebenaktionen. Was sich umkehren lässt, etwa einen Kontakt
archivieren, erledigt Klein.Buch ohne Rückfrage; die Meldung danach
bietet "Rückgängig" an. Vor folgenreichen Schritten wie Ausstellen,
Stornieren oder Wiederherstellen fragt Klein.Buch nach. Besonders
geschützte Aktionen verlangen zusätzlich Daten-Passwort und App-Code
in einem Dialog, der die Aktion nennt ("Aktion bestätigen: …"); die
Bestätigung gilt danach fünf Minuten. "Abbrechen" ändert in all
diesen Dialogen nichts.

Beträge gibst Du in deutscher Schreibweise ein, zum Beispiel
"1.234,56": Das Komma trennt die Cent ab, Punkte gliedern die
Tausender in Dreiergruppen, ein Eurozeichen darf dabei sein. Ein
Minuszeichen nehmen Betrags- und Mengenfelder nicht an. Hat eine Zahl
mehr Nachkommastellen als erlaubt, meldet das Feld einen Fehler,
statt still zu runden. Verlässt Du das Feld, zeigt es den erkannten
Wert im deutschen Format, aus "1.234" wird also "1.234,00". Felder
mit einem Stern sind Pflicht. Fehler stehen direkt am Feld, und beim
Speichern springt der Fokus zum ersten fehlerhaften Feld. Meldungen
erscheinen unten im Fenster: Erfolgsmeldungen verschwinden nach
einigen Sekunden (nicht, solange Mauszeiger oder Fokus darauf
liegen), Fehler und Warnungen bleiben stehen, bis Du sie schließt.

## Mit der Tastatur arbeiten

Du kannst Klein.Buch ganz ohne Maus bedienen. Die Oberfläche
orientiert sich an WCAG 2.2 AA, einem internationalen Standard dafür,
dass Webseiten auch mit Tastatur, Bildschirmleser oder Vergrößerung
bedienbar sind. Mit der Tab-Taste springst Du zum nächsten
Bedienelement, mit Umschalt und Tab zurück. Das Element, auf dem Du
gerade stehst (der Fokus), trägt einen gut sichtbaren Rahmen. Knöpfe
löst Du mit der Eingabetaste oder der Leertaste aus.

Beim ersten Druck auf Tab erscheint oben links der Sprunglink "Zum
Inhalt". Drückst Du dann die Eingabetaste, springst Du an der
Navigation vorbei direkt zum Inhalt der Seite.

F1 öffnet das Kapitel dieses Handbuchs, das zur gerade geöffneten
Seite passt; gibt es keins, öffnet es die Startseite des Handbuchs.
Steht der Fokus in einem Eingabefeld, überlässt Klein.Buch die Taste
dem Browser, damit beim Tippen nichts dazwischenkommt.

Die Escape-Taste schließt den obersten Dialog, das Menü "Mehr" und
auf dem Handy, auf Tablets und in schmalen Fenstern die Navigation.
Der Fokus kehrt danach zu dem Knopf zurück, mit dem Du sie geöffnet
hast. In einem Suchfeld leert Escape zuerst die Suche. Nur die
Anmeldung lässt sich nicht mit Escape schließen. Im Menü "Mehr"
wählst Du einen Eintrag mit den Pfeiltasten aus und löst ihn mit der
Eingabetaste oder der Leertaste aus.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
