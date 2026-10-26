<script lang="ts">
  // Block P2b — Paket bearbeiten (= neue Revision) + Revisions-Panel
  // (Vergleich zweier Versionen + Rollback). Append-only: nichts wird überschrieben.
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import PageBar from "#lib/PageBar.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { euro, date } from "#lib/format.ts";
  import { mdPreview } from "#lib/markdownPreview.ts";
  import MarkdownEditor from "#lib/MarkdownEditor.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import {
    packagesGet,
    packageRevisionsList,
    packagesUpdateAsNewRevision,
    packageRevisionsRollback,
    packageCategoriesList,
    packagePreview,
    downloadUrl,
    sellerProfileGet,
  } from "#lib/api.ts";
  import type { Package, PackageRevision, PackageCategory } from "#lib/types.ts";

  let id = $derived(page.params.id ?? "");

  let pkg = $state<Package | null>(null);
  let revisions = $state<PackageRevision[]>([]);
  let categories = $state<PackageCategory[]>([]);
  let isKlein = $state(true);
  let loading = $state(true);
  let loadError = $state<string | null>(null);

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  /** Netto-Preis in Cent (MoneyInput, U1); null = leer oder ungültig. */
  let priceCents = $state<number | null>(null);
  let unit = $state("Pauschal");

  let saving = $state(false);
  let previewing = $state(false);
  let busy = $state(false);

  // U1: Feldfehler am Feld statt Toast, Fokus aufs erste fehlerhafte Feld.
  let errors = $state<{ name?: string; title?: string }>({});
  let nameEl: HTMLInputElement | undefined = $state();
  let titleEl: HTMLInputElement | undefined = $state();
  let priceInput: MoneyInput | undefined = $state();

  // Vergleich
  let compareA = $state<number | null>(null);
  let compareB = $state<number | null>(null);

  function prefillFrom(rev: PackageRevision | undefined) {
    if (!rev) return;
    title = rev.title;
    body = rev.bodyMarkup;
    priceCents = rev.defaultUnitPriceCents;
    unit = rev.unitCode;
  }

  async function load() {
    loading = true;
    try {
      const [p, revs, cats, seller] = await Promise.all([
        packagesGet(id),
        packageRevisionsList(id),
        packageCategoriesList(),
        sellerProfileGet(),
      ]);
      if (!p) {
        loadError = "Paket nicht gefunden.";
        return;
      }
      pkg = p;
      revisions = revs;
      categories = cats;
      if (seller) isKlein = seller.isKleinunternehmer === 1;
      name = p.name;
      categoryId = p.categoryId ?? "";
      const current = revs.find((r) => r.revision === p.currentRevision) ?? revs[0];
      prefillFrom(current);
      compareA = revs.length > 1 ? revs[1].revision : (revs[0]?.revision ?? null);
      compareB = revs[0]?.revision ?? null;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  const cents = () => priceCents ?? 0;
  const revByNum = (n: number | null) => revisions.find((r) => r.revision === n);

  async function save() {
    const e: { name?: string; title?: string } = {};
    if (!name.trim()) e.name = "Bitte einen Paket-Namen eingeben.";
    if (!title.trim()) e.title = "Bitte einen Positions-Titel eingeben.";
    errors = e;
    const priceOk = (priceInput?.validate() ?? false) && priceCents !== null;
    if (e.name) return nameEl?.focus();
    if (e.title) return titleEl?.focus();
    if (!priceOk) return priceInput?.focus();
    saving = true;
    try {
      await packagesUpdateAsNewRevision(id, categoryId || null, name.trim(), {
        title: title.trim(),
        bodyMarkup: body,
        defaultUnitPriceCents: cents(),
        unitCode: unit.trim() || "Pauschal",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Neue Version gespeichert.");
      await load();
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      saving = false;
    }
  }

  async function rollback(toRevision: number) {
    busy = true;
    try {
      await packageRevisionsRollback(id, toRevision);
      flash(`Auf Version ${toRevision} zurückgesetzt (als neue Version).`);
      await load();
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function preview() {
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

<PageBar back="/packages" backLabel="Pakete" title="Paket bearbeiten" />

{#if loading}
  <p class="muted" role="status" aria-busy="true">Lädt …</p>
{:else if loadError}
  <div class="error-card">{loadError}</div>
{:else if pkg}
  <div class="editor">
    <section class="card">
      <div class="head-row">
        <h2>Aktuelle Version bearbeiten</h2>
        <span class="ver">Aktiv: Version {pkg.currentRevision ?? "–"}</span>
      </div>
      <p class="muted small">
        Speichern erzeugt eine <strong>neue Version</strong>. Bestehende Versionen
        bleiben unverändert erhalten.
      </p>
      <p class="legend"><span aria-hidden="true">*</span> Pflichtfeld</p>
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
      <div class="md-grid">
        <MarkdownEditor bind:value={body} />
        <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
        <div class="md-preview" role="region" aria-label="Vorschau der Beschreibung" tabindex="0">
          {@html mdPreview(body)}
        </div>
      </div>
    </section>

    <div class="actions">
      <Button variant="primary" disabled={saving} onclick={save}>
        {saving ? "Speichert …" : "Als neue Version speichern"}
      </Button>
      <Button variant="ghost" href="/packages">Zurück zu den Paketen</Button>
    </div>

    <h2 class="section-h">Versionen</h2>
    <div class="revs">
      {#each revisions as r (r.id)}
        <div class="rev">
          <div class="rev-main">
            <strong>Version {r.revision}</strong>
            {#if r.revision === pkg.currentRevision}<Badge tone="primary">aktiv</Badge>{/if}
            <span class="muted small">· {date(r.createdAt)} · {r.title} · <span class="kb-num">{euro(r.defaultUnitPriceCents)}</span></span>
            {#if r.note}<span class="muted small">· {r.note}</span>{/if}
          </div>
          {#if r.revision !== pkg.currentRevision}
            <Button
              variant="ghost"
              disabled={busy}
              onclick={() => rollback(r.revision)}
              ariaLabel={`Auf diese Version zurücksetzen (Version ${r.revision})`}
            >
              Auf diese Version zurücksetzen
            </Button>
          {/if}
        </div>
      {/each}
    </div>

    {#if revisions.length > 1}
      <h2 class="section-h">Versionen vergleichen</h2>
      <div class="cmp-pick">
        <label>
          Links
          <select class="kb-input" bind:value={compareA}>
            {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
          </select>
        </label>
        <label>
          Rechts
          <select class="kb-input" bind:value={compareB}>
            {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
          </select>
        </label>
      </div>
      <div class="cmp-grid">
        {#each [revByNum(compareA), revByNum(compareB)] as rev}
          <div class="cmp-col">
            {#if rev}
              <div class="cmp-h">Version {rev.revision} · {date(rev.createdAt)}</div>
              <div class="cmp-meta">{rev.title} · {euro(rev.defaultUnitPriceCents)}</div>
              <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
              <div class="md-preview" role="region" aria-label={`Beschreibung Version ${rev.revision}`} tabindex="0">
                {@html mdPreview(rev.bodyMarkup)}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .muted { color: var(--c-text-muted); }
  .small { font-size: var(--fs-sm); }
  .editor { max-width: 64rem; }
  .error-card {
    color: var(--c-danger-700);
    background: var(--c-danger-50);
    border: 1px solid var(--c-danger-500);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    max-width: 46rem;
  }
  /* .card entfernt — globale Definition aus tokens.css. */
  .head-row { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: 0.5rem; }
  .head-row h2 { font-size: var(--fs-lg); margin: 0; }
  .ver { font-size: var(--fs-sm); color: var(--c-text-muted); }
  .legend { margin: 0 0 0.6rem; font-size: var(--fs-sm); color: var(--text-muted); }
  .field { margin-bottom: 14px; min-width: 0; }
  .field label { display: block; margin-bottom: 5px; font-size: var(--fs-sm); font-weight: 600; }
  .req, .legend span { color: var(--status-danger-fg); margin-left: 2px; }
  .field input, .field select { scroll-margin-block: var(--sp-6); }
  .err { margin: 4px 0 0; font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); }
  .row { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; align-items: start; }
  @media (max-width: 600px) { .row { grid-template-columns: 1fr; gap: 0; } }
  .cmp-pick label { display: block; font-size: var(--fs-sm); font-weight: 600; }
  .cmp-pick select { margin-top: 0.25rem; }
  .md-head { display: flex; flex-wrap: wrap; gap: 0.5rem; align-items: center; justify-content: space-between; }
  .md-head h2 { font-size: var(--fs-lg); margin: 0; }
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .md-grid > :global(*) { min-width: 0; }
  @media (max-width: 600px) { .md-grid { grid-template-columns: 1fr; } }
  .md-preview {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 0.5rem 1rem;
    background: var(--surface-card);
    overflow: auto;
    line-height: 1.5;
  }
  .actions { display: flex; flex-wrap: wrap; gap: 0.5rem; margin: 0.5rem 0 1.5rem; }
  .section-h { font-size: var(--fs-lg); margin: 1.5rem 0 0.8rem; }
  .revs { display: flex; flex-direction: column; gap: 0.4rem; }
  .rev {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 0.6rem 1rem;
  }
  .rev-main { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .cmp-pick { display: flex; flex-wrap: wrap; gap: 1rem; margin-bottom: 0.8rem; }
  .cmp-pick label { flex: 0 1 12rem; min-width: 0; }
  .cmp-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  @media (max-width: 600px) { .cmp-grid { grid-template-columns: 1fr; } }
  .cmp-col {
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 0.8rem 1rem;
    background: var(--c-surface);
  }
  .cmp-h { font-weight: 600; }
  .cmp-meta { font-size: var(--fs-sm); color: var(--c-text-muted); margin-bottom: 0.5rem; }
</style>
