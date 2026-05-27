<script lang="ts">
  // Block G1-RESET — Factory Reset (ADR 0036).
  //
  // Die EINE sanktionierte Total-Löschung: nukt die gesamte lokale Instanz und
  // bringt die App zurück auf den Onboarding-Zustand. Mehrstufig abgesichert:
  // GoBD-Warnung + Off-Site-Hinweis → Export-First → Tipp-Bestätigung (LÖSCHEN)
  // → Daten-Passwort → finaler confirmDialog. Selektives Beleg-Löschen bleibt
  // verboten (das ist KEIN Beleg-Editor).
  import { onMount } from "svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import {
    backupGetSettings,
    migrationExportRun,
    factoryResetCheck,
    factoryReset,
    type FactoryResetCheck,
  } from "$lib/api";

  let check = $state<FactoryResetCheck | null>(null);
  let loadError = $state<string | null>(null);

  // Export-First.
  let exportPath = $state("");
  let exporting = $state(false);
  let exportDone = $state(false);
  let exportedTo = $state<string | null>(null);

  // Bestätigung.
  let receiptText = $state("");
  let confirmText = $state("");
  let passphrase = $state("");
  let busy = $state(false);

  onMount(async () => {
    try {
      check = await factoryResetCheck();
    } catch (e) {
      loadError = String(e);
      return;
    }
    try {
      const s = await backupGetSettings();
      const stamp = new Date().toISOString().slice(0, 10);
      const sep = s.defaultSuggestion.includes("\\") ? "\\" : "/";
      exportPath = `${s.defaultSuggestion}${sep}klein-buch-export-${stamp}.zip`;
    } catch {
      exportPath = "klein-buch-export.zip";
    }
  });

  // Aufbewahrungs-Gating: ohne festgeschriebene Belege frei; sonst Export ODER
  // exakt getippte Quittung.
  let gateSatisfied = $derived(
    !!check &&
      (check.lockedDocuments === 0 ||
        exportDone ||
        receiptText.trim() === check.retentionReceiptText),
  );

  let canReset = $derived(
    !!check &&
      !busy &&
      gateSatisfied &&
      confirmText.trim() === check.confirmWord &&
      passphrase.length > 0,
  );

  async function runExport() {
    if (!exportPath.trim()) return;
    exporting = true;
    try {
      const r = await migrationExportRun(exportPath.trim());
      exportDone = true;
      exportedTo = r.zipPath;
      flash("Daten exportiert.");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      exporting = false;
    }
  }

  async function doReset() {
    if (!check || !canReset) return;
    const ok = await confirmDialog({
      title: "Klein.Buch unwiderruflich zurücksetzen?",
      body:
        "Alle Daten auf diesem PC werden gelöscht und können NICHT wiederhergestellt werden.",
      bullets: [
        "Rechnungen, Angebote, Kosten, Belege, Kontakte und Einstellungen",
        "Das gesamte Beleg-Archiv und alle lokalen Backups",
        check.hasOffSiteTarget
          ? `Externe Backups bleiben bestehen (${check.offSiteLabel}) — ggf. selbst löschen`
          : "Es ist kein externes Backup konfiguriert",
      ],
      confirmLabel: "Jetzt zurücksetzen",
      cancelLabel: "Abbrechen",
      danger: true,
    });
    if (!ok) return;

    busy = true;
    try {
      // Auf dem Erfolgspfad startet das Backend die App neu (→ Onboarding); der
      // Aufruf kehrt dann nicht zurück. Fehler (falsches Passwort, Gating)
      // landen im catch.
      await factoryReset({
        passphrase,
        confirmWord: confirmText,
        exportConfirmed: exportDone,
        retentionReceipt: receiptText,
      });
    } catch (e) {
      busy = false;
      flash(String(e), "error");
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Zurücksetzen">
  {#snippet actions()}
    <HelpAnchor slug="factory-reset" />
  {/snippet}
</PageBar>

{#if loadError}
  <section class="card"><p class="err">{loadError}</p></section>
{:else if check}
  <!-- 1. GoBD-Warnung + Off-Site-Hinweis -->
  <section class="card danger">
    <h2>Alles löschen und neu beginnen</h2>
    <p>
      Diese Funktion löscht <strong>deine gesamte Klein.Buch-Installation auf
      diesem PC</strong> und setzt die App auf den Anfang zurück — z. B. um das
      Gerät weiterzugeben oder ganz neu anzufangen. Einzelne Belege lassen sich
      bewusst <strong>nicht</strong> löschen; das hier ist alles oder nichts.
    </p>
    <p class="caveat">
      <strong>Aufbewahrungspflicht (GoBD/§147 AO):</strong> Rechnungen, Kosten und
      andere Belege müssen 10 Jahre aufbewahrt werden. Exportiere deine Daten
      unbedingt vorher, wenn du sie noch brauchst — nach dem Zurücksetzen sind sie
      auf diesem PC unwiderruflich weg.
    </p>
    {#if check.lockedDocuments > 0}
      <p class="info-banner">
        Es bestehen <strong>{check.lockedDocuments}</strong> festgeschriebene,
        aufbewahrungspflichtige Belege. Du musst sie zuerst exportieren
        <strong>oder</strong> unten ausdrücklich bestätigen, dass du deine
        Aufbewahrungspflicht erfüllt hast.
      </p>
    {/if}
    {#if check.hasOffSiteTarget}
      <p class="info-banner">
        Hinweis: Dein <strong>externes Backup-Ziel</strong> ({check.offSiteLabel})
        wird <strong>nicht</strong> gelöscht. Möchtest du es ebenfalls entfernen,
        tu das selbst direkt am Ziel.
      </p>
    {/if}
  </section>

  <!-- 2. Export-First -->
  <section class="card">
    <h3>Schritt 1 — Daten sichern (empfohlen)</h3>
    <p class="muted">
      Speichert <strong>alle</strong> Daten als ZIP — für Steuerberater,
      Archiv oder einen Programmwechsel. Die Datei ist
      <strong>nicht verschlüsselt</strong>; bewahre sie sicher auf.
    </p>
    <label>
      Speicherort (.zip-Datei)
      <input type="text" bind:value={exportPath} />
    </label>
    <button
      class="btn-secondary"
      onclick={runExport}
      disabled={exporting || !exportPath.trim()}
    >
      {exporting ? "Exportiere …" : "Alle Daten exportieren"}
    </button>
    {#if exportDone}
      <p class="ok-line">Export erstellt: <strong>{exportedTo}</strong></p>
    {/if}

    {#if check.lockedDocuments > 0 && !exportDone}
      <div class="receipt">
        <p class="muted">
          Alternativ — falls du bereits anderweitig gesichert hast: tippe zur
          Bestätigung exakt diesen Satz:
        </p>
        <p class="receipt-phrase">{check.retentionReceiptText}</p>
        <input
          type="text"
          bind:value={receiptText}
          placeholder="Aufbewahrungs-Quittung eintippen"
        />
      </div>
    {/if}
  </section>

  <!-- 3. Bestätigung -->
  <section class="card">
    <h3>Schritt 2 — Zurücksetzen bestätigen</h3>
    {#if !gateSatisfied}
      <p class="muted">
        Bitte zuerst exportieren oder die Aufbewahrungs-Quittung oben eintippen.
      </p>
    {/if}
    <label>
      Tippe zur Bestätigung <strong>{check.confirmWord}</strong>
      <input type="text" bind:value={confirmText} disabled={!gateSatisfied} />
    </label>
    <label>
      Daten-Passwort
      <input
        type="password"
        bind:value={passphrase}
        autocomplete="current-password"
        disabled={!gateSatisfied}
      />
    </label>
    <button class="btn-danger" onclick={doReset} disabled={!canReset}>
      {busy ? "Setze zurück …" : "Klein.Buch unwiderruflich zurücksetzen"}
    </button>
  </section>
{/if}

<style>
  /* .card / .danger-card / .warn-line / .info-line entfernt — globale
     Definitionen aus tokens.css (.card, .card.danger, .caveat, .info-banner). */
  .card {
    max-width: 48rem;
  }
  h3 {
    margin: 0 0 0.6rem;
  }
  p {
    font-size: 0.92rem;
    line-height: 1.5;
    color: var(--c-text);
  }
  .muted {
    color: var(--c-text-muted);
  }
  .ok-line {
    color: #065f46;
    word-break: break-all;
  }
  label {
    display: block;
    margin: 0.8rem 0 0.2rem;
    font-weight: 600;
    font-size: 0.85rem;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 0.5rem 0.65rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.95rem;
  }
  input:disabled {
    background: #f3f4f6;
    color: #9ca3af;
  }
  .receipt {
    margin-top: 0.9rem;
    padding-top: 0.8rem;
    border-top: 1px dashed var(--c-border);
  }
  .receipt-phrase {
    font-weight: 700;
    color: #111827;
    background: #f3f4f6;
    padding: 0.4rem 0.6rem;
    border-radius: 6px;
    user-select: all;
  }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß).
     Margin-top wird über Container geregelt, nicht am Button. */
  .btn-secondary, .btn-danger { margin-top: 1rem; }
  .err {
    color: #991b1b;
    font-weight: 600;
  }
</style>
