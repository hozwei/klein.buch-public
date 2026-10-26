<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { assetsGet, assetsDispose } from "#lib/api.ts";
  import type { AssetDetail, DisposalType } from "#lib/types.ts";
  import { euro } from "#lib/format.ts";
  import { DISPOSAL_TYPES } from "#lib/labels.ts";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";

  let detail = $state<AssetDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let saveError = $state("");

  let id = $derived(page.params.id ?? "");

  let disposalDate = $state(todayBerlin());
  let disposalType = $state<DisposalType>("sale");
  // Beträge in Cent (MoneyInput, Lauf 2026.11 U1); keine negativen Werte (ADR 0041 Nr. 16).
  let saleCents = $state<number | null>(0);

  const maxDate = todayBerlin();

  // Lauf F8: Privatentnahme/Geschenk mit Entnahmewert (Teilwert, § 6 Abs. 1
  // Nr. 4 EStG; 0 bei einem betrieblichen Geschenk) — Pflichtangabe.
  let withdrawalCents = $state<number | null>(null);

  let errors = $state<{ sale?: string; withdrawal?: string; date?: string }>({});
  let saleInput: MoneyInput | undefined = $state();
  let withdrawalInput: MoneyInput | undefined = $state();
  let dateEl: HTMLInputElement | undefined = $state();

  let isSale = $derived(disposalType === "sale");
  let isWithdrawal = $derived(disposalType === "given_away");
  let proceedsCents = $derived(
    isSale ? (saleCents ?? 0) : isWithdrawal ? (withdrawalCents ?? 0) : 0,
  );
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

  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!disposalDate) e.date = "Bitte das Datum des Abgangs angeben.";
    let ok = true;
    if (isSale) ok = saleInput?.validate() ?? true;
    if (isWithdrawal) {
      ok = withdrawalInput?.validate() ?? true;
      if (ok && withdrawalCents == null) {
        e.withdrawal =
          "Bitte den Entnahmewert angeben (Zeitwert bei Privatentnahme, 0 bei einem betrieblichen Geschenk).";
      }
    }
    errors = e;
    await tick();
    if (e.date) dateEl?.focus();
    else if (isSale && !ok) saleInput?.focus();
    else if (isWithdrawal && (!ok || e.withdrawal)) withdrawalInput?.focus();
    return ok && Object.keys(e).length === 0;
  }

  async function save() {
    saveError = "";
    if (!(await validate())) return;
    busy = true;
    try {
      await assetsDispose({
        assetId: id,
        disposalDate,
        disposalType,
        proceedsCents,
      });
      await goto(`/assets/${id}`);
    } catch (e) {
      saveError = String(e).replace(/^Error:\s*/, "");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back={`/assets/${id}`} backLabel="Anlage" title="Anlage veräußern / entsorgen">
  {#snippet actions()}
    {#if detail && !error}
      <a class="btn-secondary" href={`/assets/${id}`}>Abbrechen</a>
      <button type="button" class="btn-danger" onclick={save} disabled={busy} aria-busy={busy}>
        {busy ? "Speichere …" : "Abgang bestätigen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error} <a class="inline-link" href={`/assets/${id}`}>Zur Anlage</a></Banner>
{:else if detail}
  {@const a = detail.asset}
  <p class="caveat">
    Die Anlage wird nicht gelöscht, sondern als veräußert markiert (GoBD). Beim
    Bestätigen bucht Klein.Buch die AfA bis zum Monat des Abgangs (schon gebuchte
    AfA ab dem Abgangsjahr wird dabei ersetzt) und hält den Restbuchwert danach als
    Schlussstand fest. Der aktuelle Restbuchwert vor dieser Buchung beträgt
    <strong>{euro(residual)}</strong>; Gewinn bzw. Verlust unten sind deshalb ein
    Näherungswert. Der Abgang lässt sich nicht rückgängig machen.
  </p>

  {#if saveError}
    <Banner><strong>Nicht gespeichert:</strong> {saveError}</Banner>
  {/if}

  <section class="card">
    <p class="who"><span class="doc-no">{a.assetNumber}</span> — {a.label}</p>
    <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
      <div class="grid">
        <div class="field">
          <label for="disp-date">Datum des Abgangs<span class="req" aria-hidden="true">*</span></label>
          <input
            id="disp-date"
            class="kb-input"
            type="date"
            bind:this={dateEl}
            bind:value={disposalDate}
            max={maxDate}
            required
            aria-required="true"
            aria-invalid={errors.date ? "true" : undefined}
            aria-describedby={errors.date ? "disp-date-err" : undefined}
          />
          {#if errors.date}<p id="disp-date-err" class="msg err">{errors.date}</p>{/if}
        </div>
        <div class="field">
          <label for="disp-type">Art des Abgangs</label>
          <select id="disp-type" class="kb-input" bind:value={disposalType} onchange={() => (errors = {})}>
            {#each DISPOSAL_TYPES as d}<option value={d.value}>{d.label}</option>{/each}
          </select>
        </div>
        {#if isSale}
          <MoneyInput
            bind:this={saleInput}
            bind:cents={saleCents}
            label="Verkaufserlös"
            hint="0, wenn du nichts erlöst hast."
            error={errors.sale}
          />
          <div class="field">
            <span class="lbl">{gainLoss >= 0 ? "Veräußerungsgewinn" : "Veräußerungsverlust"} (Näherung)</span>
            <p class="result {gainLoss >= 0 ? 'gain' : 'loss'}" aria-live="polite">
              <strong class="kb-num">{euro(gainLoss)}</strong>
              <span class="calc">Erlös − Restbuchwert</span>
            </p>
          </div>
        {:else if isWithdrawal}
          <MoneyInput
            bind:this={withdrawalInput}
            bind:cents={withdrawalCents}
            label="Entnahmewert"
            required
            error={errors.withdrawal}
            onchange={() => (errors.withdrawal = undefined)}
            hint="Zeitwert (Teilwert) bei Privatentnahme, 0 bei einem betrieblichen Geschenk."
          />
          <div class="field">
            <p class="info-banner">
              Bei einer <strong>Privatentnahme</strong> zählt der Zeitwert (Teilwert) als
              Betriebseinnahme, der Restbuchwert von {euro(residual)} als Ausgabe. Bei einem
              <strong>betrieblichen Geschenk</strong> an Dritte 0 eintragen (Grenzen nach
              § 4 Abs. 5 Nr. 1 EStG beachten).
            </p>
          </div>
        {:else}
          <div class="field span2">
            <p class="info-banner">
              Ohne Erlös (Verschrottung) wird der Restbuchwert von
              {euro(residual)} als Verlust ausgebucht.
            </p>
          </div>
        {/if}
      </div>
    </form>
  </section>
{/if}

<style>
  /* .caveat / .card / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
  .who { margin: 0 0 1rem; overflow-wrap: anywhere; }
  .doc-no { font-family: var(--font-mono); font-weight: 600; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); align-items: start; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field > label:first-child, .field > .lbl { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .grid :global(.money-field.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label) { color: var(--text); }
  .info-banner { margin: 0; }
  .result { margin: 0; padding: var(--sp-2) var(--sp-3); border-radius: var(--r-md); border: 1px solid; display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: baseline; }
  .result.gain { background: var(--status-success-bg); border-color: var(--status-success-line); color: var(--status-success-fg); }
  .result.loss { background: var(--status-danger-bg); border-color: var(--status-danger-line); color: var(--status-danger-fg); }
  .calc { font-size: var(--fs-sm); }
  .inline-link { text-decoration: underline; }
  .muted { color: var(--text-muted); }
</style>
