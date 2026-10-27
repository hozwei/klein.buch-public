<script lang="ts">
  // Block 17a — PDF-Vorlagen-Switcher (globale Standard-Vorlage).
  // U4: Vorlagennamen zentral aus `labels.ts` (Warteliste (17)); je Vorlage, ob sie
  // Stornos darstellt, sonst mit welcher Vorlage Storno-Belege entstehen
  // (Warteliste (11), `PdfTemplateMeta.supportsStorno`/`stornoFallback`).
  // U4 (V2-06/V2-07): Ladefehler mit „Erneut laden“; Knöpfe und Datei-Felder während eines
  // Laufs `aria-disabled` statt `disabled` (Fokus bleibt, ADR 0041 Nr. 33).
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Badge from "#lib/Badge.svelte";
  import Button from "#lib/Button.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import Banner from "#lib/Banner.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { pdfTemplateLabel } from "#lib/labels.ts";
  import {
    pdfTemplatesList,
    sellerProfileGet,
    sellerDefaultTemplateSet,
    pdfTemplatePreview,
    downloadUrl,
    sellerLogoSet,
    uploadFile,
    sellerLogoClear,
    sellerLogoData,
    sellerSignatureSet,
    sellerSignatureClear,
    sellerSignatureData,
    quoteSignatureGet,
    quoteSignatureSet,
  } from "#lib/api.ts";
  import type { PdfTemplateMeta } from "#lib/types.ts";

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

  // Beschreibungen der mitgelieferten Vorlagen; Namen über `pdfTemplateLabel()`.
  const DESCS: Record<string, string> = {
    default: "Die mitgelieferte Standard-Vorlage.",
    modern: "Petrol-Akzent, farbiger Tabellenkopf, hervorgehobener Gesamtbetrag.",
    klassisch: "Schlicht und formell — Serifenschrift, zentrierter Briefkopf, volle Tabellenlinien.",
    minimal: "Reduziert: viel Weißraum, dezente Linien, gedämpfte Beschriftungen.",
  };
  const label = (name: string) => pdfTemplateLabel(name);
  const desc = (name: string) =>
    DESCS[name] ?? "Eigene Vorlage aus dem Ordner inputs/pdf-templates/.";

  async function load() {
    loading = true;
    loadError = null;
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
  }

  onMount(load);

  async function onLogoFile(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;
    // Während eines Uploads bleibt das Feld bedienbar (aria-disabled); eine zweite Wahl zählt nicht.
    if (logoBusy) { target.value = ""; return; }
    logoBusy = true;
    try {
      const { uploadId } = await uploadFile("logo", file);
      await sellerLogoSet(uploadId);
      logoUrl = await sellerLogoData();
      flash("Logo gespeichert.");
    } catch (err) {
      flash(`Logo hochladen fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
    } finally {
      logoBusy = false;
      target.value = "";
    }
  }

  async function removeLogo() {
    if (logoBusy) return;
    logoBusy = true;
    try {
      await sellerLogoClear();
      logoUrl = null;
      flash("Logo entfernt.");
    } catch (err) {
      flash(`Logo entfernen fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
    } finally {
      logoBusy = false;
    }
  }

  async function onSignatureFile(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    const file = target.files?.[0];
    if (!file) return;
    if (signatureBusy) { target.value = ""; return; }
    signatureBusy = true;
    try {
      const { uploadId } = await uploadFile("unterschrift", file);
      signatureUrl = await sellerSignatureSet(uploadId);
      flash("Unterschrift gespeichert.");
    } catch (err) {
      flash(`Unterschrift hochladen fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
    } finally {
      signatureBusy = false;
      target.value = "";
    }
  }

  async function removeSignature() {
    if (signatureBusy) return;
    signatureBusy = true;
    try {
      await sellerSignatureClear();
      signatureUrl = null;
      flash("Unterschrift entfernt.");
    } catch (err) {
      flash(`Unterschrift entfernen fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
    } finally {
      signatureBusy = false;
    }
  }

  async function saveSignatureToggle() {
    try {
      await quoteSignatureSet(signatureEnabled);
      flash(signatureEnabled ? "Unterschriftenfelder aktiviert." : "Unterschriftenfelder deaktiviert.");
    } catch (err) {
      flash(`Unterschriftenfelder speichern fehlgeschlagen: ${err instanceof Error ? err.message : String(err)}`, "error");
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
      const ticket = await pdfTemplatePreview(t.name);
      openPdfPreview(downloadUrl(ticket), `Vorschau: ${label(t.name)}`, ticket.fileName);
    } catch (e) {
      flash(`Vorschau fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
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
      flash(`Vorlage „${label(t.name)}“ als Standard gespeichert.`);
    } catch (e) {
      flash(`Vorlage wählen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
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
  <LoadingState />
{:else if loadError}
  <Banner title="Das Rechnungs-Layout konnte nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else}
  <section class="branding" aria-label="Logo und Unterschrift">
    <div class="brand-block">
      <h2 class="brand-h">Logo</h2>
      <p class="brand-hint" id="logo-hint">Erscheint im Kopf deiner Rechnungen und Angebote. PNG, JPG, SVG, WebP oder GIF, max. 2 MB.</p>
      <div class="logo-row">
        {#if logoUrl}
          <img class="logo-preview" src={logoUrl} alt="Dein Firmenlogo" />
        {:else}
          <div class="logo-empty">Kein Logo</div>
        {/if}
        <div class="logo-actions">
          <label class="logo-btn" class:busy={logoBusy}>
            {logoBusy ? "Lädt hoch …" : logoUrl ? "Logo ersetzen" : "Logo hochladen"}
            <input
              type="file"
              accept="image/png,image/jpeg,image/svg+xml,image/webp,image/gif"
              aria-describedby="logo-hint"
              onchange={onLogoFile}
              aria-disabled={logoBusy ? "true" : undefined}
            />
          </label>
          {#if logoUrl}
            <Button variant="secondary" onclick={removeLogo} ariaDisabled={logoBusy} ariaLabel="Logo entfernen">Entfernen</Button>
          {/if}
        </div>
      </div>
    </div>

    <div class="brand-block">
      <h2 class="brand-h">Unterschrift (nur Angebote)</h2>
      <p class="brand-hint" id="sig-hint">
        Optionales Unterschrift-Bild (PNG/JPG, max. 2 MB). Erscheint auf Angeboten
        über deiner Unterschriftslinie — Ort und Datum werden automatisch ergänzt.
        Nie auf Rechnungen.
      </p>
      <div class="logo-row">
        {#if signatureUrl}
          <img class="logo-preview" src={signatureUrl} alt="Deine Unterschrift" />
        {:else}
          <div class="logo-empty">Keine Unterschrift</div>
        {/if}
        <div class="logo-actions">
          <label class="logo-btn" class:busy={signatureBusy}>
            {signatureBusy ? "Lädt hoch …" : signatureUrl ? "Unterschrift ersetzen" : "Unterschrift hochladen"}
            <input
              type="file"
              accept="image/png,image/jpeg,image/svg+xml,image/webp,image/gif"
              aria-describedby="sig-hint"
              onchange={onSignatureFile}
              aria-disabled={signatureBusy ? "true" : undefined}
            />
          </label>
          {#if signatureUrl}
            <Button variant="secondary" onclick={removeSignature} ariaDisabled={signatureBusy} ariaLabel="Unterschrift entfernen">Entfernen</Button>
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

  <h2 class="section-h" id="tpl-h">Vorlage</h2>
  <ul class="grid" aria-labelledby="tpl-h">
    {#each templates as t (t.name)}
      <li
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
            <!-- U4 (V3-08): Herkunft ist ein Merkmal, kein Status. -->
            <Badge kind="tag">Mitgeliefert</Badge>
          {:else}
            <Badge kind="tag">Eigene Vorlage</Badge>
          {/if}
          {#if t.klauselStatus.isKleinCompatible}
            <Badge tone="success">§19-konform</Badge>
          {:else}
            <Badge tone="danger">Ohne §19-Klausel</Badge>
          {/if}
          {#if !t.supportsStorno}
            <Badge tone="warning">Ohne Storno-Darstellung</Badge>
          {/if}
        </div>

        {#if !t.supportsStorno}
          <!-- Warteliste (11): Stornos entstehen dann mit einer mitgelieferten Vorlage
               (dieselbe, die die PDF-Erzeugung nimmt, ADR 0041 Nr. 84). -->
          <p class="hint">
            Diese Vorlage stellt Storno-Belege nicht dar. Stornos zu Rechnungen mit dieser
            Vorlage erzeugt Klein.Buch mit der Vorlage „{label(t.stornoFallback ?? "klassisch")}“.
          </p>
        {/if}

        {#if blocked(t)}
          <p class="hint" id="tpl-blocked-{t.name}">
            Diese Vorlage rendert die Kleinunternehmer-Klausel nicht und kann
            nicht gewählt werden, solange du Kleinunternehmer (§19) bist.
          </p>
        {/if}

        <div class="tpl-foot">
          <Button
            variant="secondary"
            busy={previewing === t.name}
            onclick={() => preview(t)}
            ariaLabel={`Vorschau, Vorlage ${label(t.name)}`}
          >
            {previewing === t.name ? "Öffnet …" : "Vorschau"}
          </Button>
          {#if t.name === selected}
            <span class="current">Aktueller Standard</span>
          {:else}
            <Button
              variant="secondary"
              disabled={blocked(t)}
              ariaDisabled={busy}
              onclick={() => choose(t)}
              ariaLabel={`Als Standard wählen, Vorlage ${label(t.name)}`}
              ariaDescribedby={blocked(t) ? `tpl-blocked-${t.name}` : undefined}
            >
              Als Standard wählen
            </Button>
          {/if}
        </div>
      </li>
    {/each}
  </ul>

  <p class="note">
    Eigene Vorlagen kannst du als <code>.typ</code>-Datei in
    <code>inputs/pdf-templates/</code> ablegen — sie erscheinen dann hier
    automatisch. Eine §19-konforme Vorlage muss den Marker
    <code>// §19-KLAUSEL-BLOCK: REQUIRED</code> tragen und den Klauseltext
    rendern.
  </p>
{/if}

<style>
  /* .lead — global aus tokens.css. */
  .branding {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(20rem, 100%), 1fr));
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  .brand-block {
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 1.1rem 1.2rem;
    box-shadow: var(--shadow-card);
    min-width: 0;
  }
  .brand-h {
    margin: 0 0 0.3rem;
    font-size: var(--fs-lg);
  }
  .brand-hint {
    color: var(--text-muted);
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
    max-width: min(220px, 100%);
    object-fit: contain;
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 4px;
  }
  .logo-empty {
    width: 130px;
    height: 64px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border: 1px dashed var(--line-control);
    border-radius: var(--r-md);
    font-size: var(--fs-sm);
  }
  .logo-actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  /* Datei-Knopf: das native Feld bleibt fokussierbar (nur unsichtbar), der
     Fokusring liegt auf dem Label. */
  .logo-btn {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-weight: 600;
    font-size: var(--fs-sm);
    color: var(--accent-on-soft);
    background: var(--surface-card);
    border: 1px solid var(--line-control);
    border-radius: var(--r-md);
    padding: 9px 14px;
    min-height: var(--target);
  }
  .logo-btn.busy { cursor: progress; }
  .logo-btn:focus-within {
    outline: var(--focus-width) solid var(--focus-ring);
    outline-offset: var(--focus-offset);
  }
  @media (hover: hover) {
    .logo-btn:hover {
      background: var(--surface-hover);
      border-color: var(--c-primary-300);
    }
  }
  .logo-btn input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    min-height: 0;
    opacity: 0;
    cursor: pointer;
  }
  .sig-toggle {
    margin-top: 0.9rem;
  }
  .section-h {
    font-size: var(--fs-lg);
    margin: 0 0 0.8rem;
  }
  .grid {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(17rem, 100%), 1fr));
    gap: 1rem;
    align-items: stretch;
  }
  .tpl {
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    background: var(--surface-card);
    border: 1px solid var(--line);
    border-radius: var(--r-lg);
    padding: 1.1rem 1.2rem;
    box-shadow: var(--shadow-card);
    min-width: 0;
  }
  .tpl.active {
    border-color: var(--c-primary-500);
    box-shadow: 0 0 0 1px var(--c-primary-500) inset, var(--shadow-card);
  }
  .tpl.disabled {
    background: var(--surface-sunken);
  }
  .tpl-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 0.5rem;
  }
  .tpl-head h3 {
    margin: 0;
    font-size: var(--fs-lg);
    overflow-wrap: anywhere;
  }
  .tpl-desc {
    margin: 0;
    color: var(--text-muted);
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
    color: var(--status-danger-fg);
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
    color: var(--accent-on-soft);
  }
  .note {
    margin-top: 1.5rem;
    max-width: 46rem;
    font-size: var(--fs-sm);
    color: var(--text-muted);
    line-height: 1.55;
  }
  code {
    font-family: var(--font-mono);
    font-size: 0.85em;
    background: var(--surface-sunken);
    padding: 1px 5px;
    border-radius: var(--r-sm);
    overflow-wrap: anywhere;
  }
</style>
