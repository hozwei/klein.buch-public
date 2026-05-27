---
slug: konten-und-mail-versand
title: Konten und Mail-Versand
category: bedienen
order: 250
keywords: [mail, smtp, oauth, exchange, microsoft, versand, mail-konto]
---

# Konten und Mail-Versand

Klein.Buch kann Rechnungen direkt aus der App heraus per E-Mail
versenden, sobald Du ein E-Mail-Konto eingerichtet hast. Das spart
den Schritt über das normale Mail-Programm.

## Welche Konto-Typen es gibt

Klein.Buch unterstützt zwei Konto-Arten:

1. SMTP-Konto: Standard-Mail-Server mit Benutzername und Passwort.
   Praktisch jeder Mail-Anbieter unterstützt das (zum Beispiel
   GMX, Web.de, mailbox.org, eigene Domain mit Hosting-Paket).
2. Microsoft 365 Exchange Online: für geschäftliche Microsoft-
   Konten. Klein.Buch nutzt OAuth (Open Authorization) und die
   Microsoft Graph API, also kein App-Passwort, sondern eine
   richtige Anmeldung über Deinen Browser.

## SMTP-Konto einrichten

Klick in den Einstellungen auf "Konten" und dann auf "Neues
SMTP-Konto". Trage Server-Adresse (zum Beispiel
"mail.beispiel-anbieter.de"), Port (meistens 465 mit SSL oder 587
mit STARTTLS), Benutzername und Passwort ein. Setze die
Absender-Adresse und den Anzeige-Namen.

Klein.Buch testet die Anmeldung sofort. Bei Erfolg landet das
Konto in der Liste. Das Passwort liegt im Schlüsselbund Deines
Betriebssystems, nicht in der Klein.Buch-Datenbank.

## Microsoft-Konto einrichten

Microsoft 365 lässt Dich keine fertigen App-Passwörter mehr
erzeugen. Stattdessen registrierst Du Klein.Buch einmalig als App
in Deinem Microsoft-Konto und meldest Dich dann per Browser an.
Klein.Buch braucht dafür zwei Werte aus dem Microsoft-Portal:
die Anwendungs-(Client-)ID und die Mandant-(Tenant-)ID.

Im Einrichtungs-Formular (Einstellungen, Konten, "Neues
Microsoft-Konto") findest Du einen ausklappbaren Block mit der
Überschrift "So richtest du die Microsoft-App ein". Er führt Dich
in fünf Schritten durch das Microsoft-Entra-Portal: App anlegen,
IDs notieren, Öffentliche Clientflows aktivieren, Mail-Sende-
Berechtigung erlauben, in Klein.Buch eintragen. Die App-
Registrierung machst Du einmalig.

Trage anschließend Anwendungs-ID und Mandant-ID in das Klein.Buch-
Formular ein und speichere. Daraufhin erscheint in der Konten-Liste
oben ein Knopf "Mit Microsoft verbinden". Klick darauf. Klein.Buch
öffnet einen lokalen Listener und schickt Dich in den Browser zur
Microsoft-Anmeldung. Du meldest Dich mit Deiner Geschäfts-Adresse
an und bestätigst, dass Klein.Buch Mails in Deinem Namen versenden
darf.

Nach der Anmeldung schickt Microsoft einen Code zurück an
Klein.Buch. Die App tauscht den Code gegen ein OAuth-Token aus und
speichert das Token verschlüsselt im Schlüsselbund. Das Konto ist
einsatzbereit.

## Standard-Konto

Wenn Du mehrere Konten hast, wählst Du in den Einstellungen ein
Standard-Konto. Klein.Buch verwendet es beim Klick auf "Per
E-Mail senden", solange Du nicht explizit ein anderes Konto
auswählst.

## Versand-Protokoll

In "Einstellungen → E-Mail-Versand" gibt es einen Abschnitt mit
allen Versand-Vorgängen: Datum, Empfänger, Betreff,
Provider-Antwort. Das Protokoll ist append-only und lässt sich
nicht ändern. Es ist Deine GoBD-Spur für Versand-Belege.

## Token-Ablauf

Microsoft-OAuth-Tokens laufen nach einer Weile ab. Klein.Buch
zeigt Dir vor Ablauf einen Hinweis. Klick auf "Konto neu anmelden"
und durchlaufe den Browser-Schritt erneut. Das gespeicherte Token
wird ausgetauscht, ohne dass Du sonst etwas verlieren würdest.

## Versand-Vorlage

Im Versand-Dialog kannst Du Betreff und Text vor dem Senden
ändern. Klein.Buch merkt sich pro Empfänger Deinen letzten Text
als Vorschlag für die nächste Mail. Du sparst Dir das wiederholte
Formulieren, ohne dass die App Dir den Text aufzwingt.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
