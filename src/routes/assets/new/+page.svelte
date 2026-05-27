<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import {
    contactsList,
    assetsAfaTable,
    assetsCreate,
    assetsUpdate,
    assetsGet,
    assetsSuggestMethod,
    expensesGet,
  } from "$lib/api";
  import type {
    Contact,
    AfaTabellen,
    DepreciationMethod,
    AssetInputDto,
  } from "$lib/types";
  import { DEPRECIATION_METHODS } from "$lib/labels";
  import { euro } from "$lib/format";
  import { flash } from "$lib/toast.svelte";

  let vendors = $state<Contact[]>([]);
  let table = $state<AfaTabellen | null>(null);
  let busy = $state(false);
  let loaded = $state(false);

  // Edit-/Quell-Kontext
  let editId = $state<string | null>(null);
  let expenseId = $state<string | null>(null);
  let sourceExpenseCategory = $state<string | null>(null);

  // Formularfelder
  let label = $state("");
  let acquisitionDate = $state(new Date().toISOString().slice(0, 10));
  let costEuros = $state<number>(0);
  let vendorContactId = $state<string>("");
  let depreciationMethod = $state<DepreciationMethod>("linear");
  let usefulLifeYears = $state<number | null>(null);
  let afaCategory = $state<string>("");
  let businessSharePercent = $state<number>(100);
  let notes = $state("");

  const maxDate = new Date().toISOString().slice(0, 10);

  let costCents = $derived(Math.round(costEuros * 100));
  let isLinear = $derived(depreciationMethod === "linear");
  let businessBookValue = $derived(Math.round((costCents * businessSharePercent) / 100));

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");
      table = await assetsAfaTable();
    } catch (e) {
      flash("Daten konnten nicht geladen werden: " + String(e), "error");
    }

    editId = $page.url.searchParams.get("id");
    expenseId = $page.url.searchParams.get("expenseId");

    if (editId) {
      try {
        const detail = await assetsGet(editId);
        if (!detail) {
          flash("Anlage nicht gefunden.", "error");
          await goto("/assets");
          return;
        }
        const a = detail.asset;
        if (a.lockedAt || a.disposed === 1) {
          flash("Diese Anlage ist festgeschrieben und kann nicht mehr bearbeitet werden.", "error");
          await goto(`/assets/${editId}`);
          return;
        }
        label = a.label;
        acquisitionDate = a.acquisitionDate.slice(0, 10);
        costEuros = a.acquisitionCostCents / 100;
        vendorContactId = a.vendorContactId ?? "";
        depreciationMethod = a.depreciationMethod as DepreciationMethod;
        usefulLifeYears = a.usefulLifeYears ?? null;
        afaCategory = a.afaCategory ?? "";
        businessSharePercent = a.businessSharePercent;
        notes = a.notes ?? "";
        expenseId = a.expenseId;
      } catch (e) {
        flash("Laden fehlgeschlagen: " + String(e), "error");
      }
    } else if (expenseId) {
      // Aus einer Kosten-Position aktivieren: Felder vorbefüllen + Vorschlag holen.
      try {
        const detail = await expensesGet(expenseId);
        if (detail) {
          const e = detail.expense;
          label = e.description;
          costEuros = e.netAmountCents / 100;
          vendorContactId = e.vendorContactId ?? "";
          sourceExpenseCategory = e.category;
          await applySuggestion();
        }
      } catch (e) {
        flash("Quell-Kosten konnten nicht geladen werden: " + String(e), "error");
      }
    }
    loaded = true;
  });

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
  }

  async function applySuggestion() {
    if (costCents <= 0) {
      flash("Bitte zuerst die Anschaffungskosten eingeben.", "error");
      return;
    }
    try {
      const s = await assetsSuggestMethod(sourceExpenseCategory, costCents);
      depreciationMethod = s.method;
      if (s.usefulLifeYears != null) usefulLifeYears = s.usefulLifeYears;
      if (s.afaCategory) afaCategory = s.afaCategory;
      flash(s.reason);
    } catch (e) {
      flash("Vorschlag fehlgeschlagen: " + String(e), "error");
    }
  }

  async function save() {
    if (!label.trim()) {
      flash("Bezeichnung ist erforderlich.", "error");
      return;
    }
    if (costCents <= 0) {
      flash("Anschaffungskosten müssen größer als 0 sein.", "error");
      return;
    }
    if (depreciationMethod === "linear" && (usefulLifeYears == null || usefulLifeYears <= 0)) {
      flash("Bei linearer Abschreibung ist die Nutzungsdauer (Jahre) erforderlich.", "error");
      return;
    }
    busy = true;
    try {
      const input: AssetInputDto = {
        label: label.trim(),
        acquisitionDate,
        acquisitionCostCents: costCents,
        expenseId: expenseId || null,
        vendorContactId: vendorContactId || null,
        depreciationMethod,
        usefulLifeYears: isLinear ? usefulLifeYears : null,
        afaCategory: afaCategory || null,
        businessSharePercent,
        notes: notes.trim() || null,
      };
      const detail = editId
        ? await assetsUpdate(editId, input)
        : await assetsCreate(input);
      flash(editId ? "Anlage geändert." : "Anlage angelegt: " + detail.asset.assetNumber);
      await goto(`/assets/${detail.asset.id}`);
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/assets" backLabel="Anschaffungen" title={editId ? "Anschaffung bearbeiten" : "Neue Anschaffung"}>
  {#snippet actions()}
    {#if loaded}
      <a class="btn-secondary" href="/assets">Abbrechen</a>
      <button class="btn-primary" onclick={save} disabled={busy}>
        {busy ? "Speichere …" : editId ? "Änderungen speichern" : "Anschaffung anlegen"}
      </button>
    {/if}
    <HelpAnchor slug="afa-grundlagen" />
  {/snippet}
</PageBar>
<p class="caveat">
  Sobald die erste Abschreibung gebucht ist, lässt sich eine Anschaffung
  <strong>nicht mehr ändern</strong> (gesetzliche Vorgabe). Bis dahin kannst du sie
  korrigieren.
</p>

<section class="card">
  <div class="grid">
    <label class="span2">
      Bezeichnung
      <input type="text" bind:value={label} placeholder="z. B. MacBook Pro 14, 2024" />
    </label>
    <label>
      Anschaffungsdatum
      <input type="date" bind:value={acquisitionDate} max={maxDate} />
    </label>
    <label>
      Anschaffungskosten netto (€)
      <input type="number" step="0.01" min="0" bind:value={costEuros} />
    </label>
    <label>
      Lieferant (Kontakt, optional)
      <select bind:value={vendorContactId}>
        <option value="">— ohne Kontakt —</option>
        {#each vendors as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
      </select>
    </label>
    <label>
      AfA-Kategorie (optional)
      <select bind:value={afaCategory} onchange={onCategorySelect}>
        <option value="">— keine —</option>
        {#each table?.categories ?? [] as c (c.code)}
          <option value={c.code}>{c.label} ({c.usefulLifeYears} J.)</option>
        {/each}
      </select>
    </label>
    <label>
      Abschreibungsmethode
      <select bind:value={depreciationMethod}>
        {#each DEPRECIATION_METHODS as m}<option value={m.value}>{m.label}</option>{/each}
      </select>
    </label>
    {#if isLinear}
      <label>
        Nutzungsdauer (Jahre)
        <input type="number" step="0.5" min="0.5" bind:value={usefulLifeYears} placeholder="z. B. 3" />
      </label>
    {:else}
      <div class="info-cell">
        <span class="lbl">Hinweis</span>
        <span class="info">Sofortabschreibung im Anschaffungsjahr — keine Nutzungsdauer nötig.</span>
      </div>
    {/if}
    <label class="span2">
      Betrieblicher Anteil: <strong>{businessSharePercent}%</strong>
      <input type="range" min="0" max="100" step="5" bind:value={businessSharePercent} />
      <span class="lbl-hint">
        Privat genutzte Anteile werden nicht abgeschrieben. Abschreibungsbasis:
        <strong>{euro(businessBookValue)}</strong> von {euro(costCents)}.
      </span>
    </label>
    <label class="span2">
      Notiz (optional)
      <textarea rows="2" bind:value={notes}></textarea>
    </label>
  </div>

  {#if !editId}
    <div class="actions">
      <button class="btn-secondary" onclick={applySuggestion} disabled={busy}>Methode vorschlagen</button>
    </div>
  {/if}
</section>

<style>
  /* .caveat / .card entfernt — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  .lbl-hint { font-weight: 400; color: #6b7280; font-size: 0.78rem; }
  .info-cell { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; justify-content: center; }
  .info-cell .lbl { color: #6b7280; }
  .info-cell .info { font-size: 0.82rem; color: #3730a3; background: #eef2ff; border: 1px solid #c7d2fe; padding: 0.4rem 0.6rem; border-radius: 4px; }
  input, select, textarea { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  input[type="range"] { padding: 0; }
  .actions { display: flex; gap: 0.75rem; margin-top: 1.25rem; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
