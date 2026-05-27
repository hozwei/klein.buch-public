<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { assetsGet, assetsDispose } from "$lib/api";
  import type { AssetDetail, DisposalType } from "$lib/types";
  import { euro } from "$lib/format";
  import { DISPOSAL_TYPES } from "$lib/labels";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";

  let detail = $state<AssetDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);

  let id = $derived($page.params.id ?? "");

  let disposalDate = $state(new Date().toISOString().slice(0, 10));
  let disposalType = $state<DisposalType>("sale");
  let proceedsEuros = $state<number>(0);

  const maxDate = new Date().toISOString().slice(0, 10);

  let isSale = $derived(disposalType === "sale");
  let proceedsCents = $derived(isSale ? Math.round(proceedsEuros * 100) : 0);
  let residual = $derived(detail?.asset.bookValueCents ?? 0);
  let gainLoss = $derived(proceedsCents - residual);

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await assetsGet(id);
      if (!detail) error = "Anlage nicht gefunden.";
      else if (detail.asset.disposed === 1) error = "Diese Anlage ist bereits veräußert.";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function save() {
    if (isSale && proceedsCents < 0) {
      flash("Der Verkaufserlös darf nicht negativ sein.", "error");
      return;
    }
    busy = true;
    try {
      await assetsDispose({
        assetId: id,
        disposalDate,
        disposalType,
        proceedsCents,
      });
      flash("Anlage veräußert.");
      await goto(`/assets/${id}`);
    } catch (e) {
      flash("Veräußerung fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back={`/assets/${id}`} backLabel="Anlage" title="Anlage veräußern / entsorgen">
  {#snippet actions()}
    {#if detail && !error}
      <a class="btn-secondary" href={`/assets/${id}`}>Abbrechen</a>
      <button class="btn-danger" onclick={save} disabled={busy}>
        {busy ? "Speichere …" : "Veräußerung bestätigen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if detail}
  {@const a = detail.asset}
  <p class="caveat">
    Die Anlage wird nicht gelöscht, sondern als veräußert markiert (GoBD). Der
    aktuelle Restbuchwert (<strong>{euro(residual)}</strong>) wird als Schlussstand
    festgehalten.
  </p>

  <section class="card">
    <p class="who"><strong>{a.assetNumber}</strong> — {a.label}</p>
    <div class="grid">
      <label>
        Datum
        <input type="date" bind:value={disposalDate} max={maxDate} />
      </label>
      <label>
        Art
        <select bind:value={disposalType}>
          {#each DISPOSAL_TYPES as d}<option value={d.value}>{d.label}</option>{/each}
        </select>
      </label>
      {#if isSale}
        <label>
          Verkaufserlös (€)
          <input type="number" step="0.01" min="0" bind:value={proceedsEuros} />
        </label>
        <div class="info-cell">
          <span class="lbl">{gainLoss >= 0 ? "Veräußerungsgewinn" : "Veräußerungsverlust"}</span>
          <span class="info {gainLoss >= 0 ? 'gain' : 'loss'}">
            {euro(gainLoss)} <span class="muted">(Erlös − Restbuchwert)</span>
          </span>
        </div>
      {:else}
        <div class="info-cell span2">
          <span class="info">
            Ohne Erlös (Verschrottung/Verschenken) wird der Restbuchwert von
            {euro(residual)} als Verlust ausgebucht.
          </span>
        </div>
      {/if}
    </div>

  </section>
{/if}

<style>
  /* .caveat / .card entfernt — globale Definitionen aus tokens.css. */
  .who { margin: 0 0 1rem; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  .info-cell { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; justify-content: center; }
  .info-cell .lbl { color: #6b7280; }
  .info { font-size: 0.85rem; padding: 0.4rem 0.6rem; border-radius: 4px; background: #eef2ff; border: 1px solid #c7d2fe; color: #3730a3; }
  .info.gain { background: #ecfdf5; border-color: #a7f3d0; color: #065f46; }
  .info.loss { background: #fef2f2; border-color: #fecaca; color: #b91c1c; }
  input, select { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
