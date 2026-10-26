<script lang="ts">
  import { onMount } from "svelte";
  import { emailLogSearch } from "#lib/api.ts";
  import type { EmailLogEntry, EmailLogFilter } from "#lib/types.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { dateTime } from "#lib/format.ts";

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


  onMount(load);

  const hasFilter = $derived(
    !!(search.trim() || dateFrom || dateTo || statusF !== "all" || kindF !== "all" || channelF !== "all"),
  );

  function kindLabel(k: string): string {
    return k === "invoice" ? "Rechnung" : k === "quote" ? "Angebot" : k === "test" ? "Test-Mail" : k;
  }
  function channelLabel(c: string): string {
    return c === "graph" ? "Microsoft 365 (früher)" : "SMTP";
  }
</script>

<PageBar back="/settings/mail" backLabel="E-Mail-Versand" title="E-Mail-Protokoll" />

<p class="lead">
  Lückenloser, unveränderlicher Nachweis jedes Versand­versuchs (Erfolg und
  Fehlschlag) mit Antwort des Mailservers (SMTP-Code) und Fehlermeldung. Such- und Filterfelder helfen, auch nach Jahren schnell den
  richtigen Vorgang zu finden.
</p>

<form class="filterbar" role="search" aria-label="E-Mail-Protokoll durchsuchen" onsubmit={(e) => { e.preventDefault(); load(); }}>
  <div class="row">
    <label class="grow">
      Suche
      <input
        type="search"
        bind:value={search}
        enterkeyhint="search"
        autocomplete="off"
        placeholder="z. B. Empfänger, Betreff, Beleg-Nr., Fehler, request-id"
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
        <option value="smtp">SMTP</option>
        <option value="graph">Microsoft 365 (früher)</option>
      </select>
    </label>
    <div class="btns">
      <Button type="submit" disabled={loading}>Suchen</Button>
      <Button variant="secondary" onclick={reset} disabled={loading && !hasFilter}>Zurücksetzen</Button>
    </div>
  </div>
</form>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else}
  <p class="muted count" role="status">
    {entries.length}{entries.length === LIMIT ? "+" : ""} Treffer{hasFilter ? " (gefiltert)" : ""},
    neueste zuerst.
    {#if entries.length === LIMIT}<span> Zeige max. {LIMIT} — Suche eingrenzen für ältere.</span>{/if}
  </p>

  {#if entries.length === 0}
    <p class="muted">
      {#if hasFilter}
        Keine Einträge zu diesem Filter. <button type="button" class="inline-link" onclick={reset}>Filter zurücksetzen</button>
      {:else}
        Noch keine E-Mail versendet. <a class="inline-link" href="/settings/mail">Postfach einrichten und Testmail senden</a>
      {/if}
    </p>
  {:else}
    <table class="cards">
      <caption class="kb-visually-hidden">E-Mail-Protokoll, neueste zuerst</caption>
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
            <td class="ts" data-label="Zeitpunkt">{dateTime(e.createdAt)}</td>
            <td data-label="Status">
              {#if e.status === "success"}
                <Badge tone="success">versendet</Badge>
              {:else}
                <Badge tone="danger">fehlgeschlagen</Badge>
              {/if}
            </td>
            <td data-label="Beleg">
              <span>{kindLabel(e.relatedKind)}
              {#if e.relatedNumber}<span class="muted docno">· {e.relatedNumber}</span>{/if}</span>
            </td>
            <td class="wrap" data-label="Empfänger">{e.toEmail}</td>
            <td class="subj cards-full" data-label="Betreff">{e.subject}</td>
            <td data-label="Kanal">{channelLabel(e.channel)}</td>
            <td class="resp cards-full" data-label="Antwort des Anbieters">
              {#if e.status === "success"}
                {#if e.providerCode}<span class="code">{e.providerCode}</span>{/if}
                {#if e.providerMessage}<span class="msg">{e.providerMessage}</span>{/if}
                {#if e.requestId}<span class="rid">request-id: {e.requestId}</span>{/if}
                {#if !e.providerCode && !e.providerMessage && !e.requestId}<span>—</span>{/if}
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
  .inline-link { text-decoration: underline; color: var(--accent); background: none; border: 0; padding: 0; font: inherit; cursor: pointer; min-height: var(--target); }
  table { width: 100%; border-collapse: collapse; }
  th, td { font-size: var(--fs-sm); text-align: left; vertical-align: top; }
  tr.bad td { background: var(--status-danger-bg); }
  .ts { font-variant-numeric: tabular-nums; }
  @media (min-width: 821px) {
    .ts { white-space: nowrap; }
  }
  .docno { font-family: var(--font-mono); white-space: nowrap; }
  .wrap, .subj { overflow-wrap: anywhere; }
  .subj { max-width: 18rem; }
  .resp { color: var(--text-muted); overflow-wrap: anywhere; }
  .resp .code { display: inline-block; font-weight: 600; margin-right: 0.4rem; font-variant-numeric: tabular-nums; }
  .resp .msg { display: block; }
  .resp .rid { display: block; font-family: var(--font-mono); font-size: var(--fs-xs); color: var(--text-muted); }
  .resp .errtext { color: var(--status-danger-fg); }
  @media (max-width: 820px) {
    .subj { max-width: none; }
  }
</style>
