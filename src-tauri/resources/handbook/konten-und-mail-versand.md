---
slug: konten-und-mail-versand
title: Konten und Mail-Versand
category: bedienen
order: 250
keywords: [mail, smtp, e-mail-versand, postfach, microsoft 365, office365, versand, protokoll, e-mail-protokoll, tls, test-mail, konten, zahlungskonto, bankkonto, iban]
---

# Konten und Mail-Versand

Klein.Buch kann Rechnungen und Angebote direkt per E-Mail
verschicken, sobald Du ein Postfach eingerichtet hast. Das spart den
Umweg über Dein normales Mail-Programm. Klein.Buch versendet dabei
über SMTP. SMTP ist der Standard-Weg, auf dem jedes Mail-Programm
seine Mails an den Mail-Server Deines Anbieters übergibt. Praktisch
jeder Anbieter unterstützt das (zum Beispiel GMX, Web.de,
mailbox.org oder das Postfach zu Deiner eigenen Domain).

Die Einstellungen dazu findest Du im Hauptmenü unter "Einstellungen"
im Eintrag "E-Mail-Versand". Die Zahlungskonten unter "Konten"
(Bankkonto, Bargeld und andere Zahlungswege) haben damit nichts zu
tun; sie beschreibt der Abschnitt "Zahlungskonten" am Ende dieses
Kapitels.

## Postfach einrichten

Im Abschnitt "Neues Postfach" trägst Du ein:

1. **Bezeichnung**, ein Name nur für Dich, zum Beispiel
   "Geschäftspostfach".
2. **Absender-Name** und **Absender-E-Mail**, so wie Dein Kunde den
   Absender sehen soll.
3. **Server (SMTP-Host)**, zum Beispiel "smtp.example.org". Den
   genauen Namen nennt Dir Dein Mail-Anbieter in seiner Hilfe.
4. **Port**. Vorbelegt ist 587, der übliche Port für eine
   Verbindung, die nach dem Aufbau verschlüsselt wird (STARTTLS).
   Manche Anbieter nutzen stattdessen 465 mit sofort verschlüsselter
   Verbindung. Klein.Buch erkennt das am Port.
5. **Benutzername**, meist Deine E-Mail-Adresse.
6. **Verschlüsselte Verbindung (TLS) verwenden**, standardmäßig an.
   Lass den Schalter an.
7. **Passwort** Deines Postfachs.
8. Optional **Als Standard-Postfach**.

Bezeichnung, Absender-Name, Absender-E-Mail, Server und Port sind
Pflicht. Fehlt eine Angabe, steht die Meldung direkt am Feld.

Klick zuerst auf **"Verbindung testen"**. Klein.Buch meldet sich mit
den eingetragenen Daten beim Server an, ohne etwas zu speichern.
Klappt das, klick auf **"Postfach speichern"**.

Für alles, was mit Postfächern zu tun hat (anlegen, ändern, löschen,
testen, Test-Mail), fragt Klein.Buch noch einmal Dein Daten-Passwort
und den App-Code aus Deiner Authenticator-App ab. Der Dialog nennt
die Aktion, zum Beispiel "Aktion bestätigen: Postfach anlegen";
"Abbrechen" ändert nichts. Die Bestätigung gilt dann fünf Minuten. So
kann niemand, der eine offene Browser-Sitzung erwischt, Deine
Zugangsdaten umleiten.

## Wo das Passwort liegt

Das Passwort speichert Klein.Buch verschlüsselt in der Datenbank
Deiner Instanz. Es ist genauso geschützt wie Deine Buchhaltung selbst.
Es erscheint nie in Protokollen und nie in Exporten.

Das Passwort gilt nur für genau den eingetragenen Server, Port und
Benutzer. Änderst Du beim Bearbeiten eines davon, löscht Klein.Buch
das gespeicherte Passwort beim Speichern, und Du gibst es für die
neue Verbindung neu ein. Klein.Buch weist Dich im Formular darauf
hin. So landet ein gespeichertes Passwort nie bei einem anderen
Server. Lässt Du das Passwort-Feld beim Bearbeiten leer, bleibt das
bisherige Passwort unverändert.

Ohne Verschlüsselung (TLS aus) speichert Klein.Buch ein Postfach nur
ohne Benutzer und Passwort. Sonst gingen die Zugangsdaten lesbar
übers Netz. Das ist nur für einen internen Mail-Server ohne
Anmeldung gedacht.

## Postfach ändern und prüfen

In der Liste "Postfächer" stehen Bezeichnung, Absender, Typ,
Verbindung ("verschlüsselt" oder "unverschlüsselt"), Standard und
"Zuletzt genutzt". Bei jedem Postfach öffnet "Bearbeiten" das
Formular "Postfach bearbeiten"; "Änderungen speichern" übernimmt die
Änderungen, "Abbrechen" verwirft sie. Im Menü "Mehr" der Zeile stehen
"Verbindung testen" und "Löschen". "Verbindung testen" prüft die
Anmeldung mit dem gespeicherten Passwort. "Löschen" fragt vorher nach
("Postfach … löschen?"); das gespeicherte Passwort wird mit gelöscht.

Unter der Liste steht "Test-Mail senden": Trag unter "Empfänger" eine
Adresse ein und klick auf "Test-Mail senden". Hast Du mehrere
Postfächer, wählst Du vorher unter "Postfach" das gewünschte aus.
Klein.Buch verschickt dann eine kurze Test-Mail über dieses Postfach.
Das prüft den echten Versandweg.

Zwei Grenzen gelten für Verbindungstests und Test-Mails:

1. Getestet wird nur auf freigegebenen Ports, standardmäßig 25, 465,
   587 und 2525. Als Betreiber kannst Du die Liste mit der Variable
   `KLEIN_BUCH_ALLOWED_TEST_PORTS` ersetzen. Sie gilt dann auch für
   die Tests von SFTP-Backup-Zielen.
2. Höchstens 10 Tests pro Minute für die ganze Instanz, egal aus
   wie vielen Browsern.

Scheitert eine Verbindung, meldet Klein.Buch "Verbindung zu Host:Port
fehlgeschlagen (Host, Port, Zugangsdaten und Firewall prüfen)."
Genauere Angaben stehen absichtlich nicht in der Oberfläche. Prüfe
Server-Name, Port, Benutzer und Passwort und ob der Server Deines
Containers ausgehende Verbindungen auf diesem Port zulässt.

## Microsoft 365

Die Anmeldung über Microsoft (per Browser beim Microsoft-Konto) gibt
es in Klein.Buch nicht. Ein Microsoft-365-Postfach richtest Du wie
jedes andere als SMTP-Postfach ein: Server `smtp.office365.com`, Port
587, TLS an, Deine Microsoft-Adresse als Benutzername und Dein
Passwort. Das funktioniert nur, wenn in Deinem Microsoft-365-Mandanten
(dem sogenannten Tenant, also der Firmen-Umgebung bei Microsoft) der
SMTP-Versand erlaubt ist. Das legt der Administrator dort fest.

Taucht in der Liste ein älteres Postfach vom Typ "Microsoft 365" mit
dem Hinweis "im Server-Betrieb nicht verfügbar" auf, kannst Du es
über "Auf SMTP umstellen" mit SMTP-Daten weiter nutzen oder löschen.
Bis zur Umstellung versendet es nicht; beim Umstellen entfernt
Klein.Buch die alten Microsoft-Anmeldedaten.

## Einstellungen für Betreiber

Zwei Werte setzt Du bei Bedarf als Betreiber in der Compose-Datei:
`KLEIN_BUCH_SMTP_HELO_NAME` legt den Namen fest, mit dem sich
Klein.Buch beim Mail-Server vorstellt (sonst der Name des
Containers). `KLEIN_BUCH_SMTP_CA_FILE` zeigt auf eine eigene
Zertifikats-Datei (PEM) im Volume, falls Dein Mail-Server ein
Zertifikat Deiner eigenen Zertifizierungsstelle nutzt. Eine Option,
Zertifikatsfehler zu ignorieren, gibt es nicht.

## Standard-Postfach

Hast Du mehrere Postfächer, steht beim Standard-Postfach in der
Spalte "Standard" der Vermerk "Standard". Klein.Buch wählt es beim
Versand vor. Schaltest Du bei einem anderen Postfach "Als
Standard-Postfach" ein, ersetzt es beim Speichern das bisherige; das
Formular weist darauf hin. Abo-Rechnungen im Modus "Ausstellen +
senden" gehen immer über das Standard-Postfach.

## Versand einer Rechnung

Bei einer ausgestellten Rechnung klickst Du oben auf **"Senden"**.
Auf der Seite "Rechnung … versenden" (mit der Rechnungsnummer im
Titel) wählst Du das Postfach, prüfst den Empfänger (vorbelegt aus
dem Kontakt) und passt Betreff und Nachricht an. Klein.Buch hängt die
Rechnung automatisch als PDF an. Das ist die ZUGFeRD-PDF, in der die
XRechnung-Datei bereits steckt. Klick auf "Senden".

Weicht der Empfänger von der Adresse ab, die beim Kontakt
hinterlegt ist, weist das Feld darauf hin, und Klein.Buch fragt beim
Senden Daten-Passwort und App-Code ab ("Aktion bestätigen: Rechnung
an eine andere Adresse senden").

Betreff und Nachricht kommen aus einer Mail-Vorlage. Du kannst sie
vor jedem Versand ändern. Die Vorlage selbst liegt als Textdatei im
Vorlagen-Ordner `/inputs/mail-templates` Deiner Instanz.

Für einen Storno-Beleg nimmt Klein.Buch einen eigenen Text: Er nennt
die stornierte Rechnung mit Nummer und Datum statt eines negativen
Betrags. Willst Du ihn anpassen, lege im selben Ordner eine Datei
`storno-de.txt` an; ohne sie gilt der mitgelieferte Text.

## E-Mail-Protokoll

Über den Knopf "E-Mail-Protokoll" oben auf der Seite "E-Mail-Versand"
oder den gleichnamigen Eintrag in den Einstellungen kommst Du zum
"E-Mail-Protokoll". Es zeigt jeden Versandversuch, auch die
gescheiterten: Zeitpunkt, Status, Beleg, Empfänger, Betreff, Kanal
und die Antwort des Mail-Servers. Das Protokoll lässt sich nicht
ändern und nicht löschen. Es ist Dein Nachweis, wann welche Rechnung
an wen ging.

Zum Suchen gibt es ein Suchfeld (Empfänger, Betreff, Belegnummer,
Fehlertext), die Datumsfelder "Von" und "Bis" und die Auswahlen
"Status" (versendet oder fehlgeschlagen), "Art" (Rechnung, Angebot,
Paket-Broschüre, Test-Mail) und "Kanal" (SMTP oder das frühere
Microsoft 365), jeweils mit "Alle". "Filtern" wendet die Angaben an,
"Filter zurücksetzen" hebt sie auf. Über der Liste steht, wie viele
Einträge passen; die neuesten stehen oben.

## Zahlungskonten

Unter "Einstellungen → Konten" verwaltest Du die Konten, über die
Geld fließt: Bankkonto, Bargeld-Kasse, PayPal, Stripe oder Sonstiges.
Du wählst sie aus, wenn Du eine Zahlung zu einer Rechnung, Kosten oder
eine Privatbewegung erfasst.

Unter "Neues Konto" trägst Du die Bezeichnung ein (Pflicht, zum
Beispiel "Geschäftskonto Sparkasse"), wählst den Typ und ergänzt
optional IBAN, BIC und Zahlungs-Details (etwa einen PayPal-Link).
"Als Standard-Konto" macht das Konto zur Vorauswahl; genau ein Konto
ist der Standard. Konten mit "Auf Beleg anzeigen" erscheinen auf
Angeboten und Rechnungen, Bankkonten zusätzlich als Überweisungsweg
in der XRechnung. "Konto anlegen" speichert das Konto.

In der Liste öffnet "Bearbeiten" die Felder eines Kontos direkt in
seiner Zeile; "Änderungen speichern" übernimmt sie, "Abbrechen"
verwirft sie. Löschen lässt sich ein Konto nicht. Im Menü "Mehr" der
Zeile deaktivierst Du es ohne Rückfrage; die Meldung danach bietet
"Rückgängig" an. Deaktivierte Konten blendest Du mit "Inaktive Konten
anzeigen" ein und aktivierst sie dort im Menü "Mehr" wieder.

Weil davon abhängt, welche Bankverbindung auf künftigen Rechnungen
steht, bestätigst Du Anlegen, Ändern, Aktivieren und Deaktivieren
eines Kontos mit Daten-Passwort und App-Code (nicht in den ersten
fünf Minuten nach der Anmeldung).

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
