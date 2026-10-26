<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    recurringInvoicesList,
    recurringInvoicesSetActive,
    recurringInvoicesRunNow,
    recurringInvoicesRunDueCheck,
    recurringInvoicesResolveBacklog,
    recurringRestoreReviewList,
    contactsList,
  } from "#lib/api.ts";
  import { askBacklogChoice, withinTerm } from "#lib/recurringChoice.ts";
  import type { RecurringInvoiceRow, Contact, ResumeMode } from "#lib/types.ts";
  import { date } from "#lib/format.ts";
  import { daysUntilBerlin } from "#lib/date.ts";
  import { frequencyLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";

  let rows = $state<RecurringInvoiceRow[]>([]);
  let contactNames = $state<Record<string, string>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let busyId = $state<string | null>(null);
  let checking = $state(false);
  // Vorlagen, die nach einer Wiederherstellung auf „Rückstand …“ warten (F1d).
  let reviewIds = $state<Set<string>>(new Set());

  async function load() {
    loading = true;
    error = null;
    try {
      rows = await recurringInvoicesList(includeInactive);
      reviewIds = new Set((await recurringRestoreReviewList()).templates);
      if (Object.keys(contactNames).length === 0) {
        const cs = await contactsList(true);
        contactNames = Object.fromEntries(cs.map((c: Contact) => [c.id, c.name]));
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Das Öffnen der Seite löst keinen Lauf aus (W14-030, Lauf F): fällige
  // Rechnungen erstellt der Scheduler-Tick; „fällige jetzt erstellen“ ist ein
  // ausdrücklicher Knopf.
  onMount(load);

  // Tage bis zur Fälligkeit (negativ = überfällig) ab dem Berliner Datum,
  // nicht in der Zeitzone des Geräts (Lauf F, D-15).
  function daysUntil(iso: string): number {
    return daysUntilBerlin(iso);
  }

  // Fällig nur innerhalb der Laufzeit (F1d, wie `has_backlog` im Server).
  function isDue(s: RecurringInvoiceRow): boolean {
    return s.active === 1 && withinTerm(s) && daysUntil(s.nextDueDate) <= 0;
  }

  // Rückstand: verpasste Termine oder offene Prüfung nach einer Wiederherstellung.
  function hasBacklog(s: RecurringInvoiceRow): boolean {
    return withinTerm(s) && (daysUntil(s.nextDueDate) < 0 || (reviewIds.has(s.id) && daysUntil(s.nextDueDate) <= 0));
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

  function dueBadge(s: RecurringInvoiceRow): { label: string; tone: Tone } | null {
    if (s.active !== 1) return null;
    if (!withinTerm(s)) return { label: "beendet", tone: "neutral" };
    const n = daysUntil(s.nextDueDate);
    if (n < 0) return { label: "überfällig", tone: "danger" };
    if (n === 0) return { label: "heute fällig", tone: "warning" };
    if (n <= 7) return { label: `in ${n} ${n === 1 ? "Tag" : "Tagen"}`, tone: "info" };
    return null;
  }

  function rhythm(s: RecurringInvoiceRow): string {
    return `${frequencyLabel(s.frequency)} · zum ${s.dayOfPeriod}.`;
  }

  function modeBadge(s: RecurringInvoiceRow): { label: string; tone: Tone } {
    switch (s.autoMode) {
      case "issue":
        return { label: "Auto-Rechnung", tone: "info" };
      case "issue_send":
        return { label: "Auto + Versand", tone: "info" };
      default:
        return { label: "Entwurf", tone: "primary" };
    }
  }

  let dueCount = $derived(rows.filter(isDue).length);
  let activeCount = $derived(rows.filter((s) => s.active === 1).length);

  async function toggleActive(s: RecurringInvoiceRow) {
    // KB-0057: Das Pausieren bestätigen lassen — es stoppt die automatische
    // Rechnungserzeugung. Beim Fortsetzen mit verpassten Terminen verlangt der
    // Server die Wahl „nachholen“ oder „ab heute fortsetzen“ (W14-026).
    let resumeMode: ResumeMode | undefined;
    if (s.active !== 1 && hasBacklog(s)) {
      const choice = await askBacklogChoice("invoice", s.label, s.nextDueDate, reviewIds.has(s.id));
      if (!choice) return;
      resumeMode = choice;
    }
    if (s.active === 1) {
      const ok = await confirmDialog({
        title: "Vorlage pausieren?",
        body:
          `„${s.label}“ erzeugt dann keine Rechnungen mehr, bis du sie wieder fortsetzt. ` +
          "Bereits erstellte Rechnungen bleiben unverändert.",
        confirmLabel: "Pausieren",
        cancelLabel: "Abbrechen",
      });
      if (!ok) return;
    }
    busyId = s.id;
    try {
      await recurringInvoicesSetActive(s.id, s.active !== 1, resumeMode);
      await load();
    } catch (e) {
      flash("Statuswechsel fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  async function createNow(s: RecurringInvoiceRow) {
    busyId = s.id;
    try {
      const invoiceId = await recurringInvoicesRunNow(s.id);
      flash("Rechnung erstellt.");
      await goto(`/invoices/${invoiceId}`);
    } catch (e) {
      flash("Erstellen fehlgeschlagen: " + String(e), "error");
      busyId = null;
    }
  }

  // Rückstand einer aktiven Vorlage (Hinweis „Entscheidung nötig“, W14-026).
  async function resolveBacklog(s: RecurringInvoiceRow) {
    const mode = await askBacklogChoice("invoice", s.label, s.nextDueDate, reviewIds.has(s.id));
    if (!mode) return;
    busyId = s.id;
    try {
      await recurringInvoicesResolveBacklog(s.id, mode);
      flash(
        mode === "catchUp"
          ? "Nachholen bestätigt — der nächste Lauf erstellt die offenen Termine."
          : "Verpasste Termine ausgelassen.",
      );
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busyId = null;
    }
  }

  async function createAllDue() {
    checking = true;
    try {
      const r = await recurringInvoicesRunDueCheck();
      if (r.skippedLocked) {
        flash("Übersprungen: Backup ist gesperrt — zuerst entsperren.", "error");
      } else if (r.skippedHold) {
        flash("Automatische Abo-Läufe sind angehalten — siehe Hinweis oben.", "error");
      } else if (r.createdInvoices === 0 && (r.awaitingDecision ?? 0) > 0) {
        flash("Rückstand über der Nachhol-Grenze — bitte bei der Vorlage „Rückstand …“ wählen.", "error");
      } else if (r.createdInvoices === 0) {
        flash("Keine fälligen Abo-Rechnungen.");
      } else {
        flash(`${r.createdInvoices} Rechnung(en) aus ${r.processedTemplates} Vorlage(n) erstellt.`);
      }
      await load();
    } catch (e) {
      flash("Erstellen fehlgeschlagen: " + String(e), "error");
    } finally {
      checking = false;
    }
  }
</script>

<PageBar back="/invoices" backLabel="Rechnungen" title="Wiederkehrende Rechnungen">
  {#snippet actions()}
    <Button variant="primary" href="/recurring-invoices/new">+ Neue Vorlage</Button>
  {/snippet}
</PageBar>

<p class="sub">
  Vorlagen für regelmäßige Rechnungen an Kunden — z. B. monatliche Server-Wartung.
  {#if !loading && rows.length > 0}
    <span class="counts">{activeCount} aktiv{#if dueCount > 0}{" · "}<strong class="due-txt">{dueCount} fällig</strong>{/if}</span>
  {/if}
</p>

<p class="legend">
  <Badge tone="primary">Entwurf</Badge> = am Stichtag wird ein Rechnungs-Entwurf vorbereitet (du gibst frei).
  <Badge tone="info">Auto-Rechnung</Badge> = wird automatisch erstellt + festgeschrieben.
</p>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if rows.length === 0}
  <div class="empty">
    <p>Noch keine Vorlagen angelegt.</p>
    <Button variant="primary" href="/recurring-invoices/new">+ Erste Vorlage anlegen</Button>
  </div>
{:else}
  <div class="toolbar">
    <label class="chk">
      <input type="checkbox" bind:checked={includeInactive} onchange={load} /> pausierte anzeigen
    </label>
    {#if dueCount > 0}
      <!-- U2 (Touch ohne Hover): Erklärung sichtbar statt nur als `title`-Tooltip,
           wie bei den Kosten-Abos. -->
      <Button variant="secondary" size="sm" onclick={createAllDue} disabled={checking}
        ariaDescribedby="ri-createall-hint">
        {checking ? "Erstelle …" : `${dueCount} fällige jetzt erstellen`}
      </Button>
    {/if}
  </div>
  {#if dueCount > 0}
    <p id="ri-createall-hint" class="toolbar-hint">Sonst erstellt der Hintergrund-Lauf fällige Abo-Rechnungen von selbst.</p>
  {/if}

  <table class="kb-table cards kb-card-grid">
    <caption class="kb-visually-hidden">Vorlagen für wiederkehrende Rechnungen</caption>
    <thead>
      <tr>
        <th>Vorlage</th>
        <th>Rhythmus</th>
        <th>Nächste Fälligkeit</th>
        <th>Modus</th>
        <th class="actions-col"><span class="kb-visually-hidden">Aktionen</span></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as s (s.id)}
        {@const due = dueBadge(s)}
        {@const mode = modeBadge(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td class="cards-full" data-label="Vorlage">
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {contactNames[s.contactId] ?? "—"}
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
          <td data-label="Modus"><Badge tone={mode.tone}>{mode.label}</Badge></td>
          <td class="actions">
            <div class="act-row">
              {#if s.active === 1}
                {#if isDue(s)}
                  <Button variant="primary" size="sm" onclick={() => createNow(s)} disabled={busyId === s.id}
                    title="Rechnung für den fälligen Stichtag jetzt erstellen" ariaLabel={`Rechnung für „${s.label}“ jetzt erstellen`}>Jetzt erstellen</Button>
                {/if}
                {#if hasBacklog(s)}
                  <Button variant="secondary" size="sm" onclick={() => resolveBacklog(s)} disabled={busyId === s.id}
                    title="Verpasste Termine nachholen oder ab heute fortsetzen" ariaLabel={`Rückstand von „${s.label}“ klären`}>Rückstand …</Button>
                {/if}
                <Button variant="secondary" size="sm" href={`/recurring-invoices/new?id=${s.id}`} ariaLabel={`„${s.label}“ bearbeiten`}>Bearbeiten</Button>
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id} ariaLabel={`„${s.label}“ pausieren`}>Pausieren</Button>
              {:else}
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id} ariaLabel={`„${s.label}“ fortsetzen`}>Fortsetzen</Button>
                <Button variant="secondary" size="sm" href={`/recurring-invoices/new?id=${s.id}`} ariaLabel={`„${s.label}“ bearbeiten`}>Bearbeiten</Button>
              {/if}
            </div>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .sub { margin: 0 0 0.5rem; color: var(--text-muted); font-size: 0.9rem; }
  .counts { margin-left: 0.25rem; }
  .due-txt { color: var(--status-warning-fg); }
  .legend { font-size: 0.82rem; color: var(--text-muted); background: var(--surface-sunken); border: 1px solid var(--line); border-radius: var(--r-md); padding: 0.5rem 0.75rem; margin: 0.9rem 0; display: flex; align-items: center; gap: 0.35rem; flex-wrap: wrap; }
  .toolbar { display: flex; align-items: center; gap: 1rem; margin: 0.5rem 0 0.75rem; flex-wrap: wrap; }
  .toolbar-hint { font-size: var(--fs-sm); color: var(--text-muted); margin: -0.25rem 0 0.75rem; }
  .chk { display: inline-flex; align-items: center; gap: 0.35rem; min-height: var(--target); font-size: var(--fs-sm); color: var(--text-muted); }
  table { width: 100%; }
  .actions-col { text-align: right; }
  /* U2: nicht mitten im Wort trennen („Betriebshaftpfli/cht“), Silbentrennung (lang="de"). */
  .abo-name { font-weight: 600; color: var(--text); overflow-wrap: break-word; hyphens: auto; }
  .abo-meta { font-size: 0.78rem; color: var(--text-muted); margin-top: 0.15rem; display: flex; align-items: center; gap: 0.3rem; flex-wrap: wrap; }
  .due-cell { display: inline-flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .muted-cell { color: var(--text-muted); }
  /* Pausiert: gedämpft über die Textfarbe statt Deckkraft (Kontrast 1.4.3). */
  tr.paused td { color: var(--text-muted); background: var(--surface-sunken); }
  tr.is-due td { background: var(--status-warning-bg); }
  td.actions { text-align: right; white-space: nowrap; }
  .act-row { display: inline-flex; gap: 0.4rem; justify-content: flex-end; flex-wrap: wrap; }
  .empty { text-align: center; padding: 2.5rem 1rem; color: var(--text-muted); background: var(--surface-card); border: 1px dashed var(--line-control); border-radius: var(--r-lg); }
  .empty p { margin: 0 0 1rem; }
  .muted { color: var(--text-muted); }
  /* W10: In der Karten-Ansicht dürfen die Aktions-Buttons umbrechen. */
  @media (max-width: 820px) {
    td.actions { white-space: normal; }
    .act-row { min-width: 0; }
  }
</style>
