<script lang="ts">
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { onMount } from "svelte";
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
  import type { InvoiceDetail, ValidationIssueDto, EmailLogEntry } from "#lib/types.ts";
  import { euro, date, dateTime } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
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

  let id = $derived(page.params.id ?? "");

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await invoicesGet(id);
      if (!detail) error = "Rechnung nicht gefunden.";
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
    if (problem) {
      flash(problem, "error");
      return;
    }
    voiding = true;
    try {
      await invoicesVoidDraft({ id: detail.invoice.id, reason: voidReason.trim() });
      flash("Entwurf verworfen — die Nummer bleibt vergeben.");
      showVoid = false;
      voidReason = "";
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

  function valLabel(s: string | null): string {
    switch (s) {
      case "passed":  return "in Ordnung";
      case "warning": return "mit Hinweisen";
      case "failed":  return "Fehler";
      default:        return "—";
    }
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
        <button class="btn-secondary" onclick={runValidation} disabled={validating}>
          {validating ? "Prüfe …" : "Prüfen"}
        </button>
        <input type="text" bind:value={buyerReference} placeholder="Referenz (nur Behörden)" class="ref-inline" />
        <button class="btn-primary" onclick={lockAndIssue} disabled={issuing}>
          {issuing ? "Stelle aus …" : "Rechnung ausstellen"}
        </button>
        <button class="btn-ghost-danger" onclick={() => (showVoid = true)} disabled={voiding || issuing}>
          Verwerfen
        </button>
      {:else}
        {#if detail.invoice.status !== "canceled" && !detail.invoice.isStornoFor}
          <a class="btn-primary" href={`/invoices/${detail.invoice.id}/payment`}>+ Zahlung</a>
        {/if}
        {#if detail.invoice.pdfArchiveId}
          <button class="btn-secondary" onclick={openPdf}>PDF öffnen</button>
          <button class="btn-secondary" onclick={downloadPdf}>PDF herunterladen</button>
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
    <span class={`status status-${inv.status}`}>{statusLabel(inv.status)}</span>
    {#if inv.isStornoFor}
      <span class="badge badge-storno">Storno-Beleg</span>
    {/if}
    {#if inv.canceledByStornoId}
      <span class="badge badge-canceled">durch Storno aufgehoben</span>
    {/if}
    {#if inv.voidedAt}
      <span class="badge badge-canceled">verworfen</span>
    {/if}
  </p>

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
      <label class="void-reason">
        Grund *
        <textarea rows="2" bind:value={voidReason} placeholder="Pflichtangabe, mindestens 10 Zeichen, keine Namen"></textarea>
      </label>
      <div class="void-actions">
        <button class="btn-secondary" onclick={() => (showVoid = false)} disabled={voiding}>Abbrechen</button>
        <button class="btn-danger" onclick={voidDraft} disabled={voiding}>
          {voiding ? "…" : "Verwerfen"}
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
        <dt>Layout</dt><dd>{inv.pdfTemplate}</dd>
        {#if inv.lockedAt}
          <dt>Ausgestellt am</dt><dd>{date(inv.lockedAt)}</dd>
          <dt>Prüfung</dt>
          <dd>
            <span class={`status status-${inv.validationStatus ?? "?"}`}>
              {valLabel(inv.validationStatus)}
            </span>
          </dd>
        {/if}
      </dl>
    </section>
  </div>

  <section class="card">
    <h2>Positionen</h2>
    <table class="cards">
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
            <td class="right" data-label="Menge">{it.quantity}</td>
            <td data-label="Einheit">{it.unitCode}</td>
            <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents)}</td>
            {#if inv.isKleinunternehmer === 0}
              <td class="right" data-label="USt %">{it.taxRatePercent}</td>
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
  .sub { color: var(--c-text-muted); margin: 0 0 1rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .ref-inline { padding: 0.4rem 0.5rem; border: 1px solid var(--c-border-strong); border-radius: var(--r-md); width: 10rem; max-width: 100%; font: inherit; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0.75rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  dl { display: grid; grid-template-columns: 6rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: #6b7280; font-size: 0.85rem; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.85rem; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid #1a1a1a; }
  .klein-note { color: #1e40af; font-size: 0.85rem; }
  .badge { padding: 0.05rem 0.4rem; border-radius: 4px; font-size: 0.7rem; margin-left: 0.4rem; }
  .badge-storno   { background: #fbbf24; color: #78350f; }
  .badge-canceled { background: #fee2e2; color: #991b1b; }
  .status { padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.75rem; }
  .status-draft           { background: #e5e7eb; color: #374151; }
  .status-issued, .status-sent, .status-passed { background: #dbeafe; color: #1e40af; }
  .status-partially_paid, .status-warning  { background: #fef3c7; color: #92400e; }
  .status-paid            { background: #d1fae5; color: #065f46; }
  .status-canceled, .status-failed  { background: #fee2e2; color: #991b1b; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .issues { padding-left: 1.25rem; }
  .issues li { color: #b91c1c; margin-bottom: 0.25rem; }
  .ok { color: #065f46; }
  .muted { color: #6b7280; }
  .err { color: #b91c1c; }
  .hist-note { font-size: 0.8rem; margin-top: 0.4rem; }
  .void-reason { display: flex; flex-direction: column; gap: 0.25rem; }
  .void-reason textarea { font: inherit; width: 100%; }
  .void-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 0.5rem; flex-wrap: wrap; }
  /* W10: Karten-Ansicht — Summenzeilen zeigen das Label per data-label an der
     Betragszelle, die colspan-Beschriftung entfällt; lange Werte brechen um. */
  @media (max-width: 820px) {
    td.sum-label { display: none; }
    td { overflow-wrap: anywhere; }
  }
</style>
