<script lang="ts">
  // Übersicht der Vorsteuerberichtigung nach § 15a UStG (ADR 0042 Nr. 14).
  //
  // Liest nur (`vat_adjustment_overview`, `assets_list` für die Jahresauswahl);
  // gebucht und geändert wird nichts. Ziel des Inbox-Hinweises nach einem
  // Statuswechsel (`commands::vat_adjustment::OVERVIEW_URL`). Gerechnet wird im
  // Server (`domain::vat_adjustment`), Texte in `../vatAdjustment.ts`.
  import { untrack } from "svelte";
  import { page } from "$app/state";
  import { yearBerlin } from "#lib/date.ts";
  import { assetsList, vatAdjustmentOverview } from "#lib/api.ts";
  import type { VatAdjustmentOverview } from "#lib/types.ts";
  import { euro } from "#lib/format.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Table from "#lib/Table.svelte";
  import {
    directionLabel,
    exemptionLabel,
    filingLabel,
    monthsLabel,
    originalStatusLabel,
    periodText,
    shareText,
    splitByChange,
    totalText,
    unknownReasonLabel,
    vatYearOptions,
    yearOutcome,
  } from "../vatAdjustment.ts";

  const currentYear = yearBerlin();
  // `?year=2027` wählt ein Jahr vor (z. B. aus einem Link); sonst das laufende.
  function initialYear(): number {
    const y = Number(page.url.searchParams.get("year"));
    return Number.isInteger(y) && y >= 2000 && y <= 2200 ? y : currentYear;
  }
  let selectedYear = $state<number>(initialYear());
  let acquisitionYears = $state<number[]>([]);
  let overview = $state<VatAdjustmentOverview | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let reloadTick = $state(0);
  // UI-12: Nach einem Jahreswechsel bzw. erneuten Laden die neue Übersicht ansagen
  // (die erste Ladung zeigt LoadingState, dort braucht es keine Ansage).
  let reloaded = $state(false);

  // Jahresauswahl aus den Anschaffungsjahren; scheitert die Liste, bleibt das Jahr wählbar.
  $effect(() => {
    assetsList()
      .then((list) => {
        acquisitionYears = list.map((a) => a.acquisitionFiscalYear);
      })
      .catch(() => {
        acquisitionYears = [];
      });
  });
  const yearOptions = $derived(vatYearOptions(currentYear, [...acquisitionYears, selectedYear]));

  $effect(() => {
    const y = selectedYear;
    void reloadTick;
    let stale = false;
    loading = true;
    error = null;
    // untrack: der Effect hängt nur am Jahr und am Neu-Laden, nicht an der Übersicht.
    const hadOverview = untrack(() => overview !== null);
    reloaded = false;
    vatAdjustmentOverview(y)
      .then((o) => {
        if (stale) return;
        overview = o;
        reloaded = hadOverview;
      })
      .catch((e) => {
        if (stale) return;
        error = String(e);
        overview = null;
      })
      .finally(() => {
        if (!stale) loading = false;
      });
    return () => {
      stale = true;
    };
  });

  const parts = $derived(splitByChange(overview?.assets ?? []));
  const projectedYear = $derived(selectedYear > currentYear);
</script>

<PageBar back="/assets" backLabel="Anschaffungen" title="Vorsteuerberichtigung (§ 15a UStG)">
  {#snippet actions()}
    <label class="year" for="va-year">Jahr</label>
    <select id="va-year" class="kb-input year-select" bind:value={selectedYear}>
      {#each yearOptions as y (y)}<option value={y}>{y}</option>{/each}
    </select>
    <HelpAnchor slug="anlagen-und-afa" heading="Vorsteuerberichtigung nach §15a UStG (Übersicht)" />
  {/snippet}
</PageBar>

<p class="lead">
  Nutzt du eine Anlage weiter, die du unter dem anderen Steuerstatus gekauft hast, wird die
  Vorsteuer fünf Jahre lang anteilig berichtigt (§ 15a UStG): nach dem Wechsel zur
  Regelbesteuerung zu deinen Gunsten, nach der Rückkehr zu § 19 zu deinen Lasten. Die Übersicht
  zeigt je Anlage den Zeitraum, den Anteil des Jahres und die Grenzen nach § 44 UStDV.
</p>

<!-- UI-12 (design.md §4.5, wie die EÜR-Seite): Beim Jahreswechsel bleibt die alte Übersicht
     stehen, bis die neue da ist; die Statuszeile sagt das, der Bereich trägt aria-busy. -->
<p class="status muted" role="status">
  {#if loading && overview}Übersicht {selectedYear} wird geladen …{:else if reloaded && overview}<span
      class="kb-visually-hidden">Übersicht {overview.year} geladen.</span>{/if}
</p>

{#if loading && !overview}
  <LoadingState />
{:else if error}
  <Banner title="Die Übersicht konnte nicht geladen werden.">
    {error}
    {#snippet actions()}<Button variant="secondary" onclick={() => reloadTick++}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if overview}
  <div class="va-body" aria-busy={loading || undefined}>
  <Banner kind="info" title="Übersicht für die Steuererklärung, keine Buchung.">{overview.notice}</Banner>

  {#if projectedYear}
    <p class="muted">
      {overview.year} liegt nach dem laufenden Jahr: Die Beträge sind unter dem heute
      eingetragenen Steuerstatus vorausgerechnet.
    </p>
  {/if}

  <dl class="kb-ledger summary" aria-label="Summe {overview.year}">
    <div>
      <dt>Anlagen mit Änderung in {overview.year}</dt>
      <dd>{parts.changed.length}</dd>
    </div>
    <div class="kb-sum">
      <dt>Zu berichtigen {overview.year}</dt>
      <dd>{totalText(overview.totalAdjustmentCents)}</dd>
    </div>
  </dl>

  {#if overview.assets.length === 0 && overview.unknown.length === 0}
    <div class="empty">
      <p class="muted">
        Für {overview.year} liegt keine Anschaffung im Berichtigungszeitraum. Gerechnet werden
        Anlagen, die du aus Kosten mit Umsatzsteuer übernommen hast.
      </p>
      <Button variant="secondary" href="/assets">Zu den Anschaffungen</Button>
    </div>
  {/if}

  {#if parts.changed.length > 0}
    <Table caption="Vorsteuerberichtigung {overview.year} je Anlage" cards busy={loading}>
      <thead>
        <tr>
          <th>Anschaffung</th>
          <th class="kb-num">Vorsteuer</th>
          <th>Zeitraum</th>
          <th>Anteil {overview.year}</th>
          <th class="kb-num">Betrag {overview.year}</th>
          <th>Grenze und Erklärung</th>
        </tr>
      </thead>
      <tbody>
        {#each parts.changed as a (a.assetId)}
          {@const y = a.yearShare}
          <tr>
            <td class="cards-full wrap" data-label="Anschaffung">
              <DocNo value={a.assetNumber} href={`/assets/${a.assetId}`} inline />{" "}{a.label}
              <span class="sub">Gekauft als {originalStatusLabel(a.adjustment.originalKlein)} · Kosten <DocNo value={a.expenseNumber} inline /></span>
              {#if a.hints.length > 0}
                <ul class="hints">
                  {#each a.hints as h (h)}<li>{h}</li>{/each}
                </ul>
              {/if}
            </td>
            <td class="kb-num" data-label="Vorsteuer">{euro(a.adjustment.inputTaxCents)}</td>
            <td data-label="Zeitraum">
              {periodText(a.adjustment.period)}
              <span class="sub">{monthsLabel(a.adjustment.period.months)}</span>
              <details class="sched">
                <summary>Alle Jahre</summary>
                <ul>
                  {#each a.adjustment.years as s (s.year)}
                    <li class:current={s.year === overview.year}>
                      <span class="sched-year">{s.year}</span>
                      <span>{yearOutcome(s)}{#if s.projected}{" "}<span class="muted">(vorausgerechnet)</span>{/if}</span>
                    </li>
                  {/each}
                </ul>
                <p class="sub">Summe: {totalText(a.adjustment.totalAdjustmentCents)}</p>
              </details>
            </td>
            <td data-label="Anteil {overview.year}">
              {#if y}{shareText(y)}{#if y.projected}{" "}<Badge kind="tag">vorausgerechnet</Badge>{/if}{/if}
            </td>
            <td class="kb-num" data-label="Betrag {overview.year}">
              {#if y && y.exemption}
                {euro(0)}
                <span class="sub">rechnerisch {euro(Math.abs(y.amountCents))} {directionLabel(y.direction)}</span>
              {:else if y}
                {euro(Math.abs(y.adjustmentCents))}
                <span class="sub">{directionLabel(y.direction)}</span>
              {/if}
            </td>
            <td class="wrap" data-label="Grenze und Erklärung">
              {#if y?.exemption}{exemptionLabel(y.exemption)}{:else if y?.filing}{filingLabel(y.filing)}{:else}—{/if}
            </td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr class="kb-sum">
          <td colspan="4" class="sum-label">Summe {overview.year}</td>
          <td class="kb-num" data-label="Summe {overview.year}">{totalText(overview.totalAdjustmentCents)}</td>
          <td class="no-card"></td>
        </tr>
      </tfoot>
    </Table>
  {:else if overview.assets.length > 0}
    <p class="muted">Bei keiner Anlage ändern sich in {overview.year} die Verhältnisse; es ist nichts zu berichtigen.</p>
  {/if}

  {#if parts.unchanged.length > 0}
    <section class="card" aria-labelledby="va-same-h">
      <h2 id="va-same-h">Ohne Änderung in {overview.year}</h2>
      <p class="muted">
        Im Berichtigungszeitraum, aber in {overview.year} unter demselben Steuerstatus wie beim Kauf.
      </p>
      <ul class="plain">
        {#each parts.unchanged as a (a.assetId)}
          <li>
            <span class="name"><DocNo value={a.assetNumber} href={`/assets/${a.assetId}`} inline />{" "}{a.label}</span>
            <span class="sub">{periodText(a.adjustment.period)} · Vorsteuer {euro(a.adjustment.inputTaxCents)}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  {#if overview.unknown.length > 0}
    <section class="card" aria-labelledby="va-unknown-h">
      <h2 id="va-unknown-h">Nicht gerechnet</h2>
      <p class="muted">
        Bei diesen Anlagen kennt Klein.Buch die Vorsteuer nicht. Ob eine Berichtigung anfällt,
        klärst du mit deiner Steuerberatung.
      </p>
      <ul class="plain">
        {#each overview.unknown as u (u.assetId)}
          <li>
            <span class="name"><DocNo value={u.assetNumber} href={`/assets/${u.assetId}`} inline />{" "}{u.label}</span>
            <span class="sub"><strong>{unknownReasonLabel(u.reason)}:</strong> {u.message}</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}
  </div>
{/if}

<style>
  /* .card / .lead / .kb-input / .kb-ledger: globale Definitionen aus tokens.css. */
  .year { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); align-self: center; }
  .year-select { width: auto; }
  .status { margin: 0 0 var(--sp-2); }
  .summary { max-width: 36rem; margin: var(--sp-4) 0; }
  .empty { display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-3); margin: var(--sp-3) 0; }
  .empty p { margin: 0; }
  td { vertical-align: top; }
  td.wrap { overflow-wrap: break-word; hyphens: auto; }
  .sub { display: block; color: var(--text-muted); font-size: var(--type-small); margin-top: var(--sp-1); }
  .hints { margin: var(--sp-2) 0 0; padding-left: 1.1rem; font-size: var(--type-small); color: var(--status-warning-fg); }
  .hints li + li { margin-top: var(--sp-1); }
  .sched { margin-top: var(--sp-1); font-size: var(--type-small); }
  .sched summary { cursor: pointer; min-height: var(--target); display: flex; align-items: center; }
  .sched ul { list-style: none; margin: 0; padding: 0; }
  .sched li { display: flex; gap: var(--sp-2); padding: var(--sp-1) 0; border-bottom: var(--rule); }
  .sched li.current { font-weight: 600; }
  .sched-year { min-width: 3rem; font-variant-numeric: tabular-nums; }
  @media screen and (max-width: 820px) {
    tfoot td.sum-label, td.no-card { display: none; }
  }
  .plain { list-style: none; margin: 0; padding: 0; }
  .plain li { padding: var(--sp-2) 0; border-bottom: var(--rule); overflow-wrap: break-word; }
  .plain li:last-child { border-bottom: none; }
  .muted { color: var(--text-muted); max-width: 72ch; line-height: 1.5; }
</style>
