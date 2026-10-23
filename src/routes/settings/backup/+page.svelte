<script lang="ts">
  // Block 4 — Backup & Restore.
  import { onMount, tick } from "svelte";
  import { waitForRestart } from "$lib/transport";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import {
    backupGetSettings,
    backupCreateNow,
    backupTestSftp,
    backupTestSftpSaved,
    backupTargetAddDirectory,
    backupTargetAddSftp,
    backupTargetRemove,
    backupList,
    backupRestoreApply,
    uploadFile,
    type BackupSettings,
    type BackupHistoryItem,
    type TargetView,
    type SftpProbe,
  } from "$lib/api";

  let settings = $state<BackupSettings | null>(null);
  let history = $state<BackupHistoryItem[]>([]);
  let busy = $state(false);

  // Verzeichnis-Ziel unter /backup-targets (Plan W7).
  let dirChoice = $state("");
  let dirSub = $state("klein-buch");
  let dirLabel = $state("");

  // SFTP-Ziel (G1-BKP.3).
  let sftpLabel = $state("");
  let sftpHost = $state("");
  let sftpPort = $state(22);
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

  onMount(load);

  async function load() {
    try {
      settings = await backupGetSettings();
      if (!dirChoice && settings.availableDirs.length > 0) dirChoice = settings.availableDirs[0];
      history = await backupList();
    } catch (e) {
      flash(String(e), "error");
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  // Rohe Backup-Anlässe in Klartext übersetzen.
  function reasonLabel(r: string): string {
    if (r === "auto_daily") return "Automatisch (täglich)";
    if (r === "manual") return "Manuell";
    if (r === "pre_restore") return "Vor einer Wiederherstellung";
    if (r.endsWith(".lock") || r.startsWith("recurring")) return "Nach dem Festschreiben";
    return r;
  }

  function retentionLabel(t: string): string {
    if (t === "daily") return "Täglich";
    if (t === "monthly") return "Monatlich";
    if (t === "yearly") return "Jährlich";
    return t;
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
    if (!dirChoice) return;
    const sub = dirSub.trim().replace(/^\/+|\/+$/g, "");
    const directory = sub ? `${dirChoice}/${sub}` : dirChoice;
    busy = true;
    try {
      await backupTargetAddDirectory(dirLabel.trim() || directory, directory);
      dirLabel = "";
      flash("Ordner-Ziel hinzugefügt — es wird ab dem nächsten Backup beschrieben.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function removeTarget(t: TargetView) {
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
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  // SFTP: Verbindung testen → liefert Host-Fingerprint zum Bestätigen + Pinnen.
  async function testSftp() {
    busy = true;
    sftpProbe = null;
    try {
      const p = await backupTestSftp(
        sftpHost.trim(),
        sftpPort,
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
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  // SFTP: als Backup-Ziel hinzufügen (Passwort je Ziel, Fingerprint → Pin).
  async function addSftp() {
    if (!sftpProbeValid) {
      flash("Bitte zuerst die Verbindung testen, um den Host-Fingerprint zu bestätigen.", "error");
      return;
    }
    busy = true;
    try {
      await backupTargetAddSftp({
        label: sftpLabel.trim() || sftpHost.trim(),
        host: sftpHost.trim(),
        port: sftpPort,
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
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  // Gespeichertes SFTP-Ziel testen (nur per ID: gespeichertes Passwort,
  // gepinnter Fingerprint — Plan W8).
  async function testSavedSftp(t: TargetView) {
    testingTargetId = t.id;
    try {
      const p = await backupTestSftpSaved(t.id);
      if (p.writeOk) {
        flash(`Verbindung zu „${t.label}“ ok, Fingerprint stimmt.`);
      } else {
        flash(`Login ok, aber Schreiben auf „${t.label}“ schlug fehl: ${p.writeError ?? ""}`, "error");
      }
    } catch (e) {
      flash(String(e), "error");
    } finally {
      testingTargetId = null;
    }
  }

  async function backupNow() {
    busy = true;
    try {
      const o = await backupCreateNow();
      flash(`Backup erstellt: ${o.fileName} (${fmtBytes(o.sizeBytes)})`);
      // Off-Site-Spiegelung (Plan W7): je Ziel zurückmelden.
      const failed = o.offsite.filter((r) => r.error);
      for (const r of failed) flash(`Kopie auf „${r.label}“ fehlgeschlagen: ${r.error}`, "error");
      if (o.offsite.length > 0 && failed.length === 0) {
        flash(`Kopie auf ${o.offsite.length} externe(s) Ziel(e) gespeichert.`);
      }
      await load();
    } catch (e) {
      flash(String(e), "error");
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
    if (
      !(await confirmDialog({
        title: "Backup wiederherstellen?",
        body:
          "Wenn du ein älteres Backup wiederherstellst, gehen seither erfasste" +
          " Belege verloren. Sicherheitshalber wird vorher automatisch ein Backup" +
          " des aktuellen Stands erstellt. Nach dem Restore gilt die Passphrase" +
          " dieses Backups.",
        confirmLabel: "Wiederherstellen",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
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
      const r = await backupRestoreApply(source, restorePass);
      restorePass = "";
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
      if (!String(e).includes("falsche Passphrase")) restoreUploadId = null;
      flash(String(e), "error");
    } finally {
      uploadPercent = null;
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Backups">
  {#snippet actions()}
    <a class="btn-secondary btn-sm" href="/settings/backup-log">Backup-Protokoll</a>
    <HelpAnchor slug="backup-und-wiederherstellen" />
  {/snippet}
</PageBar>

{#if settings}
  <section class="card">
    <h2>Status</h2>
    <p>
      Backup-Passwort eingerichtet:
      <strong>{settings.passphraseSet ? "ja" : "nein"}</strong> ·
      jetzt entsperrt: <strong>{settings.unlocked ? "ja" : "nein"}</strong>
    </p>
    <p class="muted">
      Deine Backups sind verschlüsselt. Das Backup-Passwort wird niemals
      gespeichert — bewahre es sicher auf.
    </p>
  </section>

  <section class="card">
    <h2>Externe Ziele <HelpAnchor slug="backup-ziel-einrichten" /></h2>
    {#if settings.offsiteStatus === "missing"}
      <p class="warn-box">
        Kein externes Ziel eingerichtet — die Backups liegen nur im Datenverzeichnis
        des Servers. Bitte mindestens ein Ziel hinzufügen.
      </p>
    {:else if settings.offsiteStatus === "failing"}
      <p class="warn-box">
        Mindestens ein Ziel ist beim letzten Backup fehlgeschlagen — Einzelheiten unten
        und im Backup-Protokoll.
      </p>
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
            <div>
              <strong>{t.label}</strong>
              <span class="muted">({t.kind === "sftp" ? "SFTP" : "Ordner"})</span>
              <div class="path">{targetLocation(t)}</div>
              {#if t.lastResult}
                {#if t.lastResult.ok}
                  <div class="hint">Zuletzt erfolgreich: {t.lastResult.at}</div>
                {:else}
                  <div class="bad">Fehlgeschlagen ({t.lastResult.at}): {t.lastResult.error}</div>
                {/if}
              {:else}
                <div class="hint">Noch kein Backup auf dieses Ziel.</div>
              {/if}
            </div>
            <div class="target-actions">
              {#if t.kind === "sftp"}
                <button
                  class="link"
                  onclick={() => testSavedSftp(t)}
                  disabled={busy || testingTargetId === t.id}
                >
                  {testingTargetId === t.id ? "Teste …" : "Testen"}
                </button>
              {/if}
              <button class="link" onclick={() => removeTarget(t)} disabled={busy}>Entfernen</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}

    <h3>Ordner-Ziel hinzufügen</h3>
    {#if settings.availableDirs.length === 0}
      <p class="hint">
        Unter <code>{settings.targetsRoot}</code> ist kein Ordner eingebunden. In der
        Container-Konfiguration ein Volume unter <code>{settings.targetsRoot}/…</code>
        einbinden (z. B. NAS-Freigabe oder Sync-Ordner), dann erscheint es hier.
      </p>
    {:else}
      <div class="row" style="flex-wrap: wrap">
        <label>
          Eingebundener Ordner
          <select bind:value={dirChoice}>
            {#each settings.availableDirs as d}<option value={d}>{d}</option>{/each}
          </select>
        </label>
        <label>
          Unterordner
          <input type="text" bind:value={dirSub} placeholder="klein-buch" />
        </label>
        <label>
          Bezeichnung
          <input type="text" bind:value={dirLabel} placeholder="z. B. NAS im Büro" />
        </label>
      </div>
      <button class="primary" onclick={addDirectory} disabled={busy || !dirChoice}>
        Ordner-Ziel hinzufügen
      </button>
    {/if}
  </section>

  <section class="card">
    <h2>SFTP-Ziel hinzufügen (für Fortgeschrittene)</h2>
    <p class="muted">
      Kopie auf einen eigenen Server per SSH/SFTP. Beim Test zeigt der Server seinen
      „Fingerabdruck“; bitte prüfen und bestätigen (Schutz vor manipulierten
      Verbindungen). Das Passwort wird verschlüsselt in der Datenbank dieser Instanz
      gespeichert und gilt nur für dieses Ziel; für einen anderen Server oder
      Benutzer das Ziel entfernen und neu anlegen.
    </p>

    <label>
      Bezeichnung
      <input type="text" bind:value={sftpLabel} placeholder="z. B. Server bei Hoster" />
    </label>
    <label>
      Server (Host)
      <input type="text" bind:value={sftpHost} placeholder="backup.example.org" />
    </label>
    <div class="row">
      <label style="flex:0 0 8rem">
        Port
        <input type="number" bind:value={sftpPort} min="1" max="65535" />
      </label>
      <label style="flex:1">
        Benutzer
        <input type="text" bind:value={sftpUser} placeholder="klein-buch" />
      </label>
    </div>
    <label>
      Zielordner auf dem Server
      <input type="text" bind:value={sftpRemoteDir} placeholder="klein-buch (leer = Home-Verzeichnis)" />
    </label>
    <label>
      Passwort
      <input type="password" bind:value={sftpPassword} autocomplete="off" />
    </label>

    <div class="row" style="margin-top:0.6rem">
      <button
        onclick={testSftp}
        disabled={busy || !sftpHost.trim() || !sftpUser.trim() || !sftpPassword}
      >
        Verbindung testen
      </button>
      <button class="primary" onclick={addSftp} disabled={busy || !sftpProbeValid || !sftpPassword}>
        Als SFTP-Ziel hinzufügen
      </button>
    </div>

    {#if sftpProbe && sftpProbeValid}
      <div class="preview">
        <p class:bad={!sftpProbe.writeOk}>
          Schreibtest im Zielordner: {sftpProbe.writeOk ? "ok" : "FEHLGESCHLAGEN"}
        </p>
        {#if !sftpProbe.writeOk && sftpProbe.writeError}
          <p class="fingerprint">{sftpProbe.writeError}</p>
        {/if}
        <p class="muted">Host-Fingerprint (bitte mit deinem Server vergleichen):</p>
        <p class="fingerprint">{sftpProbe.fingerprint}</p>
      </div>
    {:else if sftpFingerprint}
      <p class="hint">
        Server oder Port geändert — bitte erneut „Verbindung testen“, damit der
        Fingerabdruck zum neuen Server passt.
      </p>
    {/if}
  </section>

  <section class="card">
    <h2>Backup jetzt erstellen</h2>
    <button class="primary" onclick={backupNow} disabled={busy || !settings.unlocked}>
      Jetzt sichern
    </button>
    {#if !settings.unlocked}
      <p class="hint">Backup ist gesperrt — bitte zuerst mit dem Backup-Passwort entsperren.</p>
    {/if}
  </section>

  <section class="card wide">
    <h2>Backup-Verlauf</h2>
    <p class="muted">
      Wiederherstellbare Sicherungen, die du zurückspielen kannst. Den
      lückenlosen Nachweis jeder Sicherung (auch fehlgeschlagene externe
      Spiegelungen) findest du oben rechts unter „Backup-Protokoll".
    </p>
    {#if history.length === 0}
      <p class="muted">Noch keine Backups.</p>
    {:else}
      <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th>Erstellt</th>
            <th>Grund</th>
            <th>Ort</th>
            <th>Aufbewahrung</th>
            <th>Größe</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each history as item}
            <tr>
              <td>{item.createdAt}</td>
              <td>{reasonLabel(item.triggerReason)}</td>
              <td class="ort">
                {locationLabel(item.targetPath)}
                <div class="path">{item.targetPath}</div>
              </td>
              <td>{retentionLabel(item.retentionTag)}</td>
              <td>{fmtBytes(item.fileSizeBytes)}</td>
              <td class="actions">
                {#if isLocalPath(item.targetPath)}
                  <button class="link" onclick={() => pickForRestore(item)}>Auswählen ↓</button>
                {:else}
                  <span class="hint" title="Externe Kopie erst herunterladen, dann unten als Datei hochladen.">erst laden</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      </div>
    {/if}
  </section>

  <section class="card danger" bind:this={restoreSection}>
    <h2>Wiederherstellen</h2>
    <p class="muted">
      Backup-Datei (.kbk) hochladen oder oben in der Liste „Auswählen“ →
      Backup-Passwort eingeben. Eingespielt werden Backups ab Klein.Buch v2026.5;
      ältere Datenbank-Versionen werden beim nächsten Anmelden umgestellt (vorher
      entsteht automatisch ein Backup).
    </p>
    <label>
      Backup-Datei
      <input
        type="file"
        accept=".kbk"
        bind:this={restoreFileInput}
        onchange={pickFile}
        disabled={busy}
      />
    </label>
    {#if restoreFromList}
      <p>Ausgewählt: <strong>Backup vom {restoreFromList.createdAt}</strong></p>
    {/if}
    {#if restoreFile || restoreFromList}
      <label>
        Backup-Passwort
        <input
          type="password"
          bind:value={restorePass}
          bind:this={restorePassInput}
          autocomplete="off"
        />
      </label>
      <button class="primary" onclick={doRestore} disabled={busy || !restorePass}>
        Wiederherstellen
      </button>
      {#if uploadPercent !== null}
        <p class="muted">Hochladen … {uploadPercent} %</p>
      {/if}
    {/if}

    {#if restoreReport}
      <p class="ok-box">{restoreReport}</p>
    {/if}
  </section>
{:else}
  <p>Lade …</p>
{/if}

<style>
  /* .card / .card.danger entfernt — globale Definitionen aus tokens.css.
     Formular-Karten bleiben hier auf 48rem gedeckelt; die Verlaufs-Karte
     überschreibt das via .card.wide. */
  .card {
    max-width: 48rem;
  }
  /* Verlaufs-Tabelle nutzt die volle Panel-Breite (Formular-Karten bleiben 48rem). */
  .card.wide {
    max-width: none;
  }
  .table-wrap {
    overflow-x: auto;
  }
  h2 {
    margin: 0 0 0.6rem;
    font-size: 1.05rem;
  }
  .muted {
    color: #6b7280;
    font-size: 0.88rem;
  }
  .hint {
    color: #9ca3af;
    font-size: 0.8rem;
    margin-top: 0.4rem;
    word-break: break-all;
  }
  .path {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8rem;
    color: #6b7280;
    word-break: break-all;
  }
  td .path {
    display: block;
    margin-top: 0.15rem;
  }
  .actions {
    white-space: nowrap;
  }
  .row {
    display: flex;
    gap: 0.5rem;
  }
  input[type="text"],
  input[type="password"] {
    flex: 1;
    padding: 0.5rem 0.65rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.95rem;
  }
  label {
    display: block;
    margin: 0.7rem 0 0.2rem;
    font-weight: 600;
    font-size: 0.85rem;
  }
  /* Lokale Pre-DS-Klassen .primary/.link an DS-Tokens angeglichen
     (G2-UX.3.x Konsistenz-Fix): gleiche Specs wie globale .btn-* aus
     tokens.css. .link bleibt eigenständig (anderer Use-Case). */
  button {
    padding: 9px 16px;
    border: 1px solid var(--c-border-strong);
    background: var(--c-surface);
    color: var(--c-primary-700);
    border-radius: var(--r-md);
    cursor: pointer;
    font: inherit;
    font-weight: 600;
    transition: background var(--t-base) var(--ease-apple), border-color var(--t-base) var(--ease-apple);
  }
  button:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }
  button.primary {
    background: var(--c-primary-600);
    color: #fff;
    border-color: var(--c-primary-600);
  }
  button.primary:hover { background: var(--c-primary-700); border-color: var(--c-primary-700); }
  button.link {
    border: none;
    background: none;
    color: #2563eb;
    text-decoration: underline;
    padding: 0;
  }
  button:disabled {
    opacity: 0.55;
    cursor: default;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.88rem;
  }
  th,
  td {
    text-align: left;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid #f0f0f0;
    /* Schmale Spalten einzeilig halten → die ORT-Spalte bekommt die Restbreite. */
    white-space: nowrap;
    vertical-align: top;
  }
  /* ORT trägt den vollen Pfad: darf umbrechen + reserviert Mindestbreite. */
  td.ort {
    white-space: normal;
    min-width: 22rem;
  }
  .preview {
    margin-top: 0.8rem;
    padding: 0.8rem;
    background: #f9fafb;
    border-radius: 6px;
  }
  .bad {
    color: #991b1b;
    font-weight: 700;
  }
  .warn-box {
    padding: 0.6rem 0.8rem;
    background: #fef3c7;
    border: 1px solid #f59e0b;
    border-radius: 6px;
    color: #78350f;
  }
  ul.targets {
    list-style: none;
    padding: 0;
    margin: 0.6rem 0 1rem;
  }
  ul.targets li {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 1rem;
    padding: 0.5rem 0;
    border-bottom: 1px solid var(--c-border);
  }
  .target-actions {
    display: flex;
    gap: 0.75rem;
    white-space: nowrap;
  }
  .fingerprint {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.85rem;
    word-break: break-all;
    background: #fff;
    border: 1px solid #e5e7eb;
    border-radius: 4px;
    padding: 0.4rem 0.5rem;
    margin: 0.2rem 0 0;
  }
  .ok-box {
    margin-top: 0.8rem;
    padding: 0.7rem;
    background: #d1fae5;
    border: 1px solid #6ee7b7;
    border-radius: 6px;
    color: #065f46;
  }
</style>
