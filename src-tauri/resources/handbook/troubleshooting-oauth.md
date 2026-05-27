---
slug: troubleshooting-oauth
title: OAuth-Reauth bei Microsoft-Konten
category: troubleshooting
order: 530
keywords: [oauth, microsoft, exchange, token, reauth, mail, abgelaufen]
---

# OAuth-Reauth bei Microsoft-Konten

OAuth ist das moderne Verfahren, mit dem sich Klein.Buch bei
Deinem Microsoft-Konto anmeldet, um Rechnungen zu versenden. Die
zugehörigen Token (eine Art temporärer Schlüssel) laufen nach
einer Weile ab. Wenn das passiert, kann Klein.Buch keine Mails
mehr senden, bis Du die Anmeldung erneuerst.

## Symptome

1. Beim Mail-Versand kommt "Authentifizierung fehlgeschlagen".
2. Unter "Hinweise" liegt ein Eintrag "Microsoft-Konto neu
   anmelden".
3. Mail-Protokoll zeigt rote Einträge mit Fehler-Code 401.

## Was passiert ist

Microsoft hat das OAuth-Token Deines Klein.Buch-Kontos als
ungültig markiert. Mögliche Gründe:

1. Du hast Dein Microsoft-Passwort geändert.
2. Du hast Klein.Buch bei Microsoft als App ausgesperrt
   ("App-Berechtigung widerrufen").
3. Microsoft verlangt eine erneute Multi-Faktor-Authentifizierung.
4. Das Token ist einfach planmäßig abgelaufen.

## Lösung

Geh in den Einstellungen auf "E-Mail-Versand". In der Liste der
Postfächer klick auf "Neu verbinden" beim betroffenen Microsoft-
Konto. Klein.Buch öffnet den Browser und schickt Dich zur
Microsoft-Anmeldung. Melde Dich
mit Deinem Geschäfts-Konto an und bestätige die Klein.Buch-
Berechtigungen.

Microsoft schickt einen Code zurück an Klein.Buch. Die App tauscht
den Code gegen ein frisches Token aus. Das neue Token landet
verschlüsselt im Schlüsselbund. Du kannst sofort wieder Mails
versenden.

## Wenn der Browser nichts öffnet

Klein.Buch startet einen lokalen Listener auf einem freien Port und
schickt Dich auf eine URL der Form `http://localhost:XXXXX/callback`.
Wenn der Browser sich nicht öffnet, kopiere die URL aus dem
Klein.Buch-Dialog und öffne sie manuell im Browser. Der Rest
funktioniert dann wie üblich.

Wenn die Microsoft-Seite einen Fehler "Redirect-URI nicht
registriert" anzeigt, hast Du in Deiner Microsoft-App-Registrierung
nicht alle möglichen Loopback-Adressen erlaubt. Trage in der
Azure-Portal-Konfiguration unter "Authentifizierung" eine
Loopback-Redirect-URI ein und versuche es erneut.

## Token verschwunden nach Update

Manchmal löscht ein Windows-Update unbeabsichtigt Schlüsselbund-
Einträge. Klein.Buch zeigt dann beim Mail-Versand "Konto nicht
mehr eingerichtet". Lösung: Konto über "Neues Microsoft-Konto"
erneut hinzufügen. Du verlierst dabei nichts, weil die App-
Registrierung im Microsoft-Portal weiter besteht.

## Wenn Du mehrere Microsoft-Konten hast

Klein.Buch unterstützt mehrere Microsoft-Konten parallel. Jedes hat
ein eigenes Token im Schlüsselbund. Du erneuerst sie unabhängig
voneinander. Im Versand-Dialog wählst Du das gewünschte Konto.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
