# ADR 0040 — Dauerbetrieb: Start- und Zeitlogik

**Status:** Akzeptiert · 2026-10-24 · Lauf F (Block F1, Teile F1a, F1b, F1c und F1d — Prüferbefunde).
Keine Migration; neue Zustände stehen in `app_settings` bzw. im Seed-Manifest.
Ergänzt per Amendment ADR 0003, 0009, 0023, 0027, 0033, 0034 und 0039; PRD-Revision 9
„Dauerbetrieb (Lauf F)“ löst die Start-Logik der Desktop-Fassung im PRD ab.

## Kontext

Seit ADR 0039 läuft Klein.Buch als Server rund um die Uhr. Die Scheduler-Jobs
stammen aus der Desktop-Zeit (ADR 0023, 0027, 0033): Sie gingen davon aus, dass
die App geöffnet wird, wenn der Nutzer arbeitet, und holten beim Öffnen der App
„alles Verpasste“ nach. Im Dauerbetrieb treffen sie auf andere Lagen: Tick und manuelle
Auslöser laufen parallel, ein Container kann mitten im Lauf beendet werden, die
Instanz bleibt nach einem Neustart tagelang gesperrt (E3), Uhren springen
(NTP-Fehler, VM-Snapshot), Seiten werden in mehreren Tabs geöffnet. Die Reviews
W14 (W14-022, -025, -026, -027, -030, -031, -034), v2026.5 (R1-017,
R1-S2-NEXTDUE), A–Q (KB-0045), die ASVS-Prüfung (V11.1.5, V11.1.6) und das
Startlogik-Inventar (UI/UX-Plan §9, D-01 bis D-08, D-12 bis D-14, D-19 bis
D-24) und die Aufgaben W5-STOP-GRACE, W13-GRACE-BACKUP, W14-023, KB-0038
beschreiben die Folgen: Doppelbuchungen, zweite Rechnungen nach Abbruch,
Massenversand nach Reaktivierung oder nach einem Restore, erneut vergebene
Rechnungsnummern nach dem Restore eines älteren Backups, ältere Datenstände auf
den Off-Site-Zielen, wandernde Tages-Backups ohne Monatsklasse, Lock-Backups,
die Vortage verdrängen, Dauer-Vollscans nach einem Fehler, ein Herunterfahren,
das länger wartet als Docker, unbegrenztes Warten beim Sperren und
Schreibzugriffe beim Seitenaufruf.

Dazu kommen Reste des Desktop-Modells in Anzeige, Texten und Doku (D-10, D-15
bis D-18, D-25): Fälligkeiten und Zeitstempel in der Zeitzone des Geräts,
`.neu`-Dateien, die nach dem Löschen bei jedem Start wiederkommen, ein toter
Kanal für OS-Benachrichtigungen, Kommentare, Nutzertexte, ADRs, PRD und
Arbeitsanweisungen, die den Programmstart der Desktop-Fassung beschreiben, wo im
Server etwas anderes geschieht.

## Zielmodell

Abgeleitet aus dem UI/UX-Plan §9.2; so umgesetzt:

1. **Zeitbasiert statt startbasiert.** Jeder Job bestimmt seine Fälligkeit aus
   persistierten Werten (`next_due_date`, `backup_daily_last_on`,
   Backoff-Zeitstempel) und der Uhr in Europe/Berlin; kein Job hängt am
   Prozessstart, am Entsperren oder an einem Seitenaufruf. Der Tick ist nur der
   Taktgeber (Nr. 1, 10, 15).
2. **Idempotent und atomar.** Erzeugen und Vorrücken in einer Transaktion mit
   Compare-and-Swap, Sperre je Abo bzw. Vorlage für Tick und manuelle Auslöser,
   Fehler je Element (Nr. 2, 3, 8).
3. **Nachholen mit Augenmaß.** Belegdatum-Regeln unverändert, Obergrenzen,
   Versand nur für den jüngsten Termin, Rückfrage bei größerem Rückstand (Nr. 4).
4. **Pause** nach Restore und bei unplausibler Uhr, Freigabe nur im Browser
   (Nr. 5, 6, 11–13).
5. **Kalenderfeste Backups** und Rotation nach Kalender-Eimern (Nr. 15, 16).
6. **Sauberes Anhalten** innerhalb der Docker-Frist (Nr. 9, 19).
7. **Gesperrter Zustand sichtbar, Entsperren ohne Lastspitze** (Nr. 18).
8. **Keine Seiteneffekte beim Ansehen** (Nr. 10).
9. **Aufräumen im Betrieb** (Nr. 19).
10. **Testbar** mit injizierter Uhr über alle Threads (Nr. 1).
11. **Eine Zeitzone auch in der Anzeige.** Was die Oberfläche als „heute“,
    „fällig“ oder Zeitpunkt zeigt, ist Europe/Berlin — unabhängig vom Gerät
    (Nr. 20).
12. **Begriffe des Dauerbetriebs.** Code, Texte und Doku sprechen von
    Serverstart, Containerstart, Entsperren und Tick, nicht vom Programmstart
    der Desktop-Fassung; was
    Desktop-Historie ist, steht als solche da (Nr. 21–23).

## Entscheidung

1. **Injizierbare Uhr.** Das Trait `clock::Clock` (`now_utc`, `today_berlin`)
   liegt im `AppCtx` (`AppCtx::clock`, Standard `SystemClock`). Scheduler-Jobs
   bekommen sie über `scheduler::JobCtx`; der Rechnungsweg nimmt „heute“ als
   Parameter (`create_invoice_draft_on`, `run_lock_pipeline_on`). Tests nutzen
   `clock::FixedClock`, die über alle Tokio-Worker und `spawn_blocking` gilt. Die
   thread-lokale Fixierung gibt es nur noch in den Unit-Tests des Crates.
2. **Kosten-Abos mit Sperre, Compare-and-Swap und Transaktion.** Tick, „Alle
   fälligen buchen“, „Buchen“, Bearbeiten und Pausieren laufen je Abo unter
   `locks::RECURRING_SUBSCRIPTIONS`. Nummer, Kosten-Position mit Abo-Bezug, beide
   Audit-Einträge und das Vorrücken (`… WHERE next_due_date = ?`) committen in
   einer `BEGIN IMMEDIATE`-Transaktion. Ein Fehler betrifft nur sein Abo
   (Hinweis), die übrigen laufen weiter, das Backup entsteht trotzdem.
3. **Abo-Rechnungen absturzfest.** Nummer, Entwurf, Audit `recurring_invoice.run`
   und das Vorrücken der Vorlage (Compare-and-Swap) committen in einer
   Transaktion. Bei den Stufen „festschreiben“ und „Auto + Versand“ steht in
   derselben Transaktion ein Fortsetzungs-Marker
   (`app_settings.recurring_invoice_pending:<vorlage>` = Rechnung, Termin, Stufe).
   Festschreiben und Versand folgen danach; der Marker fällt erst weg, wenn sie
   erledigt oder mit Hinweis gescheitert sind. Bricht der Prozess dazwischen ab,
   setzt der nächste Lauf genau dort fort (festschreiben, falls noch Entwurf;
   versenden, falls noch nicht versendet). Für denselben Termin entsteht nie eine
   zweite Rechnung.
4. **Nachholen mit Grenze und ohne automatischen Versand.** Bis drei fällige
   Termine je Reihe holt ein Lauf selbst nach (`catch_up::AUTO_LIMIT`). Bei
   Abo-Rechnungen folgt nur der jüngste Termin der Automatik-Stufe; ältere
   entstehen als Entwurf, ein Sammelhinweis nennt sie. Bei Kosten-Abos entsteht je
   Termin eine Position mit dem Stichtag als Belegdatum (wie bisher), dazu ein
   Sammelhinweis. Sind mehr Termine fällig, erzeugt der Lauf nichts und legt den
   Hinweis „Entscheidung nötig“ an. Der Inhaber wählt in der Liste „Rückstand …“:
   **nachholen** (Bestätigung, höchstens 24 Termine, `catch_up::CONFIRMED_CAP`) oder
   **ab heute fortsetzen** (nächster Termin = erster Termin ab heute; die
   ausgelassenen stehen im Audit). Dieselbe Wahl verlangt das Fortsetzen einer
   pausierten Reihe mit verpassten Terminen (`resumeMode`). Eine neue
   Abo-Rechnungs-Vorlage darf nicht mit einem Datum in der Vergangenheit
   beginnen. Belegdatum-Regeln bleiben unverändert: Abo-Rechnungen tragen das
   Erstellungsdatum, der Leistungszeitraum steht im Leistungsdatum (§14 Abs. 4
   Nr. 3 und 6 UStG).
5. **Pause der automatischen Belegerzeugung.** `app_settings.scheduler_hold`
   (Grund) und `scheduler_hold_since` halten Abo-Rechnungen, Abo-Kosten und die
   Auto-AfA an — im Tick und bei manuellen Auslösern. Backups,
   Integritätsprüfung und Hinweise laufen weiter. Die Pause übersteht Neustart und
   Sperren; aufheben kann sie nur der Inhaber im Browser
   (`scheduler_release_hold`, Re-Auth, Audit `scheduler.hold_released`; Banner in
   der Oberfläche). Gründe: `clock_backwards`, `clock_jump` (Nr. 6) und
   `restore` (nach einer Wiederherstellung, W14-023).
6. **Plausibilitätsbremse für die Uhr (ASVS V11.1.5).** Jeder ungebremste Lauf
   merkt sich die zuletzt gesehene Zeit (`scheduler_last_seen_at`, höchstens
   stündlich fortgeschrieben). Steht die Uhr mehr als eine Stunde davor oder mehr
   als 400 Tage danach, setzt der Lauf die Pause aus Nr. 5 und legt einen
   dringenden Hinweis an.
7. **Abgeschlossenes Geschäftsjahr bei Kosten-Abos.** Liegt der nächste Termin in
   einem abgeschlossenen Jahr, entsteht ein Hinweis statt einer Warnung bei jedem
   Tick. „Nachholen“ überspringt solche Termine ohne Beleg (Audit
   `recurring.period_skipped`) und bucht die offenen Jahre; ein abgeschlossenes
   Jahr bleibt unverändert (GoBD).
8. **Vorlagen unter Sperre, frisch gelesen.** Bearbeiten und Pausieren einer
   Abo-Rechnungs-Vorlage nehmen die Vorlagen-Sperre; der Lauf nimmt sie je Termin
   und liest die Vorlage frisch — eine Pause wirkt ab dem nächsten Termin. Vorlagen
   mit archiviertem oder anonymisiertem Kunden, ohne Positionen oder mit
   USt-Ausweis trotz §19 erzeugen keinen nummerierten Entwurf, sondern einen
   Hinweis. Die Anonymisierung eines Kontakts pausiert in derselben Transaktion
   seine Vorlagen und Kosten-Abos.
9. **Anhalten mit Zeitgrenze.** `scheduler::tick::request_stop` setzt eine
   kooperative Abbruch-Anforderung (`AppCtx::scheduler_stop`); Abo-Läufe beenden
   den laufenden Termin und hören auf, weitere Ticks lassen aus. „Datenbank
   sperren“ beendet zuerst alle Sitzungen, wartet höchstens 10 s auf den Tick
   (`SCHEDULER_STOP_WAIT`) und sperrt danach auch, wenn er noch läuft — ohne Pool
   und Passphrase läuft er leer. „Passphrase ändern“ braucht die Datei exklusiv
   und antwortet nach dieser Frist mit 503. Das Entsperren wartet nicht auf einen noch
   laufenden alten Tick: Die Pause ist ein eigenes Flag (`AppCtx::scheduler_paused`),
   das `tick::resume` ohne die Tick-Sperre zurücksetzt (F1d).
10. **Keine Schreibwirkung beim Seitenaufruf.** Die Seite „Wiederkehrende
    Rechnungen“ löst beim Öffnen keinen Lauf mehr aus (Knopf bleibt). Die
    Standard-Zahlungskonten legt der Server einmal je Instanz an — bei der
    Einrichtung bzw. beim ersten Entsperren nach dem Update, in einer Transaktion
    mit dem Marker `payment_accounts_defaults_seeded`; der Command
    `payment_accounts_ensure_defaults` entfällt. Ein Vitest prüft alle Seiten:
    `onMount` ruft keinen schreibenden Command.

11. **Pause nach einem Restore (W14-023).** Das erste Entsperren nach einem
    angewendeten Restore schreibt in **einer** Transaktion den Audit
    `backup.restore.applied`, die Pause aus Nr. 5 mit Grund `restore` (sie
    ersetzt eine aus dem Backup mitgebrachte Pause) und das Anheben der
    Nummernkreise (Nr. 12). Die Pause steht damit in der eingespielten DB und
    übersteht auch die Migration eines älteren Backups. Sie hält Abo-Rechnungen,
    Abo-Kosten, Auto-AfA und jeden automatischen Versand an, bis der Inhaber im
    Browser freigibt; der Freigabe-Dialog fragt ausdrücklich „Ist dies die
    produktive Instanz?“. In derselben Transaktion gleicht
    `backup::after_restore` die Abo-Reihen ab (F1d): Phase A hält die nächsten
    Termine aller Vorlagen und Kosten-Abos der laufenden DB fest; das erste
    Entsperren rückt jede Reihe nur vorwärts auf diesen Stand (Audit
    `recurring_invoice.advanced_after_restore` bzw.
    `recurring.advanced_after_restore`), sodass ein bereits abgerechneter
    Zeitraum nach der Freigabe nicht erneut abgerechnet wird. Reihen ohne
    bekannten Stand mit Termin bis zum Restore-Tag verlangen die Entscheidung
    „Rückstand …“ (Marker `recurring_restore_review:*`, unabhängig von der
    Nachhol-Grenze, Rechnungen nur als Entwurf). Fortsetzungs-Marker und
    Bestätigungen aus dem Backup gelten nicht mehr. Hinweis „Abo-Termine nach
    der Wiederherstellung“.
12. **Keine erneute Vergabe von Nummern nach einem Restore (D-24).** Phase A
    des Restores hält die Stände aller Nummernkreise der laufenden DB im
    Restore-Marker fest (Änderungen sind ab der Annahme gesperrt). Nach dem
    Tausch hebt das erste Entsperren jeden Kreis der eingespielten DB auf
    diesen Stand an, wo er höher war (`db::numbering::raise_in`, nur aufwärts,
    Audit `numbering.raised_after_restore` je Kreis). Die dazwischen fehlenden
    Belege sind eine dokumentierte Lücke. Bei Disaster Recovery in eine leere
    Instanz sind die späteren Stände unbekannt: Ein dringender Hinweis und das
    Handbuch führen zum neuen Command `numbering_raise` (Re-Auth, Begründung
    Pflicht, nur aufwärts, Audit `numbering.raised`; Einstellungen → Backups →
    Nummernkreise). Phase A hält außerdem die Nummern aller festgeschriebenen
    Rechnungen, Stornos und Angebote fest; ein Entwurf des eingespielten Stands
    mit einer dieser Nummern wird vorgemerkt und lässt sich erst festschreiben,
    nachdem der Inhaber den Abgleich mit dem versandten Original bestätigt hat
    (`numbering_restored_draft_confirm`, Re-Auth, Begründung, Audit; F1d,
    Entscheidung 35).
13. **Off-Site-Spiegelung erst nach der Freigabe (D-23).** Während der Pause
    mit Grund `restore` schreibt `backup::create_with` nur den lokalen Floor
    (Audit `backup.mirror.skipped`, je Ziel das Ergebnis „ausgesetzt“); das
    Pflicht-Backup vor der Migration eines gerade eingespielten Stands ist
    ebenso Floor-only. Eine Probe-Instanz legt damit nie Dateien mit neuerem
    Zeitstempel und älterem Datenstand neben die echten Backups.
14. **Historie nach dem Restore vollständig (D-22).** Das erste Entsperren
    übernimmt das Pre-Restore-Backup und alle `klein-buch-*.kbk` im Floor ohne
    Historienzeile (entstanden nach dem eingespielten Stand) in
    `backup_history` — Erstellungszeit aus dem Manifest, Klasse `manual`, also
    nie automatisch gelöscht und in der Restore-Liste wählbar — und entfernt
    Floor-Zeilen, deren Datei fehlt. Audit `backup.history_rescanned`.
15. **Tages-Backup kalenderfest (D-04, D-12).** Genau ein `auto_daily` je
    Berliner Kalendertag, frühestens ab `app_settings.backup_daily_time`
    (Standard 02:30, Command `backup_set_daily_time`), sonst beim ersten Tick
    danach — nach einer Sperre über mehrere Tage genau eines. Fällig ist der
    zuletzt fällige Slot (heute ab der Uhrzeit, sonst gestern): Eine Uhrzeit
    kurz vor Mitternacht zwischen zwei Ticks und eine Anmeldung vor der
    Uhrzeit nach Tagen der Sperre holen den verpassten Slot beim ersten Tick
    nach (F1d). Gemerkt wird der Slot in `backup_daily_last_on` (unabhängig
    von der Historie, die die Rotation ausdünnt); Lock-Backups zählen nicht. Am Tag der Zeitumstellung
    gilt die Wanduhr (übersprungene 02:30 ab 03:00, doppelte 02:30 einmal).
    Backoff nach einem Fehlschlag 1 h, persistiert
    (`backup_daily_last_failure_at`). Das Erst-Backup der Einrichtung entsteht
    vor dem Scheduler-Start und gilt als Tages-Backup dieses Tages.
16. **Rotation nach Kalender-Eimern, SFTP ohne Löschung (D-05, D-06).** Je
    Ablage bleibt das jüngste automatische Backup je Berliner Tag, Monat und
    Jahr für die jüngsten 7/3/1 (Floor) bzw. 30/12/7 (Ordner-Ziele) Eimer mit
    Backup; `retention_tag` ist nur noch informativ. Auf SFTP-Zielen löscht
    Klein.Buch nichts, auch keine Historienzeilen. Während jeder Pause ruht die
    Rotation (ein Uhrsprung oder eine Probe-Instanz löscht keine echten
    Backups). Mehr als eine Stunde in die Zukunft datierte Backups bleiben
    erhalten, belegen aber keinen Eimer, bis die echte Zeit sie einholt (F1d).
17. **Integritäts-Cron mit Sperre und Backoff (D-07, KB-0038).** Monats-Cron
    und „Jetzt prüfen“ teilen `integrity_check::SCAN_LOCK` (der Cron wartet und
    prüft danach erneut, der manuelle Lauf antwortet „läuft bereits“). Nach
    einem Fehlschlag 6 h Backoff (`integrity_last_failure_at`) und ein Hinweis
    je Monat. Der Vollscan prüft vor jeder Datei die Abbruch-Anforderung; ein
    abgebrochener Lauf zählt nicht. `started_at` kommt aus der injizierten Uhr.
    „Schon gelaufen“ heißt: ein abgeschlossener Lauf im Berliner Monatsintervall
    (F1d) — ein zukünftig datierter Lauf löst keinen Dauer-Vollscan aus.
18. **Gestaffelt nach dem Entsperren, sichtbar gesperrt (D-08, D-19, D-21).**
    Das Pflicht-Audit `backup.unlock.ok` steht vor dem Einhängen von Pool,
    Passphrase und Scheduler; scheitert es, bleibt die Instanz gesperrt. Der
    erste Tick wartet 60 s nach dem Entsperren (`tick::UNLOCK_SETTLE`) und lässt
    den Vollscan aus. Abo-Läufe eines Ticks sammeln ihre Lock-Backups
    (`JobCtx::lock_backup`); am Ende entsteht genau ein Backup — ein fälliges
    Tages-Backup deckt sie mit ab. Beim Sammeln steht der Vermerk
    `backup_pending_since` in der DB; fällt das Backup wegen „Datenbank
    sperren“ oder Herunterfahren aus, holt der erste Tick danach es nach (F1d). Das Log meldet den gesperrten Zustand beim
    Start und alle 6 h. Mehr als zwei Tage ohne Aktivität (Audit-Log,
    `scheduler_last_seen_at`) meldet das Entsperren als Hinweis. „Backup
    überfällig“ rechnet in Europe/Berlin.
19. **Herunterfahren innerhalb der Docker-Frist (W5-STOP-GRACE,
    W13-GRACE-BACKUP, W14-025b).** Das Signal setzt sofort die
    Abbruch-Anforderung. Nach dem Abwarten der Verbindungen (höchstens 30 s)
    wartet `finish_shutdown` gleichzeitig höchstens 25 s auf den Tick und 30 s
    auf abgekoppelte Tasks, dann höchstens 5 s auf das Schließen des Pools.
    `stop_grace_period` im Compose-Beispiel ist 90 s. Ein abgebrochenes Backup
    hinterlässt nur eine `.part`-Datei (Temp + rename), die der Start bzw. der
    stündliche Sweeper entfernt; Reste in `data/tmp` älter als 6 h räumt der
    Sweeper im Betrieb (D-13).

20. **Anzeige in Europe/Berlin (D-15, D-25).** `src/lib/date.ts` liest
    Zeitstempel der API zentral (`parseTimestamp`: DB-Format
    `YYYY-MM-DD HH:MM:SS` ohne Zone gilt als UTC, RFC 3339 mit Zone wie
    angegeben, ungültig ergibt `null` statt `RangeError`) und rechnet Tage über
    reine Kalenderdaten (`daysBetweenIso`, `daysUntilBerlin`, `berlinDateOf`).
    `src/lib/format.ts` formatiert mit `timeZone: Europe/Berlin` (`date`,
    neu `dateTime`); ein reines Datum bleibt der Tag, der es ist. Alle Seiten
    nutzen diese Helfer — die lokalen Formatierer der Protokoll-, Hinweis-,
    Geschäftsjahr-, Sicherheits-, Angebots- und Rechnungsseiten entfallen; die
    Backup-Liste zeigt die Zeitpunkte nicht mehr roh. „Tage bis Fälligkeit“,
    „heute fällig“ und der Zähler „fällig“ der Abo-Listen, die überfälligen Tage
    und das Backup-Alter der Übersicht folgen dem Berliner Kalender.
21. **Seed-Manifest merkt sich angebotene Fassungen (D-10).** Das Manifest
    führt `offered` (Datei → SHA-256 der Image-Fassung, die schon als
    `<name>.neu` angeboten wurde). Dieselbe Fassung wird nicht erneut angelegt
    oder gemeldet, auch wenn der Nutzer die `.neu`-Datei gelöscht hat; erst eine
    weitere Image-Fassung bietet erneut an. Übernimmt der Nutzer die neue
    Fassung, ist das Angebot erledigt. Hinweise tragen einen `dedup_key`
    (`seed:new|updated:<datei>:<hash>`), damit eine abgebrochene Auslieferung
    keinen Hinweis doppelt anlegt.
22. **Nur die Inbox (D-16).** Der Trait `notify::Notifier` mit `InboxOnly`,
    `AppCtx::notifier` und `JobCtx::notifier` sowie die Auflösung von
    `deliver_os_native` je Hinweis entfallen; `notify::emit` schreibt in die
    Inbox. Die Spalte bleibt (forward-only).
23. **Texte und Doku des Dauerbetriebs (D-16, D-17, D-18).** Kommentare und
    Meldungen sprechen von Server- bzw. Containerstart, Container-Neustart und
    Image statt Programmstart, Neustart und Update der Desktop-App („Bitte den Container neu
    starten“, „Bitte das Image der Version einsetzen, mit der die Datenbank
    zuletzt lief“, „die Instanz entsperren“). Die Schema-Hardline lautet: Der
    Server startet gesperrt; beim Entsperren lehnt Klein.Buch eine neuere
    Datenbank ab und bleibt gesperrt — geprüft vor dem Migrations-Runner, damit
    die Meldung das passende Image nennt statt „Migration fehlt“ (Test
    `db::tests::newer_database_is_rejected_on_unlock`, Amendment ADR 0003).
    ADRs bekommen datierte Amendments statt umgeschriebener Historie; das PRD
    bekommt Revision 9.

## Konsequenzen

- Kein Termin wird doppelt gebucht oder doppelt in Rechnung gestellt, auch nicht
  bei parallelen Auslösern oder einem Abbruch mitten im Lauf; eine
  zurückgerollte Transaktion verbraucht keine Belegnummer.
- Nach langer Sperre, Reaktivierung oder Uhrsprung entstehen keine
  Massenbuchungen und kein Massenversand mehr; der Inhaber entscheidet ab vier
  offenen Terminen. Kleinere Rückstände holt Klein.Buch wie bisher selbst nach,
  aber nachgeholte Rechnungen gehen nie automatisch hinaus.
- „Datenbank sperren“ ist bei einem hängenden Sidecar nach Sekunden wirksam.
- Neue Commands: `recurring_resolve_backlog`, `recurring_invoices_resolve_backlog`,
  `scheduler_hold_status`, `scheduler_release_hold` (Re-Auth). `recurring_set_active`
  und `recurring_invoices_set_active` nehmen `resumeMode`.
- Nach einem Restore geht nichts automatisch hinaus, keine Nummer wird doppelt
  vergeben, die Off-Site-Ziele bleiben bis zur Freigabe unberührt, und neuere
  Floor-Backups bleiben wählbar. Eine Probe-Instanz ist gefahrlos, solange sie
  nicht freigegeben wird.
- Backups: ein Tages-Backup je Kalendertag zu planbarer Zeit; je Tag, Monat und
  Jahr bleibt ein Stand, egal wie viele Lock-Backups entstehen. SFTP-Ziele
  wachsen, bis der Betreiber dort rotiert (Handbuch „Speicher und Ressourcen“).
- Neue Commands (F1b): `backup_set_daily_time`, `numbering_counters_list`,
  `numbering_raise` (Re-Auth); `backup_get_settings` liefert `dailyTime`.
- Neue Commands (F1d): `numbering_restored_drafts_list`,
  `numbering_restored_draft_confirm` (Re-Auth; damit 24 Re-Auth-Commands) und
  `recurring_restore_review_list`. Nach einem Restore wird kein bereits
  abgerechneter Abo-Zeitraum erneut abgerechnet, und ein wiederhergestellter
  Entwurf mit vergebener Nummer wird nur nach ausdrücklicher Bestätigung
  festgeschrieben.
- Nach dem Entsperren keine Lastspitze mehr (ein Backup, kein Vollscan im ersten
  Tick); das Herunterfahren bleibt unter 70 s.
- Tests laufen mit `FixedClock`: Zeitumstellung, Jahres- und Monatswechsel,
  parallele Auslöser, Abbruch, Uhrsprung, Tages-Backup über beide
  Zeitumstellungen, Restore eines älteren Backups (Nummern, Pause, Spiegelung,
  Historie), Herunterfahren während eines Ticks.
- Die Gestaltung der Auswahl- und Freigabe-Dialoge übernimmt der UI/UX-Lauf; die
  hier gebauten Dialoge sind funktional.
- Ein Gerät in einer anderen Zeitzone (Reise, falsch eingestellter Laptop) sieht
  dieselben Tage und Uhrzeiten wie der Server; ein Beleg „vom 24.10.“ erscheint
  nicht mehr als 23.10. Neue Seiten nutzen `date`/`dateTime`/`daysUntilBerlin`
  (Arbeitsanweisung in `klein-buch/CLAUDE.md`).
- Wer eine `.neu`-Datei nach dem Vergleich löscht, sieht sie und ihren Hinweis
  nicht wieder; ein Bestand vor Lauf F, in dem die `.neu` bereits gelöscht war,
  bekommt sie einmal neu (das Manifest kannte das Angebot noch nicht).
- Die Suche nach Begriffen des Desktop-Modells (Done-Check des Blocks F1) trifft
  nur noch Historie: CHANGELOG-Altabschnitte, ADR-Text vor den
  Amendments, PRD-Originalstellen mit Ablösung in Revision 8/9, Migrationen
  (Prüfsummen), Block-Notizen unter `memory/`, den Test der Tauri-Freiheit und
  die Spalte `deliver_os_native`; dazu bis F6 die Lizenzseite der
  Desktop-Fassung.

## Alternativen

| Option | Contra |
|---|---|
| Neue Spalten `invoices.recurring_invoice_id` + UNIQUE (Vorlage, Periode) | Migration; nach Nr. 3 überflüssig: ein Entwurf entsteht nur zusammen mit dem Vorrücken, der Termin einer Vorlage lässt sich nicht zurückstellen |
| Alle verpassten Perioden automatisch festschreiben und versenden (bisher) | Massenversand nach Reaktivierung oder Uhrsprung, Kunde erhält viele Rechnungen auf einmal |
| Nur noch den aktuellen Termin, verpasste stillschweigend auslassen | verliert Belege ohne Wissen des Inhabers (Vollständigkeit, GoBD) |
| Uhr-Bremse ohne gespeicherte Pause | eine Nutzeraktion mit falscher Uhr schriebe neue Zeitstempel und höbe die Bremse unbemerkt auf |
| Sperren erst nach Ende des Ticks | Sperren hängt bei hängendem Sidecar bis zu mehreren Minuten, fremde Sitzung bleibt so lange gültig |
| Pause nach Restore als Datei neben der DB statt in `app_settings` | ginge bei einem späteren Restore verloren und wäre nicht im Audit-Zusammenhang; in der DB übersteht sie Sperren, Neustart und Migration |
| Zähler nach Restore aus dem Pre-Restore-Backup lesen | verlangt Entschlüsseln eines Voll-Backups beim Entsperren; die Stände im Restore-Marker sind gleichwertig (Änderungen ab Annahme gesperrt) und billig |
| Nummernkreise nach Restore nicht anheben, nur warnen (bisher) | Doppelvergabe bereits versandter Nummern (§14 Abs. 4 Nr. 4 UStG) |
| Tages-Backup 24 h nach dem letzten Backup (bisher) | Uhrzeit wandert, Kalendertage ohne Tages-Backup, Lock-Backups verschieben es |
| Rotation nach Anzahl je Klasse (bisher) | ein reger Tag verdrängt alle Vortage, Monatsklasse hängt am Monatsersten |
| SFTP-Rotation durch Klein.Buch | eine kompromittierte Instanz könnte alle Off-Site-Kopien löschen; der Betreiber rotiert serverseitig |
| Off-Site-Spiegelung nach Restore sofort (bisher) | Probe-Instanz legt ältere Datenstände mit neuerem Zeitstempel neben echte Backups |

## Entscheidungen während der Umsetzung

1. **Nachhol-Grenze drei, Bestätigung bis 24 (W14-026).** Drei Termine decken
   eine Sperre von bis zu einem Quartal bei Monatsabos ab, ohne Rückfrage; mehr
   offene Termine deuten auf eine bewusste Pause oder einen Fehler hin. 24
   begrenzt auch den bestätigten Lauf (zwei Jahre monatlich); größere Rückstände
   rechnet man bewusst einzeln ab.
2. **Ältere Termine als Entwurf, nicht festgeschrieben-ohne-Versand (W14-026).**
   Ein Entwurf lässt sich vor dem Festschreiben prüfen; eine festgeschriebene,
   aber nicht versandte Rechnung müsste bei Fehlern storniert werden. Die Nummer
   vergibt schon der Entwurf (wie bisher, keine neue Lückenklasse).
3. **„Buchen“ und „Jetzt erstellen“ bleiben ausdrückliche Einzelaktionen.** Sie
   erzeugen genau einen Termin nach der Automatik-Stufe der Vorlage (auch
   Versand), weil der Nutzer sie für genau diesen Termin auslöst. Während einer
   Pause sind auch sie gesperrt.
4. **Fortsetzungs-Marker statt Schema (W14-025).** Je Vorlage ist höchstens ein
   Termin zwischen Entwurf und Festschreiben; ein `app_settings`-Schlüssel reicht
   und braucht keine Migration (Regel Lauf F: Schema bündelt F2). Ein Abbruch
   zwischen erfolgreichem SMTP-Versand und dem Vermerk `sent_at` kann beim
   Fortsetzen einen zweiten Versand derselben Rechnung auslösen (gleiche Nummer,
   kein neuer Beleg); das ist seltener und harmloser als eine zweite Rechnung und
   wird hingenommen.
5. **Uhr-Bremse: eine Stunde zurück, 400 Tage vor (ASVS V11.1.5).** Eine Stunde
   Toleranz deckt Zeitkorrekturen ab; 400 Tage folgen dem Review W14-026 und
   lösen auch aus, wenn eine Instanz über ein Jahr gesperrt war — dann ist eine
   Rückfrage ohnehin angebracht. Kleinere Uhrsprünge fängt die Nachhol-Grenze ab.
6. **Pause gilt auch für die Auto-AfA.** Ein Uhrsprung nach vorn buchte sonst
   Abschreibungen für künftige Jahre.
7. **§14c-Prüfung auch im Scheduler (R1-017).** Statt den Befund bewusst zu
   schließen: Der §19-Status kann sich nach dem Anlegen einer Vorlage ändern
   (Verzicht, Rückkehr); ohne Prüfung vor der Materialisierung entstünden
   nummerierte Entwürfe, die erst beim Festschreiben scheitern.
8. **Modus „Erinnerung“ erzeugt einen Hinweis (SL-REMINDER-MODE, D-01).** Im
   Dauerbetrieb öffnet niemand regelmäßig die Abo-Liste; das Handbuch versprach
   den Hinweis bereits. Er entsteht einmal je Abo und Stichtag
   (`recurring_due:<abo>:<stichtag>`) ohne Regel-Zeile; die Seed-Zeile
   `rule_recurring_due` (abschaltbar) bringt die gebündelte Migration 0032. Das
   Handbuch zur Zahlungsdatum-Logik ist korrigiert (D-02: die Position entsteht
   „noch nicht bezahlt“).
9. **Auto-AfA atomar, Fehler mit Hinweis und Backoff (D-03).** Eintrag,
   Restbuchwert und Audit sind je Anlage und Jahr eine Transaktion, der
   Restbuchwert wird darin frisch gelesen. Ein Fehlschlag erzeugt den Hinweis
   `afa_auto_failed:<jahr>` und wartet sechs Stunden (persistiert,
   `depreciation_auto_last_failure_at`; gemeinsamer Helfer `scheduler::backoff`).
10. **Keine Caches aus `/inputs` (SL-AFA-CACHES).** AfA-Tabellen, PDF- und
    Mail-Vorlagen werden bei jeder Verwendung gelesen; eine im Betrieb gepflegte
    Datei gilt sofort (Test `load_reads_current_file_without_cache`). Die
    PRD-Angabe, die Tabellen würden beim Programmstart geladen und gecacht, ist
    überholt (PRD-Revision 9). Nur die in das Programm eingebetteten Schriften
    sind gecacht.
11. **Standardkonten einmal je Instanz (D-14).** Der Marker verhindert, dass nach
    bewusstem Deaktivieren aller Konten neue Standardkonten erscheinen; Zählen
    und Anlegen in einer Transaktion verhindern Duplikate.
12. **Race Jahresabschluss ↔ Buchung.** Die Sperre `FISCAL_YEARS` und die
    Prüfung innerhalb der Schreib-Transaktion setzt Block F2 um (W14-034, zweiter
    Teil); F1 behandelt nur den Abo-Fall (Nr. 7).

13. **Grund `restore` ersetzt eine mitgebrachte Pause (W14-023).** Bringt das
    Backup eine Pause wegen der Uhr mit, gilt nach dem Restore trotzdem
    `restore`: Nur dieser Grund setzt auch die Off-Site-Spiegelung aus, und die
    Freigabe fragt nach der produktiven Instanz.
14. **Anheben im Restore automatisch, ohne Rückfrage (D-24).** Die Stände der
    ersetzten DB sind belegt (dieselbe Instanz, Änderungen seit der Annahme
    gesperrt). Eine Rückfrage böte nur die Möglichkeit, Nummern doppelt zu
    vergeben. Die Lücke ist zulässig, wenn sie dokumentiert ist (Audit, Hinweis);
    GoBD verlangt Nachvollziehbarkeit, nicht Lückenlosigkeit um jeden Preis.
15. **`numbering_raise` statt Zählerstand aus dem Pre-Restore-Backup (D-24,
    Disaster Recovery).** In eine leere Instanz gibt es kein älteres Backup
    derselben Instanz; die letzte vergebene Nummer kennt nur der Inhaber
    (Mail-Ausgang, Korrespondenz). Re-Auth, Pflicht-Begründung (≥ 10 Zeichen,
    im Audit), Obergrenze 999 999 gegen Tippfehler, nur aufwärts. Belege werden
    nicht angefasst.
16. **Übernommene Floor-Backups als `manual` (D-22).** Der ursprüngliche
    Auslöser steht nicht im Manifest. `manual` heißt „nie automatisch löschen“ —
    der sichere Fall, falls der Restore ein Irrtum war. Das Wachstum ist je
    Restore auf die Floor-Reihe begrenzt (höchstens rund 11 Dateien plus
    manuelle).
17. **Kein `backup_log`-Eintrag für die ausgesetzte Spiegelung (D-23).** Die
    CHECK-Liste kennt nur `ok`/`failed`; eine ausgesetzte Kopie ist kein
    Fehlschlag. Nachweis über das Audit `backup.mirror.skipped` und das Ergebnis
    je Ziel. Bleibt die Pause länger bestehen, meldet „Off-Site-Backup
    überfällig“ das nach sieben Tagen — gewollt, als Erinnerung an die
    Freigabe.
18. **Rotation ruht in jeder Pause.** Ein Uhrsprung nach vorn ließe sonst neue
    Backups in künftigen Eimern entstehen und ältere verdrängen; auf einer
    Probe-Instanz löschte die Rotation nach der Sicht des alten Stands.
19. **Tages-Backup mit eigenem Datumsvermerk (D-04).** Die Historie allein
    reicht nicht: Die Kalender-Rotation behält je Tag nur das jüngste Backup,
    ein späteres Lock-Backup verdrängte das `auto_daily` des Tages, und der Tick
    legte ein zweites an. Ohne Vermerk (Bestand) zählt einmalig das jüngste
    `auto_daily` der Historie. Uhrzeit-Einstellung als `app_settings`-Schlüssel
    mit eigenem Command (Feld in Einstellungen → Backups, Gestaltung im
    UI/UX-Lauf); `HH:MM` mit zwei Stellen.
20. **Lock-Backups des Ticks am Ende des Ticks (D-19).** Die Backup-Hardline
    verlangt ein Backup nach jedem Festschreiben, nicht eines je Beleg; ein
    Backup nach allen Abo-Läufen desselben Ticks erfüllt das. Bei einer
    Abbruch-Anforderung entsteht es trotzdem (das Tages-Backup dagegen nicht).
    Die Auto-AfA legt ihr Backup weiter selbst an (höchstens einmal im Jahr).
21. **Integritäts-Kopf mit der Zeit der injizierten Uhr.** Ohne das prüfte der
    Cron „schon gelaufen“ mit der Testuhr gegen einen Systemzeit-Stempel und
    liefe in Tests jeden Tick neu; im Betrieb sind beide gleich.
22. **Herunterfahren: Tick und Tasks gleichzeitig abwarten (W5-STOP-GRACE).**
    Die Abbruch-Anforderung steht seit dem Signal; nacheinander zu warten
    verlängerte das Herunterfahren ohne Nutzen. 30 s für Jobs genügen für
    Festschreiben und übliche Backups; ein größeres Backup bricht ab und wird
    vom nächsten Tages-Backup ersetzt. `stop_grace_period` 90 s lässt Reserve
    für das Abwarten der Verbindungen (30 s). Ein Test prüft, dass die Fristen
    in das Compose-Beispiel passen. Der Tick läuft nicht im `TaskTracker` der
    Server-Tasks: Er hält während jedes Laufs `AppCtx::scheduler_gate`, und
    `finish_shutdown` wartet über `tick::stop_within` genau auf dessen Ende —
    derselbe Mechanismus wie bei Restore und „Datenbank sperren“; die
    angehaltene Schleife hat danach nichts mehr zu tun und endet mit dem
    Prozess.
23. **Lücke nach langer Sperre: Schwelle zwei Tage (D-08).** Ein Wochenende
    ohne Anmeldung nach einem Update ist üblich und meldet nichts; ab dem
    dritten Tag fehlen spürbar Tages-Backups. Nach einem Restore ist der
    jüngste Audit-Eintrag der Restore selbst — kein irreführender Hinweis.
24. **`data/tmp` nach Alter, nicht nach Inhalt (D-13).** Laufende Snapshots
    und Exporte sind jünger als sechs Stunden; Up- und Downloads haben eigene
    Ablaufregeln und bleiben ausgenommen. Leere alte Unterordner (etwa
    `dsgvo/<id>`) fallen mit.

25. **Zeitstempel ohne Zone sind UTC (D-25).** Die DB schreibt seit W9 (ADR
    0039 Nr. 94) unabhängig von `TZ` UTC; ein Zeitstempel ohne Zone ist daher
    nie Ortszeit. Ein Zeitstempel mit Zone (RFC 3339 aus `chrono`) wird wie
    angegeben gelesen. Bruchteile über Millisekunden werden abgeschnitten
    (Safari liest nur drei Stellen).
26. **`dateTime` mit Sekunden, Rohwert bei unbekanntem Format.** Protokolle
    (Audit, Mail, Backup) brauchen die Sekunde; das Format entspricht dem
    bisherigen `toLocaleString("de-DE")`. Ein unbekanntes Format zeigt den
    Rohwert statt eines Strichs, damit im Protokoll nichts verschwindet;
    `date` zeigt bei ungültiger Eingabe „—“. Die Gestaltung (z. B. HH:MM nach
    DIN 5008 in Tabellen) bleibt dem UI/UX-Lauf.
27. **Backup-Alter nach Kalendertagen.** Die Übersicht zählte 24-Stunden-Fenster
    in Gerätezeit: ein Backup von gestern 23 Uhr galt am Morgen als „Heute“.
    Jetzt zählt der Berliner Kalendertag („Gestern“); die Schwellen der Ampel
    (7/14 Tage) bleiben.
28. **Ein offener Tab rechnet „heute“ nicht neu.** Die Übersicht berechnet
    Datum und Geschäftsjahr beim Laden (D-27); das Neuberechnen über
    Mitternacht gehört zur Oberfläche und liegt beim UI/UX-Lauf.
29. **Bestand ohne `offered` (D-10).** Liegt die angebotene Fassung schon als
    `.neu`, gilt sie als angeboten (kein neuer Hinweis). Eine vor dem Update
    gelöschte `.neu` lässt sich davon nicht unterscheiden, dass nie angeboten
    wurde; sie kommt einmal wieder. Eine Übernahme ins Manifest „auf Verdacht“
    hätte dagegen eine nie gezeigte Fassung unterschlagen.
30. **Umfang „toter Desktop-Code“ in F1.** F1 entfernt den Benachrichtigungs-
    Kanal (D-16), die Watcher-Datei `.taurignore` der Desktop-Shell und
    bereinigt Kommentare, `.gitignore` und Meldungen. Die gesperrten
    Desktop-Commands (`Route::Dropped`), `DesktopActions`, `mail::oauth_ms`,
    die Lizenzseite `handbook/third-party-licenses.html` und die Icons entfernt
    Block F6 (N2-DESKTOP-ALTCODE, W13-OAUTH-CODE, W13-ALTBESTAND-LICENSES).
31. **Historie bleibt Historie (D-18).** Migrationen (Prüfsummen), die
    Block-Notizen unter `memory/`, CHANGELOG-Abschnitte vergangener Versionen
    und ADR-Texte vor ihren Amendments werden nicht umgeschrieben; das PRD
    behält seine Originalstellen, die Revision 9 tabellarisch ablöst.

Teil F1d — Prüferbefunde zu F1 (24.10.2026):

32. **Abo-Termine nach einem Restore nur vorwärts (S1).** Wie die Nummernkreise
    (Nr. 14) übernimmt das erste Entsperren die nächsten Termine der ersetzten
    DB automatisch, wo sie später liegen. Die Stände sind belegt (dieselbe
    Instanz, Änderungen seit der Annahme gesperrt); eine Rückfrage böte nur die
    Möglichkeit, einen Zeitraum doppelt abzurechnen. Der Nachteil — ein Termin,
    der in der ersetzten DB nur als begonnener Entwurf existierte, gilt als
    abgerechnet — ist selten und steht mit den ausgelassenen Terminen im Audit
    und im Hinweis („bitte von Hand erstellen“). Vorrücken über die
    Compare-and-Swap-Funktionen der Repos, ohne `last_*`-Felder zu ändern.
33. **Reihen ohne bekannten Stand: Entscheidung statt Automatik (S1, Disaster
    Recovery).** Fehlt eine Reihe im Restore-Marker (leere Instanz, Reihe in der
    ersetzten DB nicht vorhanden) und ist ein Termin bis zum Restore-Tag offen,
    steht der Marker `recurring_restore_review:<art>:<id>` = Restore-Tag. Bis
    zur Entscheidung entsteht für die Reihe nichts automatisch, auch unter der
    Nachhol-Grenze. „Nachholen“ erzeugt Rechnungen bis zum Restore-Tag nur als
    Entwurf (der Inhaber prüft gegen den Mail-Ausgang); „ab heute fortsetzen“
    beginnt beim ersten Termin **nach** heute, weil ein heute fälliger Termin
    vor dem Ausfall schon abgerechnet sein kann. „Jetzt erstellen“ erzeugt in
    diesem Zustand ebenfalls nur einen Entwurf; „Buchen“ bei Kosten bleibt eine
    ausdrückliche Einzelaktion (Nr. 3). Der Marker fällt weg, sobald der
    nächste Termin nach dem Restore-Tag liegt. Die Listen zeigen „Rückstand …“
    dann auch bei einem heute fälligen Termin (`recurring_restore_review_list`).
34. **Marker aus dem Backup verwerfen (S1).** `recurring_invoice_pending:*`
    würde nach der Freigabe einen Entwurf des alten Stands festschreiben und
    versenden, `recurring_catch_up_ok:*` bis zu 24 Termine ohne Rückfrage
    nachholen. Beide gehören zum Zustand der ersetzten Instanz; das erste
    Entsperren löscht sie in der Restore-Transaktion (Audit
    `recurring.markers_cleared_after_restore` mit den Nummern der betroffenen
    Entwürfe), der Hinweis nennt die Entwürfe.
35. **Wiederhergestellte Entwürfe mit vergebener Nummer: vormerken und
    bestätigen lassen, nicht neu nummerieren (S2).** Ein Entwurf trägt seine
    Nummer ab dem Anlegen. Wurde dieselbe Nummer in der ersetzten DB
    festgeschrieben (und versandt), ist der Entwurf ein älterer Stand genau
    dieses Belegs. Erwogen: (a) Neunummerieren — schüfe für denselben Vorgang
    eine zweite Nummer und ließe die beim Kunden liegende Nummer ohne Beleg in
    den Büchern; (b) Festschreiben sperren, bis ein „Verwerfen“ existiert
    (F2) — ließe den versandten Beleg gar nicht nachtragen; (c) gewählt:
    vormerken (`restored_number_conflict:<art>:<id>`, Audit
    `numbering.restored_draft_flagged`), dringender Hinweis mit den Nummern,
    Festschreiben (Lock-Pipeline für Rechnungen und Stornos, `quotes_issue`) erst
    nach der Bestätigung „entspricht dem versandten Original“ mit Re-Auth und
    Begründung (Audit `numbering.restored_draft_confirmed`). So steht der
    versandte Beleg unter seiner Nummer genau einmal in den Büchern
    (Vollständigkeit, GoBD), und zwei verschiedene Belege mit derselben Nummer
    (§14 Abs. 4 Nr. 4 UStG) entstehen nicht aus Versehen. Geprüft werden
    Rechnungen, Stornos und Angebote; Kosten und Privatbewegungen entstehen
    festgeschrieben, Anlagen-Nummern verlassen die Instanz nicht. Die
    Fortsetzungs-Marker sind verworfen (Nr. 34), also setzt kein Lauf einen
    solchen Entwurf automatisch fort.
36. **Bestätigung „nachholen“ nur bis zu ihrem Tag (S3).** Der bestätigte Lauf
    zählt nur die Termine bis zum Tag der Bestätigung; spätere Termine folgen
    der Nachhol-Grenze (bis drei werden mitgenommen, mehr verlangen nach dem
    bestätigten Abschnitt eine neue Entscheidung). Übersprungen werden
    Termine in abgeschlossenen Jahren nur innerhalb des bestätigten Zeitraums.
37. **Tages-Backup über den zuletzt fälligen Slot (S2).** Der Tick läuft alle
    fünf Minuten im Takt des Prozessstarts; eine Uhrzeit wie 23:58 fiel sonst
    nie in ein Tick-Fenster, und eine Anmeldung vor der Uhrzeit holte nach
    Tagen der Sperre nichts nach. Ohne jeden Vermerk ist das Backup sofort
    fällig (sicherer Fall). Am Tag der Zeitumstellung gilt weiter die Wanduhr.
38. **Integritäts-Cron nach Monatsintervall (S2).** Die Prüfung „schon
    gelaufen“ sucht einen abgeschlossenen Lauf mit `started_at` zwischen den
    Berliner Monatsanfängen (in UTC) statt den jüngsten Lauf auf Gleichheit zu
    vergleichen; ein Lauf aus einer vorgestellten Uhr zählt nicht für den
    aktuellen Monat, blockiert ihn aber auch nicht.
39. **Zukünftig datierte Backups in der Rotation (S3).** Behalten, aber nicht
    gegen die Eimer-Grenzen gerechnet (Toleranz eine Stunde wie die Uhr-Bremse);
    eine Umklassifizierung als `manual` hätte sie dauerhaft vor der Rotation
    geschützt.
40. **Ausstehendes Lock-Backup als Vermerk in der DB (S3).** Seit F1 sperrt
    „Datenbank sperren“ nach höchstens zehn Sekunden, auch wenn der Tick noch
    läuft; ein am Ende des Ticks gesammeltes Backup konnte so ausfallen. Der
    Vermerk `backup_pending_since` entsteht beim Sammeln (der Pool ist dann
    noch offen) bzw. wenn ein Lock-Backup mangels Passphrase ausfällt; jedes
    erfolgreiche Backup, das danach begann, löscht ihn. Der erste Tick nach
    Entsperren oder Serverstart legt bei gesetztem Vermerk sofort ein Backup
    an, unabhängig vom Tages-Vermerk; nach einem Fehlschlag wartet er eine
    Stunde (`backup_pending_last_failure_at`), damit ein volles Ziel das
    append-only `backup_log` nicht alle fünf Minuten füllt.
41. **Pause des Schedulers als Flag (S3).** Der Mutex-Wert `scheduler_gate`
    diente zugleich als Pause; `resume` musste den Mutex nehmen und wartete
    damit auf einen beim Sperren noch laufenden Tick (bis zu Minuten bei
    Sidecar- oder SMTP-Zeitgrenzen). Jetzt ist die Pause
    `AppCtx::scheduler_paused`; der Mutex dient nur dem Abwarten (`stop`,
    `stop_within`). Ein alter Tick nach dem Entsperren scheitert am
    geschlossenen Pool und endet.
42. **„Jetzt erstellen“ schließt einen begonnenen Termin zuerst ab (S3).** Unter
    der Vorlagen-Sperre wird ein vorhandener Fortsetzungs-Marker derselben
    Vorlage fortgesetzt (festschreiben, versenden), danach entsteht der neue
    Termin. Zusätzlich lehnt das Anlegen eines Termins ab, solange ein Marker
    der Vorlage offen ist — kein Marker wird je überschrieben. Ein Marker je
    Vorlage bleibt ausreichend (Entscheidung 4).
43. **Race Anonymisierung ↔ Abo-Entwurf: Prüfung in der Transaktion (S3).**
    Innerhalb der `BEGIN IMMEDIATE`-Transaktion, vor der Nummernvergabe, wird
    geprüft, dass die Vorlage aktiv und ihr Kunde weder archiviert noch
    anonymisiert ist (die Anonymisierung pausiert die Vorlage in ihrer eigenen
    Transaktion). Sonst entsteht kein Beleg und keine Nummer; der Lauf hört für
    die Vorlage auf.
44. **Lücken-Hinweis trotz bestehender Restore-Pause (S3).** Unterdrückt wird
    der Hinweis „Instanz war N Tage gesperrt“ nur in dem Entsperren, das den
    Restore anwendet (Pause seit weniger als 15 Minuten). Bei jedem späteren
    Entsperren zählt der Beginn der Pause als weitere Aktivität.
45. **Nummernkreis-Hinweis ohne Anhebung (S3).** Drei Fälle: Stände der
    ersetzten DB höher → Liste der angehobenen Kreise (dringend); Stände
    vorhanden, keiner höher → kurzer Hinweis „keine Lücke, keine
    Doppelvergabe“ (Info); keine Stände (leere Instanz) → Anleitung zum
    Anheben (dringend). Vorgemerkte Entwürfe (Nr. 35) machen jeden Fall
    dringend.

## Referenzen

`clock`, `app_ctx`, `scheduler::{mod, tick, hold, catch_up, backoff, recurring,
recurring_invoice, reminders, depreciation_year_close, integrity_check_cron}`,
`commands::{recurring, recurring_invoice, system, invoices, backup,
notifications}`, `db::{mod (run_migrations, finish_restore), numbering}`,
`db::repo::{recurring, recurring_invoice, payment_accounts, contacts}`,
`backup::{mod (create_with, auto_daily_if_due, pending_backup_if_due,
rescan_floor), rotation, restore, after_restore}`, `archive::integrity_check`, `server::{auth::{lock_database, rekey},
state::finish_shutdown, startup::{sweep_tmp, sweep_partial_backups}, mod}`;
Tests `tests/recurring_scheduler_test.rs`, `tests/backup_schedule_test.rs`,
`tests/server_restore_test.rs` (`older_backup_restore_keeps_numbers_pauses_and_mirrors_only_after_release`,
`numbering_raise_only_upwards_with_reason`,
`older_backup_restore_bills_no_period_twice_and_guards_used_draft_numbers`,
`restore_without_series_state_requires_decision_and_never_auto_issues`),
`tests/server_auth_test.rs` (`unlock_does_not_wait_for_tick_still_running_from_lock`), `tests/server_auth_test.rs`
(`shutdown_does_not_wait_forever_for_running_tick`),
`scheduler::recurring_invoice::pipeline_tests`, `backup::rotation::tests`,
`src/routes/no-side-effects-on-open.test.ts`, `src/lib/date.test.ts`,
`src/lib/format.test.ts`, `seed::tests` (`deleted_neu_file_is_not_offered_again`),
`tests/inputs_seed_test.rs` (`neu_file_is_offered_and_notified_once`),
`db::tests::newer_database_is_rejected_on_unlock`. Frontend `src/lib/date.ts`,
`src/lib/format.ts`; `seed`, `notify`. `compose.example.yaml`
(`stop_grace_period`). ADR 0009, 0023, 0027, 0033, 0034, 0039;
UI/UX-Plan §9; Review W14 (`docs/REVIEW-PHASE-W.md`).
