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
  import ActionMenu, { type ActionMenuItem } from "#lib/ActionMenu.svelte";
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
    // U4 (V3-09): dieselben Wörter wie bei den Abo-Rechnungen.
    if (n < 0) return { label: "Überfällig", tone: "danger" };
    if (n === 0) return { label: "Heute fällig", tone: "warning" };
    if (n <= 7) return { label: `In ${n} ${n === 1 ? "Tag" : "Tagen"}`, tone: "info" };
    return null;
  }

  function rhythm(s: RecurringSubscription): string {
    return `${frequencyLabel(s.frequency)} · zum ${s.dayOfPeriod}.`;
  }

  // Zeilenaktionen (U3-036, design.md §4.5) wie bei den Abo-Rechnungen: eine
  // sichtbare Aktion — die, die den Zustand der Zeile auflöst — und der Rest im
  // Menü „Mehr“. Bei einem Rückstand „Rückstand …“ (Nachholen bucht auch den
  // fälligen Termin), sonst bei Fälligkeit „Buchen“, sonst „Bearbeiten“;
  // pausiert „Fortsetzen“.
  type RowAction = { label: string; ariaLabel: string; href?: string; onclick?: () => void };
  function rowActions(s: RecurringSubscription): { main: RowAction; more: ActionMenuItem[] } {
    const busy = busyId === s.id;
    const edit: RowAction = { label: "Bearbeiten", ariaLabel: `„${s.label}“ bearbeiten`, href: `/expenses/recurring/new?id=${s.id}` };
    if (s.active !== 1) {
      return {
        main: { label: "Fortsetzen", ariaLabel: `„${s.label}“ fortsetzen`, onclick: () => toggleActive(s) },
        more: [{ label: edit.label, href: edit.href }],
      };
    }
    const book: RowAction = {
      label: "Buchen",
      ariaLabel: `Fälligen Termin von „${s.label}“ buchen`,
      onclick: () => bookNow(s),
    };
    const backlog: RowAction = {
      label: "Rückstand …",
      ariaLabel: `Rückstand von „${s.label}“ klären`,
      onclick: () => resolveBacklog(s),
    };
    const main = hasBacklog(s) ? backlog : isDue(s) ? book : edit;
    const more: ActionMenuItem[] = [];
    if (main !== book && isDue(s)) more.push({ label: book.label, onSelect: book.onclick, disabled: busy });
    if (main !== edit) more.push({ label: edit.label, href: edit.href });
    more.push({ label: "Pausieren", onSelect: () => toggleActive(s), disabled: busy });
    return { main, more };
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
    const pausing = s.active === 1;
    busyId = s.id;
    try {
      await recurringSetActive(s.id, !pausing, resumeMode);
      // U4 (V2-12, design.md §5): Pausieren ist umkehrbar — keine Rückfrage, dafür
      // „Rückgängig“ im Toast (setzt das Abo fort, bei Rückstand mit der Wahl).
      if (pausing) {
        flash(`„${s.label}“ pausiert.`, "ok", undefined, {
          action: { label: "Rückgängig", run: () => void resumeById(s.id) },
        });
      } else {
        flash(`„${s.label}“ fortgesetzt.`);
      }
      await load();
    } catch (e) {
      flash("Statuswechsel fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  async function resumeById(id: string) {
    const cur = subs.find((x) => x.id === id);
    if (cur && cur.active !== 1) await toggleActive(cur);
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
    if (checking) return;
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
  <span><Badge kind="tag">Automatisch</Badge> wird am Stichtag selbst als Kosten gebucht.</span>
  <span><Badge kind="tag">Erinnerung</Badge> am Stichtag ein Hinweis „Abo fällig“, du buchst per Knopf.</span>
</p>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if subs.length === 0}
  <!-- U4 (V2-09): Erststart mit derselben Beschriftung wie die PageBar, als secondary. -->
  <div class="empty">
    <p>Noch keine Abos angelegt.</p>
    <Button variant="secondary" href="/expenses/recurring/new">+ Neues Abo</Button>
  </div>
{:else}
  <div class="toolbar">
    <label class="chk">
      <input type="checkbox" bind:checked={includeInactive} onchange={load} /> Pausierte anzeigen
    </label>
    {#if autoDueCount > 0}
      <Button variant="secondary" size="sm" onclick={bookAllDue} ariaDisabled={checking}
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
        {@const act = rowActions(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td class="cards-full" data-label="Abo">
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {expenseCategoryLabel(s.category)}
              <!-- U4 (V3-08/V3-09): Merkmal ohne Statusfarbe; „Pausiert“ wie bei den Abo-Rechnungen. -->
              {#if s.reverseCharge13bDefault === 1}<Badge kind="tag">§13b</Badge>{/if}
              {#if s.active !== 1}<Badge tone="neutral">Pausiert</Badge>{/if}
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
            <!-- U4 (V3-08): Ein Modus ist kein Status (ADR 0041 Nr. 177). -->
            {#if s.autoCreateExpense === 1}
              <Badge kind="tag">Automatisch</Badge>
            {:else}
              <Badge kind="tag">Erinnerung</Badge>
            {/if}
          </td>
          <td class="actions">
            <div class="kb-actions">
              <Button variant="secondary" href={act.main.href} onclick={act.main.onclick} ariaDisabled={!act.main.href && busyId === s.id} ariaLabel={act.main.ariaLabel}>{act.main.label}</Button>
              <ActionMenu variant="ghost" ariaLabel={`Mehr zu „${s.label}“`} items={act.more} />
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
  td.actions { text-align: right; white-space: nowrap; }
  /* W10: In der Karten-Ansicht dürfen die Aktions-Knöpfe umbrechen. */
  @media screen and (max-width: 820px) {
    td.actions { white-space: normal; }
  }
  .empty { text-align: center; padding: 2.5rem 1rem; color: var(--text-muted); background: var(--surface-card); border: 1px dashed var(--line-control); border-radius: var(--r-lg); }
  .empty p { margin: 0 0 1rem; }
  .muted { color: var(--text-muted); }
</style>
