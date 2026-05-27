---
slug: troubleshooting-smartscreen
title: SmartScreen-Warnung beim Installieren
category: troubleshooting
order: 500
keywords: [smartscreen, windows, warnung, installation, unbekannter herausgeber, signatur]
---

# SmartScreen-Warnung beim Installieren

Beim Start des Klein.Buch-Installers zeigt Windows manchmal eine
blaue Box mit dem Text "Der Computer wurde durch Windows
geschützt" und einem Knopf "Nicht ausführen". Das ist die
SmartScreen-Warnung. Sie ist kein Hinweis auf eine echte
Bedrohung; sie taucht bei allen Programmen auf, die noch nicht von
ausreichend vielen Windows-Nutzern installiert wurden.

## Warum die Warnung kommt

Microsoft betreibt einen Reputations-Dienst. Programme mit einer
Code-Signatur eines bekannten großen Herstellers landen direkt im
"vertrauenswürdig"-Pool. Kleine Programme oder Open-Source-Projekte
ohne teure EV-Code-Signing-Zertifikate müssen sich diese
Reputation erst über Installations-Zahlen aufbauen. Klein.Buch ist
neu und klein, deshalb fehlt diese Reputation in der ersten Zeit
einfach.

Die Warnung ist also keine Aussage über die Sicherheit der
Software. Sie ist eine Aussage darüber, dass Microsoft die
Software noch nicht oft genug gesehen hat.

## Was Du tust

Klick in der blauen Box auf "Weitere Informationen". Es erscheint
ein neuer Knopf "Trotzdem ausführen". Klick darauf. Der Installer
startet wie üblich.

Beim nächsten Update wird die Warnung mit hoher Wahrscheinlichkeit
nicht mehr erscheinen, weil die Klein.Buch-Reputation bei Microsoft
schon etwas gewachsen ist.

## Wie Du prüfst, ob der Installer echt ist

Du kannst die Klein.Buch-Installations-Datei gegen den
veröffentlichten Hash-Wert prüfen. Auf der Klein.Buch-Release-Seite
(GitHub) liegt eine Datei `SHA256SUMS` mit dem Prüfwert jedes
veröffentlichten Installers.

Öffne PowerShell und führe diesen Befehl aus:

```
Get-FileHash -Algorithm SHA256 .\klein-buch_1.0.0_x64-setup.exe
```

Vergleiche das Ergebnis mit der Zeile zur selben Datei in
`SHA256SUMS`. Wenn beide Werte gleich sind, hast Du den echten,
unveränderten Installer.

## Was Du nicht tun solltest

Wenn das "Weitere Informationen / Trotzdem ausführen"-Knopfpaar
fehlt und stattdessen nur "Nicht ausführen" angezeigt wird, hat
ein Administrator (oder eine Unternehmens-Richtlinie) SmartScreen
verschärft. Lade die Datei dann nicht über einen Umweg neu
herunter, sondern frag den Administrator. Versuche nicht, das
Setup über einen anderen Weg auszuführen oder umzukopieren; das
verursacht oft nur weiteren Ärger.

---

*Letzte Aktualisierung: 27.05.2026 · Klein.Buch 1.0*
