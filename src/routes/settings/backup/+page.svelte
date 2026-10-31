<script lang="ts">
  // Block 4 — Backup & Restore.
  // U4: Seitenraster ab 60rem (U2-021), Ergebnis je externem Ziel mit „ausgesetzt“
  // (Warteliste (12)), Stand der externen Kopien (Lauf F3), Belegnummern über DocNo.
  // U4 (V1-14/V1-15, Korrekturrunde): Rückfrage vor dem Wiederherstellen nennt den ganzen Ablauf;
  // Knöpfe bleiben bedienbar und melden fehlende Eingaben am Feld (ADR 0041 Nr. 135); „Jetzt
  // sichern“ mit Status während des Laufs und EINEM zusammenfassenden Toast; Ladefehler mit
  // „Erneut laden“ (V2-07); „Abbrechen“ vor der Hauptaktion (V2-11).
  import { onMount, tick } from "svelte";
  import { waitForRestart } from "#lib/transport.ts";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import { dateTime, dateTimeShort } from "#lib/format.ts";
  import { daysUntilBerlin } from "#lib/date.ts";
  import { dayClock } from "#lib/dayClock.svelte.ts";
  import Button from "#lib/Button.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import {
    backupGetSettings,
    backupCreateNow,
    backupTestSftp,
    backupTestSftpSaved,
    backupTargetAddDirectory,
    backupTargetAddSftp,
    backupTargetRemove,
    backupList,
    backupDownload,
    saveDownload,
    backupRestoreApply,
    backupSetDailyTime,
    numberingCountersList,
    numberingRaise,
    numberingRestoredDraftsList,
    numberingRestoredDraftConfirm,
    uploadFile,
    type BackupSettings,
    type BackupOutcome,
    type OffsiteResult,
    type NumberingCounter,
    type RestoredDraftConflict,
    type BackupHistoryItem,
    type TargetView,
    type SftpProbe,
  } from "#lib/api.ts";

  let settings = $state<BackupSettings | null>(null);
  let history = $state<BackupHistoryItem[]>([]);
  let busy = $state(false);
  /** Ergebnis von „Jetzt sichern“ je Ziel (U4, Warteliste (12)); bleibt bis zum Verlassen. */
  let lastOutcome = $state<BackupOutcome | null>(null);
  /** U4 (V1-15): „Jetzt sichern“ läuft; Statuszeile (role=status) sagt den Lauf an. */
  let backingUp = $state(false);
  /** Wiederherstellen läuft (Upload und Anfrage). */
  let restoring = $state(false);
  let backupStatus = $state("");
  /** U4 (V2-07): Fehler beim ersten Laden (danach meldet ein Toast). */
  let loadError = $state<string | null>(null);
  // U4 (V1-14, ADR 0041 Nr. 135): Knöpfe bleiben bedienbar, fehlende Eingaben stehen am Feld.
  let restorePassError = $state("");
  let dailyTimeError = $state("");
  let sftpUserError = $state("");
  let sftpPassError = $state("");
  let sftpAddError = $state("");
  let raiseReasonError = $state("");
  let confirmReasonError = $state("");

  // Verzeichnis-Ziel unter /backup-targets (Plan W7).
  let dirChoice = $state("");
  let dirSub = $state("klein-buch");
  let dirLabel = $state("");

  // SFTP-Ziel (G1-BKP.3).
  let sftpLabel = $state("");
  let sftpHost = $state("");
  let sftpPort = $state<number | null>(22);
  let sftpPortInput: QuantityInput | undefined = $state();
  // U1/F4: Feldfehler am Host (u. a. 422 „Ungültiger Server-Name …“) und Port.
  let sftpHostError = $state("");
  let sftpPortError = $state("");
  let raiseError = $state("");
  let raiseInput: QuantityInput | undefined = $state();
  let sftpUser = $state("");
  let sftpRemoteDir = $state("");
  let sftpPassword = $state("");
  // Host-Key-Fingerprint aus dem letzten Test (Pflicht zum Speichern). Der
  // Server übernimmt nur den Fingerprint seiner letzten Probe für Host + Port
  // (Plan W8); ändern sich Host oder Port, ist ein neuer Test nötig.
  let sftpFingerprint = $state("");
  let sftpProbe = $state<SftpProbe | null>(null);
  let sftpProbedFor = $state("");
  const sftpEndpoint = $derived(`${sftpHost.trim().toLowerCase()}:${sftpPort}`);
  const sftpProbeValid = $derived(sftpFingerprint !== "" && sftpProbedFor === sftpEndpoint);
  let testingTargetId = $state<string | null>(null);

  // Restore (Plan W7): hochgeladene .kbk oder lokales Backup aus der Liste.
  let restoreFile = $state<File | null>(null);
  // Upload der gewählten Datei; nach falscher Passphrase wiederverwendet
  // (der Server lässt ihn dann liegen, ADR 0039 Nr. 72).
  let restoreUploadId = $state<string | null>(null);
  let restoreFromList = $state<BackupHistoryItem | null>(null);
  let restorePass = $state("");
  let uploadPercent = $state<number | null>(null);
  let restoreReport = $state<string | null>(null);
  // Refs zum gezielten Scrollen/Fokussieren.
  let restoreSection = $state<HTMLElement | null>(null);
  let restorePassInput = $state<HTMLInputElement | null>(null);
  let restoreFileInput = $state<HTMLInputElement | null>(null);

  // Tages-Backup-Uhrzeit (Lauf F, D-04) und Nummernkreise (D-24).
  let dailyTime = $state("02:30");
  let counters = $state<NumberingCounter[]>([]);
  let raiseFor = $state<NumberingCounter | null>(null);
  let raiseValue = $state<number | null>(null);
  let raiseReason = $state("");
  // Wiederhergestellte Entwürfe mit vor dem Restore festgeschriebener Nummer (F1d).
  let restoredDrafts = $state<RestoredDraftConflict[]>([]);
  let confirmFor = $state<RestoredDraftConflict | null>(null);
  let confirmReason = $state("");

  // U7 (R3-05): Sprungziele (#bk-numbering-h aus dem Restore-Banner, den Rechnungen
  // und Angeboten) entstehen erst nach dem Laden — danach selbst hinscrollen und die
  // Überschrift fokussieren, sonst landet man oben auf der Seite.
  onMount(async () => {
    await load();
    await scrollToHash();
  });

  async function scrollToHash() {
    const id = location.hash.slice(1);
    if (!id) return;
    await tick();
    const el = document.getElementById(id);
    if (!el) return;
    el.scrollIntoView({ block: "start" });
    if (!el.matches("a[href], button, input, select, textarea, [tabindex]")) el.setAttribute("tabindex", "-1");
    el.focus({ preventScroll: true });
  }

  async function load() {
    loadError = null;
    try {
      settings = await backupGetSettings();
      dailyTime = settings.dailyTime;
      if (!dirChoice && settings.availableDirs.length > 0) dirChoice = settings.availableDirs[0];
      history = await backupList();
      counters = await numberingCountersList();
      restoredDrafts = await numberingRestoredDraftsList();
    } catch (e) {
      // Ohne geladene Einstellungen: Fehlerzustand statt ewigem „Wird geladen …“ (V2-07);
      // ein späteres Nachladen nach einer Aktion meldet sich als Toast.
      if (!settings) loadError = String(e);
      else flash(`Neu laden fehlgeschlagen: ${String(e)}`, "error");
    }
  }

  async function saveDailyTime() {
    if (busy) return;
    dailyTimeError = "";
    if (!dailyTime) {
      dailyTimeError = "Bitte gib eine Uhrzeit ein, z. B. 02:30.";
      document.getElementById("bk-daily-time")?.focus();
      return;
    }
    busy = true;
    try {
      dailyTime = await backupSetDailyTime(dailyTime);
      flash(`Tages-Backup künftig ab ${dailyTime} Uhr.`);
    } catch (e) {
      flash(`Uhrzeit speichern fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  function startRaise(c: NumberingCounter) {
    raiseFor = c;
    raiseValue = c.lastValue + 1;
    raiseReason = "";
    raiseError = "";
    raiseReasonError = "";
  }

  async function doRaise() {
    const c = raiseFor;
    raiseError = "";
    raiseReasonError = "";
    if (!c || busy) return;
    if (raiseInput && !raiseInput.validate()) { raiseInput.focus(); return; }
    if (raiseValue === null || raiseValue <= c.lastValue) {
      raiseError = `Bitte gib eine Nummer größer als ${c.lastValue} ein.`;
      raiseInput?.focus();
      return;
    }
    if ([...raiseReason.trim()].length < 10) {
      raiseReasonError = "Bitte begründe die Änderung mit mindestens 10 Zeichen.";
      document.getElementById("bk-raise-reason")?.focus();
      return;
    }
    if (
      !(await confirmDialog({
        title: `Nummernkreis ${c.label} ${c.fiscalYear} anheben?`,
        body:
          `Als zuletzt vergeben gilt danach ${raiseValue}; die nächste Nummer ist ${raiseValue + 1}. ` +
          "Die Nummern dazwischen bleiben als dokumentierte Lücke (Audit-Log). " +
          "Absenken ist danach nicht möglich.",
        confirmLabel: "Anheben",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
    try {
      counters = await numberingRaise({
        docType: c.docType,
        fiscalYear: c.fiscalYear,
        lastValue: raiseValue,
        reason: raiseReason,
      });
      raiseFor = null;
      flash("Nummernkreis angehoben.");
    } catch (e) {
      flash(`Nummernkreis anheben fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  // F1d: Abgleich eines wiederhergestellten Entwurfs mit dem versandten
  // Original bestätigen (Re-Auth, Begründung im Audit-Log). Die Gestaltung
  // übernimmt der UI/UX-Lauf.
  async function doConfirmDraft() {
    const c = confirmFor;
    confirmReasonError = "";
    if (!c || busy) return;
    if ([...confirmReason.trim()].length < 10) {
      confirmReasonError = "Bitte begründe den Abgleich mit mindestens 10 Zeichen.";
      document.getElementById("bk-confirm-reason")?.focus();
      return;
    }
    busy = true;
    try {
      restoredDrafts = await numberingRestoredDraftConfirm({
        kind: c.kind,
        id: c.id,
        reason: confirmReason,
      });
      confirmFor = null;
      confirmReason = "";
      flash(`${c.number} lässt sich jetzt festschreiben.`);
    } catch (e) {
      flash(`Abgleich bestätigen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  const SIZE_DE = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${SIZE_DE.format(n / 1024)} KB`;
    return `${SIZE_DE.format(n / 1024 / 1024)} MB`;
  }

  /** Alter einer Kopie in Berliner Kalendertagen („heute“, „vor 3 Tagen“; F3). */
  function ageLabel(ts: string): string {
    // U7 (R4-07): über `dayClock`, damit „heute“ über Mitternacht zu „gestern“ wird.
    const d = -daysUntilBerlin(ts, dayClock.now);
    if (!Number.isFinite(d)) return "";
    return daysAgoLabel(d);
  }

  /** Tage → „heute“, „gestern“, „vor N Tagen“ (auch für `offsiteAgeDays` des Servers). */
  function daysAgoLabel(d: number): string {
    if (d <= 0) return "heute";
    if (d === 1) return "gestern";
    return `vor ${d} Tagen`;
  }

  /** Jüngste lokale Sicherung (Statusblock, ersetzt „jetzt entsperrt“, D-28). */
  const lastLocal = $derived.by(() => {
    let best: BackupHistoryItem | null = null;
    for (const h of history) {
      if (!isLocalPath(h.targetPath)) continue;
      if (!best || h.createdAt > best.createdAt) best = h;
    }
    return best;
  });

  // Rohe Backup-Anlässe in Klartext übersetzen.
  function reasonLabel(r: string): string {
    if (r === "auto_daily") return "Automatisch (täglich)";
    if (r === "manual") return "Manuell";
    if (r === "pre_restore") return "Vor einer Wiederherstellung";
    // Der Server speichert Sicherungen nach dem Festschreiben als `auto_critical`
    // (U4: stand bisher roh in der Liste).
    if (r === "auto_critical" || r.endsWith(".lock") || r.startsWith("recurring")) return "Nach dem Festschreiben";
    return r;
  }

  function retentionLabel(t: string): string {
    if (t === "daily") return "Täglich";
    if (t === "monthly") return "Monatlich";
    if (t === "yearly") return "Jährlich";
    // `manual` (manuell und vor einer Wiederherstellung) räumt die Rotation nie auf.
    if (t === "manual") return "Bleibt erhalten";
    return t;
  }

  /** Ergebnis einer Spiegelung (ADR 0041 Nr. 85: `suspended` vor `error`). */
  function offsiteState(r: OffsiteResult): "suspended" | "failed" | "copied" {
    if (r.suspended) return "suspended";
    return r.error ? "failed" : "copied";
  }

  // Ort eines Backups (G1-BKP.4): lokaler Floor vs. Off-Site-Spiegelung.
  function locationLabel(targetPath: string): string {
    if (settings && targetPath.startsWith(settings.floorPath)) return "Lokal";
    if (targetPath.startsWith("sftp://")) return "Off-Site (SFTP)";
    return "Off-Site";
  }

  // Direkt einspielbar sind nur Backups im lokalen Backup-Ordner; externe
  // Kopien erst herunterladen und hochladen.
  function isLocalPath(targetPath: string): boolean {
    return !!settings && targetPath.startsWith(settings.floorPath);
  }

  function targetLocation(t: TargetView): string {
    if (t.kind === "directory") return `${settings?.targetsRoot ?? ""}/${t.directory ?? ""}`;
    const dir = (t.remoteDir ?? "").replace(/^\/+/, "");
    return `sftp://${t.user}@${t.host}:${t.port}/${dir}`;
  }

  async function addDirectory() {
    if (!dirChoice || busy) return;
    const sub = dirSub.trim().replace(/^\/+|\/+$/g, "");
    const directory = sub ? `${dirChoice}/${sub}` : dirChoice;
    busy = true;
    try {
      await backupTargetAddDirectory(dirLabel.trim() || directory, directory);
      dirLabel = "";
      flash("Ordner-Ziel hinzugefügt — es wird ab dem nächsten Backup beschrieben.");
      await load();
    } catch (e) {
      flash(`Ordner-Ziel hinzufügen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  async function removeTarget(t: TargetView) {
    if (busy) return;
    if (
      !(await confirmDialog({
        title: `Backup-Ziel „${t.label}“ entfernen?`,
        body: "Vorhandene Backups am Ziel bleiben liegen; künftige Backups gehen nicht mehr dorthin.",
        confirmLabel: "Entfernen",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
    try {
      await backupTargetRemove(t.id);
      flash("Backup-Ziel entfernt.");
      await load();
    } catch (e) {
      flash(`Backup-Ziel entfernen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  // SFTP: Verbindung testen → liefert Host-Fingerprint zum Bestätigen + Pinnen.
  /** Prüft Host und Port vor dem Test/Speichern; true = ok. */
  function checkSftpFields(): boolean {
    sftpHostError = "";
    sftpPortError = "";
    sftpUserError = "";
    sftpPassError = "";
    if (!sftpHost.trim()) {
      sftpHostError = "Bitte gib den Server ein (Hostname oder IP-Adresse).";
      document.getElementById("sftp-host")?.focus();
      return false;
    }
    if (sftpPortInput && !sftpPortInput.validate()) {
      sftpPortInput.focus();
      return false;
    }
    if (sftpPort == null) {
      sftpPortError = "Bitte gib den Port ein, meist 22.";
      sftpPortInput?.focus();
      return false;
    }
    // U4 (ADR 0041 Nr. 135): statt gesperrter Knöpfe der Grund am Feld.
    if (!sftpUser.trim()) {
      sftpUserError = "Bitte gib den Benutzer ein.";
      document.getElementById("sftp-user")?.focus();
      return false;
    }
    if (!sftpPassword) {
      sftpPassError = "Bitte gib das Passwort für diesen Server ein.";
      document.getElementById("sftp-pass")?.focus();
      return false;
    }
    return true;
  }
  /** Server-Meldung zum Host (422, Lauf F4) am Feld zeigen statt als Toast. */
  function sftpHostErrorFrom(msg: string): boolean {
    if (/server-name|hostname/i.test(msg)) {
      sftpHostError = msg;
      document.getElementById("sftp-host")?.focus();
      return true;
    }
    return false;
  }

  async function testSftp() {
    if (busy) return;
    sftpAddError = "";
    if (!checkSftpFields()) return;
    busy = true;
    sftpProbe = null;
    try {
      const p = await backupTestSftp(
        sftpHost.trim(),
        sftpPort as number,
        sftpUser.trim(),
        sftpRemoteDir.trim(),
        sftpPassword,
      );
      sftpProbe = p;
      sftpFingerprint = p.fingerprint;
      sftpProbedFor = sftpEndpoint;
      if (p.writeOk) {
        flash("Verbindung ok — Host-Fingerprint bitte prüfen und speichern.");
      } else {
        flash("Verbindung/Login ok, aber Schreiben im Zielordner schlug fehl.", "error");
      }
    } catch (e) {
      if (!sftpHostErrorFrom(String(e))) flash(`Verbindungstest fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  // SFTP: als Backup-Ziel hinzufügen (Passwort je Ziel, Fingerprint → Pin).
  async function addSftp() {
    if (busy) return;
    sftpAddError = "";
    if (!checkSftpFields()) return;
    if (!sftpProbeValid) {
      sftpAddError = "Teste zuerst die Verbindung und prüfe den Fingerabdruck des Servers.";
      document.getElementById("sftp-test-btn")?.focus();
      return;
    }
    busy = true;
    try {
      await backupTargetAddSftp({
        label: sftpLabel.trim() || sftpHost.trim(),
        host: sftpHost.trim(),
        port: sftpPort as number,
        user: sftpUser.trim(),
        remoteDir: sftpRemoteDir.trim(),
        hostFingerprint: sftpFingerprint,
        password: sftpPassword,
      });
      sftpPassword = "";
      sftpProbe = null;
      sftpFingerprint = "";
      sftpProbedFor = "";
      flash("SFTP-Ziel hinzugefügt.");
      await load();
    } catch (e) {
      if (!sftpHostErrorFrom(String(e))) flash(`SFTP-Ziel hinzufügen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  // Gespeichertes SFTP-Ziel testen (nur per ID: gespeichertes Passwort,
  // gepinnter Fingerprint — Plan W8).
  async function testSavedSftp(t: TargetView) {
    if (testingTargetId) return;
    testingTargetId = t.id;
    try {
      const p = await backupTestSftpSaved(t.id);
      if (p.writeOk) {
        flash(`Verbindung zu „${t.label}“ ok, Fingerprint stimmt.`);
      } else {
        flash(`Login ok, aber Schreiben auf „${t.label}“ schlug fehl: ${p.writeError ?? ""}`, "error");
      }
    } catch (e) {
      flash(`Verbindungstest fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      testingTargetId = null;
    }
  }

  async function backupNow() {
    if (busy) return;
    busy = true;
    backingUp = true;
    // U4 (V1-15): Status während des Laufs (role="status"); der Knopf bleibt fokussierbar
    // (ariaDisabled, ADR 0041 Nr. 33). Danach EIN zusammenfassender Toast — die Einzelheiten
    // je Ziel stehen nur in der Ergebnisliste darunter (ADR 0041 Nr. 134), nicht als Toast je Ziel.
    backupStatus = "Backup wird erstellt …";
    try {
      const o = await backupCreateNow();
      lastOutcome = o;
      // Off-Site-Spiegelung (Plan W7): je Ziel in der Liste. U4 (Warteliste (12), ADR 0041
      // Nr. 85): `suspended` vor `error` — nach einer Wiederherstellung ruht die Spiegelung
      // bis zur Freigabe; das ist kein Fehlschlag des Ziels.
      const total = o.offsite.length;
      const suspended = o.offsite.filter((r) => r.suspended).length;
      const failed = o.offsite.filter((r) => !r.suspended && r.error).length;
      const copied = total - suspended - failed;
      const size = fmtBytes(o.sizeBytes);
      // Beginnt immer mit „Backup erstellt:“ (E2E-Harness wartet auf diesen Text).
      if (total === 0) {
        flash(`Backup erstellt: lokal gespeichert (${size}).`);
      } else if (suspended + failed === 0) {
        flash(`Backup erstellt: lokal und auf ${total === 1 ? "dem externen Ziel" : `allen ${total} externen Zielen`} gespeichert (${size}).`);
      } else {
        const parts = [`auf ${copied} von ${total} externen Zielen gespeichert`];
        if (failed > 0) parts.push(`${failed} fehlgeschlagen`);
        if (suspended > 0) parts.push(`${suspended} ausgesetzt`);
        flash(`Backup erstellt: lokal gespeichert (${size}), ${parts.join(", ")}. Einzelheiten unter „Backup jetzt erstellen“.`, "warning");
      }
      await load();
    } catch (e) {
      flash(`Backup fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      backupStatus = "";
      backingUp = false;
      busy = false;
    }
  }

  // Lauf F3a: verschlüsselte .kbk herunterladen (Umzug, Disaster Recovery ohne
  // externes Ziel). Re-Auth fragt der Transport selbst ab.
  async function downloadBackup(item: BackupHistoryItem) {
    if (busy) return;
    busy = true;
    try {
      saveDownload(await backupDownload(item.id));
    } catch (e) {
      flash(`Herunterladen fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  async function pickForRestore(item: BackupHistoryItem) {
    restoreFromList = item;
    restoreFile = null;
    if (restoreFileInput) restoreFileInput.value = "";
    restoreReport = null;
    await tick();
    restoreSection?.scrollIntoView({ behavior: "smooth", block: "start" });
    restorePassInput?.focus();
  }

  function pickFile(e: Event) {
    restoreFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    restoreUploadId = null;
    if (restoreFile) restoreFromList = null;
    restoreReport = null;
  }

  async function doRestore() {
    if (busy) return;
    // U4 (V1-14, ADR 0041 Nr. 135): Knopf bleibt bedienbar, ein fehlendes Passwort steht am Feld.
    restorePassError = "";
    if (!restorePass) {
      restorePassError = "Bitte gib das Passwort dieses Backups ein.";
      restorePassInput?.focus();
      return;
    }
    // U4 (V1-14): Die Rückfrage nennt den ganzen Ablauf bis zur Freigabe (ADR 0040 Nr. 11–14).
    if (
      !(await confirmDialog({
        title: "Backup wiederherstellen?",
        body:
          "Ist das Backup älter als der aktuelle Stand, gehen seither erfasste Belege verloren. " +
          "So geht es weiter:",
        bullets: [
          "Klein.Buch legt zuerst ein Sicherheits-Backup des aktuellen Stands an.",
          "Du bestätigst mit Daten-Passwort und App-Code.",
          "Der Server startet neu und spielt das Backup ein.",
          "Danach meldest du dich mit dem Passwort dieses Backups an.",
          "Abo-Läufe, automatischer Versand und externe Backups ruhen, bis du sie über " +
            "„Prüfen und freigeben“ wieder freigibst.",
        ],
        confirmLabel: "Wiederherstellen",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
    restoring = true;
    // R5-S2-UI (Lauf F4a): das Passwort des Backups nur für diesen Aufruf —
    // danach aus dem Zustand der Seite (auch nach einem Fehler; nicht in
    // Stores, URL oder Storage).
    const passphrase = restorePass;
    restorePass = "";
    try {
      let source: { uploadId: string } | { backupId: string };
      if (restoreFile) {
        if (!restoreUploadId) {
          uploadPercent = 0;
          const t = await uploadFile("restore", restoreFile, (loaded, total) => {
            uploadPercent = total > 0 ? Math.round((loaded / total) * 100) : null;
          });
          restoreUploadId = t.uploadId;
        }
        source = { uploadId: restoreUploadId };
      } else if (restoreFromList) {
        source = { backupId: restoreFromList.id };
      } else {
        return;
      }
      uploadPercent = null;
      const r = await backupRestoreApply(source, passphrase);
      restoreUploadId = null;
      // Plan W5: der Server startet nach dem Restore selbst neu und wendet ihn
      // dabei an; danach ist die Anmeldung mit der Passphrase des Backups nötig.
      restoreReport =
        `Wiederherstellung vorbereitet. Sicherheits-Backup: ${r.preRestoreBackup}. ` +
        "Der Server startet jetzt neu und wendet die Wiederherstellung an — bitte warten." +
        (r.notice ? ` ${r.notice}` : "");
      flash("Wiederherstellung vorgemerkt — Server startet neu.", "ok");
      if (!(await waitForRestart())) {
        restoreReport +=
          " Der Server ist nach drei Minuten nicht zurück. Bitte den Container prüfen und die Seite neu laden.";
      }
    } catch (e) {
      // Nur nach falscher Passphrase bleibt der Upload auf dem Server liegen.
      const msg = String(e);
      const wrongPass = /falsche[ns]? (Passphrase|Passwort)/i.test(msg);
      if (!wrongPass) restoreUploadId = null;
      if (wrongPass) {
        // Ablehnung des Backup-Passworts am Feld (Fokus dorthin), sonst Toast.
        restorePassError = msg;
        restorePassInput?.focus();
      } else {
        flash(`Wiederherstellen fehlgeschlagen: ${msg}`, "error");
      }
    } finally {
      uploadPercent = null;
      restoring = false;
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Backups">
  {#snippet actions()}
    <Button variant="secondary" href="/settings/backup-log">Backup-Protokoll</Button>
    <HelpAnchor slug="backup-und-wiederherstellen" />
  {/snippet}
</PageBar>

{#if settings}
  <!-- U4 (U2-021): Ab 60rem Inhaltsbreite (Tablet quer, Desktop) zwei Spalten statt
       48rem-Karten mit leerer rechter Hälfte; links Stand und Sichern, rechts die Ziele.
       Lesereihenfolge = DOM-Reihenfolge (erst links, dann rechts). -->
  <div class="bk-cols">
  <div class="bk-col">
  <section class="card" aria-labelledby="bk-status-h">
    <h2 id="bk-status-h">Status</h2>
    <dl class="bk-status">
      <dt>Backup-Passwort</dt>
      <dd>{settings.passphraseSet ? "eingerichtet" : "nicht eingerichtet"}</dd>
      <dt>Letzte lokale Sicherung</dt>
      <dd>{lastLocal ? `${dateTimeShort(lastLocal.createdAt)} (${ageLabel(lastLocal.createdAt)})` : "noch keine"}</dd>
      <dt>Tägliches Backup</dt>
      <dd>ab {settings.dailyTime} Uhr</dd>
      <dt>Externe Ziele</dt>
      <dd>
        {settings.targets.length === 0
          ? "keins eingerichtet"
          : settings.targets.length === 1
            ? "1 Ziel"
            : `${settings.targets.length} Ziele`}
      </dd>
      {#if settings.targets.length > 0}
        <!-- Lauf F3/U4: älteste der je Ziel jüngsten Kopien (offsiteAgeDays, Berliner Tage). -->
        <dt>Stand extern</dt>
        <dd>
          {settings.offsiteLastCopyAt && settings.offsiteAgeDays !== null
            ? `${dateTimeShort(settings.offsiteLastCopyAt)} (${daysAgoLabel(settings.offsiteAgeDays)})`
            : "noch nicht auf jedem Ziel eine Kopie"}
        </dd>
      {/if}
    </dl>
    <p class="muted">
      Deine Backups sind verschlüsselt. Das Backup-Passwort wird niemals
      gespeichert — bewahre es sicher auf.
    </p>
  </section>

  <section class="card" aria-labelledby="bk-now-h">
    <h2 id="bk-now-h">Backup jetzt erstellen</h2>
    <!-- U4 (V1-15): während des Laufs fokussierbar (busy) mit Text „Sichert …“ und Statuszeile;
         gesperrt (disabled) nur, solange die Datenbank gesperrt ist — mit sichtbarem Grund. -->
    <Button
      onclick={backupNow}
      disabled={!settings.unlocked}
      busy={backingUp}
      ariaDisabled={busy && !backingUp}
      ariaDescribedby={!settings.unlocked ? "bk-now-locked" : undefined}
    >{backingUp ? "Sichert …" : "Jetzt sichern"}</Button>
    <p class="sub" role="status">{backupStatus}</p>
    {#if !settings.unlocked}
      <p id="bk-now-locked" class="sub">Die Datenbank ist gesperrt — bitte neu anmelden.</p>
    {/if}
    {#if lastOutcome}
      <!-- U4 (Warteliste (12)): Ergebnis je externem Ziel, „ausgesetzt“ getrennt von
           „fehlgeschlagen“ (Pause nach einer Wiederherstellung, ADR 0041 Nr. 85). -->
      <div class="outcome">
        <p class="outcome-h">
          Zuletzt gesichert: <span class="path">{lastOutcome.fileName}</span> ({fmtBytes(lastOutcome.sizeBytes)}), lokal gespeichert.
        </p>
        {#if lastOutcome.offsite.length > 0}
          <ul class="outcome-list" aria-label="Ergebnis je externem Ziel">
            {#each lastOutcome.offsite as r (r.targetId)}
              {@const st = offsiteState(r)}
              <li>
                <span class="outcome-label">{r.label}</span>
                {#if st === "copied"}
                  <Badge tone="success">Gespeichert</Badge>
                {:else if st === "suspended"}
                  <Badge tone="warning">Ausgesetzt</Badge>
                  <span class="sub">{r.error ?? "Pause nach einer Wiederherstellung bis zur Freigabe."}</span>
                {:else}
                  <Badge tone="danger">Fehlgeschlagen</Badge>
                  <span class="sub">{r.error}</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}

    <h3>Tägliches Backup</h3>
    <div class="row daily">
      <div class="f">
        <label for="bk-daily-time">Uhrzeit (Europe/Berlin)</label>
        <input id="bk-daily-time" class="kb-w-date" type="time" bind:value={dailyTime}
          aria-invalid={dailyTimeError ? "true" : undefined}
          aria-describedby={dailyTimeError ? "bk-daily-err bk-daily-hint" : "bk-daily-hint"} />
        {#if dailyTimeError}<span id="bk-daily-err" class="err">{dailyTimeError}</span>{/if}
      </div>
      <div class="btnrow-inline">
        <Button variant="secondary" onclick={saveDailyTime} ariaDisabled={busy}>Uhrzeit speichern</Button>
      </div>
    </div>
    <p id="bk-daily-hint" class="muted">
      Einmal je Kalendertag, ab der eingestellten Uhrzeit oder beim ersten Lauf danach (etwa nach
      einer Sperre). Sicherungen nach dem Festschreiben laufen unabhängig davon.
    </p>
  </section>
  </div>

  <div class="bk-col">
  <section class="card" aria-labelledby="bk-targets-h">
    <h2 id="bk-targets-h">Externe Ziele <HelpAnchor slug="backup-ziel-einrichten" /></h2>
    {#if settings.offsiteStatus === "missing"}
      <Banner kind="warning">
        Kein externes Ziel eingerichtet — die Backups liegen nur im Datenverzeichnis
        des Servers. Bitte mindestens ein Ziel hinzufügen.
      </Banner>
    {:else if settings.offsiteStatus === "failing"}
      <Banner kind="warning">
        Mindestens ein Ziel ist beim letzten Backup fehlgeschlagen — Einzelheiten unten
        und im Backup-Protokoll.
      </Banner>
    {:else if settings.offsiteStatus === "stale"}
      <Banner kind="warning">
        Mindestens ein Ziel hat keine aktuelle Kopie: noch keine (z. B. gerade
        hinzugefügt) oder die letzte ist älter als die Frist der Regel „Backup
        überfällig“. Nach einer Wiederherstellung ruht die Spiegelung bis zur Freigabe;
        sonst schreibt das nächste Backup oder „Jetzt sichern“ eine neue Kopie. Den
        Stand je Ziel zeigt die Liste unten.
      </Banner>
    {/if}
    <p class="muted">
      Eine <strong>lokale Sicherung läuft immer</strong> automatisch:
      <code class="path">{settings.floorPath}</code>. Zusätzlich geht jede Sicherung
      an jedes externe Ziel (verschlüsselt).
    </p>

    {#if settings.targets.length > 0}
      <ul class="targets">
        {#each settings.targets as t (t.id)}
          <li>
            <div class="target-info">
              <strong>{t.label}</strong>
              <span class="muted">({t.kind === "sftp" ? "SFTP" : "Ordner"})</span>
              <div class="path">{targetLocation(t)}</div>
              {#if t.lastResult}
                {#if t.lastResult.ok}
                  <div class="sub">
                    Letzte Kopie: {dateTimeShort(t.lastResult.at)} <strong>({ageLabel(t.lastResult.at)})</strong>
                  </div>
                {:else}
                  <div class="bad">Fehlgeschlagen ({dateTimeShort(t.lastResult.at)}): {t.lastResult.error}</div>
                  <!-- U4 (Lauf F3): letzte gute Kopie auch nach einem Fehlschlag. -->
                  <div class="sub">
                    {#if t.lastCopyAt}
                      Letzte erfolgreiche Kopie: {dateTimeShort(t.lastCopyAt)} <strong>({ageLabel(t.lastCopyAt)})</strong>
                    {:else}
                      Noch keine erfolgreiche Kopie auf diesem Ziel.
                    {/if}
                  </div>
                {/if}
              {:else if t.lastCopyAt}
                <div class="sub">
                  Letzte Kopie: {dateTimeShort(t.lastCopyAt)} <strong>({ageLabel(t.lastCopyAt)})</strong>
                </div>
              {:else}
                <div class="sub">Noch kein Backup auf dieses Ziel.</div>
              {/if}
            </div>
            <div class="target-actions kb-actions">
              {#if t.kind === "sftp"}
                <Button
                  variant="secondary"
                  onclick={() => testSavedSftp(t)}
                  busy={testingTargetId === t.id}
                  ariaDisabled={busy}
                  ariaLabel={`Verbindung testen, Ziel ${t.label}`}
                >
                  {testingTargetId === t.id ? "Teste …" : "Testen"}
                </Button>
              {/if}
              <Button variant="danger" onclick={() => removeTarget(t)} ariaDisabled={busy} ariaLabel={`Entfernen, Ziel ${t.label}`}>Entfernen</Button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <h3>Ordner-Ziel hinzufügen</h3>
    {#if settings.availableDirs.length === 0}
      <p class="sub">
        Unter <code>{settings.targetsRoot}</code> ist kein Ordner eingebunden. In der
        Container-Konfiguration ein Volume unter <code>{settings.targetsRoot}/…</code>
        einbinden (z. B. NAS-Freigabe oder Sync-Ordner), dann erscheint es hier.
      </p>
    {:else}
      <div class="row">
        <div class="f">
          <label for="dir-choice">Eingebundener Ordner</label>
          <select id="dir-choice" bind:value={dirChoice}>
            {#each settings.availableDirs as d (d)}<option value={d}>{d}</option>{/each}
          </select>
        </div>
        <div class="f">
          <label for="dir-sub">Unterordner</label>
          <input id="dir-sub" type="text" bind:value={dirSub} autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="z. B. klein-buch" />
        </div>
        <div class="f">
          <label for="dir-label">Bezeichnung</label>
          <input id="dir-label" type="text" bind:value={dirLabel} autocomplete="off" placeholder="z. B. NAS im Büro" />
        </div>
      </div>
      <div class="btnrow">
        <Button onclick={addDirectory} ariaDisabled={busy}>Ordner-Ziel hinzufügen</Button>
      </div>
    {/if}
  </section>

  <section class="card" aria-labelledby="bk-sftp-h">
    <h2 id="bk-sftp-h">SFTP-Ziel hinzufügen (für Fortgeschrittene)</h2>
    <p class="muted">
      Kopie auf einen eigenen Server per SSH/SFTP. Beim Test zeigt der Server seinen
      „Fingerabdruck“; bitte prüfen und bestätigen (Schutz vor manipulierten
      Verbindungen). Das Passwort wird verschlüsselt in der Datenbank dieser Instanz
      gespeichert und gilt nur für dieses Ziel; für einen anderen Server oder
      Benutzer das Ziel entfernen und neu anlegen.
    </p>

    <!-- U2: ab rund 37rem zwei Spalten (Bezeichnung | Server, Port + Benutzer | Zielordner). -->
    <div class="kb-form-grid">
      <div class="f">
        <label for="sftp-label">Bezeichnung</label>
        <input id="sftp-label" type="text" bind:value={sftpLabel} autocomplete="off" placeholder="z. B. Server bei Hoster" />
      </div>
      <div class="f">
        <label for="sftp-host">Server (Host)</label>
        <input
          id="sftp-host"
          type="text"
          bind:value={sftpHost}
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          inputmode="url"
          placeholder="z. B. backup.example.org"
          aria-invalid={sftpHostError ? "true" : undefined}
          aria-describedby={sftpHostError ? "sftp-host-err" : "sftp-host-hint"}
        />
        {#if sftpHostError}
          <span id="sftp-host-err" class="err">{sftpHostError}</span>
        {:else}
          <span id="sftp-host-hint" class="sub">Hostname oder IP-Adresse, ohne Port und ohne „sftp://“.</span>
        {/if}
      </div>
      <div class="row">
        <div class="port">
          <QuantityInput
            bind:this={sftpPortInput}
            bind:value={sftpPort}
            label="Port"
            integer
            grouping={false}
            max={65535}
            noun="Port"
            hint="Meist 22."
            error={sftpPortError || undefined}
          />
        </div>
        <div class="f grow">
          <label for="sftp-user">Benutzer</label>
          <input id="sftp-user" type="text" bind:value={sftpUser} autocomplete="off" autocapitalize="off" spellcheck="false" placeholder="z. B. klein-buch"
            aria-invalid={sftpUserError ? "true" : undefined} aria-describedby={sftpUserError ? "sftp-user-err" : undefined} />
          {#if sftpUserError}<span id="sftp-user-err" class="err">{sftpUserError}</span>{/if}
        </div>
      </div>
      <div class="f">
        <label for="sftp-dir">Zielordner auf dem Server</label>
        <input id="sftp-dir" type="text" bind:value={sftpRemoteDir} autocomplete="off" autocapitalize="off" spellcheck="false" aria-describedby="sftp-dir-hint" />
        <span id="sftp-dir-hint" class="sub">z. B. klein-buch; leer = Home-Verzeichnis</span>
      </div>
      <div class="f">
        <label for="sftp-pass">Passwort</label>
        <input id="sftp-pass" type="password" bind:value={sftpPassword} autocomplete="new-password" autocapitalize="off" spellcheck="false"
          aria-invalid={sftpPassError ? "true" : undefined} aria-describedby={sftpPassError ? "sftp-pass-err" : undefined} />
        {#if sftpPassError}<span id="sftp-pass-err" class="err">{sftpPassError}</span>{/if}
      </div>
    </div>

    <div class="btnrow">
      <!-- U4 (ADR 0041 Nr. 135, V2-06): Knöpfe bleiben bedienbar; fehlende Angaben stehen am Feld,
           ein fehlender Test unter den Knöpfen. -->
      <Button
        id="sftp-test-btn"
        variant="secondary"
        onclick={testSftp}
        ariaDisabled={busy}
        ariaDescribedby={sftpAddError ? "sftp-add-err sftp-btn-hint" : "sftp-btn-hint"}
      >
        Verbindung testen
      </Button>
      <Button onclick={addSftp} ariaDisabled={busy} ariaDescribedby={sftpAddError ? "sftp-add-err sftp-btn-hint" : "sftp-btn-hint"}>
        Als SFTP-Ziel hinzufügen
      </Button>
    </div>
    {#if sftpAddError}<p id="sftp-add-err" class="err">{sftpAddError}</p>{/if}
    <p id="sftp-btn-hint" class="sub">
      Zum Testen brauchst du Server, Benutzer und Passwort; speichern lässt sich das Ziel erst nach einem
      erfolgreichen Test.
    </p>

    {#if sftpProbe && sftpProbeValid}
      <div class="preview" role="status">
        <p class:bad={!sftpProbe.writeOk}>
          Schreibtest im Zielordner: {sftpProbe.writeOk ? "ok" : "fehlgeschlagen"}
        </p>
        {#if !sftpProbe.writeOk && sftpProbe.writeError}
          <p class="fingerprint">{sftpProbe.writeError}</p>
        {/if}
        <p class="muted">Host-Fingerprint (bitte mit deinem Server vergleichen):</p>
        <p class="fingerprint">{sftpProbe.fingerprint}</p>
      </div>
    {:else if sftpFingerprint}
      <p class="sub">
        Server oder Port geändert — bitte erneut „Verbindung testen“, damit der
        Fingerabdruck zum neuen Server passt.
      </p>
    {/if}
  </section>
  </div>
  </div>

  <section class="card wide" aria-labelledby="bk-history-h">
    <h2 id="bk-history-h">Backup-Verlauf</h2>
    <p class="muted">
      Wiederherstellbare Sicherungen, die du zurückspielen oder herunterladen kannst. Den
      lückenlosen Nachweis jeder Sicherung (auch fehlgeschlagene externe
      Spiegelungen) findest du im <a class="kb-link" href="/settings/backup-log">Backup-Protokoll</a>.
    </p>
    {#if history.length === 0}
      <p class="muted">Noch keine Backups. Erstelle das erste mit „Jetzt sichern“.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Backup-Verlauf</caption>
        <thead>
          <tr>
            <th>Erstellt</th>
            <th>Grund</th>
            <th>Ort</th>
            <th>Aufbewahrung</th>
            <th class="kb-num">Größe</th>
            <th><span class="kb-visually-hidden">Aktionen</span></th>
          </tr>
        </thead>
        <tbody>
          {#each history as item (item.id)}
            <tr>
              <td class="nowrap" data-label="Erstellt">{dateTime(item.createdAt)}</td>
              <td data-label="Grund">{reasonLabel(item.triggerReason)}</td>
              <td class="ort cards-full" data-label="Ort">
                {locationLabel(item.targetPath)}
                <div class="path">{item.targetPath}</div>
              </td>
              <td data-label="Aufbewahrung">{retentionLabel(item.retentionTag)}</td>
              <td class="kb-num nowrap" data-label="Größe">{fmtBytes(item.fileSizeBytes)}</td>
              <td>
                <!-- U4 (U3-036): Aktionen untereinander gleich breit. -->
                <div class="kb-actions kb-stack">
                  {#if isLocalPath(item.targetPath)}
                    <Button variant="secondary" onclick={() => pickForRestore(item)} ariaLabel={`Zum Wiederherstellen auswählen, Backup vom ${dateTime(item.createdAt)}`}>Auswählen</Button>
                  {:else if item.targetPath.startsWith("sftp://")}
                    <span class="sub">Liegt auf dem SFTP-Server: dort holen, dann unten als Datei hochladen.</span>
                  {/if}
                  {#if !item.targetPath.startsWith("sftp://")}
                    <Button variant="secondary" onclick={() => downloadBackup(item)} ariaDisabled={busy} ariaLabel={`Herunterladen, Backup vom ${dateTime(item.createdAt)}`}>Herunterladen</Button>
                  {/if}
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <!-- U4 (U2-021): Wiederherstellen und Nummernkreise ab 60rem nebeneinander. -->
  <div class="bk-cols">
  <div class="bk-col">
  <section class="card danger" bind:this={restoreSection} aria-labelledby="bk-restore-h">
    <h2 id="bk-restore-h">Wiederherstellen</h2>
    <p class="muted">
      Backup-Datei (.kbk) hochladen oder oben im Verlauf „Auswählen“ →
      Backup-Passwort eingeben. Eingespielt werden Backups ab Klein.Buch v2026.5;
      ältere Datenbank-Versionen werden beim nächsten Anmelden umgestellt (vorher
      entsteht automatisch ein Backup).
    </p>
    <div class="f">
      <label for="bk-restore-file">Backup-Datei (.kbk)</label>
      <input
        id="bk-restore-file"
        type="file"
        accept=".kbk"
        bind:this={restoreFileInput}
        onchange={pickFile}
        disabled={busy}
      />
    </div>
    {#if restoreFromList}
      <p>Ausgewählt: <strong>Backup vom {dateTime(restoreFromList.createdAt)}</strong></p>
    {/if}
    {#if restoreFile || restoreFromList}
      <div class="f">
        <label for="bk-restore-pass">Backup-Passwort</label>
        <input
          id="bk-restore-pass"
          type="password"
          bind:value={restorePass}
          bind:this={restorePassInput}
          autocomplete="current-password"
          autocapitalize="off"
          spellcheck="false"
          aria-invalid={restorePassError ? "true" : undefined}
          aria-describedby={restorePassError ? "bk-restore-pass-err bk-restore-pass-hint" : "bk-restore-pass-hint"}
          oninput={() => (restorePassError = "")}
        />
        {#if restorePassError}<span id="bk-restore-pass-err" class="err">{restorePassError}</span>{/if}
        <span id="bk-restore-pass-hint" class="sub">Das Passwort, mit dem dieses Backup erstellt wurde.</span>
      </div>
      <!-- U4 (V1-14, ADR 0041 Nr. 135): bleibt bedienbar; ein fehlendes Passwort steht am Feld. -->
      <div class="btnrow">
        <Button variant="danger" onclick={doRestore} busy={restoring} ariaDisabled={busy}>Wiederherstellen</Button>
      </div>
      {#if uploadPercent !== null}
        <p class="muted" role="status">Hochladen … {uploadPercent} %</p>
      {/if}
    {/if}

    {#if restoreReport}
      <Banner kind="success">{restoreReport}</Banner>
    {/if}
  </section>
  </div>

  <div class="bk-col">
  <section class="card" aria-labelledby="bk-numbering-h">
    <h2 id="bk-numbering-h">Nummernkreise <HelpAnchor slug="disaster-recovery" /></h2>
    <p class="muted">
      Zuletzt vergebene Nummer je Belegart und Geschäftsjahr. Nach einer Wiederherstellung
      in eine neue Instanz: Wurden nach dem Stand des Backups noch Belege nummeriert, den
      Nummernkreis hier auf die zuletzt vergebene Nummer anheben — nur aufwärts, mit
      Begründung im Audit-Log.
    </p>
    {#if restoredDrafts.length > 0}
      <Banner kind="warning">
        <p class="drafts-text">
          Diese Entwürfe stammen aus dem eingespielten Backup. Ihre Nummern waren vor der
          Wiederherstellung bereits festgeschrieben und vermutlich versandt. Passe den Entwurf an
          das versandte Original an und bestätige danach den Abgleich — erst dann lässt er sich
          festschreiben (nie zwei verschiedene Belege mit derselben Nummer). Bearbeitest du den
          Entwurf nach dem Bestätigen noch einmal, erscheint er hier wieder.
        </p>
      </Banner>
      <!-- V6-R04 (ADR 0042 Nr. 16): Liste mit Knöpfen außerhalb der Live-Region des Banners. -->
      <ul class="drafts">
        {#each restoredDrafts as c (c.kind + c.id)}
          <li>
            <DocNo value={c.number} href={c.kind === "quote" ? `/quotes/${c.id}` : `/invoices/${c.id}`} />
            <Button variant="secondary" onclick={() => ((confirmFor = c), (confirmReason = ""), (confirmReasonError = ""))} ariaDisabled={busy} ariaLabel={`Abgleich bestätigen, ${c.number}`}>
              Abgleich bestätigen …
            </Button>
          </li>
        {/each}
      </ul>
      {#if confirmFor}
        <form class="raise-form" novalidate onsubmit={(e) => { e.preventDefault(); doConfirmDraft(); }}>
          <p><strong>{confirmFor.number}</strong> entspricht dem versandten Original</p>
          <div class="f">
            <label for="bk-confirm-reason">Begründung (Audit-Log)</label>
            <input
              id="bk-confirm-reason"
              type="text"
              maxlength="500"
              autocomplete="off"
              bind:value={confirmReason}
              aria-invalid={confirmReasonError ? "true" : undefined}
              aria-describedby={confirmReasonError ? "bk-confirm-reason-err bk-confirm-reason-hint" : "bk-confirm-reason-hint"}
            />
            {#if confirmReasonError}<span id="bk-confirm-reason-err" class="err">{confirmReasonError}</span>{/if}
            <span id="bk-confirm-reason-hint" class="sub">Mindestens 10 Zeichen, z. B. „an das Original laut Mail-Ausgang angepasst“.</span>
          </div>
          <div class="btnrow">
            <!-- U4 (V2-11): „Abbrechen“ zuerst, Hauptaktion zuletzt; fehlende Begründung am Feld. -->
            <Button variant="secondary" onclick={() => (confirmFor = null)} ariaDisabled={busy}>Abbrechen</Button>
            <Button type="submit" ariaDisabled={busy}>Abgleich bestätigen</Button>
          </div>
        </form>
      {/if}
    {/if}
    {#if counters.length === 0}
      <p class="muted">Noch keine Nummern vergeben.</p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Nummernkreise</caption>
        <thead>
          <tr>
            <th>Belegart</th>
            <th class="kb-num">Jahr</th>
            <th>Zuletzt vergeben</th>
            <th><span class="kb-visually-hidden">Aktionen</span></th>
          </tr>
        </thead>
        <tbody>
          {#each counters as c (c.docType + c.fiscalYear)}
            <tr>
              <td data-label="Belegart">{c.label}</td>
              <td class="kb-num" data-label="Jahr">{c.fiscalYear}</td>
              <td data-label="Zuletzt vergeben"><DocNo value={c.lastNumber} /></td>
              <td>
                <div class="kb-actions">
                  <Button variant="secondary" onclick={() => startRaise(c)} ariaDisabled={busy} ariaLabel={`Anheben, ${c.label} ${c.fiscalYear}`}>Anheben …</Button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
    {#if raiseFor}
      <form class="raise-form" novalidate onsubmit={(e) => { e.preventDefault(); doRaise(); }}>
        <p><strong>{raiseFor.label} {raiseFor.fiscalYear}</strong> — derzeit {raiseFor.lastValue}</p>
        <QuantityInput
          bind:this={raiseInput}
          bind:value={raiseValue}
          fieldClass="kb-w-short"
          label="Zuletzt vergebene Nummer (laufende Nummer)"
          integer
          grouping={false}
          noun="Nummer"
          hint={`Größer als ${raiseFor.lastValue}.`}
          error={raiseError || undefined}
        />
        <div class="f">
          <label for="bk-raise-reason">Begründung (Audit-Log)</label>
          <input
            id="bk-raise-reason"
            type="text"
            maxlength="500"
            autocomplete="off"
            bind:value={raiseReason}
            aria-invalid={raiseReasonError ? "true" : undefined}
            aria-describedby={raiseReasonError ? "bk-raise-reason-err bk-raise-reason-hint" : "bk-raise-reason-hint"}
          />
          {#if raiseReasonError}<span id="bk-raise-reason-err" class="err">{raiseReasonError}</span>{/if}
          <span id="bk-raise-reason-hint" class="sub">Mindestens 10 Zeichen, z. B. „letzte Rechnung laut Mail-Ausgang RE-2026-0042“.</span>
        </div>
        <div class="btnrow">
          <!-- U4 (V2-11): „Abbrechen“ zuerst, Hauptaktion zuletzt; fehlende Angaben am Feld. -->
          <Button variant="secondary" onclick={() => (raiseFor = null)} ariaDisabled={busy}>Abbrechen</Button>
          <Button type="submit" ariaDisabled={busy}>Nummernkreis anheben</Button>
        </div>
      </form>
    {/if}
  </section>
  </div>
  </div>
{:else if loadError}
  <Banner title="Die Backup-Einstellungen konnten nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else}
  <LoadingState />
{/if}

<style>
  /* .card / .card.danger — globale Definitionen aus tokens.css; Hinweise über Banner (V3-18).
     Formular-Karten bleiben auf 48rem gedeckelt; Tabellen-Karten (.wide) nicht. */
  .card { max-width: 48rem; }
  .card.wide { max-width: none; }
  /* U4 (U2-021): Seitenraster. Einspaltig wie bisher, ab 60rem Inhaltsbreite (gemessen
     an <main>, Container „kb-main“, ADR 0041 Nr. 40) zwei gleich breite Spalten; die
     Karten füllen dann ihre Spalte. */
  .bk-cols { display: grid; grid-template-columns: minmax(0, 1fr); column-gap: var(--sp-4); align-items: start; }
  .bk-col { min-width: 0; }
  @container kb-main (width >= 60rem) {
    .bk-cols { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .bk-col > .card { max-width: none; }
  }
  .outcome {
    margin-top: var(--sp-3);
    padding: var(--sp-3);
    background: var(--surface-sunken);
    border: var(--rule);
    border-radius: var(--radius-control);
  }
  .outcome-h { margin: 0; font-size: var(--fs-sm); }
  .outcome-list { list-style: none; margin: var(--sp-2) 0 0; padding: 0; }
  .outcome-list li {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: var(--sp-1) var(--sp-2);
    padding: var(--sp-1) 0;
    border-top: var(--rule);
  }
  .outcome-label { font-weight: 600; overflow-wrap: anywhere; }
  .outcome-list .sub { flex-basis: 100%; margin-top: 0; }
  h2 { margin: 0 0 0.6rem; font-size: var(--fs-lg); }
  h3 { margin: 1.1rem 0 0.4rem; font-size: var(--fs-md); }
  .muted { color: var(--text-muted); font-size: var(--fs-sm); }
  .sub { display: block; color: var(--text-muted); font-size: var(--fs-sm); margin-top: 0.3rem; overflow-wrap: anywhere; }
  .err { display: block; color: var(--status-danger-fg); font-weight: 600; font-size: var(--fs-sm); margin-top: 0.3rem; }
  .path {
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--text-muted);
    overflow-wrap: anywhere;
  }
  td .path { display: block; margin-top: 0.15rem; }
  /* U1: Spalten dürfen schrumpfen (max-content + nowrap-Datum ließ die Karte
     auf 320–360 px in sich scrollen, axe scrollable-region-focusable). */
  .bk-status { display: grid; grid-template-columns: minmax(0, max-content) minmax(0, 1fr); gap: 0.3rem 1rem; margin: 0 0 0.75rem; }
  .bk-status dt { font-weight: 600; color: var(--text-muted); font-size: var(--fs-sm); overflow-wrap: anywhere; }
  .bk-status dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  @media (max-width: 480px) {
    .bk-status { grid-template-columns: minmax(0, 1fr); gap: 0 0; }
    .bk-status dd { margin-bottom: 0.4rem; }
  }
  .f { display: flex; flex-direction: column; gap: 0.25rem; margin: 0.7rem 0 0; min-width: 0; }
  .f > label { font-weight: 600; font-size: var(--fs-sm); }
  .row { display: flex; flex-wrap: wrap; gap: 0 0.75rem; align-items: flex-start; }
  .row > .f { flex: 1 1 12rem; }
  .row > .port { flex: 0 0 9rem; margin-top: 0.7rem; }
  .row > .grow { flex: 1 1 14rem; }
  /* U2: Port und Benutzer bleiben auch in der halben Breite des Formular-Rasters nebeneinander
     (nur ab Tablet; auf dem Handy bleibt der Umbruch wie bisher). */
  @media (min-width: 601px) { .row > .grow { flex-basis: 10rem; } }
  .row.daily { align-items: flex-end; }
  .row.daily > .f { flex: 0 1 12rem; }
  .btnrow { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.8rem; }
  .btnrow-inline { display: flex; align-items: flex-end; }
  input[type="text"],
  input[type="password"],
  input[type="time"],
  select {
    width: 100%;
    padding: 0.5rem 0.65rem;
    border: 1px solid var(--line-control);
    border-radius: var(--r-md);
    font: inherit;
    min-height: var(--target);
    background: var(--surface-card);
    color: var(--text);
  }
  table { width: 100%; border-collapse: collapse; font-size: var(--fs-sm); }
  th, td { vertical-align: top; }
  td.ort { overflow-wrap: anywhere; }
  @media (min-width: 821px) {
    .nowrap { white-space: nowrap; }
    td.ort { min-width: 18rem; }
  }
  .preview {
    margin-top: 0.8rem;
    padding: 0.8rem;
    background: var(--surface-sunken);
    border-radius: var(--r-md);
  }
  .bad { color: var(--status-danger-fg); font-weight: 700; }
  .drafts-text { margin: 0; }
  .drafts { list-style: none; padding: 0; margin: 0; }
  .drafts li { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem 1rem; padding: 0.25rem 0; }
  ul.targets { list-style: none; padding: 0; margin: 0.6rem 0 1rem; }
  ul.targets li {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: flex-start;
    gap: 0.5rem 1rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--line);
  }
  .target-info { min-width: 0; flex: 1 1 16rem; }
  .fingerprint {
    font-family: var(--font-mono);
    font-size: var(--fs-sm);
    overflow-wrap: anywhere;
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-sm);
    padding: 0.4rem 0.5rem;
    margin: 0.2rem 0 0;
  }
  .raise-form {
    margin-top: 0.8rem;
    padding: 0.8rem;
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    background: var(--surface-sunken);
  }
  .raise-form p { margin: 0 0 0.4rem; }
</style>
