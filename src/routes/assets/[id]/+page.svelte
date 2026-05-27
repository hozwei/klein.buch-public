<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { assetsGet, depreciationResetAsset } from "$lib/api";
  import type { AssetDetail } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { depreciationMethodLabel, disposalTypeLabel } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Badge from "$lib/Badge.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";

  let detail = $state<AssetDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  let id = $derived($page.params.id ?? "");

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

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await assetsGet(id);
      if (!detail) error = "Anlage nicht gefunden.";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function resetAfa() {
    const ok = await confirmDialog({
      title: "Abschreibung zurücksetzen?",
      body:
        "Die noch nicht festgeschriebene Abschreibung dieser Anlage wird zurückgesetzt " +
        "und der Restbuchwert wiederhergestellt. Der Vorgang wird protokolliert. " +
        "Danach kannst du die Anlage wieder bearbeiten oder neu abschreiben.",
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

<PageBar back="/assets" backLabel="Anschaffungen" title={detail?.asset.assetNumber}>
  {#snippet actions()}
    {#if detail}
      {#if canEdit}
        <a class="btn-secondary btn-sm" href={`/assets/new?id=${detail.asset.id}`}>Bearbeiten</a>
      {/if}
      {#if hasUnlockedAfa && detail.asset.disposed === 0}
        <button class="btn-ghost-danger btn-sm" onclick={resetAfa} disabled={busy}>Abschreibung zurücksetzen</button>
      {/if}
      {#if detail.asset.disposed === 0}
        <a class="btn-ghost-danger btn-sm" href={`/assets/${detail.asset.id}/dispose`}>Veräußern / Entsorgen</a>
      {/if}
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if detail}
  {@const a = detail.asset}
  <p class="sub">
    {#if a.disposed === 1}
      <Badge tone="neutral">veräußert</Badge>
    {:else if a.bookValueCents <= 0}
      <Badge tone="success">abgeschrieben</Badge>
    {:else if a.lockedAt}
      <Badge tone="primary">festgeschrieben</Badge>
    {:else if a.lastDepreciationYear != null}
      <Badge tone="info">in Abschreibung</Badge>
    {:else}
      <Badge tone="warning">neu</Badge>
    {/if}
  </p>

  {#if hasUnlockedAfa && a.disposed === 0}
    <p class="edit-hint">
      Es ist bereits eine Abschreibung gebucht. Zum Korrigieren der Stammdaten erst
      „Abschreibung zurücksetzen" — möglich, solange das Geschäftsjahr nicht
      abgeschlossen ist.
    </p>
  {/if}

  <section class="card">
    <dl>
      <div><dt>Bezeichnung</dt><dd>{a.label}</dd></div>
      <div><dt>Anschaffungsdatum</dt><dd>{date(a.acquisitionDate)}</dd></div>
      <div><dt>Anschaffungskosten (netto)</dt><dd>{euro(a.acquisitionCostCents)}</dd></div>
      <div><dt>Abschreibung</dt><dd>{depreciationMethodLabel(a.depreciationMethod)}</dd></div>
      {#if a.usefulLifeYears}<div><dt>Nutzungsdauer</dt><dd>{a.usefulLifeYears} Jahre</dd></div>{/if}
      <div><dt>Betrieblicher Anteil</dt><dd>{a.businessSharePercent}%</dd></div>
      <div><dt>Restbuchwert</dt><dd><strong>{euro(a.bookValueCents)}</strong></dd></div>
      {#if a.lastDepreciationYear}<div><dt>Zuletzt gebucht</dt><dd>AfA {a.lastDepreciationYear}</dd></div>{/if}
      <div><dt>Lieferant</dt><dd>{detail.vendor ? detail.vendor.name : "—"}</dd></div>
      {#if detail.sourceExpenseNumber}
        <div><dt>Aus Kosten-Beleg</dt><dd>{detail.sourceExpenseNumber}</dd></div>
      {/if}
      {#if a.notes}<div class="span2"><dt>Notiz</dt><dd>{a.notes}</dd></div>{/if}
    </dl>
  </section>

  {#if a.disposed === 1}
    {@const gl = (a.disposalProceedsCents ?? 0) - (a.disposalResidualBookValueCents ?? 0)}
    <section class="card">
      <h2>Veräußerung</h2>
      <dl>
        <div><dt>Datum</dt><dd>{date(a.disposalDate)}</dd></div>
        <div><dt>Art</dt><dd>{disposalTypeLabel(a.disposalType)}</dd></div>
        <div><dt>Erlös</dt><dd>{euro(a.disposalProceedsCents ?? 0)}</dd></div>
        <div><dt>Restbuchwert bei Veräußerung</dt><dd>{euro(a.disposalResidualBookValueCents ?? 0)}</dd></div>
        <div>
          <dt>{gl >= 0 ? "Gewinn" : "Verlust"}</dt>
          <dd class={gl >= 0 ? "gain" : "loss"}>{euro(gl)}</dd>
        </div>
      </dl>
      <p class="muted small">
        Der Veräußerungsgewinn/-verlust fließt in die EÜR des Veräußerungsjahres
        (kommt mit der Steuer-Auswertung).
      </p>
    </section>
  {/if}

  <section class="card">
    <h2>Abschreibungs-Verlauf</h2>
    {#if detail.depreciationEntries.length === 0}
      <p class="muted">
        Noch keine Abschreibung gebucht. Buche sie über „Abschreibung buchen" in der
        Anschaffungs-Übersicht.
      </p>
    {:else}
      <table>
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
              <td>{e.fiscalYear}{#if e.isFullWriteoff === 1}<span class="badge ok">Sofort</span>{/if}</td>
              <td class="num">{e.monthsInYear}</td>
              <td class="num">{euro(e.bookValueBeforeCents)}</td>
              <td class="num">{euro(e.depreciationAmountCents)}</td>
              <td class="num">{euro(e.bookValueAfterCents)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>
{/if}

<style>
  .sub { margin: 0 0 1rem; }
  /* .card / .card h2 entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem 1.5rem; margin: 0; }
  dl div { display: flex; flex-direction: column; }
  .span2 { grid-column: 1 / -1; }
  dt { font-size: 0.78rem; color: #6b7280; }
  dd { margin: 0; font-size: 0.95rem; }
  dd.gain { color: #065f46; font-weight: 600; }
  dd.loss { color: #b91c1c; font-weight: 600; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.45rem 0.5rem; text-align: left; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.8rem; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .badge { padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.72rem; margin-left: 0.35rem; }
  .badge.ok { background: #d1fae5; color: #065f46; }
  .muted { color: #6b7280; }
  .small { font-size: 0.82rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .edit-hint { color: #6b7280; font-size: 0.82rem; margin: 0 0 0.5rem; max-width: 46rem; }
</style>
