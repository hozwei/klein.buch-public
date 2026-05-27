<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import {
    quotesGet,
    quotesValidateDraft,
    quotesIssue,
    quotesAccept,
    quotesReject,
    quotesCancel,
    quotesGeneratePdf,
    quotesOpenBundle,
    quotesLegalBindings,
    attachmentsOpen,
    attachmentsReveal,
    emailLogFor,
  } from "$lib/api";
  import type {
    QuoteDetail,
    QuoteLegalBinding,
    ValidationIssueDto,
    EmailLogEntry,
  } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";

  let detail = $state<QuoteDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let issues = $state<ValidationIssueDto[] | null>(null);
  let validating = $state(false);
  let busy = $state(false);
  let bindings = $state<QuoteLegalBinding[]>([]);
  let sendLog = $state<EmailLogEntry[]>([]);

  // Annahme-Panel
  let showAccept = $state(false);
  let acceptedDate = $state(new Date().toISOString().slice(0, 10));
  let signedContractFile = $state<File | null>(null);
  let attachmentLabel = $state("");

  // Ablehnungs-/Storno-Panel (kein window.prompt — in WebView2 unzuverlässig).
  let showReject = $state(false);
  let rejectReason = $state("");
  let showCancel = $state(false);
  let cancelReason = $state("");

  function closePanels() {
    showAccept = false;
    showReject = false;
    showCancel = false;
  }

  let id = $derived($page.params.id ?? "");

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await quotesGet(id);
      if (!detail) error = "Angebot nicht gefunden.";
      else if (detail.quote.lockedAt) await loadBindings();
      try {
        sendLog = await emailLogFor("quote", id);
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

  async function loadBindings() {
    if (!detail) return;
    try {
      bindings = await quotesLegalBindings(detail.quote.id);
    } catch {
      // Bindungen sind optional fürs Anzeigen — Fehler hier nicht hochziehen.
    }
  }

  onMount(load);

  async function openQuotePdf() {
    if (!detail) return;
    busy = true;
    try {
      const q = await quotesGeneratePdf(detail.quote.id);
      if (q.pdfArchiveId) await attachmentsOpen(q.pdfArchiveId);
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function openBundle() {
    if (!detail) return;
    busy = true;
    try {
      await quotesOpenBundle(detail.quote.id);
      await loadBindings();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  function docTypeLabel(t: string): string {
    return t === "agb" ? "AGB" : t === "privacy" ? "Datenschutz" : t;
  }

  async function runValidation() {
    if (!detail) return;
    validating = true;
    issues = null;
    try {
      issues = await quotesValidateDraft(detail.quote.id);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      validating = false;
    }
  }

  async function issue() {
    if (!detail) return;
    if (
      !(await confirmDialog({
        title: "Angebot jetzt fertigstellen?",
        bullets: [
          "Das Angebot wird verbindlich und kann danach versendet werden.",
          "Die Inhalte lassen sich danach nicht mehr ändern.",
        ],
        confirmLabel: "Fertigstellen",
      }))
    )
      return;
    await run(() => quotesIssue(detail!.quote.id), "Angebot fertiggestellt.");
  }

  async function accept() {
    if (!detail) return;
    await run(async () => {
      let bytes: number[] | null = null;
      let filename: string | null = null;
      if (signedContractFile) {
        const buf = await signedContractFile.arrayBuffer();
        bytes = Array.from(new Uint8Array(buf));
        filename = signedContractFile.name;
      }
      return quotesAccept({
        quoteId: detail!.quote.id,
        acceptedDate: acceptedDate || null,
        signedContractBytes: bytes,
        signedContractFilename: filename,
        attachmentLabel: attachmentLabel.trim() || null,
      });
    }, "Angebot angenommen.");
    signedContractFile = null;
    attachmentLabel = "";
  }

  async function reject() {
    if (!detail) return;
    await run(() => quotesReject({
      quoteId: detail!.quote.id,
      reason: rejectReason.trim() || null,
    }), "Angebot als abgelehnt markiert.");
    rejectReason = "";
  }

  async function cancel() {
    if (!detail) return;
    if (!cancelReason.trim()) {
      flash("Storno-Grund ist Pflicht.", "error");
      return;
    }
    await run(() => quotesCancel({ quoteId: detail!.quote.id, reason: cancelReason.trim() }), "Angebot storniert.");
    cancelReason = "";
  }

  async function run(fn: () => Promise<unknown>, okMsg?: string) {
    busy = true;
    try {
      await fn();
      if (okMsg) flash(okMsg);
      closePanels();
      await load();
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }

  function statusLabel(s: string): string {
    switch (s) {
      case "draft":     return "Entwurf";
      case "sent":      return "Versendet";
      case "accepted":  return "Angenommen";
      case "rejected":  return "Abgelehnt";
      case "canceled":  return "Storniert";
      case "converted": return "Umgewandelt";
      default:          return s;
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  async function openAttachment(archiveEntryId: string) {
    try {
      await attachmentsOpen(archiveEntryId);
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function revealAttachment(archiveEntryId: string) {
    try {
      await attachmentsReveal(archiveEntryId);
    } catch (e) {
      flash(String(e), "error");
    }
  }
</script>

<PageBar back="/quotes" backLabel="Liste" title={detail?.quote.quoteNumber}>
  {#snippet actions()}
    {#if detail}
      {@const q = detail.quote}
      {#if !q.lockedAt}
        <button class="btn-secondary" onclick={runValidation} disabled={validating}>{validating ? "Prüfe …" : "Prüfen"}</button>
        <button class="btn-primary" onclick={issue} disabled={busy}>{busy ? "…" : "Fertigstellen"}</button>
      {:else if q.status === "sent"}
        <button class="btn-primary" onclick={() => { closePanels(); showAccept = true; }} disabled={busy}>Annehmen</button>
        <button class="btn-secondary" onclick={() => { closePanels(); showReject = true; }} disabled={busy}>Ablehnen</button>
      {:else if q.status === "accepted"}
        <a href={`/quotes/${q.id}/convert`} class="btn-primary">In Rechnung umwandeln</a>
      {:else if q.status === "converted" && q.convertedInvoiceId}
        <a href={`/invoices/${q.convertedInvoiceId}`} class="btn-secondary">Zur Rechnung →</a>
      {/if}
      {#if q.lockedAt}
        <button class="btn-secondary" onclick={openQuotePdf} disabled={busy}>PDF anzeigen</button>
        <button class="btn-secondary" onclick={openBundle} disabled={busy}>Druck-PDF (mit AGB)</button>
        {#if q.status !== "canceled" && q.status !== "rejected"}
          <a href={`/quotes/${q.id}/send`} class="btn-secondary">Versenden</a>
        {/if}
      {/if}
      {#if q.status === "draft" || q.status === "sent" || q.status === "accepted"}
        <button class="btn-ghost-danger" onclick={() => { closePanels(); showCancel = true; }} disabled={busy}>Stornieren</button>
      {/if}
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error && !detail}
  <Banner>Fehler: {error}</Banner>
{:else if detail}
  {@const q = detail.quote}
  <p class="sub">
    {date(q.quoteDate)} ·
    <span class={`status status-${q.status}`}>{statusLabel(q.status)}</span>
    · gültig bis {date(q.validUntil)}
  </p>

  {#if showAccept}
    <section class="card accept-panel">
      <h2>Angebot annehmen</h2>
      <p class="muted">
        Optional: unterschriebenen Vertrag hochladen (wird sicher gespeichert und ans
        Angebot angehängt). Ohne Datei wird das Angebot nur als angenommen markiert.
      </p>
      <div class="grid">
        <label>
          Annahmedatum
          <input type="date" bind:value={acceptedDate} />
        </label>
        <label>
          Unterschriebener Vertrag (PDF/Bild)
          <input
            type="file"
            accept="application/pdf,image/png,image/jpeg"
            onchange={(e) => { signedContractFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null; }}
          />
          {#if signedContractFile}
            <small class="muted">{signedContractFile.name} · {fmtBytes(signedContractFile.size)}</small>
          {/if}
        </label>
        <label>
          Bezeichnung (optional)
          <input type="text" bind:value={attachmentLabel} placeholder="Unterschriebener Vertrag" />
        </label>
      </div>
      <div class="actions">
        <button class="btn-secondary" onclick={() => (showAccept = false)} disabled={busy}>Abbrechen</button>
        <button class="btn-primary" onclick={accept} disabled={busy}>
          {busy ? "Speichere …" : "Annahme speichern"}
        </button>
      </div>
    </section>
  {/if}

  {#if showReject}
    <section class="card accept-panel">
      <h2>Angebot ablehnen</h2>
      <label class="block">
        Ablehnungsgrund (optional)
        <textarea rows="2" bind:value={rejectReason} placeholder="z. B. Kunde hat sich für einen Mitbewerber entschieden"></textarea>
      </label>
      <div class="actions">
        <button class="btn-secondary" onclick={() => (showReject = false)} disabled={busy}>Abbrechen</button>
        <button class="btn-primary" onclick={reject} disabled={busy}>
          {busy ? "…" : "Als abgelehnt markieren"}
        </button>
      </div>
    </section>
  {/if}

  {#if showCancel}
    <section class="card cancel-panel">
      <h2>Angebot stornieren</h2>
      <p class="muted">Das Angebot wird nicht gelöscht, sondern als storniert markiert (so verlangt es das Gesetz).</p>
      <label class="block">
        Storno-Grund *
        <textarea rows="2" bind:value={cancelReason} placeholder="Pflichtangabe"></textarea>
      </label>
      <div class="actions">
        <button class="btn-secondary" onclick={() => (showCancel = false)} disabled={busy}>Abbrechen</button>
        <button class="btn-danger" onclick={cancel} disabled={busy}>
          {busy ? "…" : "Stornieren"}
        </button>
      </div>
    </section>
  {/if}

  {#if issues !== null}
    <section class="card">
      <h2>Prüfung</h2>
      {#if issues.length === 0}
        <p class="ok">✓ Alles in Ordnung — das Angebot kann fertiggestellt werden.</p>
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
        {q.sellerName}<br/>
        {q.sellerStreet}<br/>
        {q.sellerPostalCode} {q.sellerCity}<br/>
        {#if q.sellerTaxNumber}<small>StNr: {q.sellerTaxNumber}</small><br/>{/if}
        {#if q.sellerVatId}<small>USt-IdNr: {q.sellerVatId}</small>{/if}
      </p>
      {#if q.isKleinunternehmer === 1}
        <p class="klein-note">Kleinunternehmer (§19) — es wird keine Mehrwertsteuer ausgewiesen.</p>
      {/if}
    </section>
    <section class="card">
      <h2>Empfänger</h2>
      {#if detail.quote.lockedAt && detail.quote.buyerName}
        <!-- Block 19: Festgeschriebenes Angebot → Empfänger-Snapshot (eingefroren),
             unabhängig von späteren Kontakt-Änderungen/Anonymisierung. -->
        <p>
          {detail.quote.buyerName}<br/>
          {detail.quote.buyerStreet ?? ""}<br/>
          {detail.quote.buyerPostalCode ?? ""} {detail.quote.buyerCity ?? ""}<br/>
          {#if detail.quote.buyerVatId}<small>USt-IdNr: {detail.quote.buyerVatId}</small>{/if}
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
        <dt>Jahr</dt><dd>{q.fiscalYear}</dd>
        <dt>Gültig bis</dt><dd>{date(q.validUntil)}</dd>
        <dt>Layout</dt><dd>{q.pdfTemplate}</dd>
        {#if q.lockedAt}<dt>Fertiggestellt</dt><dd>{date(q.lockedAt)}</dd>{/if}
        {#if q.acceptedAt}<dt>Angenommen</dt><dd>{date(q.acceptedAt)}</dd>{/if}
        {#if q.rejectedAt}<dt>Abgelehnt</dt><dd>{date(q.rejectedAt)}</dd>{/if}
        {#if q.convertedAt}
          <dt>Umgewandelt</dt>
          <dd>
            {date(q.convertedAt)}
            {#if q.convertedInvoiceId}
              · <a href={`/invoices/${q.convertedInvoiceId}`}>zur Rechnung</a>
            {/if}
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
          {#if q.isKleinunternehmer === 0}
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
            {#if q.isKleinunternehmer === 0}
              <td class="right">{it.taxRatePercent}</td>
              <td>{it.taxCategoryCode}</td>
            {/if}
            <td class="right">{euro(it.netAmountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan={q.isKleinunternehmer === 1 ? 6 : 8} class="right">Netto</td>
            <td class="right">{euro(q.netAmountCents)}</td></tr>
        {#if q.taxAmountCents !== 0}
          <tr><td colspan={q.isKleinunternehmer === 1 ? 6 : 8} class="right">USt</td>
              <td class="right">{euro(q.taxAmountCents)}</td></tr>
        {/if}
        <tr class="grand"><td colspan={q.isKleinunternehmer === 1 ? 6 : 8} class="right">
              <strong>Brutto</strong></td>
            <td class="right"><strong>{euro(q.grossAmountCents)}</strong></td></tr>
      </tfoot>
    </table>
  </section>

  <section class="card">
    <h2>Anhänge</h2>
    {#if detail.attachments.length === 0}
      <p class="muted">Keine Anhänge. (Beim Annehmen kann der unterschriebene Vertrag angehängt werden.)</p>
    {:else}
      <ul class="att">
        {#each detail.attachments as a (a.id)}
          <li>
            <div class="att-info">
              <strong>{a.label ?? a.fileName}</strong>
              <span class="muted">— {a.fileName} · {fmtBytes(a.fileSizeBytes)} · {date(a.createdAt)}</span>
            </div>
            <div class="att-actions">
              <button class="btn-secondary btn-sm" onclick={() => openAttachment(a.archiveEntryId)}>Öffnen</button>
              <button class="btn-secondary btn-sm" onclick={() => revealAttachment(a.archiveEntryId)}>Im Ordner</button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  {#if q.lockedAt}
    <section class="card">
      <h2>Beigefügte Dokumente (AGB &amp; Datenschutz)</h2>
      {#if bindings.length === 0}
        <p class="muted">
          Noch nichts beigefügt. Deine aktuellen AGB und Datenschutz-Bedingungen werden
          automatisch fest mit dem Angebot verknüpft, sobald du das Druck-PDF erzeugst
          oder das Angebot versendest — als Nachweis, welche Fassung galt.
        </p>
      {:else}
        <ul class="att">
          {#each bindings as b (b.id)}
            <li>
              <div class="att-info">
                <strong>{docTypeLabel(b.docType)} v{b.version}</strong>
                <span class="muted">— {b.title} · gebunden {date(b.boundAt)}</span>
              </div>
              <div class="att-actions">
                <button class="btn-secondary btn-sm" onclick={() => openAttachment(b.archiveEntryId)}>Öffnen</button>
              </div>
            </li>
          {/each}
        </ul>
      {/if}
    </section>
  {/if}

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

  {#if q.notes}
    <section class="card"><h2>Notiz</h2><p>{q.notes}</p></section>
  {/if}

  {#if q.canceledReason}
    <section class="card"><h2>Storno-Grund</h2><p>{q.canceledReason}</p></section>
  {/if}
{/if}

<style>
  .sub { color: var(--c-text-muted); margin: 0 0 1rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .hint { color: #6b7280; font-size: 0.85rem; font-style: italic; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0.75rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  .accept-panel { border-left: 4px solid #2563eb; }
  .cancel-panel { border-left: 4px solid #ef4444; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; }
  label.block { width: 100%; }
  input, textarea { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  .actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem; }
  dl { display: grid; grid-template-columns: 7rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: #6b7280; font-size: 0.85rem; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid #e5e7eb; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.85rem; }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid #1a1a1a; }
  .klein-note { color: #1e40af; font-size: 0.85rem; }
  .att { list-style: none; padding: 0; margin: 0; }
  .att li { display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; padding: 0.4rem 0; border-bottom: 1px solid #f0f0f0; flex-wrap: wrap; }
  .att-actions { display: flex; gap: 0.4rem; }
  /* .btn-sm entfernt — Buttons app-weit gleich groß (Manuel-Hardline 2026-05-26). */
  .status { padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.75rem; }
  .status-draft      { background: #e5e7eb; color: #374151; }
  .status-sent       { background: #dbeafe; color: #1e40af; }
  .status-accepted   { background: #d1fae5; color: #065f46; }
  .status-rejected   { background: #fee2e2; color: #991b1b; }
  .status-canceled   { background: #fee2e2; color: #991b1b; }
  .status-converted  { background: #ede9fe; color: #5b21b6; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .issues { padding-left: 1.25rem; }
  .issues li { color: #b91c1c; margin-bottom: 0.25rem; }
  .ok { color: #065f46; }
  .muted { color: #6b7280; }
  .err { color: #b91c1c; }
  .hist-note { font-size: 0.8rem; margin-top: 0.4rem; }
</style>
