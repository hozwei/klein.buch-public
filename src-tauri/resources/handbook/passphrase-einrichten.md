---
slug: passphrase-einrichten
title: Passphrase einrichten
category: erste-schritte
order: 20
keywords: [passphrase, daten-passwort, login, verschlüsselung, sqlcipher, onboarding, sicherheit]
---

# Passphrase einrichten

Klein.Buch verschlüsselt Deine Datenbank und alle Backups mit einer
Passphrase. Eine Passphrase ist ein Passwort, meistens länger und
mit mehreren Wörtern. Ohne Passphrase startet die App nicht. Mit der
Passphrase entsperrst Du jedes Mal die Daten beim Start.

Du legst die Passphrase ein einziges Mal beim allerersten Start an.
In Klein.Buch heißt sie an manchen Stellen auch "Daten-Passwort" —
gemeint ist immer das Gleiche.

## Bevor Du beginnst

Notiere Dir die Passphrase außerhalb der App, am besten in einem
Passwort-Manager. Wenn Du die Passphrase verlierst, sind Deine
Daten und Deine Backups unwiederbringlich weg. Es gibt absichtlich
keine Hintertür und keinen Reset-Knopf, der Dir die Daten
zurückbringt. Das ist die Grundlage dafür, dass Deine Daten auf
der Festplatte und in Cloud-Backups nicht für andere lesbar sind.

## Was eine gute Passphrase ist

Eine gute Passphrase besteht aus drei bis vier zusammenhängenden,
aber nicht offensichtlichen Wörtern und ergibt insgesamt mindestens
16 Zeichen. Beispiel-Muster (nicht zum Übernehmen): `<wort>-<wort>-<wort>-<zahl><sonderzeichen>`.
Vermeide Geburtstage, Namen von Familienmitgliedern, "passwort123"
und alle Passwörter, die Du bereits irgendwo anders verwendest.

Klein.Buch erzwingt nur die Mindestlänge von 16 Zeichen. Alles
andere ist Empfehlung. Du musst die Passphrase zweimal eingeben,
damit Tippfehler nicht unbemerkt bleiben.

## Schritte

Beim ersten Start der App siehst Du die Einrichtungs-Karte
"Daten-Passwort festlegen". Tipp Deine Passphrase ein und
wiederhole sie im zweiten Feld. Klick auf "Festlegen & erstes
Backup erstellen".

Klein.Buch erzeugt jetzt im Hintergrund die verschlüsselte
Datenbank, richtet die Verschlüsselung ein und schreibt ein erstes
Backup. Das dauert wenige Sekunden. Danach ist die App entsperrt
und Du landest im Start-Bildschirm.

Als Nächstes solltest Du Deine Firmendaten eintragen. Geh dafür auf
"Einstellungen" und "Meine Firmendaten" (siehe Kapitel
"Verkäuferprofil anlegen").

## Was passiert technisch im Hintergrund

Klein.Buch verschlüsselt die Datenbank-Datei mit SQLCipher und
leitet aus Deiner Passphrase über ein standardisiertes Verfahren
(PBKDF2-HMAC-SHA512) den Datenbank-Schlüssel ab. Backups erhalten
zusätzlich eine eigene verschlüsselte Hülle mit einem zweiten
Standard-Verfahren (Argon2id). Die Passphrase selbst liegt nirgends
auf der Festplatte, weder im Klartext noch verschlüsselt. Solange
die App entsperrt ist, hält sie den Schlüssel nur im Arbeitsspeicher.

Dasselbe Geheimnis schützt also Datenbank und Backups. Eine
kopierte Datenbank oder ein kopierter Backup-Ordner ohne Passphrase
ist wertlos. Selbst der Hersteller von Klein.Buch kann das nicht
entschlüsseln.

## Passphrase ändern

In Klein.Buch v1.0 gibt es keine eingebaute Funktion, um die
Passphrase nachträglich zu ändern. Wenn Du sie wechseln musst,
ist der Weg: Steuerberater-Paket exportieren, alle Daten über
"Einstellungen → Zurücksetzen" löschen und Klein.Buch mit neuer
Passphrase neu einrichten. Anschließend richtest Du Backup-Ziele
und Postfach neu ein. Eine eingebaute Wechsel-Funktion kommt in
einer späteren Version.

## Wenn Du die Passphrase vergessen hast

Es gibt keinen Weg zurück. Wirklich keinen. Wenn Du keine
funktionierende Sicherung in Form eines anderen entsperrten Geräts
oder eines im Passwort-Manager gespeicherten Eintrags mehr hast,
sind Deine Buchhaltungs-Daten endgültig verloren. Auch ein
Klein.Buch-Reset hilft nicht, weil der Reset selbst die Passphrase
zur Bestätigung verlangt. Du musst dann den Klein.Buch-Daten-Ordner
außerhalb der App im Datei-Explorer löschen und die App neu
einrichten — Deine alten Belege sind weg.

Deshalb dieser Satz noch einmal: Passphrase in den
Passwort-Manager. Sofort.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
