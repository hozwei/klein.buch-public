<script lang="ts">
  // Block P2b — Paket bearbeiten (= neue Revision) + Revisions-Panel
  // (Vergleich zweier Versionen + Rollback). Append-only: nichts wird überschrieben.
  // U4 (U1-067, U3-038): Hauptaktion in der PageBar und am Formularende, „PDF-Vorschau“
  // als Knopf, Einheit im Klartext (U1-060). Versionen als Liste im Table-Baustein;
  // „Zurücksetzen“ legt nach Rückfrage eine NEUE Version mit dem alten Inhalt an
  // (Hardline Pakete: Revisionen sind append-only). `packageRevisionsGet` bleibt ohne
  // Oberfläche: `packageRevisionsList` liefert jede Version bereits vollständig.
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import PageBar from "#lib/PageBar.svelte";
  import RequiredLegend from "#lib/RequiredLegend.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Button from "#lib/Button.svelte";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Table from "#lib/Table.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { euro, date } from "#lib/format.ts";
  import { mdPreview } from "#lib/markdownPreview.ts";
  import { UNIT_CODES, unitLabel, unitOptions } from "#lib/labels.ts";
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
  /** U4 (V4-08): eigener Zustand „nicht gefunden“ mit passendem Titel. */
  let notFound = $state(false);

  let name = $state("");
  let categoryId = $state<string>("");
  let title = $state("");
  let body = $state("");
  /** Netto-Preis in Cent (MoneyInput, U1); null = leer oder ungültig. */
  let priceCents = $state<number | null>(null);
  /** Einheit als UN/ECE-Code; ältere Pakete können Freitext tragen (z. B. „Pauschal“). */
  let unit = $state("LS");
  const unitKnown = $derived(UNIT_CODES.some((u) => u.value === unit));

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

  /** `quiet`: nach dem Speichern ohne Ladezustand nachladen — Formular und Knöpfe bleiben stehen. */
  async function load(quiet = false) {
    if (!quiet) {
      loading = true;
      loadError = null;
      notFound = false;
    }
    try {
      const [p, revs, cats, seller] = await Promise.all([
        packagesGet(id),
        packageRevisionsList(id),
        packageCategoriesList(),
        sellerProfileGet(),
      ]);
      if (!p) {
        notFound = true;
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
      const msg = e instanceof Error ? e.message : String(e);
      if (quiet) flash(`Neu laden fehlgeschlagen: ${msg}`, "error");
      else loadError = msg;
    } finally {
      loading = false;
    }
  }

  onMount(() => load());

  const cents = () => priceCents ?? 0;
  const revByNum = (n: number | null) => revisions.find((r) => r.revision === n);

  async function save() {
    if (saving) return;
    const e: { name?: string; title?: string } = {};
    if (!name.trim()) e.name = "Bitte gib einen Paket-Namen ein.";
    if (!title.trim()) e.title = "Bitte gib einen Positions-Titel ein.";
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
        unitCode: unit.trim() || "LS",
        taxCategoryCode: "E",
        note: null,
      });
      flash("Neue Version gespeichert.");
      await load(true);
    } catch (e) {
      flash(`Speichern fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      saving = false;
    }
  }

  async function rollback(r: PackageRevision) {
    if (busy || !pkg) return;
    const next = Math.max(...revisions.map((x) => x.revision)) + 1;
    if (
      !(await confirmDialog({
        title: `Version ${r.revision} wiederverwenden?`,
        body:
          `Klein.Buch legt Version ${next} mit dem Inhalt von Version ${r.revision} an und macht sie ` +
          "zur aktiven Version. Alle bisherigen Versionen bleiben unverändert erhalten; Positionen in " +
          "bestehenden Angeboten und Rechnungen ändern sich nicht. Nicht gespeicherte Änderungen im " +
          "Formular oben gehen verloren.",
        confirmLabel: "Als neue Version übernehmen",
      }))
    )
      return;
    busy = true;
    try {
      await packageRevisionsRollback(id, r.revision);
      flash(`Inhalt von Version ${r.revision} als Version ${next} übernommen.`);
      await load(true);
    } catch (e) {
      flash(`Version wiederverwenden fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
    } finally {
      busy = false;
    }
  }

  async function preview() {
    if (previewing) return;
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

<!-- U4 (V4-08/V2-11): Titel auch beim Laden und bei „nicht gefunden“; „Abbrechen“ vor der
     Hauptaktion (Änderungen im Formular werden verworfen, nichts wird gespeichert). -->
<PageBar back="/packages" backLabel="Pakete" title={pkg ? `Paket „${pkg.name}“ bearbeiten` : notFound ? "Paket nicht gefunden" : "Paket bearbeiten"}>
  {#snippet actions()}
    {#if pkg}
      <Button variant="secondary" href="/packages">Abbrechen</Button>
      <Button variant="primary" busy={saving} onclick={save}>
        {saving ? "Speichert …" : "Als neue Version speichern"}
      </Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <LoadingState />
{:else if notFound}
  <Banner title="Paket nicht gefunden." href="/packages" linkLabel="Zur Paketliste">
    Unter dieser Adresse gibt es kein Paket.
  </Banner>
{:else if loadError}
  <Banner title="Das Paket konnte nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={() => load()}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if pkg}
  <div class="editor">
    <section class="card" aria-labelledby="pkg-edit-h">
      <div class="head-row">
        <h2 id="pkg-edit-h">Aktuelle Version bearbeiten</h2>
        <span class="ver">Aktiv: Version {pkg.currentRevision ?? "–"}</span>
      </div>
      <p class="muted small">
        Speichern erzeugt eine <strong>neue Version</strong>. Bestehende Versionen
        bleiben unverändert erhalten.
      </p>
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
          hint={isKlein ? "§19: Preis ist netto, es wird keine Umsatzsteuer ausgewiesen." : undefined}
        />
        <div class="field">
          <label for="pkg-unit">Einheit</label>
          <select id="pkg-unit" class="kb-input kb-w-medium" bind:value={unit} aria-describedby="pkg-unit-hint">
            {#each unitOptions(unit) as u (u.value)}<option value={u.value}>{u.label}</option>{/each}
          </select>
          {#if unitKnown}
            <p id="pkg-unit-hint" class="hint">Erscheint auf dem Beleg; in der E-Rechnung als Einheiten-Code.</p>
          {:else}
            <!-- Ältere Pakete speicherten die Einheit als Freitext; die E-Rechnung braucht einen Code. -->
            <p id="pkg-unit-hint" class="hint warn">
              „{unit}“ ist keine Einheit der E-Rechnung. Wähl eine Einheit aus der Liste, z. B.
              „Pauschale“; sie gilt ab der nächsten Version.
            </p>
          {/if}
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
        Die Vorschau rechts ist eine Annäherung; die PDF-Vorschau zeigt das exakte Ergebnis auf dem Beleg.
      </p>
      <div class="md-grid">
        <MarkdownEditor bind:value={body} />
        <div class="md-preview-wrap">
          <p class="md-preview-label" id="pkg-md-preview-label">Vorschau</p>
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div class="md-preview" role="region" aria-labelledby="pkg-md-preview-label" tabindex="0">
            {@html mdPreview(body)}
          </div>
        </div>
      </div>
    </section>

    <!-- Hauptaktion am Formularende (≤ 600 px Breite, ≤ 500 px Höhe; ADR 0041 Nr. 35/49). -->
    <div class="kb-form-end">
      <Button variant="secondary" href="/packages">Abbrechen</Button>
      <Button variant="primary" busy={saving} onclick={save}>
        {saving ? "Speichert …" : "Als neue Version speichern"}
      </Button>
    </div>

    <section class="versions" aria-labelledby="pkg-revs-h">
      <h2 class="section-h" id="pkg-revs-h">Versionen</h2>
      <p class="muted small">
        Jede Version bleibt unverändert erhalten. „Wiederverwenden“ legt eine neue Version mit dem
        Inhalt einer älteren an.
      </p>
      <Table cards>
        <caption class="kb-visually-hidden">Versionen von {pkg.name}, neueste zuerst</caption>
        <thead>
          <tr>
            <th class="kb-num">Version</th>
            <th>Angelegt</th>
            <th>Positions-Titel</th>
            <th class="kb-num">Netto-Preis</th>
            <th class="kb-col-opt2">Einheit</th>
            <th><span class="kb-visually-hidden">Status und Aktion</span></th>
          </tr>
        </thead>
        <tbody>
          {#each revisions as r (r.id)}
            <tr>
              <td class="kb-num" data-label="Version">{r.revision}</td>
              <td data-label="Angelegt">{date(r.createdAt)}</td>
              <td class="rev-title" data-label="Positions-Titel">
                {r.title}
                {#if r.note}<span class="muted small rev-note">{r.note}</span>{/if}
              </td>
              <td class="kb-num" data-label="Netto-Preis">{euro(r.defaultUnitPriceCents)}</td>
              <td class="kb-col-opt2" data-label="Einheit">{unitLabel(r.unitCode)}</td>
              <td data-label={r.revision === pkg.currentRevision ? "Status" : undefined}>
                <div class="kb-actions">
                  {#if r.revision === pkg.currentRevision}
                    <Badge tone="success">Aktiv</Badge>
                  {:else}
                    <Button
                      variant="secondary"
                      ariaDisabled={busy}
                      onclick={() => rollback(r)}
                      ariaLabel={`Wiederverwenden: Version ${r.revision}`}
                    >Wiederverwenden</Button>
                  {/if}
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </Table>
    </section>

    {#if revisions.length > 1}
      <section aria-labelledby="pkg-cmp-h">
        <h2 class="section-h" id="pkg-cmp-h">Versionen vergleichen</h2>
        <div class="cmp-pick">
          <div class="field">
            <label for="pkg-cmp-a">Links</label>
            <select id="pkg-cmp-a" class="kb-input" bind:value={compareA}>
              {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
            </select>
          </div>
          <div class="field">
            <label for="pkg-cmp-b">Rechts</label>
            <select id="pkg-cmp-b" class="kb-input" bind:value={compareB}>
              {#each revisions as r (r.id)}<option value={r.revision}>Version {r.revision}</option>{/each}
            </select>
          </div>
        </div>
        <div class="cmp-grid">
          {#each [revByNum(compareA), revByNum(compareB)] as rev}
            <div class="cmp-col">
              {#if rev}
                <div class="cmp-h">Version {rev.revision} · {date(rev.createdAt)}</div>
                <div class="cmp-meta">{rev.title} · {euro(rev.defaultUnitPriceCents)} · {unitLabel(rev.unitCode)}</div>
                <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
                <div class="md-preview" role="region" aria-label={`Beschreibung Version ${rev.revision}`} tabindex="0">
                  {@html mdPreview(rev.bodyMarkup)}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </section>
    {/if}
  </div>
{/if}

<style>
  .muted { color: var(--text-muted); }
  .small { font-size: var(--type-small); }
  .editor { max-width: 64rem; }
  /* .card global aus tokens.css. */
  .head-row { display: flex; flex-wrap: wrap; align-items: baseline; justify-content: space-between; gap: var(--sp-2); }
  .head-row h2, .md-head h2 { font-size: var(--type-heading); margin: 0; }
  .ver { font-size: var(--type-small); color: var(--text-muted); }
  .field { margin-bottom: var(--sp-4); min-width: 0; }
  .field label { display: block; margin-bottom: var(--sp-1); font-size: var(--type-small); font-weight: 600; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .field input, .field select { scroll-margin-block: var(--sp-6); }
  .err { margin: var(--sp-1) 0 0; font-size: var(--type-small); font-weight: 600; color: var(--status-danger-fg); }
  .hint { margin: var(--sp-1) 0 0; font-size: var(--type-small); color: var(--text-muted); }
  .hint.warn { color: var(--status-warning-fg); font-weight: 600; }
  .md-head { display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: center; justify-content: space-between; }
  .md-help { font-size: var(--type-small); color: var(--text-muted); line-height: var(--lh-body); }
  .md-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); }
  .md-grid > :global(*) { min-width: 0; }
  @media (max-width: 600px) { .md-grid { grid-template-columns: 1fr; } }
  .md-preview-label { margin: 0 0 var(--sp-1); font-size: var(--type-small); font-weight: 600; }
  .md-preview {
    border: var(--rule);
    min-height: max(var(--target), 4.5rem);
    border-radius: var(--radius-control);
    padding: var(--sp-2) var(--sp-4);
    background: var(--surface-card);
    overflow: auto;
    line-height: var(--lh-body);
  }
  .section-h { font-size: var(--type-heading); margin: var(--sp-5) 0 var(--sp-2); }
  .versions { margin-bottom: var(--sp-5); }
  .rev-title { overflow-wrap: break-word; }
  .rev-note { display: block; }
  .cmp-pick { display: flex; flex-wrap: wrap; gap: 0 var(--sp-4); }
  .cmp-pick .field { flex: 0 1 12rem; }
  .cmp-grid { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-4); }
  @media (max-width: 600px) { .cmp-grid { grid-template-columns: 1fr; } }
  .cmp-col {
    border: var(--rule);
    border-radius: var(--radius-card);
    padding: var(--sp-3) var(--sp-4);
    background: var(--surface-card);
    min-width: 0;
  }
  .cmp-h { font-weight: 600; }
  .cmp-meta { font-size: var(--type-small); color: var(--text-muted); margin-bottom: var(--sp-2); }
</style>
