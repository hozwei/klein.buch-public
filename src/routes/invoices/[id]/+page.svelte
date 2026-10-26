<script lang="ts">
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pdfTemplateLabel } from "#lib/labels.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    invoicesGet,
    invoicesValidateDraft,
    invoicesLockAndIssue,
    invoicesVoidDraft,
    archiveUrl,
    saveArchiveFile,
    emailLogFor,
  } from "#lib/api.ts";
  import type { Invoice, InvoiceDetail, ValidationIssueDto, EmailLogEntry } from "#lib/types.ts";
  import { euro, date, dateTime, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { voidReasonError } from "#lib/voidDraft.ts";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let issues = $state<ValidationIssueDto[] | null>(null);
  let validating = $state(false);
  let issuing = $state(false);
  let buyerReference = $state("N/A");
  let sendLog = $state<EmailLogEntry[]>([]);
  // Lauf F3b: Entwurf verwerfen (schlicht; Gestaltung im UI/UX-Lauf).
  let showVoid = $state(false);
  let voidReason = $state("");
  let voiding = $state(false);
  // Feldfehler am Grund (statt Toast), erst nach dem ersten Versuch.
  let voidError = $state("");
  let voidReasonEl: HTMLTextAreaElement | undefined = $state();
  // Storno-Bezug: Original bzw. aufhebender Storno-Beleg (Nummer, Datum, Link).
  let relatedOriginal = $state<Invoice | null>(null);
  let relatedStorno = $state<Invoice | null>(null);

  let id = $derived(page.params.id ?? "");

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await invoicesGet(id);
      if (!detail) error = "Rechnung nicht gefunden.";
      else await loadRelated(detail);
      try {
        sendLog = await emailLogFor("invoice", id);
      } catch {
        sendLog = [];
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  // Nur zur Anzeige; fehlt der Bezug, bleibt der Hinweis ohne Nummer.
  async function loadRelated(d: InvoiceDetail) {
    relatedOriginal = null;
    relatedStorno = null;
    try {
      if (d.invoice.isStornoFor) relatedOriginal = (await invoicesGet(d.invoice.isStornoFor))?.invoice ?? null;
      if (d.invoice.canceledByStornoId) relatedStorno = (await invoicesGet(d.invoice.canceledByStornoId))?.invoice ?? null;
    } catch {
      // Bezug ist Zusatzinformation — Fehler hier nicht hochziehen.
    }
  }

  onMount(load);

  async function runValidation() {
    if (!detail) return;
    validating = true;
    issues = null;
    try {
      issues = await invoicesValidateDraft(detail.invoice.id);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      validating = false;
    }
  }

  // Plan W7: das archivierte PDF kommt hash-geprüft über `/files/archive/{id}`.
  function openPdf() {
    const id = detail?.invoice.pdfArchiveId;
    if (!detail || !id) return;
    openPdfPreview(archiveUrl(id), `Rechnung ${detail.invoice.invoiceNumber}`, "");
  }

  function downloadPdf() {
    const id = detail?.invoice.pdfArchiveId;
    if (id) saveArchiveFile(id);
  }

  async function lockAndIssue() {
    if (!detail) return;
    if (
      !(await confirmDialog({
        title: "Rechnung jetzt ausstellen?",
        bullets: [
          "Es wird eine fertige PDF- und E-Rechnung erzeugt und gespeichert.",
          "Danach lässt sich die Rechnung nicht mehr ändern (gesetzlich vorgeschrieben).",
          "Korrigieren geht dann nur noch über eine Storno-Rechnung.",
        ],
        confirmLabel: "Ausstellen",
      }))
    )
      return;
    issuing = true;
    try {
      await invoicesLockAndIssue(detail.invoice.id, buyerReference || "N/A");
      flash("Rechnung ausgestellt.");
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      issuing = false;
    }
  }

  // Verwerfen eines nummerierten Entwurfs (W14-032): kein Löschen, die Nummer
  // bleibt verbraucht; der Server verlangt Daten-Passwort und App-Code
  // (Re-Auth-Dialog über transport.ts).
  async function voidDraft() {
    if (!detail) return;
    const problem = voidReasonError(voidReason);
    voidError = problem ?? "";
    if (problem) {
      await tick();
      voidReasonEl?.focus();
      return;
    }
    voiding = true;
    try {
      await invoicesVoidDraft({ id: detail.invoice.id, reason: voidReason.trim() });
      flash("Entwurf verworfen — die Nummer bleibt vergeben.");
      showVoid = false;
      voidReason = "";
      voidError = "";
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      voiding = false;
    }
  }

  function statusLabel(s: string): string {
    switch (s) {
      case "draft":           return "Entwurf";
      case "issued":          return "Ausgestellt";
      case "sent":            return "Versendet";
      case "partially_paid":  return "Teilzahlung";
      case "paid":            return "Bezahlt";
      case "canceled":        return "Storniert";
      default:                return s;
    }
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";
  function statusTone(s: string): Tone {
    switch (s) {
      case "issued":          return "primary";
      case "sent":            return "info";
      case "partially_paid":  return "warning";
      case "paid":            return "success";
      case "canceled":        return "danger";
      default:                return "neutral";
    }
  }

  function valLabel(s: string | null): string {
    switch (s) {
      case "passed":  return "in Ordnung";
      case "warning": return "mit Hinweisen";
      case "failed":  return "Fehler";
      default:        return "—";
    }
  }
  function valTone(s: string | null): Tone {
    return s === "passed" ? "success" : s === "warning" ? "warning" : s === "failed" ? "danger" : "neutral";
  }
</script>

<PageBar back="/invoices" backLabel="Liste" title={detail?.invoice.invoiceNumber}>
  {#snippet actions()}
    {#if detail}
      {#if detail.invoice.voidedAt}
        <!-- Verworfener Entwurf: endgültig, keine Aktionen (W14-032). -->
      {:else if !detail.invoice.lockedAt}
        {#if !detail.invoice.isStornoFor}
          <!-- Neuer Fund F2a (Lauf F5): Entwurf bearbeiten; die Nummer bleibt. -->
          <a class="btn-secondary" href={`/invoices/new?edit=${detail.invoice.id}`}>Bearbeiten</a>
        {/if}
        <button type="button" class="btn-secondary" onclick={runValidation} disabled={validating}>
          {validating ? "Prüfe …" : "Prüfen"}
        </button>
        <!-- U1: sichtbare Bezeichnung statt nur aria-label/title (§8.5 Eingabefelder). -->
        <label class="ref-wrap">
          <span class="ref-lbl">Leitweg-ID <span class="kb-visually-hidden">(Referenz für die E-Rechnung, nur bei Behörden; sonst N/A)</span></span>
          <input
            type="text"
            bind:value={buyerReference}
            class="ref-inline"
            autocomplete="off"
            spellcheck="false"
          />
        </label>
        <button type="button" class="btn-primary" onclick={lockAndIssue} disabled={issuing}>
          {issuing ? "Stelle aus …" : "Rechnung ausstellen"}
        </button>
        <button type="button" class="btn-ghost-danger" onclick={() => (showVoid = true)} disabled={voiding || issuing}>
          Entwurf verwerfen
        </button>
      {:else}
        {#if detail.invoice.status !== "canceled" && !detail.invoice.isStornoFor}
          <a class="btn-primary" href={`/invoices/${detail.invoice.id}/payment`}>+ Zahlung</a>
        {/if}
        {#if detail.invoice.pdfArchiveId}
          <button type="button" class="btn-secondary" onclick={openPdf}>PDF öffnen</button>
          <button type="button" class="btn-secondary" onclick={downloadPdf}>PDF herunterladen</button>
          {#if detail.invoice.status !== "canceled"}
            <a class="btn-secondary" href={`/invoices/${detail.invoice.id}/send`}>Senden</a>
          {/if}
        {/if}
        {#if detail.invoice.status !== "canceled" && !detail.invoice.isStornoFor}
          <a class="btn-ghost-danger" href={`/invoices/${detail.invoice.id}/cancel`}>Stornieren</a>
        {/if}
      {/if}
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error && !detail}
  <Banner>Fehler: {error}</Banner>
{:else if detail}
  {@const inv = detail.invoice}
  <p class="sub">
    {date(inv.invoiceDate)} ·
    {#if inv.voidedAt}
      <Badge tone="neutral" strike>Verworfen</Badge>
    {:else}
      <Badge tone={statusTone(inv.status)} strike={inv.status === "canceled"}>{statusLabel(inv.status)}</Badge>
    {/if}
    {#if inv.isStornoFor}
      <Badge tone="warning">Storno-Beleg</Badge>
    {/if}
    {#if inv.canceledByStornoId}
      <Badge tone="danger">durch Storno aufgehoben</Badge>
    {/if}
  </p>

  {#if inv.isStornoFor}
    <Banner kind="info">
      Dieser Beleg storniert
      {#if relatedOriginal}
        die Rechnung <a class="txt-link doc-no" href={`/invoices/${inv.isStornoFor}`}>{relatedOriginal.invoiceNumber}</a>
        vom {date(relatedOriginal.invoiceDate)}.
      {:else}
        <a class="txt-link" href={`/invoices/${inv.isStornoFor}`}>die ursprüngliche Rechnung</a>.
      {/if}
    </Banner>
  {/if}
  {#if inv.canceledByStornoId}
    <Banner kind="warning">
      Diese Rechnung ist aufgehoben durch
      {#if relatedStorno}
        den Storno-Beleg <a class="txt-link doc-no" href={`/invoices/${inv.canceledByStornoId}`}>{relatedStorno.invoiceNumber}</a>
        vom {date(relatedStorno.invoiceDate)}.
      {:else}
        <a class="txt-link" href={`/invoices/${inv.canceledByStornoId}`}>einen Storno-Beleg</a>.
      {/if}
    </Banner>
  {/if}

  {#if inv.voidedAt}
    <section class="card">
      <h2>Verworfen</h2>
      <p>
        Am {dateTime(inv.voidedAt)} verworfen. Der Entwurf wurde nie ausgestellt; die Nummer
        {inv.invoiceNumber} bleibt vergeben und wird nicht erneut benutzt.
      </p>
      {#if inv.voidReason}<p><strong>Grund:</strong> {inv.voidReason}</p>{/if}
    </section>
  {:else if showVoid && !inv.lockedAt}
    <section class="card">
      <h2>Entwurf verwerfen</h2>
      <p class="muted">
        Der Entwurf wird nicht gelöscht: Zeitpunkt und Grund stehen daran und im Protokoll, die
        Nummer {inv.invoiceNumber} bleibt vergeben. Danach lässt er sich weder ändern noch
        ausstellen. Klein.Buch fragt zur Bestätigung Daten-Passwort und App-Code ab.
      </p>
      <label class="void-reason" for="void-reason">
        <span>Grund <span class="req" aria-hidden="true">*</span><span class="kb-visually-hidden">(Pflicht)</span></span>
      </label>
      <textarea
        id="void-reason"
        class="void-text"
        rows="3"
        bind:this={voidReasonEl}
        bind:value={voidReason}
        oninput={() => { if (voidError && !voidReasonError(voidReason)) voidError = ""; }}
        required
        aria-required="true"
        aria-invalid={voidError ? "true" : undefined}
        aria-describedby={voidError ? "void-reason-err" : "void-reason-hint"}
      ></textarea>
      {#if voidError}
        <p id="void-reason-err" class="field-err">{voidError}</p>
      {:else}
        <p id="void-reason-hint" class="field-hint">Mindestens 10 Zeichen, ohne Namen von Personen.</p>
      {/if}
      <div class="void-actions">
        <button type="button" class="btn-secondary" onclick={() => { showVoid = false; voidError = ""; }} disabled={voiding}>Abbrechen</button>
        <button type="button" class="btn-danger" onclick={voidDraft} disabled={voiding}>
          {voiding ? "Verwerfe …" : "Entwurf verwerfen"}
        </button>
      </div>
    </section>
  {/if}

  {#if issues !== null}
    <section class="card">
      <h2>Prüfung</h2>
      {#if issues.length === 0}
        <p class="ok">✓ Alles in Ordnung — die Rechnung kann ausgestellt werden.</p>
      {:else}
        <ul class="issues">
          {#each issues as iss (iss.code + iss.message)}
            <li><strong>{iss.code}:</strong> {iss.message}</li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

  <div class="cols">
    <section class="card">
      <h2>Verkäufer</h2>
      <p>
        {inv.sellerName}<br/>
        {inv.sellerStreet}<br/>
        {inv.sellerPostalCode} {inv.sellerCity}<br/>
        {#if inv.sellerTaxNumber}<small>StNr: {inv.sellerTaxNumber}</small><br/>{/if}
        {#if inv.sellerVatId}<small>USt-IdNr: {inv.sellerVatId}</small>{/if}
      </p>
      {#if inv.isKleinunternehmer === 1}
        <p class="klein-note">Kleinunternehmer (§19) — es wird keine Mehrwertsteuer ausgewiesen.</p>
      {/if}
    </section>
    <section class="card">
      <h2>Empfänger</h2>
      {#if detail.invoice.lockedAt && detail.invoice.buyerName}
        <!-- Block 19: Festgeschriebene Rechnung → Empfänger-Snapshot (eingefroren),
             unabhängig von späteren Kontakt-Änderungen/Anonymisierung. -->
        <p>
          {detail.invoice.buyerName}<br/>
          {detail.invoice.buyerStreet ?? ""}<br/>
          {detail.invoice.buyerPostalCode ?? ""} {detail.invoice.buyerCity ?? ""}<br/>
          {#if detail.invoice.buyerVatId}<small>USt-IdNr: {detail.invoice.buyerVatId}</small>{/if}
        </p>
      {:else if detail.buyer}
        <p>
          {detail.buyer.name}<br/>
          {detail.buyer.street ?? ""}<br/>
          {detail.buyer.postalCode ?? ""} {detail.buyer.city ?? ""}<br/>
          {#if detail.buyer.vatId}<small>USt-IdNr: {detail.buyer.vatId}</small>{/if}
        </p>
      {:else}
        <p class="muted">Kontakt nicht gefunden.</p>
      {/if}
    </section>
    <section class="card">
      <h2>Daten</h2>
      <dl>
        <dt>Jahr</dt><dd>{inv.fiscalYear}</dd>
        <dt>Leistung</dt><dd>{date(inv.deliveryDate)}</dd>
        <dt>Fällig</dt><dd>{date(inv.dueDate)}</dd>
        <dt>PDF-Vorlage</dt><dd>{pdfTemplateLabel(inv.pdfTemplate)}</dd>
        {#if inv.lockedAt}
          <dt>Ausgestellt am</dt><dd>{date(inv.lockedAt)}</dd>
          <dt>Prüfung</dt>
          <dd>
            <Badge tone={valTone(inv.validationStatus)}>{valLabel(inv.validationStatus)}</Badge>
          </dd>
        {/if}
      </dl>
    </section>
  </div>

  <section class="card">
    <h2>Positionen</h2>
    <table class="cards">
      <caption class="kb-visually-hidden">Positionen der Rechnung {inv.invoiceNumber}</caption>
      <thead>
        <tr>
          <th>#</th>
          <th>Beschreibung</th>
          <th class="right">Menge</th>
          <th>Einheit</th>
          <th class="right">Einzelpreis</th>
          {#if inv.isKleinunternehmer === 0}
            <th class="right">USt %</th>
            <th>USt-Art</th>
          {/if}
          <th class="right">Netto</th>
        </tr>
      </thead>
      <tbody>
        {#each detail.items as it (it.id)}
          <tr>
            <td data-label="Pos.">{it.position}</td>
            <td class="cards-full" data-label="Beschreibung">{it.description}</td>
            <td class="right" data-label="Menge">{formatQuantity(it.quantity)}</td>
            <td data-label="Einheit">{it.unitCode}</td>
            <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents)}</td>
            {#if inv.isKleinunternehmer === 0}
              <td class="right" data-label="USt %">{percent(it.taxRatePercent)}</td>
              <td data-label="USt-Art">{it.taxCategoryCode}</td>
            {/if}
            <td class="right" data-label="Netto">{euro(it.netAmountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan={inv.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">Netto</td>
            <td class="right" data-label="Netto">{euro(inv.netAmountCents)}</td></tr>
        {#if inv.taxAmountCents !== 0}
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">USt</td>
              <td class="right" data-label="USt">{euro(inv.taxAmountCents)}</td></tr>
        {/if}
        <tr class="grand"><td colspan={inv.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">
              <strong>Brutto</strong></td>
            <td class="right" data-label="Brutto"><strong>{euro(inv.grossAmountCents)}</strong></td></tr>
        {#if inv.paidAmountCents > 0}
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">Bezahlt</td>
              <td class="right" data-label="Bezahlt">{euro(inv.paidAmountCents)}</td></tr>
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">Offen</td>
              <td class="right" data-label="Offen">{euro(inv.grossAmountCents - inv.paidAmountCents)}</td></tr>
        {/if}
      </tfoot>
    </table>
  </section>

  {#if sendLog.length > 0}
    <section class="card">
      <h2>Versand-Historie</h2>
      <table class="cards">
        <caption class="kb-visually-hidden">Versand-Historie</caption>
        <thead>
          <tr>
            <th>Zeitpunkt</th>
            <th>Status</th>
            <th>Empfänger</th>
            <th>Kanal</th>
            <th>Antwort des Anbieters</th>
          </tr>
        </thead>
        <tbody>
          {#each sendLog as e (e.id)}
            <tr>
              <td data-label="Zeitpunkt">{dateTime(e.createdAt)}</td>
              <td data-label="Status">{e.status === "success" ? "✓ versendet" : "✗ fehlgeschlagen"}</td>
              <td data-label="Empfänger">{e.toEmail}</td>
              <td data-label="Kanal">{e.channel === "graph" ? "Microsoft 365" : "SMTP"}</td>
              <td class="cards-full" data-label="Antwort des Anbieters">
                {#if e.status === "success"}
                  {[e.providerCode, e.providerMessage].filter(Boolean).join(" ")}
                  {#if e.requestId}<br /><small class="muted">request-id: {e.requestId}</small>{/if}
                {:else}
                  <span class="err">{e.error ?? "—"}</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
      <p class="muted hist-note">Vollständiges Protokoll: Einstellungen → E-Mail-Protokoll.</p>
    </section>
  {/if}

  {#if inv.notes}
    <section class="card">
      <h2>Notiz</h2>
      <p>{inv.notes}</p>
    </section>
  {/if}

  {#if inv.cancelReason}
    <section class="card">
      <h2>Storno-Grund</h2>
      <p>{inv.cancelReason}</p>
    </section>
  {/if}
{/if}

<style>
  .sub { color: var(--text-muted); margin: 0 0 1rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .ref-inline { padding: 0.4rem 0.6rem; border: 1px solid var(--line-control); border-radius: var(--r-md); width: 10rem; max-width: 100%; font: inherit; min-height: var(--target); }
  .ref-wrap { display: inline-flex; align-items: center; gap: var(--sp-2); }
  .ref-lbl { font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); white-space: nowrap; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0.75rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 6rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid var(--line); }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid var(--text); }
  .klein-note { color: var(--status-info-fg); font-size: var(--fs-sm); }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .issues { padding-left: 1.25rem; }
  .issues li { color: var(--status-danger-fg); margin-bottom: 0.25rem; }
  .ok { color: var(--status-success-fg); }
  .muted { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); }
  .hist-note { font-size: var(--fs-sm); margin-top: 0.4rem; }
  .txt-link { text-decoration: underline; text-underline-offset: 2px; }
  .doc-no { font-family: var(--font-mono); }
  .req { color: var(--status-danger-fg); }
  .field-err { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--status-danger-fg); }
  .field-hint { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-muted); }
  .void-reason { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); margin-bottom: 4px; }
  .void-text {
    font: inherit; width: 100%; box-sizing: border-box; resize: vertical;
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
  }
  .void-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 0.5rem; flex-wrap: wrap; }
  /* W10: Karten-Ansicht — Summenzeilen zeigen das Label per data-label an der
     Betragszelle, die colspan-Beschriftung entfällt; lange Werte brechen um. */
  @media (max-width: 820px) {
    td.sum-label { display: none; }
    td { overflow-wrap: anywhere; }
  }
</style>
