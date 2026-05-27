<script lang="ts">
  // Block 4 — Backup & Restore.
  import { onMount, tick } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import {
    backupGetSettings,
    backupCreateNow,
    backupSetTarget,
    backupTestSftp,
    backupSetSftpTarget,
    backupList,
    backupOpenFolder,
    backupRevealPath,
    backupRestorePreview,
    backupRestoreApply,
    type BackupSettings,
    type BackupHistoryItem,
    type RestorePreview,
    type DetectedTarget,
    type SftpProbe,
  } from "$lib/api";

  let settings = $state<BackupSettings | null>(null);
  let history = $state<BackupHistoryItem[]>([]);
  let targetInput = $state("");
  let busy = $state(false);

  // SFTP-Ziel (G1-BKP.3).
  let sftpHost = $state("");
  let sftpPort = $state(22);
  let sftpUser = $state("");
  let sftpRemoteDir = $state("");
  let sftpPassword = $state("");
  // Bestätigter/gepinnter Host-Key-Fingerprint (Pflicht zum Speichern).
  let sftpFingerprint = $state("");
  let sftpProbe = $state<SftpProbe | null>(null);

  // Restore-Wizard-State.
  let restorePath = $state("");
  let restorePass = $state("");
  let preview = $state<RestorePreview | null>(null);
  let restoreReport = $state<string | null>(null);
  // Refs zum gezielten Scrollen/Fokussieren.
  let restoreSection = $state<HTMLElement | null>(null);
  let previewBox = $state<HTMLElement | null>(null);
  let restorePassInput = $state<HTMLInputElement | null>(null);

  onMount(load);

  async function load() {
    try {
      settings = await backupGetSettings();
      targetInput = settings.targetPath ?? "";
      if (settings.sftp) {
        sftpHost = settings.sftp.host;
        sftpPort = settings.sftp.port;
        sftpUser = settings.sftp.user;
        sftpRemoteDir = settings.sftp.remoteDir;
        sftpFingerprint = settings.sftp.hostFingerprint ?? "";
      }
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

  // Nur lokale Pfade kann der Restore-Assistent direkt lesen (SFTP erst laden).
  function isLocalPath(targetPath: string): boolean {
    return !targetPath.startsWith("sftp://");
  }

  // Lokalen Ordner im Explorer/Finder öffnen (Floor / Off-Site-Verzeichnis).
  async function openFolder(path: string) {
    try {
      await backupOpenFolder(path);
    } catch (e) {
      flash(String(e), "error");
    }
  }

  // Eine Backup-Datei im enthaltenden Ordner anzeigen (Verlaufs-Zeile).
  async function revealBackup(path: string) {
    try {
      await backupRevealPath(path);
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function saveTarget() {
    busy = true;
    try {
      await backupSetTarget(targetInput.trim());
      flash("Backup-Ordner gespeichert.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  // Erkannten Cloud-Ordner per 1-Klick übernehmen und direkt speichern.
  async function useDetected(dt: DetectedTarget) {
    targetInput = dt.path;
    await saveTarget();
  }

  // Nativen Ordner-Auswahl-Dialog öffnen (tauri-plugin-dialog) und gewählten
  // Pfad übernehmen + speichern.
  async function browseFolder() {
    try {
      const picked = await openDialog({
        directory: true,
        multiple: false,
        title: "Off-Site-Backup-Ordner wählen",
      });
      if (typeof picked === "string" && picked.trim()) {
        targetInput = picked;
        await saveTarget();
      }
    } catch (e) {
      flash(String(e), "error");
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

  // SFTP: als Backup-Ziel speichern (Passwort → Keychain, Fingerprint → Pin).
  async function saveSftp() {
    if (!sftpFingerprint) {
      flash("Bitte zuerst die Verbindung testen, um den Host-Fingerprint zu bestätigen.", "error");
      return;
    }
    busy = true;
    try {
      await backupSetSftpTarget(
        sftpHost.trim(),
        sftpPort,
        sftpUser.trim(),
        sftpRemoteDir.trim(),
        sftpFingerprint,
        sftpPassword,
      );
      sftpPassword = "";
      flash("SFTP-Ziel gespeichert.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function backupNow() {
    busy = true;
    try {
      const o = await backupCreateNow();
      flash(`Backup erstellt: ${o.fileName} (${fmtBytes(o.sizeBytes)})`);
      // Off-Site-Spiegelung (G1-BKP.4): Erfolg/Fehler separat zurückmelden.
      if (o.mirrorError) {
        flash(`Off-Site-Kopie fehlgeschlagen: ${o.mirrorError}`, "error");
      } else if (o.mirrorTarget) {
        flash("Off-Site-Kopie ebenfalls gespeichert.");
      }
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function pickForRestore(item: BackupHistoryItem) {
    restorePath = item.targetPath;
    preview = null;
    restoreReport = null;
    await tick();
    restoreSection?.scrollIntoView({ behavior: "smooth", block: "start" });
    // Direkt prüfen — ein Klick reicht, dann nur noch Passphrase eingeben.
    await doPreview();
  }

  async function doPreview() {
    busy = true;
    preview = null;
    try {
      preview = await backupRestorePreview(restorePath.trim());
      await tick();
      if (restorePassInput) {
        restorePassInput.focus();
        restorePassInput.scrollIntoView({ behavior: "smooth", block: "center" });
      } else {
        previewBox?.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function doRestore() {
    if (
      !(await confirmDialog({
        title: "Backup wiederherstellen?",
        body:
          "Wenn du ein älteres Backup wiederherstellst, gehen seither erfasste" +
          " Belege verloren. Sicherheitshalber wird vorher automatisch ein Backup" +
          " des aktuellen Stands erstellt.",
        confirmLabel: "Wiederherstellen",
        danger: true,
      }))
    ) {
      return;
    }
    busy = true;
    try {
      const r = await backupRestoreApply(restorePath.trim(), restorePass);
      restorePass = "";
      restoreReport =
        `Wiederherstellung vorbereitet. Sicherheits-Backup: ${r.preRestoreBackupPath}. ` +
        "Bitte starte die App jetzt neu — dann wird die Wiederherstellung angewendet.";
      flash("Wiederherstellung vorgemerkt — App neu starten.", "ok");
    } catch (e) {
      flash(String(e), "error");
    } finally {
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
    <h2>Off-Site-Kopie <HelpAnchor slug="backup-ziel-einrichten" /></h2>
    <p class="muted">
      Eine <strong>lokale Sicherung läuft immer</strong> automatisch — auch ohne
      Einstellung hier:
    </p>
    <div class="pathline">
      <code class="path">{settings.floorPath}</code>
      <button class="link" onclick={() => openFolder(settings!.floorPath)}>Ordner öffnen</button>
    </div>
    <p class="muted">
      Zusätzlich kann jede Sicherung in einen <strong>zweiten Ordner</strong>
      gespiegelt werden — ideal ein Cloud-Ordner (OneDrive, iCloud), dann liegt
      automatisch eine Kopie außer Haus. Leer lassen = nur lokale Sicherung.
    </p>

    {#if settings.detectedTargets.length > 0}
      <p class="muted">Erkannte Cloud-Ordner — ein Klick übernimmt sie als Ziel:</p>
      <div class="chips">
        {#each settings.detectedTargets as dt}
          <button class="chip" onclick={() => useDetected(dt)} disabled={busy} title={dt.path}>
            {dt.label}
          </button>
        {/each}
      </div>
    {/if}

    <div class="row" style="flex-wrap: wrap">
      <input
        type="text"
        bind:value={targetInput}
        placeholder={settings.defaultSuggestion}
      />
      <button onclick={browseFolder} disabled={busy}>Durchsuchen…</button>
      <button
        onclick={() => openFolder(targetInput.trim())}
        disabled={busy || !targetInput.trim()}
      >
        Ordner öffnen
      </button>
      <button class="primary" onclick={saveTarget} disabled={busy}>Speichern</button>
    </div>
    <p class="hint">Vorschlag: {settings.defaultSuggestion}</p>
  </section>

  <section class="card">
    <h2>SFTP-Server (für Fortgeschrittene)</h2>
    <p class="muted">
      Off-Site-Kopie auf einen eigenen Server per SSH/SFTP statt in einen Ordner
      (die lokale Sicherung läuft weiterhin immer). Das Passwort wird sicher im
      Schlüsselbund deines Systems
      gespeichert — nie in der App. Beim ersten Mal zeigt der Test den
      „Fingerabdruck" des Servers; bitte prüfen und bestätigen (Schutz vor
      manipulierten Verbindungen).
    </p>

    {#if settings.sftp}
      <p class="muted"><strong>Aktuelles Ziel ist dieser SFTP-Server.</strong></p>
    {/if}

    <label>
      Server (Host)
      <input type="text" bind:value={sftpHost} placeholder="backup.example.de" />
    </label>
    <div class="row">
      <label style="flex:0 0 8rem">
        Port
        <input type="number" bind:value={sftpPort} min="1" max="65535" />
      </label>
      <label style="flex:1">
        Benutzer
        <input type="text" bind:value={sftpUser} placeholder="manuel" />
      </label>
    </div>
    <label>
      Zielordner auf dem Server
      <input type="text" bind:value={sftpRemoteDir} placeholder="klein-buch (leer = Home-Verzeichnis)" />
    </label>
    <label>
      Passwort
      <input
        type="password"
        bind:value={sftpPassword}
        autocomplete="off"
        placeholder={settings.sftp ? "leer lassen = gespeichertes behalten" : ""}
      />
    </label>

    <div class="row" style="margin-top:0.6rem">
      <button onclick={testSftp} disabled={busy || !sftpHost.trim() || !sftpUser.trim()}>
        Verbindung testen
      </button>
      <button class="primary" onclick={saveSftp} disabled={busy || !sftpFingerprint}>
        Als SFTP-Ziel speichern
      </button>
    </div>

    {#if sftpProbe}
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
      <div class="preview">
        <p class="muted">Gepinnter Host-Fingerprint:</p>
        <p class="fingerprint">{sftpFingerprint}</p>
        <p class="hint">
          Host geändert? Bitte erneut „Verbindung testen", damit der Fingerabdruck
          zum neuen Server passt.
        </p>
      </div>
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
                  <button class="link" onclick={() => revealBackup(item.targetPath)}>Ordner öffnen</button>
                {:else}
                  <span class="hint" title="SFTP-Backup erst herunterladen, dann unten den Pfad einfügen.">erst laden</span>
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
      Backup über „Auswählen" in der Liste oben übernehmen (oder Pfad einfügen) →
      prüfen → Backup-Passwort eingeben.
    </p>
    <div class="row">
      <input type="text" bind:value={restorePath} placeholder="Pfad zur Backup-Datei" />
      <button onclick={doPreview} disabled={busy || !restorePath.trim()}>Backup prüfen</button>
    </div>

    {#if preview}
      <div class="preview" bind:this={previewBox}>
        <p>Erstellt: <strong>{preview.createdAt}</strong></p>
        <p>App-Version: {preview.appVersion}</p>
        <p>Inhalt: {fmtBytes(preview.contentSizeBytes)}</p>
        <p class:bad={!preview.compatible}>
          Passt zu dieser App-Version:
          {preview.compatible
            ? "ja"
            : "NEIN — dieses Backup stammt aus einer anderen Version"}
        </p>
        {#if preview.compatible}
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
        {/if}
      </div>
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
  .pathline {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    margin: 0.3rem 0 0.5rem;
  }
  .path {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8rem;
    color: #6b7280;
    word-break: break-all;
  }
  .pathline .path {
    flex: 1 1 14rem;
  }
  td .path {
    display: block;
    margin-top: 0.15rem;
  }
  .actions {
    white-space: nowrap;
  }
  .actions button.link + button.link {
    margin-left: 0.6rem;
  }
  .row {
    display: flex;
    gap: 0.5rem;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 0.5rem;
    margin: 0.2rem 0 0.7rem;
  }
  button.chip {
    background: #eff6ff;
    border-color: #bfdbfe;
    color: #1e40af;
    border-radius: 999px;
    padding: 0.35rem 0.85rem;
    font-size: 0.85rem;
    font-weight: 600;
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
  .preview .bad {
    color: #991b1b;
    font-weight: 700;
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
