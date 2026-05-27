# ADR 0037 — E-Rechnungs-Empfangs-Politur + Drop-Folder

**Status:** Akzeptiert · 2026-05-27 · Post-v1.0 (Blöcke PV1-A2 + PV1-A5 + PV1-DROP, Doku-Abschluss PV1-DOC).

**Code-Referenzen:**

- `block-pv1-a2: zugferd profile whitelist on receive` (`93fa228`)
- `block-pv1-a5: raw xml viewer for received einvoices` (`52753e2`) +
  `block-pv1-a5-tests: integration test for xml viewer` (`58f1fc4`)
- `block-pv1-drop: watched drop folder for incoming documents` (`5b995fa`) +
  `block-pv1-drop-tests: integration tests for drop folder sync` (`6ad8297`)
- `block-pv1-rename: ui language plain-german (drop-folder → rechnungs-eingang)`
  (Migration `0030_rename_drop_folder_labels.sql`, Schema v29 → v30; UI-Strings
  + Notification-Rule-Labels auf „Rechnungs-Eingang", Code-Identifier
  `drop_folder_*` bleiben englisch nach CLAUDE.md-Hard-Rule).

**Sprachregel (Stand 2026-05-27):** das in diesem ADR diskutierte Feature
wird im UI als **„Rechnungs-Eingang"** bezeichnet (Settings-Card, PageBar-
Title, Toggle, Notification-Labels). Code-Identifier — Routen-Pfad
`/settings/drop-folder/`, Module `domain::drop_folder`/`scheduler::drop_folder`,
Settings-Keys `drop_folder_enabled`/`drop_folder_path`, Rule-IDs
`rule_drop_folder_import_ok`/`_failed`, Migration-Filenames, dieses ADR —
bleiben englisch. Tech-Doku verwendet im Engineer-Text frei den Code-Term
`drop_folder`; Endnutzer-sichtbarer Text spricht von „Rechnungs-Eingang".

## Kontext

Nach dem E-Rechnungs-Konformitäts-Audit am 2026-05-27 (Manuel + Claude) sind drei
kleine Lücken bzw. Komfort-Posten am Empfangs-Pfad übrig geblieben, die alle
**kein** Compliance-Stopper für v1.0 sind, das Werkzeug aber spürbar
abrunden. Sie hängen fachlich zusammen (E-Rechnung-Empfang) und teilen die
Pipeline aus ADR 0024:

1. **ZUGFeRD-Profile-Filter beim Empfang.** Der Parser akzeptierte bisher
   jedes `GuidelineSpecifiedDocumentContextParameter`, inklusive
   `MINIMUM` und `BASIC-WL`. Beide Profile sind seit der E-Rechnungs-Pflicht
   ab 2025 **keine gültigen E-Rechnungen** mehr (sie tragen nicht die
   vollständigen § 14-UStG-Pflichtangaben). Eingehend werden sie zwar oft als
   buchhalterische Beilage zur Papier- oder PDF-Rechnung mitgeschickt, sollten
   in Klein.Buch aber als „kein gültiger E-Beleg" markiert werden statt
   indirekt über einen unspezifischen KoSIT-Fail aufzuschlagen.
2. **Roh-XML im Eingangsbeleg-Detail anzeigen.** Steuerberater und
   Betriebsprüfer möchten gelegentlich in das maschinenlesbare XML einer
   importierten E-Rechnung schauen (BT-Felder verifizieren, §13b-Markierung
   prüfen, Buyer-Snapshot vergleichen). Bisher gab es dafür kein UI;
   die XML lag nur als archivierte Original-Datei vor, lesbar nur über den
   Dateipfad. Außerdem sollte das Anzeigen kein zusätzlicher
   `archive.read`-Audit-Eintrag bei jedem Klick erzeugen — der Auditing-Sinn
   ist Beleg-Akt, nicht Anzeige.
3. **Watched Drop-Folder für eingehende E-Rechnungen.** Manuel bekommt E-
   Rechnungen per E-Mail. Heute heißt das: Datei manuell aus dem Postfach
   auf die Festplatte ziehen, in Klein.Buch über „Kosten → E-Rechnung
   importieren" hochladen, einen Datei-Dialog durchklicken. Das skaliert
   schlecht, sobald es mehr als „hier und da mal eine Rechnung" sind. Eine
   automatische Importpipeline aus einem überwachten Ordner würde den
   Workflow „aus Mail in Klein.Buch" auf „Datei in Ordner kopieren" verkürzen.

Alle drei Posten teilen sich denselben Empfangs-Code: den `einvoice::parser`,
das write-once-Archiv und die `expenses_create_from_einvoice`-Pipeline. Sie
zusammen in einem ADR zu führen vermeidet drei parallele Dokumente für ein
Thema, das in Wahrheit eine konsolidierte Politur-Iteration ist (Block-17c-
Pattern: ein zusammenfassender ADR mit konkreten Commit-Hashes statt drei
mikroskopischer ADRs).

Querverweise: ADR 0024 (E-Rechnung-Empfang Foundation, CII+UBL, KoSIT
beratend), ADR 0023 (Scheduler-Modell, 5-Min-Tick), ADR 0006 (GoBD-Archive),
ADR 0027 (Notification-Regeln, OS-native-Disziplin).

## Entscheidungen

### D-71 — Polling statt Filesystem-Watcher für den Drop-Folder

Der Drop-Folder wird per **Scheduler-Tick (5-Minuten-Polling) plus App-Start-
Sweep** auf neue Dateien geprüft. **Kein** `notify`-Crate, kein Live-Watcher.

Gründe: OneDrive (das primäre Sync-Ziel auf Manuels Rechner) reicht Datei-
Events verzögert oder gar nicht durch, weil Sync-Clients Dateien zunächst nur
als Platzhalter materialisieren. Ein Live-Watcher würde dadurch entweder
unzuverlässig laufen oder unter Race-Conditions zwischen Sync-Progress und
Import-Pipeline leiden. Polling mit `read_dir` umgeht das vollständig, weil
es jedes Mal den real auf der Platte sichtbaren Zustand fragt. Zusätzlich
spart Polling eine Dependency und ein cross-platform-Verhalten-Risiko ein.

Die 5-Minuten-Latenz ist für „hier und da mal eine Rechnung" mehr als genug;
für Eilfälle gibt es einen `drop_folder_sync_now`-Button („Jetzt
synchronisieren") auf der Settings-Seite.

### D-72 — Roh-XML-Viewer liest still (`read_and_verify_silent`)

Der Roh-XML-Viewer ruft `archive::store::read_and_verify_silent` statt
`read_and_verify`. **Kein** `archive.read`-Audit-Eintrag pro Klick — Anzeige
ist Lesepfad, kein Beleg-Akt.

Gründe: Manuel kann beim Recherchieren eines Falls innerhalb von Minuten
zwanzig Mal in das XML schauen. Jeder dieser Klicks würde sonst einen
Audit-Eintrag erzeugen und das `audit_log` mit echtem Signal verstopfen.
GoBD verlangt die Audit-Spur für mutierende Akte (Lock, Storno, Restore),
nicht für read-only-Anzeigen. Die Tamper-Detection (Hash-Mismatch) wird
weiterhin innerhalb von `archive::store` als `archive.tamper`-Audit
geschrieben, das geht durch beide Lese-Pfade gleich.

### D-73 — Profile-Whitelist für PV1-A2

Hardcoded Whitelist mit **Substring-Match** (case-insensitive) im
`GuidelineSpecifiedDocumentContextParameter.ID`:

- akzeptiert: `en16931`, `extended`, `xrechnung`
- abgelehnt: `minimum`, `basic-wl`

Bei Match auf einen abgelehnten Wert wirft der Parser
`ParseError::UnsupportedProfile(<URN>)` mit einer klaren Fehlermeldung;
die KoSIT-Bewertung bleibt für alle anderen Profile beratend (kein
Doppel-Reject).

Gründe: ein harter Reject mit benanntem Profil ist schärfer als ein
indirekter KoSIT-Fail über fehlende Pflichtangaben. Manuel sieht im UI sofort
„Profil MINIMUM ist seit 2025 keine gültige E-Rechnung", anstatt sich durch
eine generische Validator-Meldung zu kämpfen. Substring-Match (nicht
Equal-Match), weil die echten Identifier in der Praxis lang sind und
Versions-Suffixe tragen (`urn:cen.eu:en16931:2017#conformant#urn:…`).

### D-74 — Headless Pipeline-Reuse für den Drop-Folder

Die automatische Import-Pipeline wird **nicht** als zweiter Tauri-Command-Pfad
gebaut, sondern als headless Library-Funktion `import_einvoice_from_file` im
Modul `scheduler::drop_folder`. UI-Trigger („E-Rechnung importieren"-Button)
und Scheduler-Pfad teilen die Helfer aus `commands::expenses`
(`parse_einvoice_with_paths`, `create_from_einvoice_with`).

Gründe: ein zweiter Pipeline-Pfad würde unweigerlich auseinanderdriften
(GoBD-Lock-Reihenfolge, Audit-Events, Archiv-Pfade). Tauri-Commands brauchen
`State`/`AppHandle`, headless-Code nicht — gemeinsame Helfer werden so
extrahiert, dass beide Aufrufer denselben Code laufen lassen und nur die
äußere Bridge (State-Beschaffung vs. Polling) sich unterscheidet.

### D-75 — Failure-Routing `processed/YYYY-MM/` und `failed/`

Erfolgreich importierte Dateien wandern in den Sub-Ordner
`processed/{YYYY-MM}/{original-name}` (Monats-Sub-Ordner nach Sync-Datum,
nicht Beleg-Datum). Fehlerhafte Dateien wandern in `failed/{original-name}`
(plain, keine Monatsstruktur — die Datei soll auffallen, nicht im Archiv
versacken). **Kein Auto-Delete** — siehe D-79.

Gründe: ein Top-Level-Drop-Folder, der sich beim ersten Import füllt, ist
nach einer Woche unübersichtlich. Monats-Sub-Ordner für erfolgreiche Importe
erlauben einen sauberen Verlauf, ohne die Datei-Klassifikation kompliziert
zu machen. `failed/` bleibt flach, damit Manuel im Datei-Manager sofort
sieht, was wartet.

### D-76 — Trigger für den Drop-Folder

Der Drop-Folder-Sync läuft an **drei Stellen**: (1) im 5-Minuten-Scheduler-
Tick (`scheduler::tick`, Job 6), (2) beim ersten Tick nach dem App-Start
(Bootstrap-Sweep, weil neu eingegangene Dateien sonst bis zu fünf Minuten
ungesehen blieben), (3) manuell via Settings-Button `drop_folder_sync_now`.

Gründe: Memory-Hinweis zu OneDrive-Quirks (siehe D-71). Ein App-Start-Sweep
ist nötig, weil Manuel die App typischerweise frisch öffnet, wenn er prüfen
will, was reingekommen ist; ohne Sweep müsste er fünf Minuten warten. Die
manuelle Variante deckt Eil-Fälle und Smoke-Tests ab. Latenz unkritisch bei
„hier und da mal eine Rechnung".

### D-77 — Library-Funktion statt zweitem Command-Pfad

Pipeline-Reuse aus D-74 wird konkret als Library-Funktion in
`scheduler/drop_folder.rs` realisiert (`import_einvoice_from_file`), nicht als
zweiter Tauri-Command. Tauri-Commands brauchen `State`/`AppHandle`,
headless-Code nicht — gemeinsame Helfer (`parse_einvoice_with_paths`,
`create_from_einvoice_with`) werden so extrahiert, dass beide Aufrufer
denselben Pfad laufen.

Gründe: UI- und Scheduler-Pfad sollen byte-identisch durchs Archiv und in den
Lock. Ein zweiter Command-Pfad würde unweigerlich subtil abweichen (Audit-
Reihenfolge, Backup-Trigger). Die Library-Funktion ist außerdem direkt
testbar (`tests/drop_folder_sync_test.rs`), ohne Tauri-Setup.

### D-78 — Notification-Regeln für den Drop-Folder

Zwei neue Notification-Regeln (Migration `0029_drop_folder.sql`):

- `rule_drop_folder_import_ok` — Default **off**, Inbox-only via
  `notify::store::create`. Erfolg ist Routine, kein Pop-up-Spam.
- `rule_drop_folder_import_failed` — Default **on**, Inbox **plus** OS-Toast.
  Fehler braucht Sichtbarkeit.

Konsistent mit ADR 0027 (`rule_backup_result`-Disziplin: Fehler immer
laut, Erfolg nur wenn explizit).

**Disziplin (R4-007, ADR 0027 Pt. 5):** Der Scheduler-Pfad ist strikt
**Inbox-only via `notify::store::create`** — kein `notify::emit`, kein
`os_native::push`. Sobald ein Integrationstest-Binary die OS-Push-Kette
verlinkt, scheitert es beim Laden (`STATUS_ENTRYPOINT_NOT_FOUND`,
`TaskDialogIndirect` in `comctl32`). Wer später einen OS-Toast für Fehler
will, baut eine separate Reminder-Regel (Polling im UI-Layer), nicht im
Scheduler.

### D-79 — `failed/` behält das Original ohne Auto-Delete

Bei einem Import-Fehler wird die Datei nach `failed/` verschoben, aber
**nicht** gelöscht. Klein.Buch löscht **nie** automatisch im Drop-Folder.

Gründe: Manuel muss bei einem Fehler die Original-Datei prüfen können
(„XML kaputt? Lieferant fragen? Falsches Profil?"). Auto-Delete würde diese
Diagnose-Ebene zerstören. GoBD bleibt sauber: ein nicht-importierter Beleg
ist auch kein Archiv-Eintrag, dort gibt es nichts zu schützen.

### D-80 — XML-Viewer liest `source_format` statt MIME-Sniffing

Der Roh-XML-Viewer entscheidet anhand des Felds `expenses.source_format`
(`xrechnung-cii` / `xrechnung-ubl` / `zugferd`), wie er die archivierte Datei
liest: bei `zugferd` ruft er `mustang_bridge::extract_xml` (XML aus PDF/A-3-
Embedded-File), bei `xrechnung-*` liest er die XML-Datei direkt als UTF-8.

Gründe: `source_format` ist bei Import bereits gesetzt — kein MIME-Sniffing,
kein Magic-Bytes-Check, keine Heuristik. Eine spätere Format-Änderung
(z. B. PDF mit anderer Magic-Sequenz) wirkt sich nicht auf die Anzeige-Logik
aus.

### D-81 — Doku-Block-Strategie

PV1-DOC läuft als **Abschluss-Block** (Block-17c-Pattern), nicht als
separater ADR pro Code-Block. Ein einziger zusammenfassender ADR 0037
enthält die fünf Entscheidungen D-71 bis D-75 plus die ergänzenden
Operationalisierungs-Entscheidungen D-76 bis D-80 mit konkreten Commit-Hashes
der drei Code-Blöcke.

Gründe: drei eng zusammenhängende Entscheidungen (Empfangs-Politur)
gehören in einen ADR. Drei separate ADRs würden die zusammenhängende
Begründung künstlich auseinanderreißen.

## Konsequenzen

- **`einvoice::parser`** wirft `ParseError::UnsupportedProfile` bei
  MIN/BASIC-WL. Die UI im Kosten-Import zeigt die Fehlermeldung mit
  Profil-Bezeichnung; KoSIT-Validierung läuft danach gar nicht erst an.
  Vorhandene MIN-Anhänge aus alten Mails muss Manuel weiterhin als
  „buchhalterische Beilage zur Papier-Rechnung" behandeln und entweder den
  Absender um einen gültigen E-Beleg bitten oder die Position manuell
  erfassen.
- **`commands::expenses::expenses_receipt_xml_text`** liefert die Original-
  XML als String für den Anzeige-Dialog. Tamper-Detection und Hash-Verify
  laufen weiter im Archiv-Modul; nur der `archive.read`-Audit-Eintrag entfällt
  für diesen Pfad. Bei ZUGFeRD wird das XML über die Mustang-Bridge aus dem
  PDF/A-3 extrahiert.
- **`scheduler::drop_folder`** läuft als zusätzlicher Tick-Job. Bei
  deaktiviertem Toggle (`drop_folder_enabled = '0'`) ist der Job ein no-op
  und protokolliert nichts. Aktiviert + Pfad gesetzt: jede Top-Level-Datei
  wird klassifiziert (XML/PDF/IgnoreHidden/IgnoreOther) und entsprechend
  weiterbehandelt. Unterordner werden bewusst nicht rekursiv durchsucht —
  Wer Unterordner-Struktur will, baut sie sich später selbst.
- **Migration `0029_drop_folder.sql`** legt die zwei Settings-Keys
  (`drop_folder_enabled`, `drop_folder_path`) und die zwei Notification-Regeln
  an. Schema **v28 → v29**, `EXPECTED_SCHEMA_VERSION = 29`.
- **PV1-A2 und PV1-A5** sind reine Code-Erweiterungen ohne Migration —
  Schema-Stand vor diesen Blöcken war v28 (R1-Review `0028_append_only_
  hardening`), beide ändern nur Pure-FC- bzw. Command-/Frontend-Code.
- **Tests**: `parser.rs::tests` (10 zusätzliche Profile-Whitelist-Cases),
  `tests/receive_einvoice_xml_view_test.rs` (6 Cases für den Viewer-Pfad),
  `tests/drop_folder_sync_test.rs` (XML- und PDF-Import end-to-end, Hidden-
  Files, Failure-Routing, Disabled-Skip, Windows-Pfad-Sicherheit).
- **GoBD bleibt intakt.** Importierte Belege durchlaufen denselben Lock + die
  gleiche Archivierung wie der UI-Pfad. `failed/`-Dateien sind keine Belege
  und entstehen daher gar nicht erst im Archiv.
- **Local-First bleibt intakt.** Keine neue Netzwerk-Schnittstelle, kein
  Cloud-Sync — der Drop-Folder ist ein lokaler Pfad, den der Cloud-Client
  des Nutzers (OneDrive, iCloud, Nextcloud, …) selbständig spiegelt.

## Alternativen

| Option | Contra |
|---|---|
| `notify`-Crate für Live-Watching | OneDrive-Quirks (verzögerte Events, Sync-Phantome), zusätzliche Dependency, cross-platform-Risiko |
| KoSIT-Validierung für MIN/BASIC-WL als „Profil-Reject" missbrauchen | indirekt, unklare Fehlermeldung, kein klares „Profil ungültig"-Signal |
| `archive.read`-Audit bei jedem XML-Viewer-Klick | Audit-Spam, verstopft echtes Signal |
| Separater Command-Pfad für den Drop-Folder | unweigerliche Drift zur UI-Pipeline (Audit-Reihenfolge, Lock-Schritte) |
| Auto-Delete im Drop-Folder bei Erfolg | Manuel verliert Reproduzierbarkeit; bei Fehler wäre Diagnose erschwert |
| Mail-Inbox-Integration (IMAP / Graph `/messages`) statt Drop-Folder | OAuth-Komplexität, Mail-Filter-Race-Conditions, Provider-Quirks; Drop-Folder löst das Problem mit minimaler Komplexität |
| Rekursive Drop-Folder-Suche | erhöht False-Positives (z. B. archivierte Rechnungen in Unter-Ordnern werden re-importiert), unklare Verantwortung pro Ordner |

## Referenzen

`einvoice::parser::{check_profile_whitelist, ParseError::UnsupportedProfile}`
(PV1-A2), `commands::expenses::{expenses_receipt_xml_text,
parse_einvoice_with_paths, create_from_einvoice_with}` (PV1-A5 + Reuse-Helfer),
`scheduler::drop_folder::{run_sync, import_einvoice_from_file,
DropSyncReport}` (PV1-DROP), `domain::drop_folder::{classify_file,
processed_subdir, DropClassification}`, `archive::store::read_and_verify_silent`,
Migration `0029_drop_folder.sql`, `notify::rules::{rule_drop_folder_import_ok,
rule_drop_folder_import_failed}`, Frontend `routes/settings/drop-folder/+page.svelte`,
Frontend `routes/expenses/[id]/+page.svelte` (Roh-XML-Modal),
`src/lib/{XmlViewerDialog,xmlViewerModal}`.

Verwandte ADRs: 0024 (E-Rechnung-Empfang), 0023 (Scheduler-Modell), 0006
(GoBD-Archiv), 0027 (Notifications + GJ-Lock + Integrity-Cron).
PRD: `PRD-klein-buch.md` Revision 7.
