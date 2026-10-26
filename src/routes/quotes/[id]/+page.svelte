<script lang="ts">
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pdfTemplateLabel } from "#lib/labels.ts";
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
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
    archiveUrl,
    downloadUrl,
    saveArchiveFile,
    uploadFile,
    emailLogFor,
  } from "#lib/api.ts";
  import type {
    QuoteDetail,
    QuoteLegalBinding,
    ValidationIssueDto,
    EmailLogEntry,
  } from "#lib/types.ts";
  import { euro, date, dateTime, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";

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
  let acceptedDate = $state(todayBerlin());
  let signedContractFile = $state<File | null>(null);
  let attachmentLabel = $state("");

  // Ablehnungs-/Storno-Panel (kein window.prompt — in WebView2 unzuverlässig).
  let showReject = $state(false);
  let rejectReason = $state("");
  let showCancel = $state(false);
  let cancelReason = $state("");
  // Feldfehler am Grund (statt Toast).
  let cancelError = $state("");
  let cancelReasonEl: HTMLTextAreaElement | undefined = $state();
  // Annahmedatum: nicht vor dem Angebotsdatum, nicht in der Zukunft (domain::quote).
  const today = todayBerlin();
  let acceptError = $state("");

  function closePanels() {
    showAccept = false;
    showReject = false;
    showCancel = false;
    cancelError = "";
    acceptError = "";
  }

  let id = $derived(page.params.id ?? "");

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
      if (q.pdfArchiveId)
        openPdfPreview(archiveUrl(q.pdfArchiveId), `Angebot ${q.quoteNumber}`, "");
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
      const ticket = await quotesOpenBundle(detail.quote.id);
      openPdfPreview(downloadUrl(ticket), "Angebot mit AGB und Datenschutz", ticket.fileName);
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
    const qd = detail.quote.quoteDate.slice(0, 10);
    acceptError =
      acceptedDate && acceptedDate < qd
        ? "Das Annahmedatum darf nicht vor dem Angebotsdatum liegen."
        : acceptedDate && acceptedDate > today
          ? "Das Annahmedatum darf nicht in der Zukunft liegen."
          : "";
    if (acceptError) {
      await tick();
      document.getElementById("accept-date")?.focus();
      return;
    }
    await run(async () => {
      const signedContractUploadId = signedContractFile
        ? (await uploadFile("vertrag", signedContractFile)).uploadId
        : null;
      return quotesAccept({
        quoteId: detail!.quote.id,
        acceptedDate: acceptedDate || null,
        signedContractUploadId,
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
    if (cancelReason.trim().length < 10) {
      cancelError = "Bitte einen Grund angeben (mindestens 10 Zeichen).";
      await tick();
      cancelReasonEl?.focus();
      return;
    }
    cancelError = "";
    const wasDraft = !detail.quote.lockedAt;
    await run(
      () => quotesCancel({ quoteId: detail!.quote.id, reason: cancelReason.trim() }),
      wasDraft ? "Angebots-Entwurf verworfen." : "Angebot storniert.",
    );
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

  // F2: Ein nie fertiggestelltes Angebot wird nicht storniert, sondern verworfen.
  function statusLabel(s: string, locked: boolean): string {
    switch (s) {
      case "draft":     return "Entwurf";
      case "sent":      return "Versendet";
      case "accepted":  return "Angenommen";
      case "rejected":  return "Abgelehnt";
      case "canceled":  return locked ? "Storniert" : "Verworfen";
      case "converted": return "Umgewandelt";
      default:          return s;
    }
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";
  function statusTone(s: string): Tone {
    switch (s) {
      case "sent":      return "info";
      case "accepted":  return "success";
      case "rejected":  return "danger";
      case "canceled":  return "danger";
      case "converted": return "primary";
      default:          return "neutral";
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  // Plan W7: Anlagen und gebundene Rechtsdokumente als Download.
  function openAttachment(archiveEntryId: string) {
    saveArchiveFile(archiveEntryId);
  }
</script>

<PageBar back="/quotes" backLabel="Liste" title={detail?.quote.quoteNumber}>
  {#snippet actions()}
    {#if detail}
      {@const q = detail.quote}
      {#if !q.lockedAt}
        <button type="button" class="btn-secondary" onclick={runValidation} disabled={validating}>{validating ? "Prüfe …" : "Prüfen"}</button>
        <button type="button" class="btn-primary" onclick={issue} disabled={busy}>{busy ? "…" : "Fertigstellen"}</button>
      {:else if q.status === "sent"}
        <button type="button" class="btn-primary" onclick={() => { closePanels(); showAccept = true; }} disabled={busy}>Annehmen</button>
        <button type="button" class="btn-secondary" onclick={() => { closePanels(); showReject = true; }} disabled={busy}>Ablehnen</button>
      {:else if q.status === "accepted"}
        <a href={`/quotes/${q.id}/convert`} class="btn-primary">In Rechnung umwandeln</a>
      {:else if q.status === "converted" && q.convertedInvoiceId}
        <a href={`/invoices/${q.convertedInvoiceId}`} class="btn-secondary">Zur Rechnung →</a>
      {/if}
      {#if q.lockedAt}
        <button type="button" class="btn-secondary" onclick={openQuotePdf} disabled={busy}>PDF anzeigen</button>
        <button type="button" class="btn-secondary" onclick={openBundle} disabled={busy}>Druck-PDF (mit AGB)</button>
        {#if q.status !== "canceled" && q.status !== "rejected"}
          <a href={`/quotes/${q.id}/send`} class="btn-secondary">Versenden</a>
        {/if}
      {/if}
      {#if q.status === "draft" || q.status === "sent" || q.status === "accepted"}
        <button type="button" class="btn-ghost-danger" onclick={() => { closePanels(); showCancel = true; }} disabled={busy}>
          {q.lockedAt ? "Stornieren" : "Entwurf verwerfen"}
        </button>
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
    <Badge tone={statusTone(q.status)} strike={q.status === "canceled"}>{statusLabel(q.status, !!q.lockedAt)}</Badge>
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
          <input
            id="accept-date"
            type="date"
            bind:value={acceptedDate}
            min={q.quoteDate.slice(0, 10)}
            max={today}
            aria-invalid={acceptError ? "true" : undefined}
            aria-describedby={acceptError ? "accept-date-err" : undefined}
          />
          {#if acceptError}<span id="accept-date-err" class="field-err">{acceptError}</span>{/if}
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
          <input type="text" bind:value={attachmentLabel} autocomplete="off" aria-describedby="att-label-hint" />
          <span id="att-label-hint" class="field-hint">Ohne Angabe: „Unterschriebener Vertrag“ bzw. der Dateiname.</span>
        </label>
      </div>
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={() => { showAccept = false; acceptError = ""; }} disabled={busy}>Abbrechen</button>
        <button type="button" class="btn-primary" onclick={accept} disabled={busy}>
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
        <textarea rows="2" bind:value={rejectReason} aria-describedby="reject-hint"></textarea>
        <span id="reject-hint" class="field-hint">Zum Beispiel: Kunde hat sich für einen Mitbewerber entschieden.</span>
      </label>
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={() => (showReject = false)} disabled={busy}>Abbrechen</button>
        <button type="button" class="btn-primary" onclick={reject} disabled={busy}>
          {busy ? "…" : "Als abgelehnt markieren"}
        </button>
      </div>
    </section>
  {/if}

  {#if showCancel}
    <section class="card cancel-panel">
      {#if q.lockedAt}
        <h2>Angebot stornieren</h2>
        <p class="muted">Das Angebot wird nicht gelöscht, sondern als storniert markiert; Grund und Zeitpunkt bleiben nachvollziehbar.</p>
      {:else}
        <h2>Angebots-Entwurf verwerfen</h2>
        <p class="muted">Der Entwurf wird nicht gelöscht, sondern als verworfen markiert; Grund und Zeitpunkt bleiben nachvollziehbar. Danach lässt er sich nicht mehr ändern.</p>
      {/if}
      <label class="block">
        <span>Grund <span class="req" aria-hidden="true">*</span><span class="kb-visually-hidden">(Pflicht)</span></span>
        <textarea
          rows="2"
          bind:this={cancelReasonEl}
          bind:value={cancelReason}
          oninput={() => { if (cancelError && cancelReason.trim().length >= 10) cancelError = ""; }}
          required
          aria-required="true"
          aria-invalid={cancelError ? "true" : undefined}
          aria-describedby={cancelError ? "cancel-reason-err" : "cancel-reason-hint"}
        ></textarea>
        {#if cancelError}
          <span id="cancel-reason-err" class="field-err">{cancelError}</span>
        {:else}
          <span id="cancel-reason-hint" class="field-hint">Mindestens 10 Zeichen.</span>
        {/if}
      </label>
      <div class="actions">
        <button type="button" class="btn-secondary" onclick={() => { showCancel = false; cancelError = ""; }} disabled={busy}>Abbrechen</button>
        <button type="button" class="btn-danger" onclick={cancel} disabled={busy}>
          {busy ? "…" : q.lockedAt ? "Stornieren" : "Entwurf verwerfen"}
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
        <dt>PDF-Vorlage</dt><dd>{pdfTemplateLabel(q.pdfTemplate)}</dd>
        {#if q.lockedAt}<dt>Fertiggestellt</dt><dd>{date(q.lockedAt)}</dd>{/if}
        {#if q.acceptedAt}<dt>Angenommen</dt><dd>{date(q.acceptedAt)}</dd>{/if}
        {#if q.rejectedAt}<dt>Abgelehnt</dt><dd>{date(q.rejectedAt)}</dd>{/if}
        {#if q.convertedAt}
          <dt>Umgewandelt</dt>
          <dd>
            {date(q.convertedAt)}
            {#if q.convertedInvoiceId}
              · <a class="txt-link tgt" href={`/invoices/${q.convertedInvoiceId}`}>zur Rechnung</a>
            {/if}
          </dd>
        {/if}
      </dl>
    </section>
  </div>

  <section class="card">
    <h2>Positionen</h2>
    <table class="cards">
      <caption class="kb-visually-hidden">Positionen des Angebots {q.quoteNumber}</caption>
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
            <td data-label="Pos.">{it.position}</td>
            <td class="cards-full" data-label="Beschreibung">{it.description}</td>
            <td class="right" data-label="Menge">{formatQuantity(it.quantity)}</td>
            <td data-label="Einheit">{it.unitCode}</td>
            <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents)}</td>
            {#if q.isKleinunternehmer === 0}
              <td class="right" data-label="USt %">{percent(it.taxRatePercent)}</td>
              <td data-label="USt-Art">{it.taxCategoryCode}</td>
            {/if}
            <td class="right" data-label="Netto">{euro(it.netAmountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan={q.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">Netto</td>
            <td class="right" data-label="Netto">{euro(q.netAmountCents)}</td></tr>
        {#if q.taxAmountCents !== 0}
          <tr><td colspan={q.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">USt</td>
              <td class="right" data-label="USt">{euro(q.taxAmountCents)}</td></tr>
        {/if}
        <tr class="grand"><td colspan={q.isKleinunternehmer === 1 ? 5 : 7} class="right sum-label">
              <strong>Brutto</strong></td>
            <td class="right" data-label="Brutto"><strong>{euro(q.grossAmountCents)}</strong></td></tr>
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
              <button type="button" class="btn-secondary btn-sm" onclick={() => openAttachment(a.archiveEntryId)}>Herunterladen</button>
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
                <button type="button" class="btn-secondary btn-sm" onclick={() => openAttachment(b.archiveEntryId)}>Herunterladen</button>
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

  {#if q.notes}
    <section class="card"><h2>Notiz</h2><p>{q.notes}</p></section>
  {/if}

  {#if q.canceledReason}
    <section class="card"><h2>{q.lockedAt ? "Storno-Grund" : "Grund für das Verwerfen"}</h2><p>{q.canceledReason}</p></section>
  {/if}
{/if}

<style>
  .sub { color: var(--text-muted); margin: 0 0 1rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0.75rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  .accept-panel { border-left: 4px solid var(--status-info-line); }
  .cancel-panel { border-left: 4px solid var(--status-danger-line); }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 0.75rem; }
  label { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); min-width: 0; }
  label.block { width: 100%; }
  input:not([type="file"]), textarea { padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md); font: inherit; font-weight: 400; color: var(--text); }
  .req { color: var(--status-danger-fg); }
  .field-err { font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
  .txt-link { text-decoration: underline; text-underline-offset: 2px; }
  .tgt { display: inline-flex; align-items: center; min-height: var(--target); }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  .actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem; flex-wrap: wrap; }
  dl { display: grid; grid-template-columns: 7rem 1fr; gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid var(--line); }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  tr.grand td { border-top: 2px solid var(--text); }
  .klein-note { color: var(--status-info-fg); font-size: var(--fs-sm); }
  .att { list-style: none; padding: 0; margin: 0; }
  .att li { display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; padding: 0.4rem 0; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .att-info { min-width: 0; overflow-wrap: anywhere; }
  .att-actions { display: flex; gap: 0.4rem; }
  /* .btn-sm entfernt — Buttons app-weit gleich groß (Manuel-Hardline 2026-05-26). */
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .issues { padding-left: 1.25rem; }
  .issues li { color: var(--status-danger-fg); margin-bottom: 0.25rem; }
  .ok { color: var(--status-success-fg); }
  .muted { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); }
  .hist-note { font-size: var(--fs-sm); margin-top: 0.4rem; }
  /* W10: Karten-Ansicht — Summenzeilen zeigen das Label per data-label an der
     Betragszelle, die colspan-Beschriftung entfällt; lange Werte brechen um. */
  @media (max-width: 820px) {
    td.sum-label { display: none; }
    td { overflow-wrap: anywhere; }
  }
</style>
