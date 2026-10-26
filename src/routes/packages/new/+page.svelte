<script lang="ts">
  // Block P2b — Neues Paket anlegen (= erste Revision).
  // U1: Preis über MoneyInput (Integer-Cent, ADR 0041 Nr. 15/16), Feldfehler
  // am Feld statt Toast, Fokus aufs erste fehlerhafte Feld.
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import PageBar from "#lib/PageBar.svelte";
  import Button from "#lib/Button.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { mdPreview } from "#lib/markdownPreview.ts";
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

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  /** Netto-Preis in Cent (MoneyInput); null = leer oder ungültig. */
  let priceCents = $state<number | null>(null);
  let unit = $state("Pauschal");

  let saving = $state(false);
  let previewing = $state(false);

  let errors = $state<{ name?: string; title?: string }>({});
  let nameEl: HTMLInputElement | undefined = $state();
  let titleEl: HTMLInputElement | undefined = $state();
  let priceInput: MoneyInput | undefined = $state();

  onMount(async () => {
    try {
      const [cats, seller] = await Promise.all([packageCategoriesList(), sellerProfileGet()]);
      categories = cats;
      if (seller) isKlein = seller.isKleinunternehmer === 1;
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading = false;
    }
  });

  const cents = () => priceCents ?? 0;

  function checkForm(): boolean {
    const e: { name?: string; title?: string } = {};
    if (!name.trim()) e.name = "Bitte einen Paket-Namen eingeben.";
    if (!title.trim()) e.title = "Bitte einen Positions-Titel eingeben.";
    errors = e;
    const priceOk = (priceInput?.validate() ?? false) && priceCents !== null;
    if (e.name) nameEl?.focus();
    else if (e.title) titleEl?.focus();
    else if (!priceOk) priceInput?.focus();
    return !e.name && !e.title && priceOk;
  }

  async function save() {
    if (!checkForm()) return;
    saving = true;
    try {
      const pkg = await packagesCreate(categoryId || null, name.trim(), {
        title: title.trim(),
        bodyMarkup: body,
        defaultUnitPriceCents: cents(),
        unitCode: unit.trim() || "Pauschal",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Paket angelegt.");
      goto(`/packages/${pkg.id}`);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      saving = false;
    }
  }

  async function preview() {
    if (!title.trim()) {
      errors = { ...errors, title: "Für die Vorschau zuerst einen Positions-Titel eingeben." };
      titleEl?.focus();
      return;
    }
    previewing = true;
    try {
      const ticket = await packagePreview(title.trim(), body, cents());
      openPdfPreview(downloadUrl(ticket), "Paket-Vorschau", ticket.fileName);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      previewing = false;
    }
  }
</script>

<PageBar back="/packages" backLabel="Pakete" title="Neues Paket" />

{#if loading}
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
{:else}
  <div class="editor">
    <section class="card">
      <p class="legend"><span aria-hidden="true">*</span> Pflichtfeld</p>
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
      </div>
      <div class="field">
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
      <div class="row">
        <MoneyInput
          bind:this={priceInput}
          bind:cents={priceCents}
          id="pkg-price"
          label="Netto-Preis"
          required
          hint={isKlein ? "§19: Preis ist netto, es wird keine Umsatzsteuer ausgewiesen." : undefined}
        />
        <div class="field">
          <label for="pkg-unit">Einheit</label>
          <input id="pkg-unit" class="kb-input" type="text" autocomplete="off" bind:value={unit} />
        </div>
      </div>
    </section>

    <section class="card">
      <div class="md-head">
        <h2>Beschreibung</h2>
        <Button variant="ghost" disabled={previewing} onclick={preview}>
          {previewing ? "Öffnet …" : "PDF-Vorschau"}
        </Button>
      </div>
      <p class="md-help">
        Formatierung: <code># Überschrift</code>, <code>**fett**</code>, <code>*kursiv*</code>,
        Listen mit <code>-</code>, Tabellen (GFM). Die PDF-Vorschau zeigt das exakte Ergebnis.
      </p>
      <div class="md-grid">
        <MarkdownEditor bind:value={body} placeholder="# Leistungen" />
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="md-preview" role="region" aria-label="Vorschau der Beschreibung" tabindex="0">
          {@html mdPreview(body)}
        </div>
      </div>
    </section>

    <div class="actions">
      <Button variant="primary" disabled={saving} onclick={save}>
        {saving ? "Speichert …" : "Paket anlegen"}
      </Button>
      <Button variant="ghost" href="/packages">Abbrechen</Button>
    </div>
  </div>
{/if}

<style>
  .muted { color: var(--c-text-muted); }
  .editor { max-width: 60rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  .legend { margin: 0 0 0.6rem; font-size: var(--fs-sm); color: var(--text-muted); }
  .field { margin-bottom: 14px; min-width: 0; }
  .field label { display: block; margin-bottom: 5px; font-size: var(--fs-sm); font-weight: 600; }
  .req, .legend span { color: var(--status-danger-fg); margin-left: 2px; }
  .field input, .field select { scroll-margin-block: var(--sp-6); }
  .err { margin: 4px 0 0; font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); }
  .row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; align-items: start; }
  @media (max-width: 600px) { .row { grid-template-columns: 1fr; gap: 0; } }
  .md-head { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; justify-content: space-between; }
  .md-head h2 { font-size: var(--fs-lg); margin: 0; }
  .md-help { font-size: var(--fs-sm); color: var(--c-text-muted); line-height: 1.5; }
  .md-help code {
    font-family: var(--font-mono, monospace);
    font-size: 0.85em;
    background: var(--surface-sunken, var(--c-surface-2));
    padding: 1px 5px;
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
  }
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .md-grid > :global(*) { min-width: 0; }
  @media (max-width: 600px) { .md-grid { grid-template-columns: 1fr; } }
  /* U1 (2.5.8): fokussierbare Vorschau auch leer mindestens --target hoch. */
  .md-preview {
    border: 1px solid var(--line);
    min-height: max(var(--target), 4.5rem);
    border-radius: var(--r-md);
    padding: 0.5rem 1rem;
    background: var(--surface-card);
    overflow: auto;
    line-height: 1.5;
  }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; margin-top: 0.5rem; }
</style>
