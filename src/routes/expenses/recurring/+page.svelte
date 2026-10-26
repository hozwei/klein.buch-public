<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    recurringList,
    recurringSetActive,
    recurringRunNow,
    recurringRunDueCheck,
    recurringResolveBacklog,
    recurringRestoreReviewList,
  } from "#lib/api.ts";
  import type { RecurringSubscription, ResumeMode } from "#lib/types.ts";
  import { askBacklogChoice } from "#lib/recurringChoice.ts";
  import { euro, date } from "#lib/format.ts";
  import { daysUntilBerlin } from "#lib/date.ts";
  import { expenseCategoryLabel, frequencyLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  let subs = $state<RecurringSubscription[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let busyId = $state<string | null>(null);
  let checking = $state(false);
  // Abos, die nach einer Wiederherstellung auf „Rückstand …“ warten (F1d).
  let reviewIds = $state<Set<string>>(new Set());

  async function load() {
    loading = true;
    error = null;
    try {
      subs = await recurringList(includeInactive);
      reviewIds = new Set((await recurringRestoreReviewList()).subscriptions);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  // Tage bis zur Fälligkeit (negativ = überfällig) ab dem Berliner Datum,
  // nicht in der Zeitzone des Geräts (Lauf F, D-15).
  function daysUntil(iso: string): number {
    return daysUntilBerlin(iso);
  }

  function isDue(s: RecurringSubscription): boolean {
    return s.active === 1 && daysUntil(s.nextDueDate) <= 0;
  }

  // Rückstand: verpasste Termine oder offene Prüfung nach einer Wiederherstellung (F1d).
  function hasBacklog(s: RecurringSubscription): boolean {
    const n = daysUntil(s.nextDueDate);
    return s.autoCreateExpense === 1 && (n < 0 || (reviewIds.has(s.id) && n <= 0));
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
    // Beim Fortsetzen eines Auto-Abos mit verpassten Terminen verlangt der
    // Server die Wahl „nachholen“ oder „ab heute fortsetzen“ (W14-026).
    let resumeMode: ResumeMode | undefined;
    if (s.active !== 1 && hasBacklog(s)) {
      const choice = await askBacklogChoice("expense", s.label, s.nextDueDate, reviewIds.has(s.id));
      if (!choice) return;
      resumeMode = choice;
    }
    busyId = s.id;
    try {
      await recurringSetActive(s.id, s.active !== 1, resumeMode);
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

  // Rückstand eines aktiven Auto-Abos (Hinweis „Entscheidung nötig“ bzw.
  // Termin im abgeschlossenen Geschäftsjahr, W14-026/-034).
  async function resolveBacklog(s: RecurringSubscription) {
    const mode = await askBacklogChoice("expense", s.label, s.nextDueDate, reviewIds.has(s.id));
    if (!mode) return;
    busyId = s.id;
    try {
      await recurringResolveBacklog(s.id, mode);
      flash(
        mode === "catchUp"
          ? "Nachholen bestätigt — der nächste Lauf bucht die offenen Termine."
          : "Verpasste Termine ausgelassen.",
      );
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  async function bookAllDue() {
    checking = true;
    try {
      const r = await recurringRunDueCheck();
      if (r.skippedLocked) {
        flash("Übersprungen: Backup ist gesperrt — zuerst entsperren.", "error");
      } else if (r.skippedHold) {
        flash("Automatische Abo-Läufe sind angehalten — siehe Hinweis oben.", "error");
      } else if (r.createdExpenses === 0 && (r.awaitingDecision ?? 0) > 0) {
        flash("Rückstand über der Nachhol-Grenze — bitte beim Abo „Rückstand …“ wählen.", "error");
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
    <span class="counts">{activeCount} aktiv{#if dueCount > 0}{" · "}<strong class="due-txt">{dueCount} fällig</strong>{/if}</span>
  {/if}
</p>

<p class="legend">
  <span><Badge tone="info">Automatisch</Badge> wird am Stichtag selbst als Kosten gebucht.</span>
  <span><Badge tone="primary">Erinnerung</Badge> am Stichtag ein Hinweis „Abo fällig“, du buchst per Knopf.</span>
</p>

{#if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
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
        ariaDescribedby="rec-bookall-hint">
        {checking ? "Buche …" : `${autoDueCount} fällige Auto-Abos jetzt buchen`}
      </Button>
    {/if}
  </div>
  {#if autoDueCount > 0}
    <p id="rec-bookall-hint" class="toolbar-hint">Sonst bucht der Hintergrund-Lauf fällige Auto-Abos von selbst.</p>
  {/if}

  <Table caption="Wiederkehrende Abos" cards cardGrid>
    <thead>
      <tr>
        <th>Abo</th>
        <th>Rhythmus</th>
        <th>Nächste Fälligkeit</th>
        <th class="num">Betrag</th>
        <th>Modus</th>
        <th class="actions-col"><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each subs as s (s.id)}
        {@const due = dueBadge(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td class="cards-full" data-label="Abo">
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {expenseCategoryLabel(s.category)}
              {#if s.reverseCharge13bDefault === 1}<Badge tone="primary">§13b</Badge>{/if}
              {#if s.active !== 1}<Badge tone="neutral">pausiert</Badge>{/if}
            </div>
          </td>
          <td class="muted-cell" data-label="Rhythmus">{rhythm(s)}</td>
          <td data-label="Nächste Fälligkeit">
            {#if s.active === 1}
              <span class="due-cell">
                {date(s.nextDueDate)}
                {#if due}<Badge tone={due.tone}>{due.label}</Badge>{/if}
              </span>
            {:else}
              <span class="muted-cell">—</span>
            {/if}
          </td>
          <td class="num kb-num" data-label="Betrag">{euro(s.expectedAmountCents)}</td>
          <td data-label="Modus">
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
                    ariaLabel={`${s.label}: Buchen (fälliger Stichtag)`}>Buchen</Button>
                {/if}
                {#if hasBacklog(s)}
                  <Button variant="secondary" size="sm" onclick={() => resolveBacklog(s)} disabled={busyId === s.id}
                    ariaLabel={`${s.label}: Rückstand … (nachholen oder ab heute fortsetzen)`}>Rückstand …</Button>
                {/if}
                <Button variant="secondary" size="sm" href={`/expenses/recurring/new?id=${s.id}`} ariaLabel={`${s.label}: Bearbeiten`}>Bearbeiten</Button>
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id} ariaLabel={`${s.label}: Pausieren`}>Pausieren</Button>
              {:else}
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id} ariaLabel={`${s.label}: Fortsetzen`}>Fortsetzen</Button>
                <Button variant="secondary" size="sm" href={`/expenses/recurring/new?id=${s.id}`} ariaLabel={`${s.label}: Bearbeiten`}>Bearbeiten</Button>
              {/if}
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </Table>
{/if}

<style>
  .sub { margin: 0 0 var(--sp-2); color: var(--text-muted); }
  .counts { margin-left: var(--sp-1); }
  .due-txt { color: var(--status-warning-fg); }
  .legend { font-size: var(--fs-sm); color: var(--text-muted); background: var(--surface-sunken); border: 1px solid var(--line); border-radius: var(--r-md); padding: var(--sp-2) var(--sp-3); margin: var(--sp-3) 0; display: flex; flex-direction: column; gap: var(--sp-1); }
  .legend span { display: flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; }
  .toolbar { display: flex; align-items: center; gap: var(--sp-4); margin: var(--sp-2) 0 var(--sp-1); flex-wrap: wrap; }
  .toolbar-hint { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .chk { display: inline-flex; align-items: center; gap: var(--sp-2); min-height: var(--target); color: var(--text-muted); }
  .num { text-align: right; }
  td.num { text-align: right; font-variant-numeric: tabular-nums; }
  .actions-col { text-align: right; }
  /* U2: nicht mitten im Wort trennen („Betriebshaftpfli/cht“), Silbentrennung (lang="de"). */
  .abo-name { font-weight: 600; color: var(--text); overflow-wrap: break-word; hyphens: auto; }
  .abo-meta { font-size: var(--fs-sm); color: var(--text-muted); margin-top: 0.15rem; display: flex; align-items: center; gap: var(--sp-1); flex-wrap: wrap; }
  .due-cell { display: inline-flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; }
  .muted-cell { color: var(--text-muted); }
  /* Pausiert: Badge „pausiert“ trägt den Zustand; keine Deckkraft-Absenkung (Kontrast 1.4.3). */
  tr.paused .abo-name { font-weight: 500; }
  tr.is-due td { background: var(--status-warning-bg); }
  td.actions { text-align: right; }
  .act-row { display: inline-flex; gap: var(--sp-2); justify-content: flex-end; flex-wrap: wrap; }
  .empty { text-align: center; padding: 2.5rem 1rem; color: var(--text-muted); background: var(--surface-card); border: 1px dashed var(--line-control); border-radius: var(--r-lg); }
  .empty p { margin: 0 0 1rem; }
  .muted { color: var(--text-muted); }
</style>
