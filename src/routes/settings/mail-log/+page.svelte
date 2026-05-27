<script lang="ts">
  import { onMount } from "svelte";
  import { emailLogSearch } from "$lib/api";
  import type { EmailLogEntry, EmailLogFilter } from "$lib/types";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";

  let entries = $state<EmailLogEntry[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  // Filter-State.
  let search = $state("");
  let dateFrom = $state("");
  let dateTo = $state("");
  let statusF = $state<"all" | "success" | "failed">("all");
  let kindF = $state<"all" | "invoice" | "quote" | "test">("all");
  let channelF = $state<"all" | "smtp" | "graph">("all");

  const LIMIT = 1000;

  function buildFilter(): EmailLogFilter {
    return {
      search: search.trim() || null,
      dateFrom: dateFrom || null,
      dateTo: dateTo || null,
      status: statusF === "all" ? null : statusF,
      kind: kindF === "all" ? null : kindF,
      channel: channelF === "all" ? null : channelF,
      limit: LIMIT,
    };
  }

  async function load() {
    loading = true;
    error = null;
    try {
      entries = await emailLogSearch(buildFilter());
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
    kindF = "all";
    channelF = "all";
    load();
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") load();
  }

  onMount(load);

  const hasFilter = $derived(
    !!(search.trim() || dateFrom || dateTo || statusF !== "all" || kindF !== "all" || channelF !== "all"),
  );

  function fmtDateTime(ts: string | null): string {
    if (!ts) return "—";
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
  }
  function kindLabel(k: string): string {
    return k === "invoice" ? "Rechnung" : k === "quote" ? "Angebot" : k === "test" ? "Test-Mail" : k;
  }
  function channelLabel(c: string): string {
    return c === "graph" ? "Microsoft 365" : "SMTP";
  }
</script>

<PageBar back="/settings/mail" backLabel="E-Mail-Versand" title="E-Mail-Protokoll" />

<p class="lead">
  Lückenloser, unveränderlicher Nachweis jedes Versand­versuchs (Erfolg und
  Fehlschlag) mit Anbieter-Antwort (SMTP-Code bzw. Microsoft-Status + Anfrage-Nummer)
  und Fehlermeldung. Such- und Filterfelder helfen, auch nach Jahren schnell den
  richtigen Vorgang zu finden.
</p>

<section class="filterbar">
  <div class="row">
    <label class="grow">
      Suche
      <input
        type="search"
        bind:value={search}
        onkeydown={onSearchKey}
        placeholder="Empfänger, Betreff, Beleg-Nr., Fehler, request-id …"
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
        <option value="success">versendet</option>
        <option value="failed">fehlgeschlagen</option>
      </select>
    </label>
    <label>
      Art
      <select bind:value={kindF} onchange={load}>
        <option value="all">alle</option>
        <option value="invoice">Rechnung</option>
        <option value="quote">Angebot</option>
        <option value="test">Test-Mail</option>
      </select>
    </label>
    <label>
      Kanal
      <select bind:value={channelF} onchange={load}>
        <option value="all">alle</option>
        <option value="graph">Microsoft 365</option>
        <option value="smtp">SMTP</option>
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
          <th>Beleg</th>
          <th>Empfänger</th>
          <th>Betreff</th>
          <th>Kanal</th>
          <th>Antwort des Anbieters</th>
        </tr>
      </thead>
      <tbody>
        {#each entries as e (e.id)}
          <tr class:bad={e.status === "failed"}>
            <td class="ts">{fmtDateTime(e.createdAt)}</td>
            <td>
              {#if e.status === "success"}
                <span class="ok">✓ versendet</span>
              {:else}
                <span class="fail">✗ fehlgeschlagen</span>
              {/if}
            </td>
            <td>
              {kindLabel(e.relatedKind)}
              {#if e.relatedNumber}<span class="muted">· {e.relatedNumber}</span>{/if}
            </td>
            <td>{e.toEmail}</td>
            <td class="subj">{e.subject}</td>
            <td>{channelLabel(e.channel)}</td>
            <td class="resp">
              {#if e.status === "success"}
                {#if e.providerCode}<span class="code">{e.providerCode}</span>{/if}
                {#if e.providerMessage}<span class="msg">{e.providerMessage}</span>{/if}
                {#if e.requestId}<span class="rid">request-id: {e.requestId}</span>{/if}
              {:else}
                <span class="errtext">{e.error ?? "—"}</span>
              {/if}
            </td>
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
  .subj { max-width: 18rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .resp { font-size: 0.8rem; color: #4b5563; }
  .resp .code { display: inline-block; font-weight: 600; margin-right: 0.4rem; font-variant-numeric: tabular-nums; }
  .resp .msg { display: block; }
  .resp .rid { display: block; font-family: ui-monospace, monospace; font-size: 0.72rem; color: #9ca3af; }
  .resp .errtext { color: #b91c1c; }
</style>
