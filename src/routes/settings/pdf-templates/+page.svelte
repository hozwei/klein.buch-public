<script lang="ts">
  // Block 17a — PDF-Vorlagen-Switcher (globale Standard-Vorlage).
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Badge from "$lib/Badge.svelte";
  import Button from "$lib/Button.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import { flash } from "$lib/toast.svelte";
  import {
    pdfTemplatesList,
    sellerProfileGet,
    sellerDefaultTemplateSet,
    pdfTemplatePreview,
    sellerLogoSet,
    sellerLogoClear,
    sellerLogoData,
    sellerSignatureSet,
    sellerSignatureClear,
    sellerSignatureData,
    quoteSignatureGet,
    quoteSignatureSet,
  } from "$lib/api";
  import type { PdfTemplateMeta } from "$lib/types";

  let templates = $state<PdfTemplateMeta[]>([]);
  let selected = $state<string>("default");
  let isKlein = $state(true);
  let loading = $state(true);
  let busy = $state(false);
  let previewing = $state<string | null>(null);
  let loadError = $state<string | null>(null);

  // Branding (Logo + Angebots-Unterschrift) — wohnt jetzt hier beim PDF-Layout.
  let logoUrl = $state<string | null>(null);
  let logoBusy = $state(false);
  let signatureUrl = $state<string | null>(null);
  let signatureBusy = $state(false);
  let signatureEnabled = $state(false);

  const LABELS: Record<string, string> = {
    default: "Standard",
    modern: "Modern",
    klassisch: "Klassisch",
    minimal: "Minimal",
  };
  const DESCS: Record<string, string> = {
    default: "Die mitgelieferte Standard-Vorlage.",
    modern: "Petrol-Akzent, farbiger Tabellenkopf, hervorgehobener Gesamtbetrag.",
    klassisch: "Schlicht und formell — Serifenschrift, zentrierter Briefkopf, volle Tabellenlinien.",
    minimal: "Reduziert: viel Weißraum, dezente Linien, gedämpfte Beschriftungen.",
  };
  const label = (name: string) => LABELS[name] ?? name;
  const desc = (name: string) =>
    DESCS[name] ?? "Eigene Vorlage aus dem Ordner inputs/pdf-templates/.";

  onMount(async () => {
    try {
      const [tpls, profile] = await Promise.all([
        pdfTemplatesList(),
        sellerProfileGet(),
      ]);
      templates = tpls;
      if (profile) {
        selected = profile.defaultPdfTemplate;
        isKlein = profile.isKleinunternehmer === 1;
      }
      logoUrl = await sellerLogoData();
      signatureUrl = await sellerSignatureData();
      signatureEnabled = await quoteSignatureGet();
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  });

  async function onLogoFile(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;
    logoBusy = true;
    try {
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      await sellerLogoSet(bytes, file.name);
      logoUrl = await sellerLogoData();
      flash("Logo gespeichert.");
    } catch (err) {
      flash(err instanceof Error ? err.message : String(err), "error");
    } finally {
      logoBusy = false;
      target.value = "";
    }
  }

  async function removeLogo() {
    logoBusy = true;
    try {
      await sellerLogoClear();
      logoUrl = null;
      flash("Logo entfernt.");
    } catch (err) {
      flash(err instanceof Error ? err.message : String(err), "error");
    } finally {
      logoBusy = false;
    }
  }

  async function onSignatureFile(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;
    signatureBusy = true;
    try {
      const buf = await file.arrayBuffer();
      const bytes = Array.from(new Uint8Array(buf));
      signatureUrl = await sellerSignatureSet(bytes, file.name);
      flash("Unterschrift gespeichert.");
    } catch (err) {
      flash(err instanceof Error ? err.message : String(err), "error");
    } finally {
      signatureBusy = false;
      target.value = "";
    }
  }

  async function removeSignature() {
    signatureBusy = true;
    try {
      await sellerSignatureClear();
      signatureUrl = null;
      flash("Unterschrift entfernt.");
    } catch (err) {
      flash(err instanceof Error ? err.message : String(err), "error");
    } finally {
      signatureBusy = false;
    }
  }

  async function saveSignatureToggle() {
    try {
      await quoteSignatureSet(signatureEnabled);
      flash(signatureEnabled ? "Unterschriftenfelder aktiviert." : "Unterschriftenfelder deaktiviert.");
    } catch (err) {
      flash(err instanceof Error ? err.message : String(err), "error");
      signatureEnabled = !signatureEnabled;
    }
  }

  function blocked(t: PdfTemplateMeta): boolean {
    // §19-Schutz: Kleinunternehmer dürfen nur §19-konforme Vorlagen wählen.
    return isKlein && !t.klauselStatus.isKleinCompatible;
  }

  async function preview(t: PdfTemplateMeta) {
    if (previewing) return;
    previewing = t.name;
    try {
      await pdfTemplatePreview(t.name);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      previewing = null;
    }
  }

  async function choose(t: PdfTemplateMeta) {
    if (busy || t.name === selected || blocked(t)) return;
    busy = true;
    try {
      const p = await sellerDefaultTemplateSet(t.name);
      selected = p.defaultPdfTemplate;
      flash(`Vorlage „${label(t.name)}" als Standard gespeichert.`);
    } catch (e) {
      flash(e instanceof Error ? e.message : String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Rechnungs-Layout" />

<p class="lead">
  Wähle, wie deine Rechnungen und Angebote als PDF aussehen. Die Auswahl gilt für
  <strong>neu erzeugte</strong> Belege — bereits ausgestellte Rechnungen bleiben
  unverändert (sie sind festgeschrieben).
</p>

{#if loading}
  <p class="muted">Lädt …</p>
{:else if loadError}
  <div class="error-card">Konnte Vorlagen nicht laden: {loadError}</div>
{:else}
  <section class="branding">
    <div class="brand-block">
      <h3>Logo</h3>
      <p class="brand-hint">Erscheint im Kopf deiner Rechnungen und Angebote. PNG, JPG oder SVG, max. 2 MB.</p>
      <div class="logo-row">
        {#if logoUrl}
          <img class="logo-preview" src={logoUrl} alt="Firmenlogo" />
        {:else}
          <div class="logo-empty">Kein Logo</div>
        {/if}
        <div class="logo-actions">
          <label class="logo-btn">
            {logoBusy ? "…" : logoUrl ? "Logo ersetzen" : "Logo hochladen"}
            <input
              type="file"
              accept="image/png,image/jpeg,image/svg+xml,image/webp,image/gif"
              onchange={onLogoFile}
              disabled={logoBusy}
            />
          </label>
          {#if logoUrl}
            <button type="button" class="brand-remove" onclick={removeLogo} disabled={logoBusy}>Entfernen</button>
          {/if}
        </div>
      </div>
    </div>

    <div class="brand-block">
      <h3>Unterschrift (nur Angebote)</h3>
      <p class="brand-hint">
        Optionales Unterschrift-Bild (PNG/JPG, max. 2 MB). Erscheint auf Angeboten
        über deiner Unterschriftslinie — Ort und Datum werden automatisch ergänzt.
        Nie auf Rechnungen.
      </p>
      <div class="logo-row">
        {#if signatureUrl}
          <img class="logo-preview" src={signatureUrl} alt="Unterschrift" />
        {:else}
          <div class="logo-empty">Keine Unterschrift</div>
        {/if}
        <div class="logo-actions">
          <label class="logo-btn">
            {signatureBusy ? "…" : signatureUrl ? "Unterschrift ersetzen" : "Unterschrift hochladen"}
            <input
              type="file"
              accept="image/png,image/jpeg,image/svg+xml,image/webp,image/gif"
              onchange={onSignatureFile}
              disabled={signatureBusy}
            />
          </label>
          {#if signatureUrl}
            <button type="button" class="brand-remove" onclick={removeSignature} disabled={signatureBusy}>Entfernen</button>
          {/if}
        </div>
      </div>
      <div class="sig-toggle">
        <Toggle
          bind:checked={signatureEnabled}
          label="Unterschriftenfelder auf Angeboten anzeigen"
          onchange={saveSignatureToggle}
        />
      </div>
    </div>

  </section>

  <h2 class="section-h">Vorlage</h2>
  <div class="grid">
    {#each templates as t (t.name)}
      <div
        class="tpl"
        class:active={t.name === selected}
        class:disabled={blocked(t)}
      >
        <div class="tpl-head">
          <h3>{label(t.name)}</h3>
          {#if t.name === selected}
            <Badge tone="primary">Ausgewählt</Badge>
          {/if}
        </div>

        <p class="tpl-desc">{desc(t.name)}</p>

        <div class="tags">
          {#if t.builtin}
            <Badge tone="neutral">Mitgeliefert</Badge>
          {:else}
            <Badge tone="info">Eigene Vorlage</Badge>
          {/if}
          {#if t.klauselStatus.isKleinCompatible}
            <Badge tone="success">§19-konform</Badge>
          {:else}
            <Badge tone="danger">ohne §19-Klausel</Badge>
          {/if}
        </div>

        {#if blocked(t)}
          <p class="hint">
            Diese Vorlage rendert die Kleinunternehmer-Klausel nicht und kann
            nicht gewählt werden, solange du Kleinunternehmer (§19) bist.
          </p>
        {/if}

        <div class="tpl-foot">
          <Button
            variant="ghost"
            size="sm"
            disabled={previewing === t.name}
            onclick={() => preview(t)}
          >
            {previewing === t.name ? "Öffnet …" : "Vorschau"}
          </Button>
          {#if t.name === selected}
            <span class="current">Aktueller Standard</span>
          {:else}
            <Button
              variant="secondary"
              size="sm"
              disabled={busy || blocked(t)}
              onclick={() => choose(t)}
            >
              Als Standard wählen
            </Button>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  <p class="note">
    Eigene Vorlagen kannst du als <code>.typ</code>-Datei in
    <code>inputs/pdf-templates/</code> ablegen — sie erscheinen dann hier
    automatisch. Eine §19-konforme Vorlage muss den Marker
    <code>// §19-KLAUSEL-BLOCK: REQUIRED</code> tragen und den Klauseltext
    rendern.
  </p>
{/if}

<style>
  /* .intro entfernt — globale .lead aus tokens.css. */
  .branding {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(20rem, 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .brand-block {
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 1.1rem 1.2rem;
    box-shadow: var(--sh-sm);
  }
  .brand-block h3 {
    margin: 0 0 0.3rem;
    font-size: var(--fs-lg);
  }
  .brand-hint {
    color: var(--c-text-muted);
    font-size: var(--fs-sm);
    line-height: 1.5;
    margin: 0 0 0.8rem;
  }
  .logo-row {
    display: flex;
    align-items: center;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .logo-preview {
    max-height: 64px;
    max-width: 220px;
    object-fit: contain;
    background: #fff;
    border: 1px solid var(--c-border);
    border-radius: var(--r-md);
    padding: 4px;
  }
  .logo-empty {
    width: 130px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--c-text-subtle);
    border: 1px dashed var(--c-border-strong);
    border-radius: var(--r-md);
    font-size: var(--fs-sm);
  }
  .logo-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .logo-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-weight: 600;
    font-size: var(--fs-sm);
    color: var(--c-primary-700);
    background: var(--c-surface);
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    padding: 9px 14px;
  }
  .logo-btn:hover {
    background: var(--c-primary-50);
    border-color: var(--c-primary-300);
  }
  .logo-btn input {
    display: none;
  }
  .brand-remove {
    background: var(--c-surface);
    color: var(--c-text);
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    padding: 8px 13px;
    font-size: var(--fs-sm);
    cursor: pointer;
  }
  .brand-remove:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .sig-toggle {
    margin-top: 0.9rem;
  }
  .section-h {
    font-size: var(--fs-lg);
    margin: 0 0 0.8rem;
  }
  .muted {
    color: var(--c-text-muted);
  }
  .error-card {
    color: var(--c-danger-700);
    background: var(--c-danger-50);
    border: 1px solid var(--c-danger-500);
    border-radius: var(--r-lg);
    padding: 1rem 1.2rem;
    max-width: 46rem;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
    gap: 1rem;
    align-items: stretch;
  }
  .tpl {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    padding: 1.1rem 1.2rem;
    box-shadow: var(--sh-sm);
  }
  .tpl.active {
    border-color: var(--c-primary-500);
    box-shadow: 0 0 0 1px var(--c-primary-500) inset, var(--sh-sm);
  }
  .tpl.disabled {
    opacity: 0.7;
  }
  .tpl-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.5rem;
  }
  .tpl-head h3 {
    margin: 0;
    font-size: var(--fs-lg);
  }
  .tpl-desc {
    margin: 0;
    color: var(--c-text-muted);
    line-height: 1.5;
    flex: 1;
  }
  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 0.4rem;
  }
  .hint {
    margin: 0;
    font-size: var(--fs-sm);
    color: var(--c-danger-700);
    line-height: 1.45;
  }
  .tpl-foot {
    margin-top: 0.3rem;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .current {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--c-primary-700);
  }
  .note {
    margin-top: 1.5rem;
    max-width: 46rem;
    font-size: var(--fs-sm);
    color: var(--c-text-muted);
    line-height: 1.55;
  }
  code {
    font-family: var(--font-mono, monospace);
    font-size: 0.85em;
    background: var(--c-surface-2, #eef1f3);
    padding: 1px 5px;
    border-radius: var(--r-sm);
  }
</style>
