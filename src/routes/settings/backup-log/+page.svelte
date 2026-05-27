<script lang="ts">
  import { onMount } from "svelte";
  import { backupLogSearch } from "$lib/api";
  import type { BackupLogEntry, BackupLogFilter } from "$lib/api";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let entries = $state<BackupLogEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Filter-State.
  let search = $state("");
  let dateFrom = $state("");
  let dateTo = $state("");
  let statusF = $state<"all" | "ok" | "failed">("all");
  let triggerF = $state<"all" | "manual" | "auto_daily" | "auto_critical" | "pre_restore">("all");
  let targetF = $state<"all" | "local" | "directory" | "sftp">("all");

  const LIMIT = 1000;

  function buildFilter(): BackupLogFilter {
    return {
      search: search.trim() || null,
      dateFrom: dateFrom || null,
      dateTo: dateTo || null,
      status: statusF === "all" ? null : statusF,
      trigger: triggerF === "all" ? null : triggerF,
      targetKind: targetF === "all" ? null : targetF,
      limit: LIMIT,
    };
  }

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await backupLogSearch(buildFilter());
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function reset() {
    search = "";
    dateFrom = "";
    dateTo = "";
    statusF = "all";
    triggerF = "all";
    targetF = "all";
    load();
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") load();
  }

  onMount(load);

  const hasFilter = $derived(
    !!(search.trim() || dateFrom || dateTo || statusF !== "all" || triggerF !== "all" || targetF !== "all"),
  );

  function fmtDateTime(ts: string | null): string {
    if (!ts) return "—";
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
  }
  function triggerLabel(t: string): string {
    return t === "manual"
      ? "Manuell"
      : t === "auto_daily"
        ? "Täglich (App-Start)"
        : t === "auto_critical"
          ? "Beleg festgeschrieben"
          : t === "pre_restore"
            ? "Vor Wiederherstellung"
            : t;
  }
  function targetLabel(e: BackupLogEntry): string {
    const base =
      e.targetKind === "local"
        ? "Lokal (Pflichtkopie)"
        : e.targetKind === "directory"
          ? "Ordner (extern)"
          : e.targetKind === "sftp"
            ? "SFTP-Server"
            : e.targetKind;
    return e.targetLabel ? `${base} · ${e.targetLabel}` : base;
  }
  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${kb.toFixed(1)} KB`;
    return `${(kb / 1024).toFixed(2)} MB`;
  }
</script>

<PageBar back="/settings/backup" backLabel="Backups" title="Backup-Protokoll" />

<p class="lead">
  Lückenloser, unveränderlicher Nachweis jeder Datensicherung (Erfolg und
  Fehlschlag): wann sie lief, wodurch sie ausgelöst wurde, wohin geschrieben
  wurde, wie groß die Datei ist und ob es geklappt hat. Such- und Filterfelder
  helfen, auch nach Jahren schnell den richtigen Lauf zu finden.
</p>

<section class="filterbar">
  <div class="row">
    <label class="grow">
      Suche
      <input
        type="search"
        bind:value={search}
        onkeydown={onSearchKey}
        placeholder="Dateiname, Pfad, Ziel, Fehlertext …"
      />
    </label>
    <label>
      Von
      <input type="date" bind:value={dateFrom} />
    </label>
    <label>
      Bis
      <input type="date" bind:value={dateTo} />
    </label>
  </div>
  <div class="row">
    <label>
      Status
      <select bind:value={statusF} onchange={load}>
        <option value="all">alle</option>
        <option value="ok">gesichert</option>
        <option value="failed">fehlgeschlagen</option>
      </select>
    </label>
    <label>
      Auslöser
      <select bind:value={triggerF} onchange={load}>
        <option value="all">alle</option>
        <option value="manual">Manuell</option>
        <option value="auto_daily">Täglich (App-Start)</option>
        <option value="auto_critical">Beleg festgeschrieben</option>
        <option value="pre_restore">Vor Wiederherstellung</option>
      </select>
    </label>
    <label>
      Ziel
      <select bind:value={targetF} onchange={load}>
        <option value="all">alle</option>
        <option value="local">Lokal (Pflichtkopie)</option>
        <option value="directory">Ordner (extern)</option>
        <option value="sftp">SFTP-Server</option>
      </select>
    </label>
    <div class="btns">
      <button class="btn-primary" onclick={load} disabled={loading}>Suchen</button>
      <button class="btn-secondary" onclick={reset} disabled={loading && !hasFilter}>Zurücksetzen</button>
    </div>
  </div>
</section>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else}
  <p class="muted count">
    {entries.length}{entries.length === LIMIT ? "+" : ""} Treffer{hasFilter ? " (gefiltert)" : ""},
    neueste zuerst.
    {#if entries.length === LIMIT}<span> Zeige max. {LIMIT} — Suche eingrenzen für ältere.</span>{/if}
  </p>

  {#if entries.length === 0}
    <p class="muted">Keine Einträge gefunden.</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th>Zeitpunkt</th>
          <th>Status</th>
          <th>Auslöser</th>
          <th>Ziel</th>
          <th>Datei</th>
          <th>Größe</th>
        </tr>
      </thead>
      <tbody>
        {#each entries as e (e.id)}
          <tr class:bad={e.status === "failed"}>
            <td class="ts">{fmtDateTime(e.createdAt)}</td>
            <td>
              {#if e.status === "ok"}
                <span class="ok">✓ gesichert</span>
              {:else}
                <span class="fail">✗ fehlgeschlagen</span>
              {/if}
            </td>
            <td>{triggerLabel(e.trigger)}</td>
            <td>{targetLabel(e)}</td>
            <td class="file">
              <span class="fname">{e.fileName}</span>
              <span class="path">{e.fullPath}</span>
              {#if e.status === "failed" && e.detail}
                <span class="errtext">{e.detail}</span>
              {/if}
            </td>
            <td class="size">{fmtSize(e.sizeBytes)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
{/if}

<style>
  /* .intro entfernt — globale .lead aus tokens.css. */
  .muted { color: var(--c-text-muted); }
  .count { font-size: 0.82rem; margin: 0.5rem 0; }
  .filterbar { background: #fff; border: 1px solid #e5e7eb; border-radius: 8px; padding: 0.75rem 1rem; margin: 0.75rem 0; }
  .filterbar .row { display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: flex-end; }
  .filterbar .row + .row { margin-top: 0.6rem; }
  .filterbar label { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.78rem; color: #6b7280; }
  .filterbar label.grow { flex: 1 1 18rem; }
  .filterbar input, .filterbar select { padding: 0.4rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.9rem; font-family: inherit; }
  .btns { display: flex; gap: 0.5rem; align-items: flex-end; margin-left: auto; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem 0.5rem; border-bottom: 1px solid #f3f4f6; font-size: 0.85rem; text-align: left; vertical-align: top; }
  th { font-size: 0.74rem; color: #6b7280; text-transform: uppercase; letter-spacing: 0.03em; }
  tr.bad td { background: #fff7f7; }
  .ts { white-space: nowrap; font-variant-numeric: tabular-nums; }
  .ok { color: #15803d; font-weight: 600; white-space: nowrap; }
  .fail { color: #b91c1c; font-weight: 600; white-space: nowrap; }
  .size { white-space: nowrap; font-variant-numeric: tabular-nums; }
  .file .fname { display: block; font-family: ui-monospace, monospace; font-size: 0.8rem; }
  .file .path { display: block; color: #9ca3af; font-size: 0.72rem; word-break: break-all; }
  .file .errtext { display: block; color: #b91c1c; font-size: 0.8rem; margin-top: 0.15rem; }
</style>
