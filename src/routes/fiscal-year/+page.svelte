<script lang="ts">
  import {
    fiscalYearOverview,
    fiscalYearClose,
    fiscalYearAutoCloseSet,
  } from "$lib/api";
  import type { FiscalYearOverview, FiscalYearStatus } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import Banner from "$lib/Banner.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";

  let ov = $state<FiscalYearOverview | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busyYear = $state<number | null>(null);

  async function load() {
    loading = true;
    error = null;
    try {
      ov = await fiscalYearOverview();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    load();
  });

  async function toggleAuto(next: boolean) {
    if (!ov) return;
    try {
      await fiscalYearAutoCloseSet(next);
      flash(
        next
          ? "Abschreibungen werden zur Jahreswende automatisch gebucht."
          : "Automatische Abschreibungs-Buchung ausgeschaltet.",
      );
    } catch (e) {
      // bind:checked hat ov.autoYearClose optimistisch umgesetzt — revert.
      if (ov) ov.autoYearClose = !next;
      flash(String(e), "error");
    }
  }

  async function closeYear(y: FiscalYearStatus) {
    const bullets = [
      `Einnahmen: ${euro(y.incomeTotalCents)}`,
      `Ausgaben: ${euro(y.expenseTotalCents)}`,
      `darin Abschreibungen (AfA): ${euro(y.afaTotalCents)}`,
      `Ergebnis: ${euro(y.surplusCents)}`,
    ];
    if (y.afaPending > 0) {
      bullets.push(
        `${y.afaPending} Anschaffung(en) ohne gebuchte AfA — wird beim Abschluss automatisch nachgebucht.`,
      );
    }
    const ok = await confirmDialog({
      title: `Geschäftsjahr ${y.fiscalYear} abschließen?`,
      body:
        "Der Abschluss schreibt das Jahr endgültig fest (GoBD, §146 AO). Danach ist " +
        "keine neue Buchung mehr mit Datum in diesem Jahr möglich und die " +
        "Abschreibungen werden gesperrt. Das lässt sich nicht rückgängig machen. " +
        "Eine fehlerhafte Rechnung kannst du weiterhin stornieren.",
      bullets,
      confirmLabel: "Jetzt abschließen",
      cancelLabel: "Abbrechen",
      danger: true,
    });
    if (!ok) return;
    busyYear = y.fiscalYear;
    try {
      const lock = await fiscalYearClose(y.fiscalYear);
      flash(`Geschäftsjahr ${lock.fiscalYear} abgeschlossen und festgeschrieben.`);
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busyYear = null;
    }
  }

  function fmtDateTime(ts: string | null): string {
    if (!ts) return "—";
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
  }
</script>

<PageBar title="Geschäftsjahr">
  {#snippet actions()}
    <HelpAnchor slug="geschaeftsjahr-abschluss" />
  {/snippet}
</PageBar>

<p class="lead">
  Ein abgelaufenes Geschäftsjahr <strong>abschließen</strong> bedeutet: alle Belege
  des Jahres werden endgültig festgeschrieben und können nicht mehr geändert werden
  — so verlangt es das Finanzamt (GoBD, §&nbsp;146&nbsp;AO). Eine fehlerhafte
  Rechnung lässt sich danach nur noch <strong>stornieren</strong>, nicht löschen.
</p>

{#if error}
  <Banner>{error}</Banner>
{:else if loading}
  <p class="muted">Lade …</p>
{:else if ov}
  <div class="auto">
    <Toggle
      bind:checked={ov.autoYearClose}
      label="Abschreibungen (AfA) zur Jahreswende automatisch buchen"
      description="Empfohlen. Du prüfst sie und schließt das Jahr danach selbst ab."
      onchange={toggleAuto}
    />
  </div>

  <section class="card">
    <h2>Übersicht</h2>
    <table>
      <thead>
        <tr>
          <th>Jahr</th>
          <th>Status</th>
          <th class="num">Einnahmen</th>
          <th class="num">Ausgaben</th>
          <th class="num">Ergebnis</th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#if ov.years.length === 0}
          <tr><td colspan="6" class="muted">Noch keine Geschäftsjahre mit Daten.</td></tr>
        {/if}
        {#each ov.years as y (y.fiscalYear)}
          <tr>
            <td><strong>{y.fiscalYear}</strong></td>
            <td>
              {#if y.closed}
                <span class="badge closed">Abgeschlossen</span>
                <span class="muted small">{fmtDateTime(y.closedAt)}</span>
              {:else}
                <span class="badge open">Offen</span>
              {/if}
            </td>
            <td class="num">{euro(y.incomeTotalCents)}</td>
            <td class="num">{euro(y.expenseTotalCents)}</td>
            <td class="num" class:loss={y.surplusCents < 0}>{euro(y.surplusCents)}</td>
            <td class="act">
              {#if y.closed}
                <span class="muted small">fest</span>
              {:else if y.closable}
                {#if y.afaPending > 0}
                  <span class="warn small">{y.afaPending} AfA offen</span>
                {/if}
                <button
                  class="btn-primary sm"
                  onclick={() => closeYear(y)}
                  disabled={busyYear === y.fiscalYear}
                >
                  {busyYear === y.fiscalYear ? "Schließe …" : "Abschließen"}
                </button>
              {:else}
                <span class="muted small">läuft noch</span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>

  {#if ov.openReceivables.length > 0}
    <section class="card">
      <h2>Offene Forderungen</h2>
      <p class="muted small">
        Diese Rechnungen sind noch nicht (vollständig) bezahlt. Sie bleiben nach
        einem Jahresabschluss bestehen — der Zahlungseingang zählt im Jahr des
        tatsächlichen Geldeingangs.
      </p>
      <table>
        <thead>
          <tr>
            <th>Rechnung</th>
            <th>Datum</th>
            <th>Fällig</th>
            <th class="num">Offen</th>
          </tr>
        </thead>
        <tbody>
          {#each ov.openReceivables as r (r.id)}
            <tr>
              <td><a href={`/invoices/${r.id}`}>{r.invoiceNumber}</a></td>
              <td>{date(r.invoiceDate)}</td>
              <td>{date(r.dueDate)}</td>
              <td class="num">{euro(r.outstandingCents)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </section>
  {/if}

  <p class="muted disclaimer">
    Klein.Buch ist ein Werkzeug, kein Steuerberater. Schließe ein Jahr erst ab,
    wenn alle Belege erfasst sind. Der Abschluss erzeugt automatisch eine
    verschlüsselte Sicherung.
  </p>
{/if}

<style>
  /* .intro / .card / .card h2 entfernt — globale .lead, .card aus tokens.css. */
  .muted {
    color: var(--c-text-muted);
  }
  .small {
    font-size: 0.78rem;
  }
  .auto {
    background: var(--c-info-50);
    border: 1px solid rgba(23, 107, 135, 0.2);
    border-radius: var(--r-md);
    padding: 0.75rem 1rem;
    margin: 0.5rem 0 1.25rem;
    max-width: 46rem;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 0.45rem 0.5rem;
    border-bottom: 1px solid #f3f4f6;
    font-size: 0.9rem;
    text-align: left;
  }
  th {
    font-size: 0.78rem;
    color: #6b7280;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .num {
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  td.num.loss {
    color: #b91c1c;
  }
  .act {
    text-align: right;
    white-space: nowrap;
  }
  .badge {
    font-size: 0.72rem;
    font-weight: 700;
    padding: 0.12rem 0.45rem;
    border-radius: 4px;
  }
  .badge.open {
    /* R5-012: Tailwind-Blau (#eff6ff / #1d4ed8) durch Petrol-Skala ersetzt
       (Memory `feedback_design_direction`). */
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }
  .badge.closed {
    background: #f3f4f6;
    color: #4b5563;
  }
  .warn {
    color: #b45309;
    margin-right: 0.5rem;
  }
  a {
    color: #2563eb;
    text-decoration: none;
  }
  /* Lokale btn-Defs entfernt — globale aus tokens.css greifen
     (Manuel-Hardline 2026-05-26: alle Buttons app-weit gleich groß). */
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .disclaimer {
    max-width: 46rem;
    line-height: 1.5;
    font-size: 0.8rem;
    margin-top: 1rem;
  }
</style>
