<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { assetsGet, assetsSuggestMethod, depreciationResetAsset } from "#lib/api.ts";
  import type { AssetDetail } from "#lib/types.ts";
  import type { ActionMenuItem } from "#lib/ActionMenu.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import { euro, date, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { depreciationMethodLabel, disposalTypeLabel } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import { assetStatusView } from "../assetStatus.ts";
  import Table from "#lib/Table.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";

  let detail = $state<AssetDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notFound = $state(false);
  let busy = $state(false);

  let id = $derived(page.params.id ?? "");

  // Es gibt noch nicht festgeschriebene (offenes GJ) AfA-Buchungen → korrigierbar.
  let hasUnlockedAfa = $derived(
    (detail?.depreciationEntries ?? []).some((e) => e.lockedAt == null),
  );
  // Stammdaten editierbar, solange nichts gebucht und nichts festgeschrieben ist.
  let canEdit = $derived(
    !!detail &&
      detail.asset.disposed === 0 &&
      detail.asset.lockedAt == null &&
      detail.depreciationEntries.length === 0,
  );

  // U4 (Warteliste 4, ADR 0039 Nr. 147 (2)): Anlage aus §13b-Kosten unter §19 — die
  // selbst geschuldete Steuer gehört als eigene Kosten und eigene Anlage dazu. Nur ein
  // Hinweis mit Weg; ob die Kosten §13b und §19 sind, sagt der Server (Vorbelegung).
  let reverseChargeKlein = $state(false);

  async function load() {
    loading = true;
    error = null;
    reverseChargeKlein = false;
    notFound = false;
    try {
      detail = await assetsGet(id);
      if (!detail) {
        notFound = true;
        error = "Anschaffung nicht gefunden.";
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
    const a = detail?.asset;
    if (a?.expenseId && a.disposed === 0) {
      try {
        const s = await assetsSuggestMethod(null, 0, a.expenseId);
        reverseChargeKlein = !!s.source?.reverseCharge13b && s.source.isKleinunternehmer;
      } catch {
        // Hinweis ist Zusatz; ohne Antwort bleibt er weg.
      }
    }
  }

  // PageBar (U4, design.md §4.2): „Bearbeiten“ als Nebenaktion, seltene und nicht
  // umkehrbare Aktionen im Menü „Mehr“ (vorher Ghost-Gefahrknöpfe in der Leiste).
  let barSecondary = $derived<ActionMenuItem[]>(
    detail && canEdit ? [{ label: "Bearbeiten", href: `/assets/new?id=${detail.asset.id}` }] : [],
  );
  let barMore = $derived.by<ActionMenuItem[]>(() => {
    if (!detail || detail.asset.disposed !== 0) return [];
    const items: ActionMenuItem[] = [];
    if (hasUnlockedAfa) {
      items.push({ label: "Abschreibung zurücksetzen", onSelect: resetAfa, danger: true, disabled: busy });
    }
    items.push({ label: "Veräußern / Entsorgen", href: `/assets/${detail.asset.id}/dispose`, danger: true });
    return items;
  });

  onMount(load);

  async function resetAfa() {
    if (busy) return;
    const ok = await confirmDialog({
      title: "Abschreibung zurücksetzen?",
      body:
        "Die noch nicht festgeschriebene Abschreibung dieser Anschaffung wird zurückgesetzt " +
        "und der Restbuchwert wiederhergestellt. Der Vorgang wird protokolliert. " +
        "Danach kannst du die Anschaffung wieder bearbeiten oder neu abschreiben.",
      confirmLabel: "Zurücksetzen",
      cancelLabel: "Abbrechen",
      danger: true,
    });
    if (!ok) return;
    busy = true;
    try {
      detail = await depreciationResetAsset(id);
      flash("Abschreibung zurückgesetzt.");
    } catch (e) {
      flash("Zurücksetzen fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<!-- U4 (V2-13): Objekt heißt „Anschaffung“ wie Bereich und Handbuch; V4-08: h1 auch beim
     Laden und bei „nicht gefunden“. -->
<PageBar
  back="/assets"
  backLabel="Anschaffungen"
  title={detail ? `Anschaffung ${detail.asset.assetNumber}` : notFound ? "Anschaffung nicht gefunden" : "Anschaffung"}
  secondary={barSecondary}
  more={barMore}
/>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}{#if notFound}{" "}<a class="kb-link" href="/assets">Zu den Anschaffungen</a>{/if}</Banner>
{:else if detail}
  {@const a = detail.asset}
  <!-- U4 (V3-09): Status aus assetStatus.ts, wie in der Liste. -->
  {@const st = assetStatusView(a)}
  <p class="sub">
    <Badge tone={st.tone}>{st.label}</Badge>
  </p>

  {#if hasUnlockedAfa && a.disposed === 0}
    <p class="edit-hint">
      Es ist bereits eine Abschreibung gebucht. Zum Korrigieren der Angaben erst
      „Abschreibung zurücksetzen“ (Menü „Mehr“) — möglich, solange das Geschäftsjahr nicht
      abgeschlossen ist.
    </p>
  {/if}

  {#if reverseChargeKlein}
    <section class="card" aria-labelledby="rc-h">
      <h2 id="rc-h">Selbst geschuldete Steuer (§13b)</h2>
      <p class="rc-text">
        Diese Anschaffung kommt aus Kosten mit §13b. Als Kleinunternehmer schuldest du die
        Umsatzsteuer darauf selbst und kannst sie nicht abziehen; sie gehört zu den
        Anschaffungskosten. Falls noch nicht geschehen: Erfasse die Zahlung an das Finanzamt
        (nur den Anteil dieser Anschaffung) als eigene Kosten und übernimm sie als eigene Anlage —
        Anschaffungsdatum, Methode und Nutzungsdauer kommen dann von <DocNo value={a.assetNumber} />.
      </p>
      <div class="kb-actions kb-start">
        <a class="btn-secondary" href={`/expenses/new?taxFor=${a.id}`}>Steuer als Kosten erfassen</a>
      </div>
    </section>
  {/if}

  <section class="card">
    <dl>
      <div><dt>Bezeichnung</dt><dd>{a.label}</dd></div>
      <div><dt>Anschaffungsdatum</dt><dd>{date(a.acquisitionDate)}</dd></div>
      <div><dt>Anschaffungskosten</dt><dd class="amt">{euro(a.acquisitionCostCents)}</dd></div>
      <div><dt>Abschreibung</dt><dd>{depreciationMethodLabel(a.depreciationMethod)}</dd></div>
      {#if a.usefulLifeYears}<div><dt>Nutzungsdauer</dt><dd>{formatQuantity(a.usefulLifeYears, 2)} Jahre</dd></div>{/if}
      <div><dt>Betrieblicher Anteil</dt><dd>{percent(a.businessSharePercent)}</dd></div>
      <div><dt>Restbuchwert</dt><dd class="amt"><strong>{euro(a.bookValueCents)}</strong></dd></div>
      {#if a.lastDepreciationYear}<div><dt>Zuletzt gebucht</dt><dd>AfA {a.lastDepreciationYear}</dd></div>{/if}
      <div><dt>Lieferant</dt><dd>{detail.vendor ? detail.vendor.name : "—"}</dd></div>
      {#if detail.sourceExpenseNumber}
        <div>
          <dt>Aus Kosten-Beleg</dt>
          <dd>
            {#if a.expenseId}
              <DocNo value={detail.sourceExpenseNumber} href={`/expenses/${a.expenseId}`} />
            {:else}
              <DocNo value={detail.sourceExpenseNumber} />
            {/if}
          </dd>
        </div>
      {/if}
      {#if a.notes}<div class="span2"><dt>Notiz</dt><dd class="pre">{a.notes}</dd></div>{/if}
    </dl>
  </section>

  {#if a.disposed === 1}
    {@const gl = (a.disposalProceedsCents ?? 0) - (a.disposalResidualBookValueCents ?? 0)}
    <section class="card">
      <h2>Veräußerung</h2>
      <dl>
        <div><dt>Datum</dt><dd>{date(a.disposalDate)}</dd></div>
        <div><dt>Art</dt><dd>{disposalTypeLabel(a.disposalType)}</dd></div>
        <div><dt>{a.disposalType === "given_away" ? "Entnahmewert" : "Erlös"}</dt><dd class="amt">{euro(a.disposalProceedsCents ?? 0)}</dd></div>
        <div><dt>Restbuchwert bei Veräußerung</dt><dd class="amt">{euro(a.disposalResidualBookValueCents ?? 0)}</dd></div>
        <div>
          <dt>{gl >= 0 ? "Gewinn" : "Verlust"}</dt>
          <dd class="amt {gl >= 0 ? 'gain' : 'loss'}">{euro(gl)}</dd>
        </div>
      </dl>
      <p class="muted small">
        Der Veräußerungsgewinn bzw. -verlust fließt in die EÜR des Veräußerungsjahres.
      </p>
    </section>
  {/if}

  <section class="card">
    <h2>Abschreibungs-Verlauf</h2>
    {#if detail.depreciationEntries.length === 0}
      <p class="muted">
        Noch keine Abschreibung gebucht. Buche sie über „Abschreibung buchen“ in der
        <a class="kb-link" href="/assets">Anschaffungs-Übersicht</a>.
      </p>
    {:else}
      <Table caption="Abschreibungs-Verlauf" cards>
        <thead>
          <tr>
            <th>Jahr</th>
            <th class="num">Monate</th>
            <th class="num">Restbuchwert davor</th>
            <th class="num">Abschreibung</th>
            <th class="num">Restbuchwert danach</th>
          </tr>
        </thead>
        <tbody>
          {#each detail.depreciationEntries as e (e.id)}
            <tr>
              <td data-label="Jahr">{e.fiscalYear}{#if e.isFullWriteoff === 1}{" "}<Badge kind="tag">Sofort</Badge>{/if}</td>
              <td class="num" data-label="Monate">{e.monthsInYear}</td>
              <td class="num" data-label="Restbuchwert davor">{euro(e.bookValueBeforeCents)}</td>
              <td class="num" data-label="Abschreibung">{euro(e.depreciationAmountCents)}</td>
              <td class="num" data-label="Restbuchwert danach">{euro(e.bookValueAfterCents)}</td>
            </tr>
          {/each}
        </tbody>
      </Table>
    {/if}
  </section>
{/if}

<style>
  .sub { margin: 0 0 1rem; }
  /* .card / .card h2: globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-2) var(--sp-5); margin: 0; }
  dl div { display: flex; flex-direction: column; min-width: 0; }
  .span2 { grid-column: 1 / -1; }
  dt { font-size: var(--fs-xs); color: var(--text-muted); }
  dd { margin: 0; font-size: var(--fs-md); overflow-wrap: anywhere; }
  dd.amt { font-variant-numeric: tabular-nums; }
  dd.pre { white-space: pre-wrap; }
  @media (max-width: 600px) { dl { grid-template-columns: 1fr; } }
  dd.gain { color: var(--status-success-fg); font-weight: 600; }
  dd.loss { color: var(--status-danger-fg); font-weight: 600; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .rc-text { margin: 0 0 var(--sp-3); max-width: 72ch; line-height: var(--lh-body); }
  .muted { color: var(--text-muted); }
  .small { font-size: var(--fs-sm); }
  .edit-hint { color: var(--text-muted); font-size: var(--fs-sm); margin: 0 0 var(--sp-2); max-width: 46rem; }
</style>
