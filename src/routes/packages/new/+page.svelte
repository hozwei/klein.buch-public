<script lang="ts">
  // Block P2b — Neues Paket anlegen (= erste Revision).
  // U1: Preis über MoneyInput (Integer-Cent, ADR 0041 Nr. 15/16), Feldfehler
  // am Feld statt Toast, Fokus aufs erste fehlerhafte Feld.
  // U4 (U1-067, U3-038): Hauptaktion in der PageBar und am Formularende
  // (`.kb-form-end`), „PDF-Vorschau“ als Knopf; Einheit als Auswahl im Klartext
  // (U1-060), gespeichert wird der Code nach UN/ECE Rec 20 (Standard „LS“ = Pauschale).
  // U4 (V2-07/V2-17): Ladefehler als Banner mit „Erneut laden“ statt Formular ohne Kategorien;
  // Fehlertexte als Du-Imperativ.
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "#lib/PageBar.svelte";
  import RequiredLegend from "#lib/RequiredLegend.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { mdPreview } from "#lib/markdownPreview.ts";
  import { unitOptions } from "#lib/labels.ts";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import {
    packageCategoriesList,
    packagesCreate,
    packagePreview,
    downloadUrl,
    sellerProfileGet,
  } from "#lib/api.ts";
  import type { PackageCategory } from "#lib/types.ts";

  let categories = $state<PackageCategory[]>([]);
  let isKlein = $state(true);
  let loading = $state(true);
  /** U4 (V2-07): ohne Kategorien und §19-Stand kein Formular, sondern Fehler + „Erneut laden“. */
  let loadError = $state<string | null>(null);

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  /** Netto-Preis in Cent (MoneyInput); null = leer oder ungültig. */
  let priceCents = $state<number | null>(null);
  /** Einheit als UN/ECE-Code (XRechnung BT-130); Anzeige im Klartext. */
  let unit = $state("LS");

  let saving = $state(false);
  let previewing = $state(false);

  let errors = $state<{ name?: string; title?: string }>({});
  let nameEl: HTMLInputElement | undefined = $state();
  let titleEl: HTMLInputElement | undefined = $state();
  let priceInput: MoneyInput | undefined = $state();

  async function load() {
    loading = true;
    loadError = null;
    try {
      const [cats, seller] = await Promise.all([packageCategoriesList(), sellerProfileGet()]);
      categories = cats;
      if (seller) isKlein = seller.isKleinunternehmer === 1;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  const cents = () => priceCents ?? 0;

  function checkForm(): boolean {
    const e: { name?: string; title?: string } = {};
    if (!name.trim()) e.name = "Bitte gib einen Paket-Namen ein.";
    if (!title.trim()) e.title = "Bitte gib einen Positions-Titel ein.";
    errors = e;
    const priceOk = (priceInput?.validate() ?? false) && priceCents !== null;
    if (e.name) nameEl?.focus();
    else if (e.title) titleEl?.focus();
    else if (!priceOk) priceInput?.focus();
    return !e.name && !e.title && priceOk;
  }

  async function save() {
    if (saving) return;
    if (!checkForm()) return;
    saving = true;
    try {
      const pkg = await packagesCreate(categoryId || null, name.trim(), {
        title: title.trim(),
        bodyMarkup: body,
        defaultUnitPriceCents: cents(),
        unitCode: unit || "LS",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Paket angelegt.");
      goto(`/packages/${pkg.id}`);
    } catch (e) {
      flash(`Paket anlegen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      saving = false;
    }
  }

  async function preview() {
    if (previewing) return;
    if (!title.trim()) {
      errors = { ...errors, title: "Gib für die Vorschau zuerst einen Positions-Titel ein." };
      titleEl?.focus();
      return;
    }
    previewing = true;
    try {
      const ticket = await packagePreview(title.trim(), body, cents());
      openPdfPreview(downloadUrl(ticket), "Paket-Vorschau", ticket.fileName);
    } catch (e) {
      flash(`PDF-Vorschau fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      previewing = false;
    }
  }
</script>

<PageBar back="/packages" backLabel="Pakete" title="Neues Paket">
  {#snippet actions()}
    <Button variant="secondary" href="/packages">Abbrechen</Button>
    {#if !loading && !loadError}
      <Button variant="primary" busy={saving} onclick={save}>
        {saving ? "Speichert …" : "Paket anlegen"}
      </Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <LoadingState />
{:else if loadError}
  <Banner title="Kategorien und Firmendaten konnten nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else}
  <div class="editor">
    <section class="card" aria-labelledby="pkg-data-h">
      <h2 id="pkg-data-h">Paket</h2>
      <RequiredLegend />
      <!-- U2: Name und Kategorie ab rund 37rem nebeneinander (tokens.css .kb-form-grid). -->
      <div class="kb-form-grid">
        <div class="field">
          <label for="pkg-name">Paket-Name (intern)<span class="req" aria-hidden="true">*</span></label>
          <input
            id="pkg-name"
            class="kb-input"
            type="text"
            autocomplete="off"
            required
            aria-required="true"
            aria-invalid={errors.name ? "true" : undefined}
            aria-describedby={errors.name ? "pkg-name-err" : undefined}
            bind:this={nameEl}
            bind:value={name}
            oninput={() => errors.name && (errors = { ...errors, name: undefined })}
            placeholder="z. B. Hochzeit klein"
          />
          {#if errors.name}<p id="pkg-name-err" class="err">{errors.name}</p>{/if}
        </div>
        <div class="field">
          <label for="pkg-cat">Kategorie</label>
          <select id="pkg-cat" class="kb-input" bind:value={categoryId}>
            <option value="">Ohne Kategorie</option>
            {#each categories as c (c.id)}
              <option value={c.id}>{c.name}</option>
            {/each}
          </select>
        </div>
        <div class="field kb-span-all">
          <label for="pkg-title">Positions-Titel (auf dem Beleg)<span class="req" aria-hidden="true">*</span></label>
          <input
            id="pkg-title"
            class="kb-input"
            type="text"
            autocomplete="off"
            required
            aria-required="true"
            aria-invalid={errors.title ? "true" : undefined}
            aria-describedby={errors.title ? "pkg-title-err" : undefined}
            bind:this={titleEl}
            bind:value={title}
            oninput={() => errors.title && (errors = { ...errors, title: undefined })}
            placeholder="z. B. Fotopaket „Hochzeit klein“"
          />
          {#if errors.title}<p id="pkg-title-err" class="err">{errors.title}</p>{/if}
        </div>
        <MoneyInput
          bind:this={priceInput}
          bind:cents={priceCents}
          fieldClass="kb-w-amount"
          id="pkg-price"
          label="Netto-Preis"
          required
          hint={isKlein
            ? "§19: Preis ist netto, es wird keine Umsatzsteuer ausgewiesen. Beim Einfügen in einen Beleg gilt der Steuerstatus am Datum dieses Belegs."
            : "Beim Einfügen in einen Beleg gilt der Steuerstatus am Datum dieses Belegs."}
        />
        <div class="field">
          <label for="pkg-unit">Einheit</label>
          <select id="pkg-unit" class="kb-input kb-w-medium" bind:value={unit} aria-describedby="pkg-unit-hint">
            {#each unitOptions(unit) as u (u.value)}<option value={u.value}>{u.label}</option>{/each}
          </select>
          <p id="pkg-unit-hint" class="hint">Erscheint auf dem Beleg; in der E-Rechnung als Einheiten-Code.</p>
        </div>
      </div>
    </section>

    <section class="card" aria-labelledby="pkg-desc-h">
      <div class="md-head">
        <h2 id="pkg-desc-h">Beschreibung</h2>
        <Button variant="secondary" busy={previewing} onclick={preview} ariaDescribedby="pkg-pdf-hint">
          {previewing ? "Öffnet …" : "PDF-Vorschau"}
        </Button>
      </div>
      <p class="md-help" id="pkg-pdf-hint">
        Formatierung: <code># Überschrift</code>, <code>**fett**</code>, <code>*kursiv*</code>,
        Listen mit <code>-</code>, Tabellen (GFM). Die Vorschau rechts ist eine Annäherung;
        die PDF-Vorschau zeigt das exakte Ergebnis auf dem Beleg.
      </p>
      <div class="md-grid">
        <MarkdownEditor bind:value={body} placeholder="# Leistungen" />
        <div class="md-preview-wrap">
          <p class="md-preview-label" id="pkg-md-preview-label">Vorschau</p>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="md-preview" role="region" aria-labelledby="pkg-md-preview-label" tabindex="0">
            {@html mdPreview(body)}
          </div>
        </div>
      </div>
    </section>

    <!-- U1/U4: Hauptaktion am Formularende (≤ 600 px Breite, ≤ 500 px Höhe; ADR 0041 Nr. 35/49). -->
    <div class="kb-form-end">
      <Button variant="secondary" href="/packages">Abbrechen</Button>
      <Button variant="primary" busy={saving} onclick={save}>
        {saving ? "Speichert …" : "Paket anlegen"}
      </Button>
    </div>
  </div>
{/if}

<style>
  .editor { max-width: 60rem; }
  /* .card global aus tokens.css. */
  .card > h2:first-child, .md-head h2 { font-size: var(--type-heading); margin: 0 0 var(--sp-2); }
  .field { margin-bottom: var(--sp-4); min-width: 0; }
  .field label { display: block; margin-bottom: var(--sp-1); font-size: var(--type-small); font-weight: 600; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .field input, .field select { scroll-margin-block: var(--sp-6); }
  .err { margin: var(--sp-1) 0 0; font-size: var(--type-small); font-weight: 600; color: var(--status-danger-fg); }
  .hint { margin: var(--sp-1) 0 0; font-size: var(--type-small); color: var(--text-muted); }
  .md-head { display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: center; justify-content: space-between; }
  .md-head h2 { margin: 0; }
  .md-help { font-size: var(--type-small); color: var(--text-muted); line-height: var(--lh-body); }
  .md-help code {
    font-family: var(--font-mono);
    font-size: 0.85em;
    background: var(--surface-sunken);
    padding: 1px 5px;
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
  }
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); }
  .md-grid > :global(*) { min-width: 0; }
  @media (max-width: 600px) { .md-grid { grid-template-columns: 1fr; } }
  .md-preview-label { margin: 0 0 var(--sp-1); font-size: var(--type-small); font-weight: 600; }
  /* U1 (2.5.8): fokussierbare Vorschau auch leer mindestens --target hoch. */
  .md-preview {
    border: var(--rule);
    min-height: max(var(--target), 4.5rem);
    border-radius: var(--radius-control);
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-card);
    overflow: auto;
    line-height: var(--lh-body);
  }
</style>
