<script lang="ts">
  import { isIsoDate, todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { assetsGet, assetsDispose, assetsDisposalPreview } from "#lib/api.ts";
  import type { AssetDetail, AssetDisposalPreview, DisposalType } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { DISPOSAL_TYPES, disposalTypeLabel } from "#lib/labels.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import DocNo from "#lib/DocNo.svelte";
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

  // U4 (Warteliste 5, Übergabe F3): echte Vorschau zum gewählten Datum. Der Server
  // rechnet wie beim Abgang selbst (AfA bis zum Abgangsmonat, Ersetzen bzw. Nachholen;
  // `assets_disposal_preview`, ADR 0041 Nr. 81) und nennt den Grund, aus dem er den
  // Abgang zu diesem Datum ablehnen würde. Die Seite rechnet nichts selbst.
  let preview = $state<AssetDisposalPreview | null>(null);
  let previewError = $state("");
  let previewLoading = $state(false);
  let previewSeq = 0;

  $effect(() => {
    const d = disposalDate;
    const assetId = id;
    if (!detail || error || !isIsoDate(d)) {
      preview = null;
      previewError = "";
      previewLoading = false;
      return;
    }
    const seq = ++previewSeq;
    previewLoading = true;
    // Kurz warten, damit Tippen im Datumsfeld nicht je Ziffer eine Anfrage auslöst.
    const timer = setTimeout(() => {
      assetsDisposalPreview(assetId, d)
        .then((p) => {
          if (seq !== previewSeq) return;
          preview = p;
          previewError = "";
        })
        .catch((e) => {
          if (seq !== previewSeq) return;
          preview = null;
          previewError = String(e).replace(/^Error:\s*/, "");
        })
        .finally(() => {
          if (seq === previewSeq) previewLoading = false;
        });
    }, 250);
    return () => clearTimeout(timer);
  });

  // Vorschau passt zum Datum im Feld (nicht die einer früheren Eingabe).
  let current = $derived(preview && preview.disposalDate === disposalDate ? preview : null);
  let blockedReason = $derived(current?.blockedReason ?? null);

  let isSale = $derived(disposalType === "sale");
  let isWithdrawal = $derived(disposalType === "given_away");
  let proceedsCents = $derived(
    isSale ? (saleCents ?? 0) : isWithdrawal ? (withdrawalCents ?? 0) : 0,
  );
  // Restbuchwert zum Abgang aus der Vorschau; solange sie fehlt, der heutige Stand.
  let residual = $derived(current?.residualBookValueCents ?? detail?.asset.bookValueCents ?? 0);
  let gainLoss = $derived(proceedsCents - residual);
  let disposalYear = $derived(isIsoDate(disposalDate) ? Number(disposalDate.slice(0, 4)) : null);

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await assetsGet(id);
      if (!detail) error = "Anschaffung nicht gefunden.";
      else if (detail.asset.disposed === 1) error = "Diese Anschaffung ist bereits veräußert.";
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
    else if (blockedReason) e.date = blockedReason;
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
    if (busy) return;
    saveError = "";
    if (!(await validate())) return;
    // U7 (R1-05, design.md §5): Der Abgang ist nicht umkehrbar (GoBD) — Rückfrage mit den
    // Eckwerten wie beim Festschreiben von Kosten und beim Storno.
    const bullets = [
      `Datum des Abgangs: ${date(disposalDate)}`,
      `Art: ${disposalTypeLabel(disposalType)}`,
    ];
    if (isSale) bullets.push(`Verkaufserlös: ${euro(proceedsCents)}`);
    if (isWithdrawal) bullets.push(`Entnahmewert: ${euro(proceedsCents)}`);
    if (current) {
      bullets.push(`Restbuchwert zum Abgang: ${euro(current.residualBookValueCents)}`);
      bullets.push(`${gainLoss >= 0 ? "Gewinn" : "Verlust"}: ${euro(Math.abs(gainLoss))}`);
    } else {
      bullets.push("Restbuchwert zum Abgang: rechnet Klein.Buch beim Buchen");
    }
    const ok = await confirmDialog({
      title: `Abgang von ${detail?.asset.assetNumber ?? "der Anschaffung"} buchen?`,
      body:
        "Klein.Buch bucht die AfA bis zum Abgang und den Restbuchwert als Ausgabe. " +
        "Das lässt sich nicht rückgängig machen.",
      bullets,
      confirmLabel: "Abgang buchen",
      cancelLabel: "Abbrechen",
      danger: true,
    });
    if (!ok) return;
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

  // Meldung am Datumsfeld: Pflichtfehler aus dem Absenden, sonst der Ablehnungsgrund
  // der Vorschau (live, damit niemand erst beim Bestätigen davon erfährt).
  let dateMessage = $derived(errors.date ?? blockedReason ?? "");
</script>

<!-- U4 (V2-13/V2-19): Begriff „Anschaffung“, Titel mit Nummer, Zurück zur Detailseite;
     während des Speicherns aria-disabled statt disabled (Fokus bleibt). -->
<PageBar
  back={`/assets/${id}`}
  backLabel="Anschaffung"
  title={detail ? `Anschaffung ${detail.asset.assetNumber} veräußern / entsorgen` : "Anschaffung veräußern / entsorgen"}
>
  {#snippet actions()}
    {#if detail && !error}
      <a class="btn-secondary" href={`/assets/${id}`}>Abbrechen</a>
      <button type="button" class="btn-danger" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>
        {busy ? "Speichert …" : "Abgang bestätigen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error} <a class="kb-link" href={`/assets/${id}`}>Zur Anschaffung</a></Banner>
{:else if detail}
  {@const a = detail.asset}
  <p class="caveat">
    Die Anschaffung wird nicht gelöscht, sondern als veräußert markiert (GoBD). Beim
    Bestätigen bucht Klein.Buch die AfA bis zum Monat des Abgangs (schon gebuchte
    AfA ab dem Abgangsjahr wird dabei ersetzt, fehlende Vorjahre werden nachgeholt) und
    hält den Restbuchwert danach als Schlussstand fest. Die Vorschau unten rechnet genau
    das zum gewählten Datum. Der Abgang lässt sich nicht rückgängig machen.
  </p>

  {#if saveError}
    <Banner title="Nicht gespeichert:">{saveError}</Banner>
  {/if}

  <section class="card">
    <p class="who"><DocNo value={a.assetNumber} /> — {a.label}</p>
    <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
      <div class="grid">
        <div class="field">
          <label for="disp-date">Datum des Abgangs<span class="req" aria-hidden="true">*</span></label>
          <input
            id="disp-date"
            class="kb-input kb-w-date"
            type="date"
            bind:this={dateEl}
            bind:value={disposalDate}
            oninput={() => (errors.date = undefined)}
            max={maxDate}
            required
            aria-required="true"
            aria-invalid={dateMessage ? "true" : undefined}
            aria-describedby={dateMessage ? "disp-date-err" : undefined}
          />
          {#if dateMessage}<p id="disp-date-err" class="msg err">{dateMessage}</p>{/if}
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
            fieldClass="kb-w-amount"
            label="Verkaufserlös"
            hint="0, wenn du nichts erlöst hast."
            error={errors.sale}
          />
          <div class="field">
            <span class="lbl">{gainLoss >= 0 ? "Veräußerungsgewinn" : "Veräußerungsverlust"}{current ? "" : " (vorläufig)"}</span>
            <p class="result {gainLoss >= 0 ? 'gain' : 'loss'}">
              <strong class="kb-num">{euro(gainLoss)}</strong>
              <span class="calc">Erlös − Restbuchwert zum Abgang</span>
            </p>
          </div>
        {:else if isWithdrawal}
          <MoneyInput
            bind:this={withdrawalInput}
            bind:cents={withdrawalCents}
            fieldClass="kb-w-amount"
            label="Entnahmewert"
            required
            error={errors.withdrawal}
            onchange={() => (errors.withdrawal = undefined)}
            hint="Zeitwert (Teilwert) bei Privatentnahme, 0 bei einem betrieblichen Geschenk."
          />
          <div class="field">
            <p class="info-banner">
              Bei einer <strong>Privatentnahme</strong> zählt der Zeitwert (Teilwert) als
              Betriebseinnahme, der Restbuchwert zum Abgang von {euro(residual)} als Ausgabe. Bei
              einem <strong>betrieblichen Geschenk</strong> an Dritte 0 eintragen (Grenzen nach
              § 4 Abs. 5 Nr. 1 EStG beachten).
            </p>
          </div>
        {:else}
          <div class="field span2">
            <p class="info-banner">
              Ohne Erlös (Verschrottung) wird der Restbuchwert zum Abgang von
              {euro(residual)} als Verlust ausgebucht.
            </p>
          </div>
        {/if}
      </div>
    </form>
  </section>

  <section class="card" aria-labelledby="disp-prev-h" aria-busy={previewLoading}>
    <h2 id="disp-prev-h">
      Vorschau{#if isIsoDate(disposalDate)}{" "}zum {date(disposalDate)}{/if}
    </h2>
    {#if !isIsoDate(disposalDate)}
      <p class="muted">Wähle ein Datum, dann rechnet Klein.Buch den Restbuchwert zum Abgang.</p>
    {:else if previewError}
      <Banner>Die Vorschau ist gerade nicht möglich: {previewError}</Banner>
    {:else if !current}
      <p class="muted">Rechnet …</p>
    {:else}
      <dl class="kb-ledger dispose-ledger">
        <div>
          <dt>Restbuchwert heute</dt>
          <dd>{euro(current.currentBookValueCents)}</dd>
        </div>
        {#if current.replacedDepreciationCents !== 0}
          <div>
            <!-- U7 (R1-06): Vorzeichen ausdrücklich — die ersetzte AfA kommt zum Restbuchwert hinzu. -->
            <dt>Gebuchte AfA {current.replacedYears.join(", ")}, die der Abgang ersetzt</dt>
            <dd>+{euro(current.replacedDepreciationCents)}</dd>
          </div>
        {/if}
        {#if current.catchUpDepreciationCents !== 0}
          <div>
            <dt>Nachgeholte AfA {current.catchUpYears.join(", ")}</dt>
            <dd>−{euro(current.catchUpDepreciationCents)}</dd>
          </div>
        {/if}
        <div>
          <dt>Restbuchwert am 01.01.{disposalYear}</dt>
          <dd>{euro(current.bookValueBeforeCents)}</dd>
        </div>
        <div>
          <dt>
            AfA {disposalYear} bis zum Abgang ({current.disposalYearMonths}
            {current.disposalYearMonths === 1 ? "Monat" : "Monate"})
          </dt>
          <dd>−{euro(current.disposalYearDepreciationCents)}</dd>
        </div>
        <div class="kb-sum">
          <dt>Restbuchwert zum Abgang</dt>
          <dd>{euro(current.residualBookValueCents)}</dd>
        </div>
      </dl>
      <p class="muted small">
        AfA insgesamt von der Anschaffung bis zum Abgang: {euro(current.depreciationToDateCents)}.
        Der Restbuchwert zum Abgang geht als Ausgabe in die EÜR {disposalYear}.
      </p>
    {/if}
    <!-- Kurzfassung für Screenreader, wenn sich Datum oder Ergebnis ändern (4.1.3). -->
    <p class="kb-visually-hidden" aria-live="polite">
      {#if blockedReason}
        Abgang zu diesem Datum nicht möglich: {blockedReason}
      {:else if current}
        Restbuchwert zum Abgang {euro(current.residualBookValueCents)}.
      {/if}
    </p>
  </section>
{/if}

<style>
  /* .caveat / .card / .info-banner / .kb-input: globale Definitionen aus tokens.css. */
  .who { margin: 0 0 1rem; overflow-wrap: anywhere; }
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
  /* Kontorbuch-Lineatur wie auf /euer (ADR 0041 Nr. 25): Linie je Zeile, doppelte über dem Ergebnis. */
  .dispose-ledger { max-width: 40rem; }
  .muted { color: var(--text-muted); }
  .small { font-size: var(--fs-sm); max-width: 72ch; }
</style>
