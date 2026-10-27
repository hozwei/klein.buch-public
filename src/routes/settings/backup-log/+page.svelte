<script lang="ts">
  import { onMount } from "svelte";
  import { backupLogSearch } from "#lib/api.ts";
  import type { BackupLogEntry, BackupLogFilter } from "#lib/api.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { dateTime } from "#lib/format.ts";

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
  const SIZE_DE1 = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 1, maximumFractionDigits: 1 });
  const SIZE_DE2 = new Intl.NumberFormat("de-DE", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  const INT_DE = new Intl.NumberFormat("de-DE");

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

  function reset(fromEmpty = false) {
    search = "";
    dateFrom = "";
    dateTo = "";
    statusF = "all";
    triggerF = "all";
    targetF = "all";
    // Der Link im leeren Zustand verschwindet danach; Fokus an den Anfang der Filter.
    if (fromEmpty) document.getElementById("bl-search")?.focus();
    load();
  }

  onMount(load);

  const hasFilter = $derived(
    !!(search.trim() || dateFrom || dateTo || statusF !== "all" || triggerF !== "all" || targetF !== "all"),
  );

  function triggerLabel(t: string): string {
    return t === "manual"
      ? "Manuell"
      : t === "auto_daily"
        ? "Täglich (automatisch)"
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
    if (bytes < 1024) return `${INT_DE.format(bytes)} B`;
    const kb = bytes / 1024;
    if (kb < 1024) return `${SIZE_DE1.format(kb)} KB`;
    return `${SIZE_DE2.format(kb / 1024)} MB`;
  }
</script>

<PageBar back="/settings/backup" backLabel="Backups" title="Backup-Protokoll" />

<p class="lead">
  Lückenloser, unveränderlicher Nachweis jeder Datensicherung (Erfolg und
  Fehlschlag): wann sie lief, wodurch sie ausgelöst wurde, wohin geschrieben
  wurde, wie groß die Datei ist und ob es geklappt hat. Such- und Filterfelder
  helfen, auch nach Jahren schnell den richtigen Lauf zu finden.
</p>

<form class="filterbar" role="search" aria-label="Backup-Protokoll durchsuchen" onsubmit={(e) => { e.preventDefault(); load(); }}>
  <div class="row">
    <label class="grow">
      Suche
      <input
        id="bl-search"
        type="search"
        bind:value={search}
        enterkeyhint="search"
        autocomplete="off"
        placeholder="z. B. Dateiname, Pfad, Ziel, Fehlertext"
      />
    </label>
    <label>
      Von
      <input type="date" bind:value={dateFrom} max={dateTo || undefined} />
    </label>
    <label>
      Bis
      <input type="date" bind:value={dateTo} min={dateFrom || undefined} />
    </label>
  </div>
  <div class="row">
    <label>
      Status
      <select bind:value={statusF} onchange={load}>
        <option value="all">Alle</option>
        <option value="ok">gesichert</option>
        <option value="failed">fehlgeschlagen</option>
      </select>
    </label>
    <label>
      Auslöser
      <select bind:value={triggerF} onchange={load}>
        <option value="all">Alle</option>
        <option value="manual">Manuell</option>
        <option value="auto_daily">Täglich (automatisch)</option>
        <option value="auto_critical">Beleg festgeschrieben</option>
        <option value="pre_restore">Vor Wiederherstellung</option>
      </select>
    </label>
    <label>
      Ziel
      <select bind:value={targetF} onchange={load}>
        <option value="all">Alle</option>
        <option value="local">Lokal (Pflichtkopie)</option>
        <option value="directory">Ordner (extern)</option>
        <option value="sftp">SFTP-Server</option>
      </select>
    </label>
    <!-- U4 (V2-10/V2-06): „Filtern“ als secondary, „Filter zurücksetzen“; während des Ladens
         fokussierbar (ariaDisabled). -->
    <div class="btns">
      <Button type="submit" variant="secondary" busy={loading}>Filtern</Button>
      <Button variant="secondary" onclick={() => reset()} ariaDisabled={loading}>Filter zurücksetzen</Button>
    </div>
  </div>
</form>

<!-- U4 (V4-06/V2-02): eine dauerhafte Statuszeile für Laden und Trefferzahl (4.1.3). -->
<p class="muted count" role="status">
  {#if loading}
    Wird geladen …
  {:else if !error}
    {entries.length}{entries.length === LIMIT ? "+" : ""} Treffer{hasFilter ? " (gefiltert)" : ""}, neueste zuerst.
    {#if entries.length === LIMIT}<span>{" "}Zeige höchstens {LIMIT} — grenz die Suche ein, um ältere zu sehen.</span>{/if}
  {/if}
</p>

{#if error}
  <Banner title="Das Backup-Protokoll konnte nicht geladen werden.">{error}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if !loading}
  {#if entries.length === 0}
    <p class="muted">
      {#if hasFilter}
        Keine Treffer für diese Filter. <button type="button" class="kb-link" onclick={() => reset(true)}>Filter zurücksetzen</button>
      {:else}
        Noch keine Sicherung protokolliert. <a class="kb-link kb-target" href="/settings/backup">Zu den Backups</a>
      {/if}
    </p>
  {:else}
    <!-- U4 (V2-10, design.md §4.5): Protokoll im Table-Baustein; scrollt in sich (ADR 0041 Nr. 42). -->
    <Table cards caption="Backup-Protokoll, neueste zuerst">
      <thead>
        <tr>
          <th>Zeitpunkt</th>
          <th>Status</th>
          <th>Auslöser</th>
          <th>Ziel</th>
          <th>Datei</th>
          <th class="num">Größe</th>
        </tr>
      </thead>
      <tbody>
        {#each entries as e (e.id)}
          <tr class:bad={e.status === "failed"}>
            <td class="ts" data-label="Zeitpunkt">{dateTime(e.createdAt)}</td>
            <td data-label="Status">
              {#if e.status === "ok"}
                <Badge tone={e.detail ? "warning" : "success"}>{e.detail ? "Gesichert, unvollständig" : "Gesichert"}</Badge>
              {:else}
                <Badge tone="danger">Fehlgeschlagen</Badge>
              {/if}
            </td>
            <td data-label="Auslöser">{triggerLabel(e.trigger)}</td>
            <td data-label="Ziel">{targetLabel(e)}</td>
            <td class="file cards-full" data-label="Datei">
              <span class="fname">{e.fileName}</span>
              <span class="path">{e.fullPath}</span>
              {#if e.status === "failed" && e.detail}
                <span class="errtext">{e.detail}</span>
              {:else if e.detail}
                <!-- Lauf F3c: „Unvollständig: …“ an einer ok-Zeile (Snapshot-Auslassungen). -->
                <span class="warntext">{e.detail}</span>
              {/if}
            </td>
            <td class="size num" data-label="Größe">{fmtSize(e.sizeBytes)}</td>
          </tr>
        {/each}
      </tbody>
    </Table>
  {/if}
{/if}

<style>
  /* .lead — global aus tokens.css. */
  .muted { color: var(--text-muted); }
  .count { font-size: var(--fs-sm); margin: 0.5rem 0; }
  .filterbar { background: var(--surface-card); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0.75rem 1rem; margin: 0.75rem 0; }
  .filterbar .row { display: flex; gap: 0.75rem; flex-wrap: wrap; align-items: flex-end; }
  .filterbar .row + .row { margin-top: 0.6rem; }
  .filterbar label { display: flex; flex-direction: column; gap: 0.2rem; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); min-width: 0; }
  .filterbar label.grow { flex: 1 1 18rem; max-width: 100%; }
  .filterbar input, .filterbar select { padding: 0.4rem 0.55rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card); min-height: var(--target); }
  .btns { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: flex-end; margin-left: auto; }
  th, td { font-size: var(--fs-sm); text-align: left; vertical-align: top; }
  th.num, td.num { text-align: right; }
  tr.bad td { background: var(--status-danger-bg); }
  .ts { font-variant-numeric: tabular-nums; }
  @media (min-width: 821px) {
    .ts { white-space: nowrap; }
  }
  .size { white-space: nowrap; font-variant-numeric: tabular-nums; }
  .file .fname { display: block; font-family: var(--font-mono); font-size: var(--fs-sm); overflow-wrap: anywhere; }
  .file .path { display: block; color: var(--text-muted); font-size: var(--fs-xs); overflow-wrap: anywhere; }
  .file .errtext { display: block; color: var(--status-danger-fg); font-size: var(--fs-sm); margin-top: 0.15rem; }
  .file .warntext { display: block; color: var(--status-warning-fg); font-size: var(--fs-sm); margin-top: 0.15rem; }
</style>
