# ADR 0041 — UI/UX-Standards (2026.11)

**Status:** Akzeptiert · 2026-10-27 · Lauf 2026.11 (begonnen 2026-10-25). Keine Migration.
Ergänzt ADR 0039 (Web-/Docker-Betrieb) und ADR 0040 (Dauerbetrieb) für die Oberfläche;
abgelöste PRD-Stellen stehen in PRD-Revision 10 „UI/UX (2026.11)“.
Nr. 21, 91, 220 und 251 (§19-Wechseldialog, Leitweg-ID erst beim Ausstellen) sind durch ADR 0042
(Lauf 2026.11.1, Nr. 1–6, 12, 17) abgelöst; die Einträge bleiben als Historie stehen.

## Kontext

Mit Phase W ist Klein.Buch eine Webapp, die auf Smartphone, Tablet und Desktop bedient
wird. Die Oberfläche stammt aus der Desktop-Fassung: Betragsfelder ohne Dezimalkomma,
kleine Trefferflächen bei sekundären Bedienelementen, Eingaben ohne zugänglichen Namen,
Dialoge mit `100vh`, Status nur über Farbe an einzelnen Stellen. W10 hat nur Seitenüberlauf
und primäre Buttons in Chromium gemessen. Lauf F hat Funktion, Sicherheit und Start-/Zeitlogik
abgeschlossen und viele Oberflächenpunkte an diesen Lauf übergeben (UI/UX-Plan §2).

Maßstab ist der UI/UX-Plan (`Buchhaltung/docs/UI-UX-STARTLOGIK-PLAN.md`, nicht
veröffentlicht): Ziel WCAG 2.2 Stufe AA; EN 301 549 und BITV 2.0 nur als fachlicher Maßstab;
deutsche Standards nach DIN 5008; GoBD- und §19-Regeln gehen immer vor.

## Entscheidung

Die Oberfläche orientiert sich an WCAG 2.2 AA und wird mit einem Prüf-Harness (axe-core,
eigene Messungen, Abdeckungsbericht über alle Bedienelemente) auf Mobil, Tablet und Desktop
geprüft. Änderungen am Design laufen über Tokens und Bausteine
(`docs/architecture/design.md`). Einzelentscheidungen stehen fortlaufend nummeriert unten.

## Entscheidungen während der Umsetzung

Die Nummern sind stabil (Code, Befundliste und Harness verweisen darauf). Ab U4 wurden sie je
Arbeitsbereich in Bereichen vergeben (60–79 Designsystem, 80–89 Server-Lesefelder, 90–109 Belege,
110–129 Kosten/EÜR/Übersicht, 130–149 Einstellungen/Pakete/Hilfe, ab 150 Querschnitt); Lücken sind
gewollt.

1. **Version 2026.11.0 (U0, 25.10.2026).** Phase W erschien als v2026.10.0 (Ausgangslage A),
   der Tag v2026.11.0 ist frei. Der Lauf trägt die Version 2026.11.0 auch dann, wenn die
   Freigabe erst im Dezember kommt (Abweichung von „CalVer des Release-Monats“ auf Wunsch von
   Manuel; eine Umbenennung entscheidet er bei der Freigabe). Gesetzt in
   `src-tauri/Cargo.toml` (Server, Über-Dialog, Image-Tag). `package.json` bleibt `0.0.0`:
   Das Frontend wird nicht als npm-Paket veröffentlicht, die angezeigte Version kommt vom Server.
2. **WebKit lokal (U0, Ersatzkette Stufe a).** Playwright-WebKit 1.63 startet im Container ohne
   root, nachdem `x264` und `libgles` aus conda-forge in `werkzeuge/pw-env` liegen und
   `libmanette-0.2` (Gamepads, für die Tests ohne Bedeutung) durch eine kleine Stub-Bibliothek
   ersetzt ist; ein eigenes Startskript setzt die Bibliothekspfade, weil der mitgelieferte
   Starter `LD_LIBRARY_PATH` überschreibt. Damit laufen iPhone- und iPad-Profile, axe und E2E
   lokal in WebKit; der CI-Job (Stufe b) ist nicht nötig. Playwright-WebKit ist nicht
   iOS-Safari: Safe-Area, Bildschirmtastatur und echtes Touch-Verhalten prüft Manuel auf echten
   Geräten nach U7.
3. **Prüfadresse `localhost` (U0).** Das Sitzungscookie ist `__Host-…; Secure`. Chromium und
   Firefox nehmen es auf `http://localhost` an; Playwright-WebKit (WPE, headless) schickt es
   über unverschlüsseltes HTTP gar nicht mit. Das Harness ruft `http://localhost:<port>` auf
   (der Server lauscht weiter nur auf `127.0.0.1`), meldet sich für WebKit-Läufe in Chromium an
   und setzt den Cookie-Header für `/api/**` im Testbrowser selbst. Keine Änderung am Produkt:
   Die Cookie-Regeln bleiben, im Betrieb läuft Klein.Buch hinter TLS.
4. **Abdeckungsnachweis über einen Dev-Build (U0).** Für den Nachweis „jedes Element“ baut
   das Harness das Frontend zusätzlich im Svelte-Dev-Modus; jedes Element trägt dort seine
   Quelltextstelle (`__svelte_meta.loc`). So lässt sich jedes statisch gezählte Bedienelement
   einem geprüften Zustand zuordnen. Ausgeliefert wird weiter der normale Build.
5. **Zwei Token-Ebenen (U1).** Die Palette `--c-*` bleibt gültig; darüber liegen Rollen-Tokens
   (`--text*`, `--surface-*`, `--line`, `--line-control`, `--status-*-{bg,fg,line}`, `--focus-*`,
   Abstände `--sp-1..7`, Trefferflächen `--target-min`/`--target-touch`/`--target`). Bausteine
   nutzen nur Rollen. Ein späterer Dark Mode ist ein zweiter Rollensatz statt eines Umbaus.
6. **Kontraste (U1).** `--c-text-subtle` #5f717c (4,72:1 auf `--c-bg`, vorher 2,84:1);
   `--c-border-strong` = `--line-control` #788a94 (3,58:1 auf Weiß, vorher 1,47:1) für Feldrahmen,
   Checkbox und Toggle „aus“; Gefahr-Bestätigung Weiß auf `--c-danger-700` (7,38:1); Fokusring
   #1f7e9a (≥ 4,1:1 auf hellen Flächen), auf der Petrol-Seitenleiste weiß (11,1:1). Die Palette
   wurde direkt angehoben, damit Seiten mit `--c-border-strong` ohne Einzelkorrektur folgen.
7. **Feldrahmen und Fokusring nativer Felder global (U1).** Viele Seiten setzen eigene Rahmen
   oder `outline: none`; eine globale Regel (`:where()` als Basis, `aria-invalid` gewinnt) setzt
   Rahmen ≥ 3:1 und einen sichtbaren Fokusring durch. Rückbau, sobald alle Seiten auf Bausteinen
   stehen (U4).
8. **Trefferflächen (U1).** `--target` ist 24 px und im Drawer-Band
   (`(max-width: 900px), (pointer: coarse)`) 44 px. Kleine sichtbare Elemente (Checkbox 18 px,
   HelpAnchor 22 px) wachsen über Innenabstand bzw. `::after`, ohne dass das Layout springt. Alle
   Buttons bleiben auf dem Desktop gleich hoch (Vorgabe Manuel 2026-05-26).
9. **Touch hat immer den Drawer (U1, Plan §8.2).** Navigation als Drawer bei
   `(max-width: 900px), (pointer: coarse)` in CSS und JS; Hintergrund `inert`, eigener Knopf
   „Menü schließen“, Fokus zurück auf den Menü-Knopf; Name des Menü-Knopfs enthält die Zahl
   ungelesener Hinweise.
10. **Fokus nicht verdeckt (U1, 2.4.11).** `scroll-padding` an `<html>` und `<main>` aus
    CSS-Variablen, die PageBar (`--pagebar-h`) und Toasts (`--toast-h`) per ResizeObserver setzen.
11. **Ein Fokus-Helfer für alle Dialoge (U1).** `focusTrap` (Svelte-Action mit Stapel): Startfokus,
    Tab-Ring, Escape, Scroll-Sperre, Rücksprung zum Auslöser erst, wenn der Hintergrund nicht mehr
    `inert` ist. Gefahr-Dialoge sind `alertdialog`, Startfokus „Abbrechen“, kein Schließen über den
    Hintergrund. Dialoge ≤ 600 px sind Vollbild-Blätter mit `dvh` und Safe-Area; Abdunklung deckend,
    ohne Weichzeichner (Anti-Muster §5). Der Einrichtungs-/Anmeldedialog schließt nicht mit Escape.
12. **Toasts unten, Fehler bleiben stehen (U1, 2.2.1/4.1.3).** Oben liegen Kopfleiste und PageBar mit
    den Hauptaktionen. Fehler- und Aktions-Toasts schließen nicht automatisch, Erfolgsmeldungen nach
    5 s mit Pause bei Hover/Fokus; zwei dauerhafte Live-Regionen (`alert`/`status`).
13. **Passphrase-Felder (U1, Übergabe F4, ASVS V2.1.8/V2.1.12).** Anzeigen/Verbergen-Knopf mit
    wechselndem Namen; bei neuer Passphrase eine einfache Stärke-Schätzung (Länge, Wortzahl,
    Wiederholungen) nur als Hinweis — verbindlich bleibt die Serverprüfung. Autofill-Tokens:
    `current-password` (Anmelden, Re-Auth), `new-password` (Einrichtung, Wechsel), TOTP
    `one-time-code` mit `inputmode="numeric"`.
14. **Kontakte ohne Autofill (U1, 1.3.5).** Kontakte sind Daten Dritter: `autocomplete="off"`, sonst
    setzt der Browser die eigene Adresse ein. IBAN, BIC, USt-IdNr. und Steuernummer:
    `autocapitalize="characters"`, `spellcheck="false"`, sichtbare Formathilfe; PLZ-Hinweis
    „5 Ziffern“ nur bei Land DE. Verkäuferprofil und Einrichtung (eigene Daten) bekommen gültige
    Tokens.
15. **Betragseingabe (U1, Plan §4.2).** Ein Helfer `src/lib/money.ts` (Ziffern-Strings und `bigint`,
    kein Float) und die Bausteine `MoneyInput`/`QuantityInput` (`inputmode="decimal"`, kein
    `type="number"`). Leseregel wie im Plan; zusätzlich: (a) Leerzeichen (auch U+00A0/U+202F) als
    Tausendertrenner nur in Dreiergruppen, weil Beträge aus Bank-PDFs so eingefügt werden; (b) Mengen
    folgen derselben Punkt-Regel (`1.234` = 1234; wer 1,234 meint, tippt das Komma — die Anzeige nach
    dem Verlassen macht die Auslegung sichtbar); (c) eine Tausendergruppe, die mit `0` beginnt
    (`0.125`), ist ein Fehler statt still 125; (d) zu viele Nachkommastellen sind ein Fehler, kein
    Runden; (e) Fehler erscheinen beim Verlassen oder Absenden, nicht bei jedem Tastendruck;
    (f) Obergrenze 1 Mrd. € wie `domain::money`.
16. **Keine negativen Eingaben (U1).** Kein vom Nutzer eingegebenes Geld- oder Mengenfeld lässt
    fachlich negative Werte zu (Positionen, Kosten, Zahlungen, Anlagen, Abgang/Entnahmewert,
    Privatbewegungen, Kosten-Abos, Pakete, Kilometersatz, km); negative Einzelpreise gibt es nur in
    Storno-Belegen, die der Server erzeugt. Deshalb keine Vorzeichen-Auswahl; ein `-` ergibt die
    Meldung „Nur Beträge ab 0 möglich“. Kommt später ein Feld mit negativem Wert hinzu, nutzt es
    `allowNegative` (dann `inputmode="text"`, weil die iOS-Dezimaltastatur kein Minus hat).
17. **DIN 5008 in der Anzeige (U1).** `euro()` setzt U+00A0 vor `€`; neue Helfer `percent()`
    (`19 %` mit U+00A0) und `iban()` (Vierergruppen).
18. **Uhrzeit in Listen ohne Sekunden (U1).** `dateTimeShort()` (TT.MM.JJJJ, HH:MM) für Listen und
    Hinweise; Protokolle (Audit, Backup, Versand, Sitzungen) behalten `dateTime()` mit Sekunden, weil
    sie zum Abgleich mit dem Server-Log dienen.
19. **ISO-Daten in Server-Hinweistexten (U1).** Die Texte der Hinweise erzeugt der Server mit
    `JJJJ-MM-TT`; die Oberfläche formatiert gültige Daten darin für die Anzeige um. Die gespeicherten
    Texte bleiben unverändert.
20. **Seiteneigene Dialoge über `portal` (U1).** Das Layout setzt `<main>` bei offenem Modal `inert`;
    seiteneigene Dialoge (z. B. §19-Wechsel) hängen deshalb per Action `portal` an `document.body`
    und nutzen `.kb-dialog` und `focusTrap`.
21. **§19-Wechsel-Dialog mit zwei Fällen (U1, Übergabe F8e, R5-004).** Eigener Gefahr-Dialog statt
    `confirmDialog`, weil er den freiwilligen Verzicht (5-Jahres-Bindung) und den gesetzlichen Wegfall
    durch die 100.000-€-Grenze (keine Bindung) getrennt erklärt; Knopf „Zur Regelbesteuerung
    wechseln“. Die Logik des Wechsels ist unverändert.
22. **Fehler am Feld nach dem ersten Absenden (U1, 3.3.1).** Pflicht- und Formatfehler erscheinen
    nach dem ersten Absenden bzw. beim Verlassen eines Betragsfelds, verknüpft per
    `aria-describedby`/`aria-invalid`, außerhalb des `<label>` (der Name enthält keinen Fehlertext);
    danach springt der Fokus auf das erste fehlerhafte Feld. Client-Prüfungen bilden nur
    Server-Regeln nach (Datumsfolgen aus `domain::invoice`/`domain::quote`). Server-Ablehnungen mit
    mehreren Gründen erscheinen als stehendes Banner statt als Toast.
23. **Betrieblicher Anteil als Zahl statt Schieberegler (U1, 2.5.7).** Eingabe 0–100 % mit derselben
    Leseregel, ohne Ziehen.
24. **Anschaffungskosten aus Kosten schreibgeschützt (U1, Übergabe F8).** Der Server akzeptiert dort
    nur den Betrag aus den Kosten (brutto bei §19, netto bei Regelbesteuerung); ein Hinweis am Feld
    erklärt das, statt eine vermeidbare Ablehnung zu zeigen.
25. **Summen-Aufstellungen als Liste (U1).** Zweispaltige Aufstellungen (EÜR) sind `<dl>` mit
    Kontorbuch-Lineatur und doppelter Linie über der Summe statt Tabellen; auf 320 px lesbar.
26. **Zurückgenommene Einträge ohne Transparenz (U1, 1.4.3).** Archivierte, pausierte oder
    abgehakte Einträge mit gestricheltem Rahmen bzw. gedämpfter Fläche und Text-Badge statt
    `opacity`.
27. **Entwürfe heißen „Verwerfen“ (U1, Übergabe F2).** Bei Rechnung und Angebot heißen Knopf,
    Status und Grund für Entwürfe einheitlich „Verwerfen“; „Stornieren“ bleibt festgeschriebenen
    Belegen vorbehalten (GoBD).
28. **PDF-Vorlage als Auswahl (U1, KB-0050).** Die Option „default“ heißt „Wie in den Einstellungen
    (…)“, weil der Server „default“ als Platzhalter für die Standardvorlage liest.
29. **Foto-Knopf mit `capture` (U1, §8.5).** Das Datei-Feld für Belege hat kein `capture` (Kamera,
    Galerie und Dateien bleiben wählbar); daneben ein ausdrücklich beschrifteter Knopf „Foto
    aufnehmen“ mit `capture` — Ausnahme im Harness (`ausnahmen.json`).
30. **Autofill im Verkäuferprofil und bei Zugangsdaten (U1, 1.3.5).** Firma `organization`, Inhaber
    `name`, Adresse `street-address`/`postal-code`/`address-level2`/`country`, `email`, `tel`; SMTP-
    und SFTP-Benutzer `off`, deren Passwörter `new-password`, damit der Browser dort nie die
    Klein.Buch-Passphrase einsetzt.
31. **Links im Fließtext (U1, 2.5.8 „inline“).** Links innerhalb eines Satzes (Banner, Hinweise,
    Erklärtexte) nutzen die Ausnahme „inline“ und sind immer unterstrichen (1.4.1); einzeln stehende
    Links und Bedienelemente bekommen mindestens `--target` (Hilfsklasse `.kb-target`). Ausnahmen
    von automatischen Kriterien stehen im Harness in `ausnahmen.json` und in
    `Buchhaltung/docs/REVIEW-2026.11.md`.
32. **Band 821–900 px bleibt Tabelle (U1, Plan §8.2).** Auch bei `pointer: coarse` (Handy quer)
    scrollen Tabellen dort in sich; keine Karten bis 900 px. Der gemessene Überlauf kam von
    visuell versteckten Spaltenköpfen ohne positionierten Vorfahren: `.kb-visually-hidden` ist gegen
    Element-Regeln abgesichert, Scroll-Container sind `position: relative`. Ein Scroll-Container ohne
    fokussierbaren Inhalt bekommt `role="region"`, einen Namen und `tabindex="0"`.
33. **Fokus-Rücksprung mit ausdrücklichem Ziel (U1).** Wird der Auslöser beim Öffnen `inert` (Drawer)
    oder `disabled` (Ladezustand), geht der Fokus beim Schließen an ein ausdrücklich genanntes Ziel
    (`returnTo`); `<body>` gilt nie als Auslöser. Knöpfe, die während des Ladens gesperrt sind,
    nutzen `aria-disabled` statt `disabled`, damit sie den Fokus behalten.
34. **Leistungsdatum vorbelegt (U1, E2E-Befund).** Bei neuer Rechnung und bei der Umwandlung aus
    einem Angebot ist das Leistungsdatum mit dem Rechnungsdatum vorbelegt und folgt ihm, bis es von
    Hand geändert wird; es bleibt Pflichtfeld (§ 14 Abs. 4 Nr. 6 UStG). Server-Logik unverändert.
35. **Hauptaktion am Formularende (U1).** Unter 600 px klebt die PageBar nicht; lange Formulare
    wiederholen die Hauptaktion deshalb am Ende (`.kb-form-end`, gleicher Handler und Name). Breitere
    Ansichten behalten nur die klebende PageBar.
36. **Pflichtkennzeichen hinter dem Label (U1).** Das `*` steht hinter dem Label (`aria-hidden`; die
    Pflicht steht in `aria-required`/`required` und einer Legende „* Pflichtfeld“). Eine globale Regel
    schützt reine Text-Labels vor seiteneigenen Flex-Spalten.
37. **Karten-Labels (U1).** Labels in Tabellenkarten nehmen höchstens die halbe Breite und
    trennen nur mit Silbentrennung, nie mitten im Wort ohne Trennstrich.
38. **Kontaktformular mit Feldfehlern (U1).** Wie Nr. 22: Fehler am Feld statt Toast.
39. **Leerzeichen am Rand von Svelte-Blöcken (Konvention, U1).** Svelte 5 entfernt Leerzeichen am
    Rand von `{#if}`-Blöcken; nötige Leerzeichen werden ausdrücklich als `{" "}` geschrieben.
40. **`<main>` als Container (U2).** `<main>` ist der Container `kb-main`; Seiten fragen die
    Inhaltsbreite ohne Seitenleiste ab (Container Queries), nicht den Viewport — ab 901 px ist der
    Inhalt am Desktop schmaler als im Drawer-Band. Fixe Elemente bleiben am Viewport; Dialoge und
    Toasts liegen ohnehin außerhalb von `<main>`.
41. **Formular-Raster (U2).** `.kb-form-grid`: höchstens zwei Spalten, ab 37rem eigener Breite (misst
    sich selbst, damit Innenabstände und `max-width` zählen); Handy einspaltig; Textfelder, Editoren
    und Hinweise über die volle Breite (`.kb-span-all`). Zusammengehörige Felder stehen nebeneinander
    (PLZ und Ort, Steuernummer und USt-IdNr., Name und Kategorie).
42. **Spaltenpriorität (U2, U1-063).** `.kb-col-opt1..3` blenden Nebenspalten unter 64/56/50rem
    Inhaltsbreite aus — nur im Tabellenmodus über 820 px, nur in Listen mit Detailseite und nur in
    Tabellen ohne Eingabefelder. Nummer, Partner, Betrag, Status und Aktion bleiben immer sichtbar;
    Protokolle blenden nichts aus (sie scrollen in sich, Nr. 32).
43. **Kartenraster (U2).** Listen mit Detailseite zeigen bei 601–820 px zwei Kartenspalten
    (`.kb-card-grid`, Opt-in über `Table cardGrid`); die Lesereihenfolge bleibt die des DOM.
44. **Umbruch in Zellen (U2).** Namen und Beschreibungen brechen mit `break-word` und Silbentrennung;
    `anywhere` bleibt für Pfade und Kennungen.
45. **Hilfe ohne verschachtelte Scrollbereiche (U2).** Im Drawer-Band fließt der Handbuchtext mit der
    Seite, Kapitelnavigation und Inhaltsverzeichnis kleben; Fehler (unbekannte Seite) erscheinen im
    Handbuch-Layout mit Weg zurück.
46. **Liste + Detail nur in der Hilfe (U2).** Einstellungen-Hub und Belegliste bleiben getrennte
    Seiten: ein Umbau über viele Seiten mit eigener PageBar und Zurück-Logik bringt dort wenig, weil
    Einstellungen selten nacheinander besucht werden.
47. **Keine Information nur per Hover (U2).** Erklärende `title`-Tooltips werden zu sichtbaren
    Hinweisen mit `aria-describedby`; reine Wiederholungen des Namens dürfen bleiben.
48. **Dialoge bei geringer Höhe (U3, 1.4.10).** Bei ≤ 420 px Höhe (Zoom 400 %, Handy quer) ist der
    ganze Dialog der Scrollbereich; Kopf und Fuß sind kompakt und scrollen mit, sonst bliebe für den
    Inhalt kein Platz.
49. **PageBar klebt nicht bei geringer Höhe (U3, 2.4.11).** Bei ≤ 500 px Höhe klebt die PageBar
    nicht; `.kb-form-end` wiederholt dort die Hauptaktion (ergänzt Nr. 35). Vorher belegten
    Kopfleiste und PageBar bei 200 % Zoom bis zu 79 % der Höhe.
50. **Inhaltsbreiten (U3).** Listen und Details höchstens 100rem, Formularseiten 72rem, Fließtext und
    Lead 72ch, linksbündig; PageBar-Aktionen bündig mit der Inhaltsbreite. Regeln mit `:where`
    (Spezifität 0), engere Grenzen einzelner Seiten gewinnen.
51. **Hilfe am Desktop wie im Drawer-Band (U3, erweitert Nr. 45).** Text fließt mit `<main>`,
    Textspalte höchstens 48rem, Kapitelnavigation und Inhaltsverzeichnis kleben.
52. **Desktop-Dichte und Hover (U3).** Mit Maus ab 901 px kleinere Zellränder (Knöpfe bleiben gleich
    groß); Zeilen-Hover nur bei `(hover: hover)`, Hover ändert die Fläche, Fokus bleibt der Ring;
    Spaltenköpfe linksbündig, Linie über der Summenzeile.
53. **Spaltenpriorität nach Zeigerart (U3, präzisiert Nr. 42).** Mit Maus blendet `.kb-col-opt1`
    erst unter 61rem aus (1280 px ergeben rund 62–63rem Inhalt), damit am Desktop ab 1280 px alle
    Spalten sichtbar sind; Touch bleibt bei 64rem.
54. **Druckansicht zentral (U3).** `@media print` in tokens.css: ohne Navigation, Leisten, Toasts,
    Knöpfe und Aktionsspalte; echte Tabellen statt Karten (Karten, Drawer-Band, Spaltenauswahl und
    Dialog-Blatt gelten nur für `screen`, A4 hat rund 700 CSS-px); Auswahlfelder als Text; keine
    Zeilenumbrüche mitten in Tabellenzeilen.
55. **Scrollposition beim Seitenwechsel (U3).** `<main>` wird nach der Navigation nach oben gesetzt
    (außer bei Ankern), weil SvelteKit nur das Fenster zurücksetzt.
56. **Fokus aus einem iframe (U3).** Der Sprunglink ist bei offenem Dialog `inert`; `focusTrap`
    holt den Fokus zurück, wenn er aus dem PDF-iframe auf `<body>` fällt.
57. **Sortierung mit deutscher Kollation (U3).** Listen sortieren in der Anzeige mit
    `Intl.Collator('de')`, solange der Server mit `NOCASE` sortiert.
58. **Inline-Bearbeitung mit mehreren Feldern (U3, 3.3.2).** Eine Formularzeile über alle Spalten
    mit sichtbaren Labels statt eines Felds je Tabellenspalte.
59. **axe „target-size“ bei teilweiser Verdeckung (U3).** Ein Ziel, das selbst mindestens 24 × 24 px
    groß ist und an einer Scrollposition nur teilweise unter der klebenden PageBar liegt, verletzt
    2.5.8 nicht; das Harness misst axe am Seitenanfang.
60. **Typo-Rollen (U4).** Rollen `--type-display/figure/title/heading/body/small/caption` mit
    Zeilenhöhen 1,2/1,3/1,55; `--fs-*` bleiben als Aliase. Display, Kennzahl und Titel wachsen mit
    `clamp()` — so wird die Begrüßung auf 320 px kompakt, ohne Seiten anzufassen (U1-064).
61. **Eine Bedienhöhe (U4).** `--control-h` 40 px am Desktop, 44 px im Drawer-Band, für Knöpfe und
    alle einzeiligen Felder (vorher 32–44 px). Größerer Text läuft in den Innenabstand und wird nicht
    abgeschnitten (geprüft in drei Browsern und mit dem Textabstands-Test).
62. **Präzisere Formen (U4, Anti-Muster §5).** Radien 8 px für Bedienelemente, 12 px für Karten und
    Dialoge; Schatten nur als Haarlinie für Karten, Overlay- und Modal-Schatten für schwebende Ebenen.
63. **Eigene Optik für Auswahlfelder (U4).** `appearance: none`, Pfeil als Inline-SVG, „…“ bei langen
    Texten, nur am Bildschirm; WebKit ohne grauen Doppelrahmen (U1-062, U3-032).
64. **Regel für Knopfvarianten (U4, U1-068).** Höchstens eine `primary` je Bereich; `secondary` ist
    Standard in Karten und Zeilen; `ghost` nur neben einer sichtbaren `primary`/`secondary`, nie allein
    und nie in der PageBar; Gefahraktionen im Menü „Mehr“ unten. Aktionsgruppen über `.kb-actions`.
65. **ActionMenu (U4, U1-059).** WAI-ARIA-Muster „Menu Button“; das Menü liegt per Popover-API in der
    obersten Ebene (sonst würden Scrollbereiche und `<main>` als Container es abschneiden), Escape wird
    vor einer umgebenden Fokusfalle abgefangen.
66. **PageBar mit Gewichten (U4).** `actions` (Hauptaktion), `secondary` (ab 601 px als Knöpfe, bis
    600 px im Menü), `more` (immer im Menü).
67. **Belegnummern (U4, U1-061, U3-030).** Monospace, nie umbrechend; als Link immer fein
    unterstrichen; der PageBar-Titel markiert Nummern automatisch (`DocNo`, `.kb-docno`).
68. **Kontaktauswahl als Suchfeld plus natives Auswahlfeld (U4, Warteliste 15).** Statt einer eigenen
    Combobox, weil iOS/Android dann die Auswahl des Betriebssystems zeigen und Screenreader sie
    zuverlässig bedienen. Suchfeld ab 8 Kontakten; Umlaute, ae/oe/ue und ß werden gleich behandelt;
    Enter übernimmt einen einzigen Treffer; die Trefferzahl wird höflich angesagt; Sortierung mit
    `Intl.Collator('de')`.
69. **Toast-Art „Warnung“ (U4).** Bleibt stehen wie Fehler (`role="status"`); jede Toast-Art trägt ein
    Symbol, damit die Art nicht nur an der Farbe hängt (1.4.1).
70. **Seitenleiste (U4, U3-040).** Gruppe „Sitzung“ (Über, Datenbank sperren, Abmelden) unten, 14 px.
71. **Feldbreiten als Höchstbreite (U4, U3-031).** `.kb-w-*` setzt `max-inline-size`, robust gegen
    `width: 100%` in Seitenregeln ohne `!important`.
72. **Web-App-Manifest ohne Service Worker (U4, Plan §5).** `display: standalone`, Theme-Farbe
    `#0d4150`, Symbole aus dem Ledger-Logo; keine Offline-Funktion, die CSP bleibt unverändert
    (`default-src 'self'` erlaubt das Manifest). Hinweis für den Gerätetest: Eine iOS-Startbildschirm-App
    hat eigene Cookies (neue Anmeldung) und keine Zurück-Taste des Browsers.
73. **Einheiten im Klartext (U4, U1-060).** Codes nach UN/ECE Rec 20 bleiben gespeichert und im XML;
    die Oberfläche zeigt Klartext (Stunde, Stück, Monat …); unbekannte Codes bleiben als Option.
74. **Grundschrift 14 px auch auf dem Handy (U4).** Eingaben haben 16 px; der Fließtext bleibt 14 px
    wegen der dichten Tabellen — U1–U3 und Zoom 200/400 % sind auf diesem Stand gemessen.
75. **`!important` zurückgebaut (U4, Warteliste 18, Rückbau zu Nr. 7).** In `tokens.css` von 94 auf 33
    Deklarationen außerhalb von Druck und reduzierter Bewegung; was bleibt, ist in `design.md`
    inventarisiert und begründet (Seiten mit Kurzschreibweisen, globale Feldrahmen).
80. **EÜR-Ansicht mit Besteuerungsart und Abschluss-Abgleich (U4, Warteliste 3).** `euer_compute_report`
    liefert zusätzlich `isKleinunternehmer` (je Jahr, Verzicht berücksichtigt) und `snapshotCheck`;
    das Öffnen der Seite schreibt weder Audit noch Hinweis (keine Seiteneffekte beim Ansehen).
81. **Abgangs-Vorschau als Lese-Command (U4, Warteliste 5).** `assets_disposal_preview` bildet mit der
    reinen Funktion `plan_disposal` die Schritte von `assets_dispose` nach; Ablehnungsgründe kommen als
    `blockedReason` mit denselben Texten. Integrationstests sichern die Gleichheit mit dem echten Abgang.
82. **Steuerkategorie K nur als Hinweis (U4, Warteliste 2).** Der Parser liefert die Steuerkategorien
    und ein Kennzeichen K; Beträge und der §13b-Vorschlag hängen nicht daran, weil die Erwerbsteuer von
    Schwelle und USt-IdNr. abhängt (§ 1a UStG) und nicht automatisiert wird.
83. **Verworfen vs. zurückgezogen über `lockedAt` (U4, Warteliste 9).** Keine Migration und kein neuer
    Status; die Wahrheit steht im Festschreibungsfeld.
84. **Vorlagen mit Storno-Darstellung (U4, Warteliste 11).** `PdfTemplateMeta.supportsStorno` und
    `stornoFallback` nennen, mit welcher Vorlage die Pipeline Stornos tatsächlich erzeugt.
85. **„Ausgesetzt“ je Off-Site-Ziel (U4, Warteliste 12).** `OffsiteResult.suspended` zusätzlich zu
    `error`; Seiten prüfen `suspended` vor `error`, damit eine Aussetzung nicht als Fehlschlag erscheint.
86. **Alter der Off-Site-Kopie (U4, Warteliste 13).** Das Alter ist die älteste der je Ziel jüngsten
    Kopien in Berliner Tagen (wie der Status `stale`); scheiterte der letzte Lauf eines Ziels, kommt die
    letzte gute Kopie aus dem Backup-Protokoll. Keine Migration.
90. **„Entwurf bearbeiten“ als eigene Route (U4, Warteliste 6).** `/invoices/[id]/edit` mit gemeinsamem
    Editor-Baustein; `/invoices/new?edit=<id>` leitet weiter (Lesezeichen gehen weiter). Ablehnungen
    stehen am Feld, mehrere Gründe als Banner.
91. **Leitweg-ID beim Ausstellen (U4, Fehlerbehebung).** Der Entwurf speichert die beim Anlegen
    eingegebene Leitweg-ID nicht — das Feld im Editor war wirkungslos, beim Ausstellen stand „N/A“ in
    der E-Rechnung. Die Leitweg-ID (BT-10) wird deshalb in einer Karte „Ausstellen“ auf dem Entwurf
    eingegeben und mit dem Ausstellen übergeben; im Editor gibt es das Feld nicht mehr. Server
    unverändert.
92. **Konto je Zahlung (U4, Warteliste 7).** Wie in der EÜR: Konto der Zahlung, sonst Eingangskonto der
    Rechnung; IBAN gekürzt („DE89 … 3000“).
93. **Restore-Konflikt im Editor (U4, Warteliste 8).** Banner, solange ein Entwurf vorgemerkt ist; ein
    erneutes Vormerken durch Speichern wird danach als Warnung gemeldet. Ein Hinweis vor dem Speichern
    bräuchte ein Lesefeld (Queue).
94. **Re-Auth nicht vorab öffnen (U4, Warteliste 10, W6-CONSOLE-403).** `/api/auth/status` liefert keine
    Frische, eine Vorabprüfung bräuchte eine im Client gepflegte Liste der Aktionen, und die Frist kann
    zwischen Prüfung und Aktion ablaufen — der 403-Weg bliebe nötig. Die 403-Zeile ist nur in der
    Konsole sichtbar (ADR 0039 Nr. 136 (9)).
95. **Kartenschwelle je Tabelle (U4, U2-020).** Positionstabellen in Detailansichten messen ihre Breite
    (6 Spalten ab 560 px, 8 ab 720 px Kartenbreite) statt der globalen 820 px.
96. **Gewichte auf Belegdetails (U4).** Hauptaktion ist der nächste Schritt (Ausstellen, „+ Zahlung“
    nur bei offenem Betrag, Annehmen, Umwandeln); PDF, Senden, Bearbeiten, Prüfen, Ablehnen sind
    Nebenaktionen; Druck/Download und Gefahraktionen im Menü. Titel „Art + Nummer“ (auch Kosten,
    Anlage, Privat-Buchung).
97. **Eine sichtbare Aktion je Abo-Zeile (U4, U3-036).** Die Aktion, die den Zustand auflöst
    („Rückstand …“, „Jetzt erstellen“, „Fortsetzen“, sonst „Bearbeiten“), der Rest im Menü.
98. **Storniert und Verworfen (U4).** Neutrales Badge, Zeile gedämpft statt durchgestrichen
    (Beträge bleiben lesbar).
99. **Hinweis zur Storno-Vorlage (U4, Warteliste 11).** Nur bei Rechnungen und Umwandlungen, wenn die
    wirksame Vorlage keine Stornos darstellt; nennt die Ersatzvorlage.
110. **Backup-Stand auf der Übersicht nur vom Server (U4, Warteliste 13).** `offsiteStatus` und
     `offsiteAgeDays`; die alte Ampel aus dem Backup-Protokoll entfällt (eine Wahrheit mit der Regel
     „Backup überfällig“).
111. **Übersicht ohne Begrüßung (U4, U1-064, Anti-Muster §5).** Kopfzeile mit Datum, Geschäftsjahr,
     Firma und §19-Zeile; Kennzahlen ohne Anheben; Schnellzugriff als Liste mit Lineatur; „Letzte
     Belege“ im gemeinsamen Raster.
112. **Storno-Beleg ohne Zahlstatus (U4, U3-039).** Ein Storno-Beleg zeigt „Storno-Beleg“, nie „offen“;
     die Summe offener Beträge schloss Stornos schon aus.
113. **DATEV-Nummern als Export-Angabe (U4, Warteliste 1).** Berater- und Mandantennummer werden wie der
     Kontenrahmen im Browser gemerkt (keine Migration, kein neuer Command); die Grenzen prüft der Server.
114. **Abgangs-Vorschau live (U4, Warteliste 5).** Die Seite rechnet nichts selbst; Ablehnungsgründe
     stehen live am Datumsfeld.
115. **§13b-Steuer-Anlage nur zur Vorbelegung (U4, Warteliste 4, ADR 0039 Nr. 147 (2)).** Wählbar sind
     schon aktivierte §13b-Kosten unter §19; übernommen werden Datum, Methode, Nutzungsdauer, Kategorie
     und Anteil; der Methodenvorschlag folgt der Hauptanlage (kein GWG ohne GWG-Hauptanlage). Es wird
     keine Verknüpfung gespeichert (keine Migration).
116. **Weg zur Steuer-Anlage (U4).** Hauptanlage → „Steuer als Kosten erfassen“
     (`/expenses/new?taxFor=`) → „Anlage übernehmen“.
117. **F6-Fähigkeiten (U4, Warteliste 14).** Mit Oberfläche: AfA je Jahr im Bereich „Abschreibung
     buchen“ (`/assets`), abgeschlossene Jahre als „Abschlussprotokoll“ (`/fiscal-year`),
     Privat-Buchung als Detailseite, Paket-Kategorien umbenennen und sortieren (`/packages`). Ohne
     eigene Oberfläche, weil vorhandene Aufrufe dasselbe liefern: `attachmentsListFor` (`expenses_get`
     enthält die Anhänge), `packageRevisionsGet` (`packageRevisionsList` liefert jede Version),
     `emailLogList` (`/settings/mail-log` nutzt `emailLogSearch`). Die Wrapper bleiben
     (Registry-Test).
119. **Kostenliste (U4, U3-016).** Kategorie als zweite Zeile; Lieferant und Beschreibung teilen die
     Restbreite ohne Mindestbreiten.
121. **Kamera-Knopf nur bei `pointer: coarse` (U4, U3-035, ergänzt Nr. 29).** Das Datei-Feld bleibt immer.
122. **Kunde bei offenen Forderungen (U4, U3-029).** Aus der Rechnungsliste, kein neues Server-Feld.
130. **Sortieren ohne Ziehen (U4, 2.5.7).** „Nach oben“/„Nach unten“; am Rand `aria-disabled`, der Fokus
     bleibt auf dem verschobenen Knopf, die neue Stelle wird angesagt.
132. **„Wiederverwenden“ statt „Auf diese Version zurücksetzen“ (U4, Hardline Pakete).** Es entsteht eine
     neue Revision (append-only); „zurücksetzen“ klang nach Überschreiben.
133. **Paket-Einheit als Code-Auswahl (U4).** Standard `LS`; Altwerte wie „Pauschal“ bleiben erhalten und
     bekommen einen Hinweis — keine Datenänderung ohne Aktion des Nutzers. Die Positions-Editoren melden
     einen Code außerhalb von UN/ECE Rec 20 am Feld, bevor die E-Rechnungs-Prüfung ablehnt.
134. **Backup-Ergebnis je Ziel bleibt stehen (U4, Warteliste 12).** „Jetzt sichern“ zeigt das Ergebnis
     je Ziel als Liste auf der Seite; „ausgesetzt“ ist eine Warnung.
135. **Knöpfe mit fehlender Auswahl bleiben aktiv (U4, 3.3.1).** Der Klick nennt den Grund am
     Auswahlbereich und setzt den Fokus dorthin; ein gesperrter Knopf in der PageBar hätte keinen
     erreichbaren Grund.
136. **Handbuch ohne doppelten Titel (U4, U3-041).** Die führende H1 einer Handbuchseite ersetzt den
     Front-Matter-Titel; die Markdown-Dateien bleiben unverändert.
137. **Begriff „E-Mail-Protokoll“ (U4).** Einheitlich in Hub, Seite und Knöpfen, wie im Handbuch.
150. **Einheiten außerhalb von UN/ECE Rec 20 (U4).** Die Positions-Editoren prüfen gegen die Liste der
     Einheiten; ein Code außerhalb (z. B. Altwert „Pauschal“ aus Paketen) zeigt sofort einen Fehler am
     Feld mit Vorschlag („Pauschale“) und sperrt das Speichern — sonst lehnte die E-Rechnungs-Prüfung
     erst beim Ausstellen ab. Bei Paket-Positionen löst die Wahl die Paket-Bindung (sauberer Bruch,
     Hardline Pakete). Keine Daten- oder Server-Änderung.
152. **Markdown-Vorschau wie CommonMark (U4).** `_kursiv_`/`__fett__` nicht mitten im Wort (Unicode),
     keine Hervorhebung in Code-Spannen; nur feste Tags ohne Attribute — Vorschau und PDF sehen gleich
     aus.
153. **Dialog-Knöpfe in Sichtreihenfolge (U4, 1.3.2/2.4.3, U2-019).** Bis 600 px Hauptaktion oben,
     „Abbrechen“ unten; darüber „Abbrechen“ links, Hauptaktion rechts — die DOM-Reihenfolge folgt der
     Breite (kein `column-reverse`). Drei Aktionen: Dialog 36rem breit, ohne Umbruch.
154. **`fieldClass` begrenzt nur die Feld-Box (U4).** Hinweise und Fehler behalten die volle Breite.
155. **Kosten-Abo-Zeilen wie Abo-Rechnungen (U4, Nr. 97).**
156. **Jahresfilter aus den geladenen Belegen (U4, U1-062).** Jahre aus dem ersten ungefilterten Abruf
     plus das laufende Jahr, als Auswahl mit „Alle“; kein neuer Server-Aufruf.
160. **Seitentitel zentral (U4-Prüfung, 2.4.2).** `document.title` = „<Titel> – Klein.Buch“; Quellen:
     Anmelde-Maske, Fehlerseite, PageBar-Titel, Handbuch-Kapitel, sonst Ersatztitel je Route (Tabelle,
     mit Test); gesetzt vor dem Zeichnen, damit die Routen-Ansage von SvelteKit den neuen Titel liest.
161. **F1 kontextbezogen.** F1 öffnet das Handbuch-Kapitel zur Route (Präfixtabelle, Test prüft die
     Slugs), sonst die Handbuch-Startseite.
162. **Fehlerseite der App.** Deutsch, mit PageBar, Erklärung und Weg zurück.
163. **Re-Auth nennt die Aktion; Abbrechen ist kein Fehler.** Titel „Aktion bestätigen: <Aktion>“
     (Namen je Command in einer Tabelle, Test gegen die Registry); Abbrechen ergibt den Hinweis
     „Abgebrochen – nichts geändert.“ (Toast-Art `info`).
164. **Ein Ladezustand.** „Wird geladen …“ mit `role="status"`; `aria-busy` nur am nachladenden
     Bereich.
165. **Pflicht-Legende „Felder mit * sind Pflicht.“ (ändert Nr. 36).** Stern `aria-hidden` plus
     verstecktes „Stern“, damit der vorgelesene Satz Sinn ergibt.
166. **Banner als einziger Hinweis-Baustein, `.kb-link` als einzige Link-Klasse.** Fünf Seiten mit
     eigenen, gleich aussehenden Link-Klassen stehen als S3 in der Queue.
167. **Anmelde-Maske.** h1; Felder während der Prüfung `readonly`, Knöpfe `aria-disabled`; Fokus aufs
     betroffene Feld; Fehlversuche als Warnung mit Aktion; Einmal-Codes kopieren oder lokal als
     Textdatei speichern; „Zurück“ aus dem QR-Schritt.
168. **Server-Meldungen in der Sprache der Oberfläche.** Feste Vorsätze („Die Rechnung kann nicht
     ausgestellt werden: …“), Gründe mit „; “, Daten TT.MM.JJJJ, „Firmendaten“ statt „Stammdaten“,
     „Leistungsdatum“, kleines „du“; Regel-IDs der XRechnung entfallen (sie stehen im Prüfbericht),
     gesetzliche Fundstellen bleiben. Nur Texte, keine Logik.
169. **Kleine Token-Regeln.** Drei Fokus-Abstände als Tokens; auf Status-Flächen nie
     `--text-subtle`; aktive Seite der Seitenleiste mit 3-px-Balken (≥ 3:1); Hinweis bleibt bei Fehler
     stehen; Toast-Regionen auch leer im Accessibility-Tree; `Button busy` (`aria-disabled` +
     `aria-busy`, fokussierbar).
170. **Angebots-Status „Fertiggestellt“.** Der Server-Status `sent` bedeutet Festschreiben, nicht
     Versand; die Oberfläche nennt ihn „Fertiggestellt“ und zeigt den Versand getrennt („versendet am …“
     aus dem E-Mail-Protokoll). Keine Migration.
171. **Hauptaktion nach dem Ausstellen (präzisiert Nr. 96).** „Senden“, bis ein Versand belegt ist;
     danach „+ Zahlung“ bzw. „Annehmen“ (beide jederzeit als Nebenaktion erreichbar).
172. **Karte „Ausstellen“ unter den Positionen.** Mit nur lesender Prüfung beim Öffnen, Leitweg-ID
     (ohne „N/A“-Vorbelegung; leer bedeutet wie bisher N/A) und Knopf; Ablehnungen als Banner mit Links
     zum Ort der Korrektur.
173. **„Überfällig“ in Liste und Detail.** Aus Fälligkeit vor dem heutigen Berliner Tag und offenem
     Betrag; nicht für Entwürfe, Storno-Belege und Storniertes; „Teilbezahlt“ einheitlich.
174. **Fälligkeit Pflicht, Währung fest Euro.** Ohne Fälligkeit lehnt die Domain das Ausstellen bei
     offenem Betrag ab (BR-CO-25); deshalb schon im Entwurf Pflicht.
175. **Einzelpreis startet leer.** Eine Gesamtsumme von 0 € ist ein Fehler an den Preisfeldern;
     einzelne Gratis-Positionen bleiben möglich.
176. **Rückfrage vor Abo-Läufen mit Auto-Modus.** Nach dem wirksamen Modus (nach einem Restore immer
     „Entwurf“, dann ohne Rückfrage); die Rückmeldung nennt das tatsächliche Ergebnis.
177. **Abo-Modi und Pausieren.** Ein Modus ist kein Status (neutrale Badges, „Ausstellen + senden“ als
     Warnung); Pausieren ist umkehrbar und läuft ohne Rückfrage mit „Rückgängig“ (ersetzt KB-0057).
178. **Storno mit Gefahr-Dialog; „Storno-Beleg“ als Belegart.** Neutrales Badge statt Warnfläche.
179. **Listen-Ergebniszeile.** Eine dauerhafte `role="status"`-Zeile trägt Laden, Trefferzahl und
     „Keine Treffer …“ (auch Kontakte, E-Mail- und Backup-Protokoll); darunter nur die Aktion.
180. **Sofort festgeschriebene Belege heißen „festschreiben“ und fragen nach (ADR 0019).** Kosten
     (manuell und Import) und Privatbewegungen: Rückfrage mit Eckwerten und eine Folgezeile über dem
     unteren Knopf („Danach nicht mehr änderbar — Korrektur nur per Storno“).
181. **Werte übernehmen statt Duplizieren (`?from=`).** Nach einem Storno „Korrigiert neu erfassen“ mit
     den Daten des Originals; sonst „Als Vorlage für neue Kosten“ (Datum heute, ohne
     Lieferanten-Rechnungsnummer). Neuer Datensatz, keine Verknüpfung, keine Server-Änderung.
182. **Kosten-Detail nach dem Detailseiten-Muster.** Stornieren im Menü; „Als unbezahlt markieren“ ist
     umkehrbar mit „Rückgängig“.
183. **Firmendaten auf der Übersicht.** Banner und erste Aufgabe, solange eine beim Ausstellen geprüfte
     Pflichtangabe fehlt.
184. **Hinweise abhaken.** Toast und Fokus auf den nächsten Eintrag; „Alle abhaken“ fragt nur nach,
     wenn dringende Hinweise dabei sind (kein Rückgängig ohne neuen Command).
185. **Begriffe: „Anschaffung“ für Bereich und Objekt, „Anlage“ nur als Steuerbegriff;
     „Privatbewegungen“ für Bereich, „Privatbewegung“ für das Objekt** — wie Navigation, Handbuch-Kapitel
     und Glossar; die Bewegungsart ist kein Status (neutrales Badge, Vorzeichen).
187. **Übersicht ohne Statusfarben für Belegarten**; Kartenüberschriften als h2.
188. **Beleg zuerst im Kosten-Formular.** Datei-Feld und Kamera-Knopf am Anfang, damit „erst
     fotografieren, dann abtippen“ ohne Scrollen geht.
190. **Pflicht fürs Ausstellen ist nicht Pflicht fürs Speichern.** Telefon (BR-DE-6) und
     Steuernummer/USt-IdNr. in den Firmendaten mit „(Pflicht für die E-Rechnung)“ und Warnung beim
     Speichern, aber ohne Sperre — der Server speichert ohne sie, eine neue Firma hat oft noch keine
     Steuernummer.
191. **Kontaktformular bildet nur die Server-Regeln nach.** Keine strengeren Prüfungen (IBAN-Prüfziffer,
     BIC-Format nimmt der Server an); Server-Meldungen am Feld, Rest als Banner.
192. **Rückfrage vor dem Wiederherstellen nennt den ganzen Ablauf** (Re-Auth, Neustart, Anmeldung mit
     dem Backup-Passwort, Pause bis zur Freigabe; ADR 0040 Nr. 11–14).
193. **„Jetzt sichern“** mit Laufstatus, genau einem Toast und dem Ergebnis je Ziel in der Liste.
194. **Nr. 135 gilt auch für fehlende Eingaben** (SFTP testen/hinzufügen, Anheben, Abgleich, Uhrzeit,
     Hochladen): Grund am Feld, Fokus dorthin.
195. **Umkehrbares ohne Rückfrage, mit „Rückgängig“.** Kontakt archivieren, Paket archivieren, Konto
     deaktivieren; das Audit bleibt append-only (zwei Einträge).
196. **Nachladen nach einer Aktion ohne Ladezustand**, damit Inhalt und Knopf (Fokus) stehen bleiben.
198. **Tabellen in Abschnittskarten bleiben `table.cards`** (keine Karte in der Karte).
200. **Zwei Badge-Arten (U4-Design).** Status mit Punkt und Ton; Merkmal oder Art (Storno-Beleg, Abo,
     §13b, Modus, Bewegungsart, Anteil, Vorlagenherkunft …) als `kind="tag"` neutral ohne Punkt — der
     Punkt bedeutet allein Status (verallgemeinert Nr. 177/185/187).
201. **Status-Wörter je Bereich aus einem Helfer, groß geschrieben.** Kosten zeigen „Bezahlt“/
     „Unbezahlt“ (sie sind ab dem Speichern festgeschrieben, ADR 0019, „erfasst“ sagte nichts);
     Anschaffungen „Noch keine AfA“, „In Abschreibung“, „Festgeschrieben“, „Abgeschrieben“, „Veräußert“.
202. **Kennzahlen ohne Umbruch.** Größe an die Kachelbreite gekoppelt, festes Raster, auf dem Handy zwei
     Spalten; kein Umordnen per CSS `order` (Fokus- und Lesereihenfolge folgen der Sicht).
203. **Kein Hero.** Das Datum der Startseite steht in `--type-heading`; `--type-display` wird nicht
     genutzt.
204. **`.kb-linklist` für Linkziele** (Startseite, Einstellungen-Hub): eine Karte mit Lineatur statt
     vieler Einzelkarten; Listen mit Aktionen (Hinweise) ebenso als eine Karte mit Lineatur.
205. **Eine Summenregel** (`tr.kb-sum`/`.kb-ledger`, doppelte Linie `--rule-sum`, Tabellenmodus und
     Druck).
206. **Echtes Minuszeichen (U+2212)** in Beträgen; so breit wie eine Ziffer, der Parser liest „−“ und „-“.
207. **Hinweis-Zähler neutral**, Gefahrfarbe nur bei dringenden Hinweisen (Name „…, dringend“).
208. **Ansichts-Dialoge:** „Schließen“ ist sekundär und steht zuerst; primär nur ein echtes Ergebnis.
209. **Keine Endlos-Animationen**; Lade-Platzhalter statisch, Übergänge höchstens 200 ms.
210. **Toast-Aktionen und Vorschläge sind Sekundärknöpfe**; `Button` verhält sich wie `.btn-*`.
211. **Gruppierte Listen** (Paket-Kategorien) teilen feste Spaltenbreiten für durchgehende Fluchten.
212. **Statische Dateien vorkomprimiert, dynamische Antworten nie komprimiert (U4, Performance).** Der
     Frontend-Build legt `.br` (Brotli 11) und `.gz` (gzip -9) neben Text-Dateien über 1 KiB ab (Vite-
     Plugin mit `node:zlib`, ohne neue Abhängigkeit; nur wenn kleiner); `ServeDir`/`ServeFile` wählt
     sie nach `Accept-Encoding` (auch für den `index.html`-Fallback) und setzt `Vary: Accept-Encoding`.
     Begründung: Die Skripte gingen roh über die Leitung; mobil lag die Ladezeit 10–11 % über der
     Baseline. Gemessen (Pixel 7, CPU 4×, Slow 4G, Median aus 5): `/` FCP 2,08 s (U0 4,09 s), LCP
     2,86 s (U0 4,65 s); `/invoices` FCP 2,07 s, LCP 2,56 s; übertragen rund 70 % weniger. BREACH-
     Abgrenzung: Die SPA-Dateien sind öffentlich, für alle gleich und spiegeln keine Eingabe; `/api/`
     (Sitzungsdaten, gespiegelte Eingaben), `/files/` (Belege und Archive bitgenau) und `/legal/` bleiben
     unkomprimiert; es gibt keine Kompression zur Laufzeit (`CompressionLayer` verworfen: CPU je
     Anfrage, BREACH-Fläche, neue Crate; Kompression nur am Proxy verworfen: hinge vom Betreiber ab).
220. **§19-Rückkehr ehrlich beschreiben (U6, präzisiert Nr. 21).** Der Grund eines §19-Wechsels wird nicht
     gespeichert; „Meine Firmendaten“ nennt deshalb beide Fälle (5-Jahres-Bindung nur beim freiwilligen
     Verzicht) und die tatsächliche Sperre des Servers bis zum genannten Datum. Eine Rückkehr nach einem
     gesetzlichen Wegfall vor Ablauf ist eine fachliche Lücke (Entscheidung Manuel).
221. **Kosten-Abo „Buchen“ ist Festschreiben (U6, ADR 0019, Nr. 180).** Gleiche Rückfrage mit Eckwerten.
222. **Seitennamen nach Glossar (U6).** „Erinnerungen einstellen“ und „Kosten-Abos“ in Hub, Titel, PageBar,
     Links und Handbuch.
223. **Rücksprung per `?next=` nur innerhalb der App (U6).** `safeNextPath` lässt nur relative Pfade mit
     genau einem führenden „/“ zu (kein `//`, kein Backslash, keine Steuerzeichen, nicht `/api`, `/files`,
     `/legal`) — keine offene Weiterleitung.
224. **Pause-Banner nach Wiederherstellung mit Prüf-Links (U6).** Rechnungen, Kosten, Nummernkreise und
     vorgemerkte Entwürfe, Abo-Rechnungen, Kosten-Abos, Hinweis zu den Abo-Terminen.
225. **Einmal-Codes überall kopieren oder lokal speichern (U6).** Gemeinsamer Helfer für Einrichtung und
     Neuerzeugung; die Textdatei entsteht nur im Browser.
226. **Hilfe-Knopf und F1 führen zum selben Kapitel (U6).**
230. **`?next=` erst nach Normalisierung sicher (U7, R2-01, präzisiert Nr. 223).** Pfad roh und dekodiert
     geprüft (keine leeren Segmente, keine prozentkodierten Trenn- oder Steuerzeichen, Server-Präfixe ohne
     Rücksicht auf Groß-/Kleinschreibung); Gegenprobe: das Ergebnis bleibt als relativer Link auf demselben
     Origin — `new URL()` machte aus `/.//host` ein `//host`.
231. **Toasts verwerfen beim Verlassen von „bereit“ (U7).** Sperre oder Sitzungsablauf verwirft alle Toasts
     samt „Rückgängig“-Aktionen.
232. **`--toast-h` als belegter Streifen (U7, 2.4.11).** `<main>` wird unten um diesen Wert verlängert, damit
     `scroll-padding-bottom` auch am Seitenende greift; Toasts bleiben unten; Escape schließt einen Toast.
233. **Fokusführung bei Toasts (U7).** Nach Schließen oder Aktion: nächster Toast, Ursprung, h1, `<main>` —
     nie `<body>`.
234. **Fokus nach ausgeblendeter Zeile (U7).** Erste Aktion der Zeile an derselben Stelle, sonst der vorigen,
     dann Filter, dann h1 (Konten, Kosten-Abos, Abo-Vorlagen).
235. **Keine leeren Anmeldefelder an den Server (U7).** Enter im Passwortfeld führt ins Code-Feld; jede leere
     Anfrage zählte sonst als Fehlversuch.
236. **Kein Personenbezug im Seitentitel (U7, DSGVO Art. 17, ergänzt Nr. 160).** `docTitle` trennt den
     Browser-Titel vom sichtbaren h1; die Kontakt-Detailseite heißt im Verlauf „Kontakt“.
237. **Formal gültige Einheiten-Codes sperren nicht (U7, präzisiert Nr. 150).** 2–3 Großbuchstaben oder
     Ziffern gelten als Rec-20/21-Code („(Code, nicht in der Liste)“); ob die Codeliste ihn kennt,
     entscheidet der KoSIT-Validator beim Ausstellen; Freitext wie „Pauschal“ bleibt gesperrt.
238. **Storno-Entwurf ohne „Ausstellen“ (U7).** Hauptaktion „Weiter zum Stornieren“ der Original-Rechnung;
     die GoBD-Sperre im Server bleibt.
239. **Unklarer Ausgang nach Zeitüberschreitung (U7).** Die Oberfläche rät zum Prüfen statt zum Wiederholen —
     ein neuer Klick erzeugt einen neuen Idempotenz-Schlüssel und bei Kosten eine zweite festgeschriebene
     Buchung.
240. **Hinweis bei „1.234“ in Mengenfeldern (U7, ergänzt Nr. 15).** Die Leseregel bleibt; bei Feldern mit
     mindestens 3 Nachkommastellen nennt ein nicht sperrender Hinweis die Auslegung und die Komma-Alternative.
241. **Startseiten-Aufgaben als Tags; „Versendet“ `info`, „Aktiv“ `success` (U7).**
242. **„Keine Seiteneffekte beim Öffnen“ als Test über alle Svelte-Dateien (U7, Plan §9.2 Nr. 8).** Wurzeln
     `onMount`, `$effect`, `$effect.pre`, `afterNavigate`, `load`; Upload- und Auth-Schreiber zählen mit.
250. **Storno übernimmt die Leitweg-ID des Originals (U7, R1-01).** BT-10 wird aus der archivierten XML des
     Originals gelesen (Hash-Prüfung und Audit `archive.read` wie überall); fehlen Archiv-ID oder Element,
     gilt „N/A“; ein manipuliertes Archiv bricht den Storno ab — keine ungeprüften Daten in einem GoBD-Beleg.
     Kein Eingabefeld, kein Speicherfeld, keine Migration.
251. **§19-Wechseldialog mit drei Fällen (U7, R1-02, präzisiert Nr. 21/220).** Verzicht nach § 19 Abs. 3 UStG
     wirkt ab Beginn des Kalenderjahres und bindet mindestens fünf Kalenderjahre; Wegfall wegen über
     25.000 € im Vorjahr ab 1. Januar, wegen über 100.000 € im laufenden Jahr ab diesem Umsatz, beide ohne
     Bindung; Hinweis auf § 15a UStG und „ersetzt keine Steuerberatung“. Die Rückkehr-Sperre des Servers
     bleibt und wird offen genannt.
252. **Abgang einer Anschaffung mit Gefahr-Rückfrage (U7, R1-05).** Eckwerte aus der Server-Vorschau.
253. **Vorzeichen in Aufstellungen (U7, R1-06).** Addierte/abgezogene Zeilen mit „+“/„−“; im Fließtext
     „Gewinn/Verlust von“ ohne Vorzeichen.
254. **Letzte Off-Site-Kopie nur am selben Ort (U7, R2-03, präzisiert Nr. 86).**
255. **Timeout-Text des Servers (U7, R1-07).** Bittet um Prüfung in der Liste statt um Wiederholung.
256. **Zahldatum im Handbuch (U7, R4-01).** Immer der tatsächliche Zahlungstag; eine Zuordnung nach der
     10-Tage-Regel (§ 11 EStG) gehört in die Erklärung, nicht in ein abweichendes Datum (GoBD).
257. **axe „target-size“ unter der Kopfleiste (U7, erweitert Nr. 59).** Eine Checkbox, die 2.5.8 am
     Seitenanfang über den Abstand erfüllt (Label umschließt sie), gilt nicht als Verstoß, wenn ein
     Prüfzustand die Seite so scrollt, dass sie halb unter der klebenden Kopfleiste liegt und axe deren
     Menü-Knopf als Nachbarn zählt (Kosten-Abos, offenes Zeilenmenü im Querformat). Ausnahme im
     Harness nur für diesen Zustand.

## Konsequenzen

- Prüfungen und Befunde stehen in `Buchhaltung/docs/REVIEW-2026.11.md` (nicht veröffentlicht),
  Ausnahmen von automatischen Kriterien zusätzlich hier.
- Öffentliche Texte machen keine Konformitätsaussagen; erlaubt ist „orientiert sich an
  WCAG 2.2 AA, geprüft mit axe-core und manuell“.
