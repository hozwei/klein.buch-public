<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import {
    recurringInvoicesList,
    recurringInvoicesSetActive,
    recurringInvoicesRunNow,
    recurringInvoicesRunDueCheck,
    contactsList,
  } from "$lib/api";
  import type { RecurringInvoiceRow, Contact } from "$lib/types";
  import { date } from "$lib/format";
  import { frequencyLabel } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import Button from "$lib/Button.svelte";
  import Badge from "$lib/Badge.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";

  let rows = $state<RecurringInvoiceRow[]>([]);
  let contactNames = $state<Record<string, string>>({});
  let loading = $state(true);
  let error = $state<string | null>(null);
  let includeInactive = $state(false);
  let busyId = $state<string | null>(null);
  let checking = $state(false);

  async function load() {
    loading = true;
    error = null;
    try {
      rows = await recurringInvoicesList(includeInactive);
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

  // Beim Öffnen der Seite zusätzlich einen Fälligkeits-Check auslösen: der
  // Scheduler-Tick beim App-Start läuft, solange das Backup noch gesperrt ist,
  // ins Leere — hier ist es entsperrt, also fällige Auto-Rechnungen sofort erstellen.
  async function triggerDueOnOpen() {
    try {
      const r = await recurringInvoicesRunDueCheck();
      if (!r.skippedLocked && r.createdInvoices > 0) {
        flash(`${r.createdInvoices} fällige Abo-Rechnung(en) automatisch erstellt.`);
        await load();
      }
    } catch {
      // Vor dem Entsperren / Bootstrap nicht verfügbar — der Scheduler-Tick holt es nach.
    }
  }

  onMount(async () => {
    await load();
    await triggerDueOnOpen();
  });

  // Tage bis zur Fälligkeit (negativ = überfällig). Lokale Mitternacht.
  function daysUntil(iso: string): number {
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    const d = new Date(iso.slice(0, 10) + "T00:00:00");
    return Math.round((d.getTime() - today.getTime()) / 86_400_000);
  }

  function isDue(s: RecurringInvoiceRow): boolean {
    return s.active === 1 && daysUntil(s.nextDueDate) <= 0;
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

  function dueBadge(s: RecurringInvoiceRow): { label: string; tone: Tone } | null {
    if (s.active !== 1) return null;
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
    // KB-0057: Nur das Pausieren bestätigen lassen — es stoppt die automatische
    // Rechnungserzeugung. Das Fortsetzen ist unkritisch und läuft direkt.
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
      await recurringInvoicesSetActive(s.id, s.active !== 1);
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

  async function createAllDue() {
    checking = true;
    try {
      const r = await recurringInvoicesRunDueCheck();
      if (r.skippedLocked) {
        flash("Übersprungen: Backup ist gesperrt — zuerst entsperren.", "error");
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
    <span class="counts">{activeCount} aktiv{#if dueCount > 0} · <strong class="due-txt">{dueCount} fällig</strong>{/if}</span>
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
      <Button variant="secondary" size="sm" onclick={createAllDue} disabled={checking}
        title="Erstellt jetzt alle fälligen Abo-Rechnungen (sonst erledigt das der Hintergrund-Scheduler)">
        {checking ? "Erstelle …" : `${dueCount} fällige jetzt erstellen`}
      </Button>
    {/if}
  </div>

  <table class="kb-table">
    <thead>
      <tr>
        <th>Vorlage</th>
        <th>Rhythmus</th>
        <th>Nächste Fälligkeit</th>
        <th>Modus</th>
        <th class="actions-col"></th>
      </tr>
    </thead>
    <tbody>
      {#each rows as s (s.id)}
        {@const due = dueBadge(s)}
        {@const mode = modeBadge(s)}
        <tr class:paused={s.active !== 1} class:is-due={isDue(s)}>
          <td>
            <div class="abo-name">{s.label}</div>
            <div class="abo-meta">
              {contactNames[s.contactId] ?? "—"}
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
          <td><Badge tone={mode.tone}>{mode.label}</Badge></td>
          <td class="actions">
            <div class="act-row">
              {#if s.active === 1}
                {#if isDue(s)}
                  <Button variant="primary" size="sm" onclick={() => createNow(s)} disabled={busyId === s.id}
                    title="Rechnung für den fälligen Stichtag jetzt erstellen">Jetzt erstellen</Button>
                {/if}
                <Button variant="secondary" size="sm" href={`/recurring-invoices/new?id=${s.id}`}>Bearbeiten</Button>
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id}>Pausieren</Button>
              {:else}
                <Button variant="secondary" size="sm" onclick={() => toggleActive(s)} disabled={busyId === s.id}>Fortsetzen</Button>
                <Button variant="secondary" size="sm" href={`/recurring-invoices/new?id=${s.id}`}>Bearbeiten</Button>
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
