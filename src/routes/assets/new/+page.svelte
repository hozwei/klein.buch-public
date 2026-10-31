<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Banner from "#lib/Banner.svelte";
  import ContactSelect from "#lib/ContactSelect.svelte";
  import DocNo, { splitDocNumbers } from "#lib/DocNo.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import {
    contactsList,
    assetsAfaTable,
    assetsCreate,
    assetsUpdate,
    assetsGet,
    assetsSuggestMethod,
    expensesGet,
    expensesList,
  } from "#lib/api.ts";
  import type {
    Contact,
    AfaTabellen,
    Asset,
    DepreciationMethod,
    AssetInputDto,
    AssetSuggestion,
    MethodSuggestion,
  } from "#lib/types.ts";
  import { DEPRECIATION_METHODS, depreciationMethodShort } from "#lib/labels.ts";
  import { euro, date } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import { roundHalfAwayFromZero } from "#lib/tax.ts";

  let vendors = $state<Contact[]>([]);
  let table = $state<AfaTabellen | null>(null);
  let busy = $state(false);
  let loaded = $state(false);

  // Edit-/Quell-Kontext
  let editId = $state<string | null>(null);
  let expenseId = $state<string | null>(null);
  let sourceExpenseCategory = $state<string | null>(null);
  let sourceExpenseNumber = $state<string | null>(null);
  // Lauf F8: Herkunft der vorbelegten Anschaffungskosten (§ 9b Abs. 1 EStG).
  let sourceIsKlein = $state<boolean | null>(null);
  // Lauf F8c: §13b-Kosten bei §19 — selbst geschuldete Steuer als eigene Kosten + Anlage.
  let sourceReverseCharge = $state(false);
  // Vorschlag und Vorbelegung aus den Quell-Kosten (zum Zurücksetzen der Verknüpfung).
  let sourceSuggestion: AssetSuggestion | null = null;

  // U4 (Warteliste 4, ADR 0039 Nr. 147 (2)): Die Zahlung der selbst geschuldeten
  // §13b-Steuer ans Finanzamt wird eine eigene Anlage mit gleichem Anschaffungsdatum,
  // gleicher Methode und Nutzungsdauer wie die Anlage aus der Lieferantenrechnung.
  // Kandidaten: nicht abgegangene Anlagen aus §13b-Kosten unter §19. Nur Vorbelegung —
  // es entsteht keine gespeicherte Verknüpfung, und der Server prüft wie immer.
  let taxCandidates = $state<Asset[]>([]);
  let taxForId = $state("");
  let taxFor = $derived(taxCandidates.find((c) => c.id === taxForId) ?? null);
  /** Notiz, die die Verknüpfung vorbelegt hat (wird beim Lösen wieder entfernt). */
  let autoNote = "";

  // Formularfelder
  let label = $state("");
  let acquisitionDate = $state(todayBerlin());
  // Betrag in Cent (MoneyInput, Lauf 2026.11 U1).
  let costCents = $state<number | null>(null);
  let vendorContactId = $state<string>("");
  let depreciationMethod = $state<DepreciationMethod>("linear");
  let usefulLifeYears = $state<number | null>(null);
  let afaCategory = $state<string>("");
  let businessSharePercent = $state<number | null>(100);
  let notes = $state("");

  // Begründung des letzten Methoden-Vorschlags (bleibt sichtbar statt Toast).
  let suggestionReason = $state("");
  // Ablehnung des Servers (Lauf F8: abgeschlossenes Jahr, stornierte Kosten,
  // falscher Betrag) bleibt als Banner stehen, statt als Toast zu verschwinden.
  let saveError = $state("");
  let loadError = $state("");

  let errors = $state<{ label?: string; cost?: string; life?: string; share?: string; date?: string }>({});
  let labelEl: HTMLInputElement | undefined = $state();
  let dateEl: HTMLInputElement | undefined = $state();
  let costInput: MoneyInput | undefined = $state();
  let lifeInput: QuantityInput | undefined = $state();
  let shareInput: QuantityInput | undefined = $state();

  const maxDate = todayBerlin();

  let isLinear = $derived(depreciationMethod === "linear");
  // Abschreibungsbasis (Anzeige): Cent ganzzahlig, Halbe weg von 0.
  let businessBookValue = $derived(
    roundHalfAwayFromZero(((costCents ?? 0) * (businessSharePercent ?? 0)) / 100),
  );
  // Aus Kosten übernommen: Betrag fest (Server lehnt andere Beträge ab, F8).
  let costFromSource = $derived(!editId && sourceIsKlein !== null);
  // Verknüpft, aber Methode von Hand auf GWG gestellt, obwohl die Hauptanlage keins ist.
  let gwgMismatch = $derived(
    !!taxFor && taxFor.depreciationMethod !== "gwg_sofort" && depreciationMethod === "gwg_sofort",
  );

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");
      table = await assetsAfaTable();
    } catch (e) {
      flash("Daten konnten nicht geladen werden: " + String(e), "error");
    }

    editId = page.url.searchParams.get("id");
    expenseId = page.url.searchParams.get("expenseId");

    if (editId) {
      try {
        const detail = await assetsGet(editId);
        if (!detail) {
          flash("Anschaffung nicht gefunden.", "error");
          await goto("/assets");
          return;
        }
        const a = detail.asset;
        if (a.lockedAt || a.disposed === 1) {
          flash("Diese Anschaffung ist festgeschrieben und kann nicht mehr bearbeitet werden.", "error");
          await goto(`/assets/${editId}`);
          return;
        }
        label = a.label;
        acquisitionDate = a.acquisitionDate.slice(0, 10);
        costCents = a.acquisitionCostCents;
        vendorContactId = a.vendorContactId ?? "";
        depreciationMethod = a.depreciationMethod as DepreciationMethod;
        usefulLifeYears = a.usefulLifeYears ?? null;
        afaCategory = a.afaCategory ?? "";
        businessSharePercent = a.businessSharePercent;
        notes = a.notes ?? "";
        expenseId = a.expenseId;
      } catch (e) {
        loadError = "Laden fehlgeschlagen: " + String(e);
      }
    } else if (expenseId) {
      // Aus einer Kosten-Position aktivieren: Felder vorbefüllen + Vorschlag holen.
      try {
        const detail = await expensesGet(expenseId);
        if (detail) {
          const e = detail.expense;
          label = e.description;
          vendorContactId = e.vendorContactId ?? "";
          sourceExpenseCategory = e.category;
          sourceExpenseNumber = e.expenseNumber;
          // Lauf F8: Anschaffungskosten brutto bei §19, netto bei Regelbesteuerung
          // (§ 9b Abs. 1 EStG), Anschaffungsdatum = Belegdatum der Kosten.
          const s = await assetsSuggestMethod(e.category, e.netAmountCents, expenseId);
          if (s.source) {
            costCents = s.source.acquisitionCostCents;
            acquisitionDate = s.source.acquisitionDate;
            sourceIsKlein = s.source.isKleinunternehmer;
            sourceReverseCharge = s.source.reverseCharge13b;
          } else {
            costCents = e.netAmountCents;
          }
          sourceSuggestion = s;
          applySuggestionResult(s);
          // Steuer-Anlage (§13b) nur für Kosten unter §19 ohne eigenen §13b-Schalter.
          if (s.source?.isKleinunternehmer && !s.source.reverseCharge13b) {
            taxCandidates = await loadTaxCandidates(expenseId ?? "");
            const pre = page.url.searchParams.get("taxFor");
            if (pre && taxCandidates.some((c) => c.id === pre)) {
              taxForId = pre;
              applyTaxFor();
            }
          }
        }
      } catch (e) {
        loadError =
          "Die Kosten lassen sich nicht als Anlage übernehmen: " + String(e).replace(/^Error:\s*/, "");
      }
    }
    loaded = true;
  });

  /** Anlagen aus §13b-Kosten unter §19 (Hauptanlagen für eine Steuer-Anlage). */
  async function loadTaxCandidates(ownExpenseId: string): Promise<Asset[]> {
    try {
      const rc = (await expensesList({ includeCanceled: false })).filter(
        (x) => x.reverseCharge13b === 1 && x.status !== "canceled" && x.id !== ownExpenseId,
      );
      const found = await Promise.all(
        rc.map(async (x) => {
          const d = await expensesGet(x.id);
          const assetId = d?.expense.capitalizedAsAssetId;
          if (!d || !assetId) return null;
          const s = await assetsSuggestMethod(d.expense.category, d.expense.netAmountCents, x.id);
          if (!s.source?.isKleinunternehmer) return null;
          const a = (await assetsGet(assetId))?.asset;
          return a && a.disposed === 0 ? a : null;
        }),
      );
      return found
        .filter((a): a is Asset => a !== null)
        .sort((a, b) => b.acquisitionDate.localeCompare(a.acquisitionDate));
    } catch {
      // Nur ein Angebot zur Vorbelegung — ohne Liste bleibt das Formular wie bisher.
      return [];
    }
  }

  /** Übernimmt Methode, Nutzungsdauer, AfA-Kategorie und Anteil der Hauptanlage. */
  function applyMainAsset(main: Asset) {
    depreciationMethod = main.depreciationMethod as DepreciationMethod;
    usefulLifeYears = main.usefulLifeYears ?? null;
    afaCategory = main.afaCategory ?? "";
    businessSharePercent = main.businessSharePercent;
    suggestionReason =
      `Wie die Anschaffung ${main.assetNumber} (${depreciationMethodShort(main.depreciationMethod)}): ` +
      "Die selbst geschuldete Steuer gehört zu ihren Anschaffungskosten und wird genauso " +
      "abgeschrieben" +
      (main.depreciationMethod === "gwg_sofort"
        ? "."
        : " — auch wenn ihr eigener Betrag unter der GWG-Grenze liegt.");
  }

  function applyTaxFor() {
    const main = taxFor;
    if (!main) {
      // Verknüpfung gelöst: zurück zur Vorbelegung aus den Kosten.
      if (sourceSuggestion?.source) acquisitionDate = sourceSuggestion.source.acquisitionDate;
      if (sourceSuggestion) applySuggestionResult(sourceSuggestion);
      if (autoNote && notes === autoNote) notes = "";
      autoNote = "";
      return;
    }
    acquisitionDate = main.acquisitionDate.slice(0, 10);
    applyMainAsset(main);
    const note = `Selbst geschuldete Steuer (§13b) zu ${main.assetNumber}`;
    if (!notes.trim() || notes === autoNote) {
      notes = note;
      autoNote = note;
    }
    errors = { ...errors, date: undefined, life: undefined, share: undefined };
  }

  // AfA-Kategorie gewählt → Methode + Nutzungsdauer passend setzen.
  // Kategorien mit BMF-Sonderregel (digitale Wirtschaftsgüter, Nutzungsdauer
  // 1 Jahr) gehören auf die Computer-Sonderregel = Sofortabschreibung im
  // Anschaffungsjahr — NICHT auf lineare AfA über „1 Jahr" (die würde sich
  // monatsgenau auf zwei Kalenderjahre verteilen → widersinnig). Alle anderen
  // Kategorien übernehmen ihre Nutzungsdauer in die lineare AfA.
  function onCategorySelect() {
    const cat = table?.categories.find((c) => c.code === afaCategory);
    if (!cat) return;
    if (cat.specialRule) {
      depreciationMethod = "computer_special_2021";
    } else {
      depreciationMethod = "linear";
      usefulLifeYears = cat.usefulLifeYears;
    }
    errors.life = undefined;
  }

  async function applySuggestion() {
    // Steuer-Anlage: Methode der Hauptanlage statt eines eigenen Vorschlags (kein GWG,
    // wenn die Hauptanlage keins ist).
    if (taxFor) {
      applyMainAsset(taxFor);
      return;
    }
    if ((costCents ?? 0) <= 0) {
      errors = { ...errors, cost: "Bitte zuerst die Anschaffungskosten eingeben." };
      await tick();
      costInput?.focus();
      return;
    }
    try {
      // Mit Quell-Kosten prüft der Server die GWG-Grenze an deren Nettobetrag.
      applySuggestionResult(
        await assetsSuggestMethod(sourceExpenseCategory, costCents ?? 0, expenseId),
      );
    } catch (e) {
      flash("Vorschlag fehlgeschlagen: " + String(e), "error");
    }
  }

  function applySuggestionResult(s: MethodSuggestion) {
    depreciationMethod = s.method;
    if (s.usefulLifeYears != null) usefulLifeYears = s.usefulLifeYears;
    if (s.afaCategory) afaCategory = s.afaCategory;
    suggestionReason = s.reason;
  }

  async function validate(): Promise<boolean> {
    const e: typeof errors = {};
    if (!label.trim()) e.label = "Bitte eine Bezeichnung angeben.";
    if (!acquisitionDate) e.date = "Bitte das Anschaffungsdatum angeben.";
    const costOk = costInput?.validate() ?? true;
    if (costOk && (costCents ?? 0) <= 0) e.cost = "Die Anschaffungskosten müssen größer als 0 sein.";
    const lifeOk = isLinear ? (lifeInput?.validate() ?? true) : true;
    if (isLinear && lifeOk && (usefulLifeYears == null || usefulLifeYears <= 0)) {
      e.life = "Bei linearer Abschreibung ist die Nutzungsdauer nötig (z. B. 3 Jahre).";
    }
    const shareOk = shareInput?.validate() ?? true;
    if (shareOk && businessSharePercent == null) e.share = "Bitte den betrieblichen Anteil angeben (0 bis 100 %).";
    errors = e;
    await tick();
    if (e.label) labelEl?.focus();
    else if (e.date) dateEl?.focus();
    else if (!costOk || e.cost) costInput?.focus();
    else if (!lifeOk || e.life) lifeInput?.focus();
    else if (!shareOk || e.share) shareInput?.focus();
    return costOk && lifeOk && shareOk && Object.keys(e).length === 0;
  }

  async function save() {
    if (busy) return;
    saveError = "";
    if (!(await validate())) return;
    busy = true;
    try {
      const input: AssetInputDto = {
        label: label.trim(),
        acquisitionDate,
        acquisitionCostCents: costCents ?? 0,
        expenseId: expenseId || null,
        vendorContactId: vendorContactId || null,
        depreciationMethod,
        usefulLifeYears: isLinear ? usefulLifeYears : null,
        afaCategory: afaCategory || null,
        businessSharePercent: businessSharePercent ?? 100,
        notes: notes.trim() || null,
      };
      const detail = editId
        ? await assetsUpdate(editId, input)
        : await assetsCreate(input);
      flash(editId ? "Anschaffung geändert." : "Anschaffung angelegt: " + detail.asset.assetNumber);
      await goto(`/assets/${detail.asset.id}`);
    } catch (e) {
      saveError = String(e).replace(/^Error:\s*/, "");
      await tick();
      document.getElementById("asset-save-error")?.focus();
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/assets" backLabel="Anschaffungen" title={editId ? "Anschaffung bearbeiten" : "Neue Anschaffung"}>
  {#snippet actions()}
    {#if loaded}
      <a class="btn-secondary" href="/assets">Abbrechen</a>
      <button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>
        {busy ? "Speichert …" : editId ? "Änderungen speichern" : "Anschaffung anlegen"}
      </button>
    {/if}
    <HelpAnchor slug="afa-grundlagen" />
  {/snippet}
</PageBar>
<Banner kind="warning">
  Sobald die erste Abschreibung gebucht ist, lässt sich eine Anschaffung
  <strong>nicht mehr ändern</strong> (gesetzliche Vorgabe). Bis dahin kannst du sie
  korrigieren.
</Banner>

{#if loadError}
  <Banner>{loadError}</Banner>
{/if}
{#if saveError}
  <div id="asset-save-error" tabindex="-1" class="save-error">
    <Banner>
      <strong>Nicht gespeichert:</strong> {saveError}
    </Banner>
  </div>
{/if}
{#if sourceExpenseNumber && !editId}
  <p class="src-note">
    Übernahme aus den Kosten <DocNo value={sourceExpenseNumber} href={`/expenses/${expenseId}`} inline />:
    Anschaffungskosten und -datum kommen aus der Kostenposition.
  </p>
{/if}

<section class="card">
  <form novalidate onsubmit={(e) => { e.preventDefault(); save(); }}>
    <p class="legend-req">Felder mit <span class="req" aria-hidden="true">*</span> sind Pflicht.</p>
    <div class="grid">
      {#if !editId && taxCandidates.length > 0}
        <div class="field span2 tax-for">
          <label for="asset-tax-for">Selbst geschuldete Steuer (§13b) zu Anschaffung</label>
          <select
            id="asset-tax-for"
            class="kb-input"
            bind:value={taxForId}
            onchange={applyTaxFor}
            aria-describedby="asset-tax-for-hint"
          >
            <option value="">— keine, eigene Anschaffung —</option>
            {#each taxCandidates as c (c.id)}
              <option value={c.id}>{c.assetNumber} · {c.label} (angeschafft {date(c.acquisitionDate)})</option>
            {/each}
          </select>
          <p id="asset-tax-for-hint" class="msg hint">
            Ist diese Kostenposition deine Zahlung der Steuer nach §13b an das Finanzamt, wähle
            die Anschaffung aus der Lieferantenrechnung. Anschaffungsdatum, Abschreibungsmethode,
            Nutzungsdauer, AfA-Kategorie und betrieblicher Anteil kommen dann von ihr.
          </p>
        </div>
      {/if}
      <div class="field span2">
        <label for="asset-label">Bezeichnung<span class="req" aria-hidden="true">*</span></label>
        <input
          id="asset-label"
          class="kb-input"
          type="text"
          autocomplete="off"
          bind:this={labelEl}
          bind:value={label}
          oninput={() => (errors.label = undefined)}
          required
          aria-required="true"
          aria-invalid={errors.label ? "true" : undefined}
          aria-describedby={errors.label ? "asset-label-err" : "asset-label-hint"}
        />
        {#if errors.label}
          <p id="asset-label-err" class="msg err">{errors.label}</p>
        {:else}
          <p id="asset-label-hint" class="msg hint">z. B. „MacBook Pro 14, 2024“</p>
        {/if}
      </div>
      <div class="field">
        <label for="asset-date">Anschaffungsdatum<span class="req" aria-hidden="true">*</span></label>
        <input
          id="asset-date"
          class="kb-input kb-w-date"
          type="date"
          bind:this={dateEl}
          bind:value={acquisitionDate}
          max={maxDate}
          required
          aria-required="true"
          aria-invalid={errors.date ? "true" : undefined}
          aria-describedby={errors.date ? "asset-date-err" : costFromSource ? "asset-date-hint" : undefined}
        />
        {#if errors.date}
          <p id="asset-date-err" class="msg err">{errors.date}</p>
        {:else if taxFor}
          <p id="asset-date-hint" class="msg hint">Vorbelegt mit dem Anschaffungsdatum von <DocNo value={taxFor.assetNumber} />.</p>
        {:else if costFromSource}
          <p id="asset-date-hint" class="msg hint">Vorbelegt mit dem Belegdatum der Kosten.</p>
        {/if}
      </div>
      <div class="cost-field">
        <MoneyInput
          bind:this={costInput}
          bind:cents={costCents}
          fieldClass="kb-w-amount"
          label="Anschaffungskosten"
          required
          readonly={costFromSource}
          error={errors.cost}
          onchange={() => (errors.cost = undefined)}
          hint={sourceIsKlein === true && sourceReverseCharge
            ? "Rechnungsbetrag aus den Kosten (§13b, ohne Umsatzsteuer)."
            : sourceIsKlein === true
              ? "Brutto aus den Kosten (Kleinunternehmer: die Umsatzsteuer gehört dazu)."
              : sourceIsKlein === false
                ? "Netto aus den Kosten (Regelbesteuerung: die Vorsteuer ziehst du gesondert ab)."
                : "Als Kleinunternehmer brutto, bei Regelbesteuerung netto."}
        />
        {#if sourceIsKlein === true && sourceReverseCharge}
          <p class="field-note">
            Die selbst geschuldete Steuer (§13b) erfasst du bei der Zahlung an das Finanzamt als
            eigene Kosten (USt-Betrag 0, ohne §13b-Schalter) und übernimmst sie als eigene Anlage.
            Dort wählst du diese Anschaffung unter „Selbst geschuldete Steuer (§13b) zu Anschaffung“;
            Anschaffungsdatum, Methode und Nutzungsdauer werden übernommen.
          </p>
        {:else if sourceIsKlein !== null}
          <p class="field-note">
            Ein anderer Betrag ist nicht möglich. Sammelrechnungen vorher in getrennte Kosten
            aufteilen. Die GWG-Grenze gilt immer für den Betrag ohne Umsatzsteuer.
          </p>
        {:else}
          <p class="field-note">Die GWG-Grenze gilt immer für den Betrag ohne Umsatzsteuer.</p>
        {/if}
      </div>
      <!-- U4 (Warteliste 15): durchsuchbar ab 8 Lieferanten, deutsche Sortierung. -->
      <ContactSelect
        bind:value={vendorContactId}
        contacts={vendors}
        label="Lieferant (Kontakt, optional)"
        id="asset-vendor"
        placeholder="— ohne Kontakt —"
      />
      <div class="field">
        <label for="asset-afa-cat">AfA-Kategorie (optional)</label>
        <select id="asset-afa-cat" class="kb-input" bind:value={afaCategory} onchange={onCategorySelect}>
          <option value="">— keine —</option>
          {#each table?.categories ?? [] as c (c.code)}
            <option value={c.code}>{c.label} ({c.usefulLifeYears} J.)</option>
          {/each}
        </select>
      </div>
      <div class="field">
        <label for="asset-method">Abschreibungsmethode</label>
        <select
          id="asset-method"
          class="kb-input"
          bind:value={depreciationMethod}
          aria-describedby={gwgMismatch ? "asset-gwg-note" : suggestionReason ? "asset-suggestion" : undefined}
        >
          {#each DEPRECIATION_METHODS as m}<option value={m.value}>{m.label}</option>{/each}
        </select>
        {#if gwgMismatch && taxFor}
          <p id="asset-gwg-note" class="msg note">
            <DocNo value={taxFor.assetNumber} /> ist kein geringwertiges Wirtschaftsgut. Die Steuer dazu gehört zu
            denselben Anschaffungskosten und wird genauso abgeschrieben, nicht sofort.
          </p>
        {/if}
      </div>
      {#if isLinear}
        <QuantityInput
          bind:this={lifeInput}
          bind:value={usefulLifeYears}
          fieldClass="kb-w-short"
          maxFractionDigits={1}
          unit="Jahre"
          noun="Nutzungsdauer"
          label="Nutzungsdauer"
          hint="z. B. 3"
          required
          error={errors.life}
          onchange={() => (errors.life = undefined)}
        />
      {:else}
        <div class="field">
          <span class="lbl">Nutzungsdauer</span>
          <Banner kind="info" flush>Sofortabschreibung im Anschaffungsjahr — keine Nutzungsdauer nötig.</Banner>
        </div>
      {/if}
      {#if suggestionReason}
        <!-- V3-18/V3-23: Hinweis über Banner, Belegnummern als DocNo (design.md §4.6/§4.10). -->
        <div class="span2">
          <Banner kind="info" id="asset-suggestion">
            <strong>Vorschlag:</strong>{" "}{#each splitDocNumbers(suggestionReason) as p, i (i)}{#if p.docNo}<DocNo value={p.text} />{:else}{p.text}{/if}{/each}
          </Banner>
        </div>
      {/if}
      <div class="span2 share">
        <QuantityInput
          bind:this={shareInput}
          bind:value={businessSharePercent}
          maxFractionDigits={2}
          max={100}
          unit="%"
          noun="Anteil"
          label="Betrieblicher Anteil"
          required
          error={errors.share}
          onchange={() => (errors.share = undefined)}
          hint="0 bis 100 %. Privat genutzte Anteile werden nicht abgeschrieben."
        />
        <p class="share-basis">
          Abschreibungsbasis: <strong class="kb-num">{euro(businessBookValue)}</strong>
          von <span class="kb-num">{euro(costCents ?? 0)}</span>
        </p>
      </div>
      <div class="field span2">
        <label for="asset-notes">Notiz (optional)</label>
        <textarea id="asset-notes" class="kb-input" rows="3" bind:value={notes}></textarea>
      </div>
    </div>

    {#if !editId}
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={() => { if (!busy) void applySuggestion(); }} aria-disabled={busy ? "true" : undefined}>Methode vorschlagen</button>
      </div>
    {/if}
    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end"><button type="button" class="btn-primary" onclick={save} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>{busy ? "Speichert …" : editId ? "Änderungen speichern" : "Anschaffung anlegen"}</button></div>
  </form>
</section>

<style>
  /* .card / .kb-input: globale Definitionen aus tokens.css; Hinweise über Banner (V3-18). */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); align-items: start; }
  @media (max-width: 600px) { .grid { grid-template-columns: 1fr; } }
  .span2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field > label:first-child, .field > .lbl { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .legend-req { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .msg.hint { color: var(--text-muted); }
  .msg.note { color: var(--status-warning-fg); background: var(--status-warning-bg); border-left: 3px solid var(--status-warning-line); padding: var(--sp-1) var(--sp-2); }
  .grid :global(.contact-select.with-label) { margin-bottom: 0; }
  .grid :global(.contact-select label) { color: var(--text); }
  .tax-for { padding-bottom: var(--sp-3); border-bottom: var(--rule); }
  .grid :global(.money-field.with-label), .grid :global(.qty-field.with-label) { margin-bottom: 0; }
  .grid :global(.money-field label), .grid :global(.qty-field label) { color: var(--text); }
  .cost-field { min-width: 0; }
  .field-note { font-size: var(--fs-sm); color: var(--text-muted); margin: var(--sp-1) 0 0; line-height: 1.45; }
  .share { display: flex; flex-wrap: wrap; gap: var(--sp-2) var(--sp-5); align-items: flex-start; }
  .share :global(.qty-field) { flex: 0 1 14rem; }
  .share-basis { margin: 1.9rem 0 0; color: var(--text-muted); }
  @media (max-width: 600px) { .share-basis { margin-top: 0; } }
  .src-note { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 var(--sp-3); }
  .save-error:focus { outline: none; }
  .save-error:focus-visible { outline: var(--focus-width) solid var(--focus-ring); }
  .actions { display: flex; gap: var(--sp-3); margin-top: var(--sp-5); }
</style>
