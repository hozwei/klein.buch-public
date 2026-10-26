<script lang="ts">
  import {
    fiscalYearOverview,
    fiscalYearClose,
    fiscalYearAutoCloseSet,
  } from "#lib/api.ts";
  import type { FiscalYearOverview, FiscalYearStatus } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Badge from "#lib/Badge.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Toggle from "#lib/Toggle.svelte";

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
    // Übergabe F2: vor offenen Entwürfen des Jahres warnen (der Server meldet sie
    // in `openDrafts`; der Abschluss bleibt möglich).
    const drafts = y.openDrafts ?? { invoices: 0, quotes: 0 };
    if (drafts.invoices > 0 || drafts.quotes > 0) {
      const parts: string[] = [];
      if (drafts.invoices > 0) parts.push(`${drafts.invoices} ${drafts.invoices === 1 ? "Rechnungs-Entwurf" : "Rechnungs-Entwürfe"}`);
      if (drafts.quotes > 0) parts.push(`${drafts.quotes} ${drafts.quotes === 1 ? "Angebots-Entwurf" : "Angebots-Entwürfe"}`);
      bullets.push(
        `Achtung: ${parts.join(" und ")} mit Datum in ${y.fiscalYear} noch offen. Prüfe sie vorher — nach dem Abschluss lässt sich in diesem Jahr nichts mehr festschreiben.`,
      );
    }
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
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if ov}
  <div class="auto">
    <Toggle
      bind:checked={ov.autoYearClose}
      label="Abschreibungen (AfA) zur Jahreswende automatisch buchen"
      description="Empfohlen. Du prüfst sie und schließt das Jahr danach selbst ab."
      onchange={toggleAuto}
    />
  </div>

  {#if ov.years.some((y) => y.snapshotMatches === false)}
    <Banner kind="warning">
      Bei mindestens einem abgeschlossenen Jahr weichen die heutigen Zahlen vom Stand beim
      Abschluss ab (markiert mit „Abweichung“). Details zeigt der
      <a class="inline-link" href="/euer/export">EÜR-Export</a> des Jahres.
    </Banner>
  {/if}

  <section class="card" aria-labelledby="fy-h">
    <h2 id="fy-h">Übersicht</h2>
    {#if ov.years.length === 0}
      <p class="muted">
        Noch keine Geschäftsjahre mit Daten. Sobald du eine Rechnung oder Kosten erfasst,
        erscheint das Jahr hier.
      </p>
    {:else}
      <table class="cards">
        <caption class="kb-visually-hidden">Geschäftsjahre mit Status und Ergebnis</caption>
        <thead>
          <tr>
            <th>Jahr</th>
            <th>Status</th>
            <th class="num">Einnahmen</th>
            <th class="num">Ausgaben</th>
            <th class="num">Ergebnis</th>
            <th><span class="kb-visually-hidden">Aktion</span></th>
          </tr>
        </thead>
        <tbody>
          {#each ov.years as y (y.fiscalYear)}
            {@const drafts = (y.openDrafts?.invoices ?? 0) + (y.openDrafts?.quotes ?? 0)}
            <tr>
              <td data-label="Jahr"><strong>{y.fiscalYear}</strong></td>
              <td data-label="Status">
                <span class="status-cell">
                  {#if y.closed}
                    <Badge tone="primary">Abgeschlossen</Badge>
                    <span class="muted small">am {date(y.closedAt)}</span>
                    {#if y.snapshotMatches === false}<Badge tone="warning">Abweichung</Badge>{/if}
                  {:else}
                    <Badge tone="neutral">Offen</Badge>
                    {#if drafts > 0}<Badge tone="warning">{drafts} {drafts === 1 ? "Entwurf" : "Entwürfe"} offen</Badge>{/if}
                  {/if}
                </span>
              </td>
              <td class="num" data-label="Einnahmen">{euro(y.incomeTotalCents)}</td>
              <td class="num" data-label="Ausgaben">{euro(y.expenseTotalCents)}</td>
              <td class="num" class:loss={y.surplusCents < 0} data-label="Ergebnis">{euro(y.surplusCents)}</td>
              <td class="act" data-label="Abschluss">
                {#if y.closed}
                  <span class="muted small">festgeschrieben</span>
                {:else if y.closable}
                  {#if y.afaPending > 0}
                    <span class="warn small">{y.afaPending} AfA offen</span>
                  {/if}
                  <button
                    type="button"
                    class="btn-primary sm"
                    onclick={() => closeYear(y)}
                    disabled={busyYear === y.fiscalYear}
                    aria-label={`Geschäftsjahr ${y.fiscalYear} abschließen`}
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
    {/if}
  </section>

  {#if ov.openReceivables.length > 0}
    <section class="card" aria-labelledby="fy-rec-h">
      <h2 id="fy-rec-h">Offene Forderungen</h2>
      <p class="muted small">
        Diese Rechnungen sind noch nicht (vollständig) bezahlt. Sie bleiben nach
        einem Jahresabschluss bestehen — der Zahlungseingang zählt im Jahr des
        tatsächlichen Geldeingangs.
      </p>
      <table class="cards">
        <caption class="kb-visually-hidden">Offene Forderungen</caption>
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
              <td data-label="Rechnung"><a class="doc-no" href={`/invoices/${r.id}`}>{r.invoiceNumber}</a></td>
              <td data-label="Datum">{date(r.invoiceDate)}</td>
              <td data-label="Fällig">{date(r.dueDate)}</td>
              <td class="num" data-label="Offen">{euro(r.outstandingCents)}</td>
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
  /* .card / .card h2 / .lead: globale Definitionen aus tokens.css. */
  .muted { color: var(--text-muted); }
  .small { font-size: var(--fs-sm); }
  .auto {
    background: var(--status-info-bg);
    border: 1px solid var(--status-info-line);
    border-radius: var(--r-md);
    padding: var(--sp-3) var(--sp-4);
    margin: var(--sp-2) 0 var(--sp-5);
    max-width: 46rem;
  }
  table { width: 100%; border-collapse: collapse; }
  .num { text-align: right; font-variant-numeric: tabular-nums; white-space: nowrap; }
  td.num.loss { color: var(--status-danger-fg); }
  .act { text-align: right; }
  .status-cell { display: inline-flex; flex-wrap: wrap; align-items: center; gap: var(--sp-1) var(--sp-2); }
  .warn { color: var(--status-warning-fg); margin-right: var(--sp-2); }
  /* Rechnungsnummer als Link: Trefferfläche ≥ --target, unterstrichen (1.4.1). */
  a.doc-no {
    display: inline-flex; align-items: center; min-height: var(--target);
    font-family: var(--font-mono); white-space: nowrap; text-decoration: underline;
    text-underline-offset: 2px;
  }
  .inline-link { text-decoration: underline; }
  .disclaimer { max-width: 46rem; line-height: 1.5; font-size: var(--fs-sm); margin-top: var(--sp-4); }
</style>
