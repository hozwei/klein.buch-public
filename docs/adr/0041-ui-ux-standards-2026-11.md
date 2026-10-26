# ADR 0041 — UI/UX-Standards (2026.11)

**Status:** In Arbeit · 2026-10-25 · Lauf 2026.11 (Blöcke U0–U7). Keine Migration.
Ergänzt ADR 0039 (Web-/Docker-Betrieb) und ADR 0040 (Dauerbetrieb) für die Oberfläche;
abgelöste PRD-Stellen stehen in PRD-Revision 10 „UI/UX (2026.11)“.

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

## Konsequenzen

- Prüfungen und Befunde stehen in `Buchhaltung/docs/REVIEW-2026.11.md` (nicht veröffentlicht),
  Ausnahmen von automatischen Kriterien zusätzlich hier.
- Öffentliche Texte machen keine Konformitätsaussagen; erlaubt ist „orientiert sich an
  WCAG 2.2 AA, geprüft mit axe-core und manuell“.
