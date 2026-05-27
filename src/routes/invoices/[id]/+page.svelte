<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import {
    invoicesGet,
    invoicesValidateDraft,
    invoicesLockAndIssue,
    invoicesOpenPdf,
    invoicesRevealPdf,
    emailLogFor,
  } from "$lib/api";
  import type { InvoiceDetail, ValidationIssueDto, EmailLogEntry } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let issues = $state<ValidationIssueDto[] | null>(null);
  let validating = $state(false);
  let issuing = $state(false);
  let buyerReference = $state("N/A");
  let sendLog = $state<EmailLogEntry[]>([]);

  let id = $derived($page.params.id ?? "");

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

  function fmtLogTime(ts: string): string {
    const iso = ts.includes("T") ? ts : ts.replace(" ", "T") + "Z";
    const d = new Date(iso);
    return isNaN(d.getTime()) ? ts : d.toLocaleString("de-DE");
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

  async function openPdf() {
    if (!detail) return;
    try {
      await invoicesOpenPdf(detail.invoice.id);
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function revealPdf() {
    if (!detail) return;
    try {
      await invoicesRevealPdf(detail.invoice.id);
    } catch (e) {
      flash(String(e), "error");
    }
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
      {#if !detail.invoice.lockedAt}
        <button class="btn-secondary" onclick={runValidation} disabled={validating}>
          {validating ? "Prüfe …" : "Prüfen"}
        </button>
        <input type="text" bind:value={buyerReference} placeholder="Referenz (nur Behörden)" class="ref-inline" />
        <button class="btn-primary" onclick={lockAndIssue} disabled={issuing}>
          {issuing ? "Stelle aus …" : "Rechnung ausstellen"}
        </button>
      {:else}
        {#if detail.invoice.status !== "canceled" && !detail.invoice.isStornoFor}
          <a class="btn-primary" href={`/invoices/${detail.invoice.id}/payment`}>+ Zahlung</a>
        {/if}
        {#if detail.invoice.pdfArchiveId}
          <button class="btn-secondary" onclick={openPdf}>PDF öffnen</button>
          <button class="btn-secondary" onclick={revealPdf}>Im Ordner zeigen</button>
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
  </p>

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
    <table>
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
            <td>{it.position}</td>
            <td>{it.description}</td>
            <td class="right">{it.quantity}</td>
            <td>{it.unitCode}</td>
            <td class="right">{euro(it.unitPriceCents)}</td>
            {#if inv.isKleinunternehmer === 0}
              <td class="right">{it.taxRatePercent}</td>
              <td>{it.taxCategoryCode}</td>
            {/if}
            <td class="right">{euro(it.netAmountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan={inv.isKleinunternehmer === 1 ? 6 : 8} class="right">Netto</td>
            <td class="right">{euro(inv.netAmountCents)}</td></tr>
        {#if inv.taxAmountCents !== 0}
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 6 : 8} class="right">USt</td>
              <td class="right">{euro(inv.taxAmountCents)}</td></tr>
        {/if}
        <tr class="grand"><td colspan={inv.isKleinunternehmer === 1 ? 6 : 8} class="right">
              <strong>Brutto</strong></td>
            <td class="right"><strong>{euro(inv.grossAmountCents)}</strong></td></tr>
        {#if inv.paidAmountCents > 0}
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 6 : 8} class="right">Bezahlt</td>
              <td class="right">{euro(inv.paidAmountCents)}</td></tr>
          <tr><td colspan={inv.isKleinunternehmer === 1 ? 6 : 8} class="right">Offen</td>
              <td class="right">{euro(inv.grossAmountCents - inv.paidAmountCents)}</td></tr>
        {/if}
      </tfoot>
    </table>
  </section>

  {#if sendLog.length > 0}
    <section class="card">
      <h2>Versand-Historie</h2>
      <table>
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
              <td>{fmtLogTime(e.createdAt)}</td>
              <td>{e.status === "success" ? "✓ versendet" : "✗ fehlgeschlagen"}</td>
              <td>{e.toEmail}</td>
              <td>{e.channel === "graph" ? "Microsoft 365" : "SMTP"}</td>
              <td>
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
  .ref-inline { padding: 0.4rem 0.5rem; border: 1px solid var(--c-border-strong); border-radius: var(--r-md); width: 10rem; font: inherit; }
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
</style>
