---
slug: lizenz-und-quellcode
title: "Lizenz und Quellcode: wenn Du Klein.Buch veränderst"
category: betrieb
order: 90
keywords: [lizenz, agpl, quellcode, source-url, open source, fork, legal, drittanbieter, quellarchiv, paragraph 13]
---

# Lizenz und Quellcode: wenn Du Klein.Buch veränderst

Klein.Buch ist freie Software unter der GNU Affero General Public
License, Version 3 oder neuer (kurz AGPL-3.0). Du darfst es kostenlos
nutzen, betreiben, untersuchen und verändern. Die Lizenz verlangt im
Gegenzug vor allem eines: Wer anderen Klein.Buch gibt, gibt ihnen auch
den Quellcode, also den lesbaren Programmtext, aus dem das Programm
gebaut ist.

## Was §13 der AGPL verlangt

§13 der AGPL ist der Abschnitt, der die Lizenz von anderen freien
Lizenzen unterscheidet. Er betrifft Programme, die andere über ein
Netzwerk benutzen, ohne selbst eine Kopie zu bekommen, also genau eine
Webanwendung wie Klein.Buch. Veränderst Du Klein.Buch und lässt andere
Menschen Deine veränderte Fassung über das Netz benutzen, musst Du ihnen
den Quellcode genau dieser Fassung anbieten, kostenlos und gut
erreichbar.

Betreibst Du das unveränderte Image, ist das schon erledigt: Klein.Buch
zeigt auf der Anmeldeseite und im Fenster "Über" die Version und einen
Link auf den exakten Quellstand im öffentlichen Projekt. Benutzt nur Du
selbst Deine Instanz, hast Du den Quellcode ohnehin. Dieses Kapitel
erklärt die Technik dahinter; eine Rechtsberatung ersetzt es nicht.

## Woher der Link kommt

Jedes veröffentlichte Image kennt drei Angaben aus seinem Bau: die
Adresse des Quellcodes, den Commit (die eindeutige Kennung eines Stands
im Quellcode) und die Versionsmarke. Daraus baut Klein.Buch den Link:
für eine Veröffentlichung `<Adresse>/releases/tag/v<Version>`, sonst
`<Adresse>/tree/<Commit>`. Der Link zeigt immer auf einen festen Stand,
nie auf einen Entwicklungszweig, der sich weiter verändert. Ist kein
Commit bekannt, steht dort "Entwicklungs-Build, Quellstand unbekannt"
und kein Link.

Dieselben Angaben liefert die Adresse `/api/public/info` ohne Anmeldung:
Version, Commit, Versionsmarke, Quell-Adresse, Link und die
Lizenzkennung `AGPL-3.0-or-later`.

## Wenn Du Klein.Buch veränderst

Veröffentliche Deinen veränderten Quellcode an einer Stelle, die Deine
Nutzer ohne Anmeldung erreichen, zum Beispiel in einem eigenen
öffentlichen Repository. Dann sorge dafür, dass Klein.Buch auf diese
Stelle zeigt. Am besten gibst Du beim Bau Deines Images Adresse und
Commit mit:

```
docker build \
  --build-arg KLEINBUCH_SOURCE_URL=https://example.org/dein-name/klein-buch \
  --build-arg KLEINBUCH_BUILD_COMMIT=$(git rev-parse HEAD) \
  -t klein-buch:eigen .
```

Ohne Commit gilt Dein Image als Entwicklungs-Build ohne Link. Zur
Laufzeit kannst Du die Adresse zusätzlich mit der Umgebungsvariable
`KLEIN_BUCH_SOURCE_URL` überschreiben. Sie gilt für Anmeldeseite,
"Über" und `/api/public/info`; die Seite `/legal/quellen.html` entsteht
beim Bau und nennt die Adresse aus dem Build-Argument. Die Adresse muss
mit `https://` oder `http://` beginnen und darf weder Benutzername noch
Abfrage-Teil enthalten. Eine ungültige Angabe meldet Klein.Buch beim
Start als Warnung im Log und benutzt dann die Adresse aus dem Bau.

Klein.Buch hängt an die Adresse `/tree/<Commit>` beziehungsweise
`/releases/tag/<Versionsmarke>` an, so wie GitHub seine Seiten
aufbaut. Prüfe, dass der Link bei Deinem Anbieter auf den richtigen
Stand führt, und dass dieser Stand dort wirklich abrufbar ist. Lizenz-
und Urheberhinweise im Quellcode musst Du erhalten.

Beiträge zum Projekt selbst nimmt Klein.Buch derzeit nicht an; Deine
Änderungen bleiben in Deinem eigenen Repository.

## Lizenzen und Quellen der Bausteine

Klein.Buch besteht neben dem eigenen Code aus vielen Bausteinen Dritter:
Rust-Bibliotheken, Bibliotheken der Oberfläche, Java-Komponenten für die
E-Rechnung, Schriften und Debian-Pakete im Image. Ihre Lizenzen stehen
ohne Anmeldung unter `/legal/` Deiner Instanz, also zum Beispiel unter
`https://buch.example.org/legal/`:

- `/legal/third-party-licenses.html`: Lizenzen und Hinweise aller
  Bausteine Dritter (auch die Startseite von `/legal/`),
- `/legal/LICENSE.txt`: der vollständige Text der AGPL,
- `/legal/quellen.html`: Quellstand von Klein.Buch, Versionen und
  Prüfsummen der Java-Komponenten, Basis-Images und die Liste der
  Debian-Pakete,
- `/legal/debian-pakete.tsv`: dieselbe Paketliste als Tabelle.

Im Image liegen dieselben Dateien zusätzlich unter
`/usr/share/doc/klein-buch/`. Die Quelltexte der Bausteine Dritter
(Java-Laufzeit, Debian-Quellpakete und weitere) gibt es zu jeder
Veröffentlichung als eigenes Archiv
`klein-buch-<Version>-quellen-dritter.tar.zst` auf der Release-Seite
des öffentlichen Projekts unter
`https://github.com/hozwei/klein.buch-public/releases`. Jedes Image
nennt die Adresse seines Archivs im Label `de.klein-buch.source-archive`;
das Archiv ist veröffentlicht, bevor das Image erscheint, und bleibt
verfügbar, solange es das Image gibt.

Gibst Du das Image selbst weiter, etwa auf einem Datenträger oder fertig
eingerichtet auf einem Gerät, reicht der Verweis auf die Release-Seite
nicht: Dann gehört das Quellarchiv mit dazu.

---

*Letzte Aktualisierung: 24.10.2026 · Klein.Buch 2026.11*
