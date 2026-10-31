---
slug: totp-einrichten-und-wiederherstellen
title: Zweiter Faktor (TOTP) einrichten und wiederherstellen
category: betrieb
order: 40
keywords: [totp, zweiter faktor, authenticator, einmal-codes, totp-reset, gerät verloren, setup-token, re-auth, anmeldung, sitzung]
---

# Zweiter Faktor (TOTP) einrichten und wiederherstellen

Weil Klein.Buch aus dem Internet erreichbar ist, verlangt die Anmeldung
zwei Dinge: Dein Daten-Passwort und einen Code aus einer
Authenticator-App auf Deinem Handy. Das Verfahren heißt TOTP
(Time-based One-Time Password, zeitbasiertes Einmal-Passwort): Die App
und Klein.Buch kennen dasselbe Geheimnis und errechnen daraus alle 30
Sekunden denselben sechsstelligen Code. Wer nur Dein Passwort kennt,
kommt nicht hinein, und wer nur Dein Handy hat, auch nicht.

## Einrichten

Den zweiten Faktor richtest Du bei der ersten Einrichtung ein (Kapitel
"Installation mit Docker Compose"). Klein.Buch zeigt einen QR-Code und
darunter denselben Schlüssel zum Abtippen. Jede gängige
Authenticator-App funktioniert, zum Beispiel Aegis, 2FAS oder Google
Authenticator. Du bestätigst mit dem ersten Code aus der App. Mit
"Zurück zu Token und Passwort" kommst Du vom QR-Code zum vorigen Schritt
zurück.

Danach zeigt Klein.Buch zehn Einmal-Codes. Jeder davon ersetzt genau
einmal den Code aus der App, nicht das Daten-Passwort. Sie werden nur
in diesem Moment angezeigt; Klein.Buch speichert sie nur als
Prüfsumme und kann sie Dir nicht noch einmal zeigen. Bei der
Einrichtung kannst Du sie mit "Codes kopieren" in die Zwischenablage
legen oder mit "Als Textdatei speichern" herunterladen. Bewahre sie
getrennt vom Handy auf, zum Beispiel ausgedruckt oder in einem
Passwort-Manager.

Unter "Einstellungen" und "Sicherheit & Anmeldung" siehst Du in der
Karte "Einmal-Codes", wie viele noch übrig sind. Mit **"Neue
Einmal-Codes erzeugen"** und der Rückfrage "Neu erzeugen" bekommst Du
zehn neue; die alten gelten dann nicht mehr. Die neuen Codes stehen in
der Karte "Neue Einmal-Codes", bis Du auf "Notiert" klickst; mit
"Codes kopieren" oder "Als Textdatei speichern" übernimmst Du sie in
Deinen Passwort-Manager (die Datei entsteht nur in Deinem Browser). Mit **"Zweiten
Faktor neu einrichten"** wechselst Du auf ein neues Handy oder eine
neue App, solange Du noch Zugang hast: neuen QR-Code scannen, Code ins
Feld "Code aus der App" eingeben, **"Neu einrichten"**. Dabei entstehen
ebenfalls zehn neue Einmal-Codes, und die alte App-Verbindung gilt
nicht mehr.

## Wenn ein Code nicht passt

Der Code hängt an der Uhrzeit. Klein.Buch nimmt den aktuellen Code und
den davor und danach an, also etwa eine halbe Minute Abweichung. Geht
die Uhr Deines Handys oder Deines Servers falsch, passt kein Code.
Stelle beide auf automatische Zeit (auf dem Server über NTP, die
Zeitabfrage im Netz).

Jeder Code gilt nur einmal, auch über einen Neustart hinweg. Ein Code
zählt als verbraucht, sobald er stimmt, auch wenn danach das
Daten-Passwort falsch war. Hast Du Dich beim Passwort vertippt, warte
also auf den nächsten Code.

Nach fünf Fehlversuchen musst Du eine Minute warten, danach verdoppelt
sich die Wartezeit bis höchstens eine Stunde; der Zähler verfällt 24
Stunden nach dem letzten Fehlversuch. Dein eigener Browser bekommt nach
der ersten Anmeldung ein Gerätecookie und muss deshalb nicht warten,
wenn jemand anderes Passwörter durchprobiert. Nach der Anmeldung meldet
Klein.Buch als Warnung, wie viele Fehlversuche es seit Deiner letzten
Anmeldung gab; der Knopf "Sicherheit öffnen" darin führt zu den
angemeldeten Sitzungen.

Gab es 100 Fehlversuche in Folge ohne erfolgreiche Anmeldung dazwischen,
nimmt Klein.Buch die Anmeldung mit App-Code von unbekannten Browsern gar
nicht mehr an (Meldung "Anmeldung mit App-Code von neuen Geräten
gesperrt"), auch nicht nach einem Neustart. Von einem Browser, mit dem
Du Dich schon angemeldet hast, oder mit einem Einmal-Code kommst Du
weiter hinein; diese Anmeldung hebt die Sperre auf, und unter
"Hinweise" erscheint ein Eintrag dazu. Die Sperre heißt nicht, dass jemand Dein Daten-Passwort
kennt: Schon falsche App-Codes zählen mit, auslösen kann sie also
jeder, der die Anmeldeseite erreicht. Auf einem neuen Gerät kostet sie
Dich einen Einmal-Code, und jeder Einmal-Code gilt nur einmal. Prüfe
danach unter "Einstellungen" → "Sicherheit & Anmeldung" die
angemeldeten Sitzungen und wie viele Einmal-Codes noch übrig sind; bei
Bedarf richte den App-Code neu ein (dabei entstehen neue Einmal-Codes).

Steht unter "Hinweise" der Eintrag "Gültiger App-Code mit falschem
Daten-Passwort", gab
es drei Anmeldeversuche in Folge mit einem gültigen App-Code oder
Einmal-Code, aber falschem Daten-Passwort. Warst Du das nicht, kennt
jemand womöglich das Geheimnis Deiner Authenticator-App oder einen
Einmal-Code: Richte den App-Code neu ein (siehe unten) und prüfe die
Sitzungen.

## Handy verloren, Einmal-Codes vorhanden

Klick bei der Anmeldung auf "Gerät verloren? Einmal-Code verwenden" und
gib Daten-Passwort und einen Einmal-Code ein. Richte danach sofort unter
"Einstellungen" und "Sicherheit & Anmeldung" den zweiten Faktor auf dem
neuen Gerät neu ein.

## Handy und Einmal-Codes verloren

Dann setzt Du den zweiten Faktor auf dem Server zurück. Dafür brauchst
Du Zugriff auf die Kommandozeile des Servers, auf dem der Container
läuft:

```
docker exec klein-buch klein-buch-server totp-reset
```

`klein-buch` ist der Name aus `container_name`. Wenn Du ohne festen
Namen arbeitest, geht es im Ordner der Compose-Datei auch mit
`docker compose exec klein-buch klein-buch-server totp-reset`.
Klein.Buch antwortet mit "TOTP-Geheimnis gelöscht." und beendet alle
Sitzungen. Der Befehl löscht nur das Geheimnis des zweiten Faktors.
Datenbank, Belege und Daten-Passwort bleiben unverändert.

Öffne dann Klein.Buch im Browser. Es zeigt "Zweiten Faktor neu
einrichten" und schreibt gleichzeitig einen neuen Setup-Token ins
Container-Log. Hol ihn Dir mit `docker compose logs klein-buch` (der
Block "=== Klein.Buch: Einrichtung nötig ==="). Gib Setup-Token und Dein
bisheriges Daten-Passwort ein, scanne den neuen QR-Code, bestätige mit
einem Code und notiere die neuen Einmal-Codes. War Klein.Buch gesperrt,
etwa nach einem Neustart, entsperrt dieser Schritt es zugleich.

Wer auf dem Server Docker-Befehle ausführen darf, kann den zweiten
Faktor also zurücksetzen. Ohne Daten-Passwort kommt er trotzdem nicht an
Deine Daten. Schütze den Zugang zum Server entsprechend.

Ein verlorenes Daten-Passwort lässt sich auf diesem Weg nicht ersetzen.
Ohne Daten-Passwort sind Datenbank und Backups nicht mehr lesbar, das
ist Absicht (Kapitel "Passphrase einrichten").

## Erneute Bestätigung bei heiklen Aktionen

Auch mit gültiger Anmeldung fragt Klein.Buch bei manchen Aktionen noch
einmal nach Daten-Passwort und Code (oder einem Einmal-Code). Das
Fenster heißt "Aktion bestätigen: …" und nennt die Aktion, zum Beispiel
"Aktion bestätigen: Backup wiederherstellen". Du gibst Dein
Daten-Passwort und den "Code aus der Authenticator-App" ein (oder
wechselst mit "Einmal-Code verwenden" zum Einmal-Code) und klickst auf
"Bestätigen". Schlägt das fehl, gibst Du beide Felder neu ein. Mit
"Abbrechen" ändert sich nichts; Klein.Buch meldet dann "Abgebrochen –
nichts geändert.".

Die Bestätigung gilt fünf Minuten. Das betrifft: ein Backup
wiederherstellen oder herunterladen, Backup-Ziele anlegen, entfernen
und testen, Mailkonten anlegen, ändern, löschen und testen, eine
Testmail senden, alle Exporte (Steuerberater-Paket, Detail-Export,
DATEV-Buchungsstapel, ELSTER-Ausfüllhilfe, EÜR als PDF,
Migrations-Export, DSGVO-Auskunft), neue Einmal-Codes, den zweiten
Faktor neu einrichten, das Daten-Passwort ändern, andere Sitzungen
abmelden, Zahlungskonten anlegen, ändern, aktivieren und deaktivieren,
einen Kontakt anonymisieren, ein Geschäftsjahr abschließen, einen
Rechnungs-Entwurf verwerfen, eine gebuchte Zahlung stornieren, die
automatischen Abo-Läufe nach einer
Pause freigeben ("Prüfen und freigeben"), einen Nummernkreis anheben
und den Abgleich eines wiederhergestellten Entwurfs bestätigen. Bei den
Firmendaten fragt Klein.Buch nur, wenn sich die Bankverbindung (IBAN,
BIC) ändert oder Du einen Wechsel des Steuerstatus einträgst oder
widerrufst, beim Versand einer Rechnung
oder eines Angebots nur, wenn der Empfänger von der hinterlegten
Adresse abweicht; in beiden Fällen nicht in den ersten fünf Minuten
nach der Anmeldung bzw. Einrichtung. So kann jemand, der Dein offenes
Browserfenster erwischt, keine Daten abziehen, keine Zugänge ändern
und keine Zahlungen auf ein fremdes Konto umleiten. Mit jeder
Bestätigung erneuert Klein.Buch Deine Anmeldung im Hintergrund; neue
Einmal-Codes oder ein neu eingerichteter zweiter Faktor melden alle
anderen Geräte ab.

Eine Anmeldung endet nach 30 Minuten ohne Aktivität und spätestens nach
12 Stunden. "Datenbank sperren" (ganz unten im Hauptmenü, Gruppe
"Sitzung") beendet alle Sitzungen und verschließt die Datenbank, ebenso
jeder Neustart des Containers.

Jede Anmeldung in einem Browser ist eine Sitzung. Unter "Einstellungen"
→ "Sicherheit & Anmeldung" zeigt die Karte "Angemeldete Sitzungen" je
Sitzung eine Kennung, wann sie angemeldet wurde und wann sie zuletzt
aktiv war, dazu "Diese Sitzung" oder "Anderes Gerät oder Browser" und
gegebenenfalls "Frisch bestätigt" (die Bestätigung der letzten fünf
Minuten gilt noch). Dieselbe Kennung steht im Server-Log hinter
`sess=`; so ordnet der Betreiber eine Sitzung den Log-Zeilen zu. Mit
"Andere Sitzungen abmelden" und der Rückfrage "Abmelden" meldest Du
alle anderen Browser und Geräte ab; Deine Sitzung bleibt. Ist nur Deine
Sitzung angemeldet, ist der Knopf gesperrt und daneben steht "Nur diese
Sitzung ist angemeldet.".

Das Geheimnis des zweiten Faktors liegt außerhalb der Datenbank unter
`/data/auth/` und gehört nicht zu den Backups. Nach einer
Wiederherstellung auf einem neuen Server richtest Du den zweiten Faktor
deshalb neu ein.

---

*Letzte Aktualisierung: 27.10.2026 · Klein.Buch 2026.11*
