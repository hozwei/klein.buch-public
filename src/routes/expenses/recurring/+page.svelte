<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    recurringList,
    recurringSetActive,
    recurringRunNow,
    recurringRunDueCheck,
  } from "$lib/api";
  import type { RecurringSubscription } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { expenseCategoryLabel, frequencyLabel } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";

  let subs = $state<RecurringSubscription[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let busyId = $state<string | null>(null);
  let checking = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      subs = await recurringList(includeInactive);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Tage bis zur Fälligkeit (negativ = überfällig). Lokale Mitternacht.
  function daysUntil(iso: string): number {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const d = new Date(iso.slice(0, 10) + "T00:00:00");
    return Math.round((d.getTime() - today.getTime()) / 86_400_000);
  }

  function isDue(s: RecurringSubscription): boolean {
    return s.active === 1 && daysUntil(s.nextDueDate) <= 0;
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

  // Relatives Fälligkeits-Label für aktive Abos.
  function dueBadge(s: RecurringSubscription): { label: string; tone: Tone } | null {
    if (s.active !== 1) return null;
    const n = daysUntil(s.nextDueDate);
    if (n < 0) return { label: "überfällig", tone: "danger" };
    if (n === 0) return { label: "heute fällig", tone: "warning" };
    if (n <= 7) return { label: `in ${n} ${n === 1 ? "Tag" : "Tagen"}`, tone: "info" };
    return null;
  }

  function rhythm(s: RecurringSubscription): string {
    return `${frequencyLabel(s.frequency)} · zum ${s.dayOfPeriod}.`;
  }

  let dueCount = $derived(subs.filter(isDue).length);
  let autoDueCount = $derived(subs.filter((s) => isDue(s) && s.autoCreateExpense === 1).length);
  let activeCount = $derived(subs.filter((s) => s.active === 1).length);

  async function toggleActive(s: RecurringSubscription) {
    busyId = s.id;
    try {
      await recurringSetActive(s.id, s.active !== 1);
      await load();
    } catch (e) {
      flash("Statuswechsel fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  async function bookNow(s: RecurringSubscription) {
    busyId = s.id;
    try {
      const expense = await recurringRunNow(s.id);
      flash("Als Kosten gebucht: " + expense.expenseNumber);
      await goto(`/expenses/${expense.id}`);
    } catch (e) {
      flash("Buchen fehlgeschlagen: " + String(e), "error");
      busyId = null;
    }
  }

  async function bookAllDue() {
    checking = true;
    try {
      const r = await recurringRunDueCheck();
      if (r.skippedLocked) {
        flash("Übersprungen: Backup ist gesperrt — zuerst entsperren.", "error");
      } else if (r.createdExpenses === 0) {
        flash("Keine fälligen Auto-Abos.");
      } else {
        flash(`${r.createdExpenses} Kosten aus ${r.processedSubscriptions} Abo(s) gebucht.`);
      }
      await load();
    } catch (e) {
      flash("Buchen fehlgeschlagen: " + String(e), "error");
    } finally {
      checking = false;
    }
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title="Wiederkehrende Abos">
  {#snippet actions()}
    <Button variant="primary" href="/expenses/recurring/new">+ Neues Abo</Button>
  {/snippet}
</PageBar>

<p class="sub">
  Vorlagen für regelmäßige Kosten — Miete, Software, Versicherungen.
  {#if !loading && subs.length > 0}
    <span class="counts">{activeCount} aktiv{#if dueCount > 0} · <strong class="due-txt">{dueCount} fällig</strong>{/if}</span>
  {/if}
</p>

<p class="legend">
  <Badge tone="info">Automatisch</Badge> = wird am Stichtag selbst als Kosten gebucht.
  <Badge tone="primary">Erinnerung</Badge> = erscheint nur als „fällig", du buchst per Knopf.
</p>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if subs.length === 0}
  <div class="empty">
    <p>Noch keine Abos angelegt.</p>
    <Button variant="primary" href="/expenses/recurring/new">+ Erstes Abo anlegen</Button>
  </div>
{:else}
  <div class="toolbar">
    <label class="chk">
      <input type="checkbox" bind:checked={includeInactive} onchange={load} /> pausierte anzeigen
    </label>
    {#if autoDueCount > 0}
      <Button variant="secondary" size="sm" onclick={bookAllDue} disabled={checking}
        title="Bucht jetzt alle fälligen Auto-Abos (sonst erledigt das der Hintergrund-Scheduler von selbst)">
        {checking ? "Buche …" : `${autoDueCount} fällige Auto-Abos jetzt buchen`}
      </Button>
    {/if}
  </div>

  <table class="kb-table">
    <thead>
      <tr>
        <th>Abo</th>
        <th>Rhythmus</th>
        <th>Nächste Fälligkeit</th>
        <th class="num">Betrag</th>
        <th>Modus</th>
        <th class="actions-col"></th>
      </tr>
    </thead>
    <tbody>
      {#each subs as s (s.id)}
        {@const due = dueBadge(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td>
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {expenseCategoryLabel(s.category)}
              {#if s.reverseCharge13bDefault === 1}<Badge tone="primary">§13b</Badge>{/if}
              {#if s.active !== 1}<Badge tone="neutral">pausiert</Badge>{/if}
            </div>
          </td>
          <td class="muted-cell">{rhythm(s)}</td>
          <td>
            {#if s.active === 1}
              <span class="due-cell">
                {date(s.nextDueDate)}
                {#if due}<Badge tone={due.tone}>{due.label}</Badge>{/if}
              </span>
            {:else}
              <span class="muted-cell">—</span>
            {/if}
          </td>
          <td class="num">{euro(s.expectedAmountCents)}</td>
          <td>
            {#if s.autoCreateExpense === 1}
              <Badge tone="info">Automatisch</Badge>
            {:else}
              <Badge tone="primary">Erinnerung</Badge>
            {/if}
          </td>
          <td class="actions">
            <div class="act-row">
              {#if s.active === 1}
                {#if isDue(s)}
                  <Button variant="primary" size="sm" onclick={() => bookNow(s)} disabled={busyId === s.id}
                    title="Eine Kostenposition für den fälligen Stichtag jetzt buchen">Buchen</Button>
                {/if}
                <Button variant="secondary" size="sm" href={`/expenses/recurring/new?id=${s.id}`}>Bearbeiten</Button>
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id}>Pausieren</Button>
              {:else}
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id}>Fortsetzen</Button>
                <Button variant="secondary" size="sm" href={`/expenses/recurring/new?id=${s.id}`}>Bearbeiten</Button>
              {/if}
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .sub { margin: 0 0 0.5rem; color: var(--c-text-muted); font-size: 0.9rem; }
  .counts { margin-left: 0.25rem; }
  .due-txt { color: var(--c-warning-700); }
  .legend { font-size: 0.82rem; color: var(--c-text-muted); background: var(--c-surface-2); border: 1px solid var(--c-border); border-radius: var(--r-md); padding: 0.5rem 0.75rem; margin: 0.9rem 0; display: flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; }
  .toolbar { display: flex; align-items: center; gap: 1rem; margin: 0.5rem 0 0.75rem; flex-wrap: wrap; }
  .chk { display: inline-flex; align-items: center; gap: 0.35rem; font-size: 0.85rem; color: var(--c-text-muted); }
  table { width: 100%; }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  .actions-col { text-align: right; }
  .abo-name { font-weight: 600; color: var(--c-text); }
  .abo-meta { font-size: 0.78rem; color: var(--c-text-muted); margin-top: 0.15rem; display: flex; align-items: center; gap: 0.3rem; flex-wrap: wrap; }
  .due-cell { display: inline-flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .muted-cell { color: var(--c-text-muted); }
  tr.paused { opacity: 0.6; }
  tr.is-due td { background: var(--c-warning-50); }
  td.actions { text-align: right; white-space: nowrap; }
  .act-row { display: inline-flex; gap: 0.4rem; justify-content: flex-end; flex-wrap: wrap; }
  .empty { text-align: center; padding: 2.5rem 1rem; color: var(--c-text-muted); background: var(--c-surface); border: 1px dashed var(--c-border-strong); border-radius: var(--r-lg); }
  .empty p { margin: 0 0 1rem; }
  .muted { color: var(--c-text-muted); }
</style>
