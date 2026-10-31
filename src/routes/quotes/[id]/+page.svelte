<script lang="ts">
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pdfTemplateLabel, unitLabel } from "#lib/labels.ts";
  import { todayBerlin } from "#lib/date.ts";
  import { relevantTaxDate } from "#lib/taxStatus.ts";
  import { taxStatusAtDate } from "#lib/taxStatusAt.svelte.ts";
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
  import { euro, date, dateTimeShort, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import type { ActionMenuItem } from "#lib/ActionMenu.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import RestoreConflictBanner from "#lib/RestoreConflictBanner.svelte";
  import { canEditQuote, quoteStatusView } from "../quoteStatus.ts";
  import {
    isTechnicalError,
    issueReasons,
    lastSentAt,
    reasonTarget,
    reasonTargets,
    type ReasonTarget,
  } from "../../invoices/invoiceStatus.ts";

  let detail = $state<QuoteDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notFound = $state(false);
  // Prüfung des Entwurfs (nur lesend, V1-03): läuft beim Öffnen von selbst.
  let issues = $state<ValidationIssueDto[] | null>(null);
  let checkError = $state("");
  let validating = $state(false);
  // Ablehnung beim Fertigstellen: Gründe als stehendes Banner (ADR 0041 Nr. 22).
  let issueFailure = $state<string[] | null>(null);
  let issueBannerEl: HTMLDivElement | undefined = $state();
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
  // ADR 0042 Nr. 3 (V1-R06): Ein Entwurf zeigt den Steuerstatus seines
  // Angebotsdatums wie der Editor; eingefroren gilt erst der Wert beim
  // Fertigstellen. Verworfene Entwürfe behalten den gespeicherten Wert.
  const draftTaxDate = $derived.by(() => {
    const q = detail?.quote;
    if (!q || q.lockedAt || q.status !== "draft") return "";
    return relevantTaxDate(null, q.quoteDate, today);
  });
  const draftTaxAt = taxStatusAtDate(() => draftTaxDate);
  let isKlein = $derived(
    draftTaxDate && draftTaxAt.date === draftTaxDate && draftTaxAt.isKlein !== null
      ? draftTaxAt.isKlein
      : detail?.quote.isKleinunternehmer !== 0,
  );
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
    notFound = false;
    try {
      detail = await quotesGet(id);
      if (!detail) {
        notFound = true;
        error = "Angebot nicht gefunden.";
      }
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
    if (detail && !detail.quote.lockedAt && detail.quote.status === "draft") await check();
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

  // Bereiche aus der PageBar bzw. dem Menü „Mehr“ öffnen: Fokus auf das erste Feld.
  async function openPanel(which: "accept" | "reject" | "cancel") {
    closePanels();
    if (which === "accept") showAccept = true;
    else if (which === "reject") showReject = true;
    else showCancel = true;
    await tick();
    const first = { accept: "accept-date", reject: "reject-reason", cancel: "cancel-reason" }[which];
    document.getElementById(first)?.focus();
  }

  async function openQuotePdf() {
    if (!detail || busy) return;
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
    if (!detail || busy) return;
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

  /** Prüft den Entwurf (nur lesend); Ergebnis und Fehler stehen in der Karte „Prüfung“. */
  async function check() {
    if (!detail || validating) return;
    validating = true;
    checkError = "";
    try {
      issues = await quotesValidateDraft(detail.quote.id);
    } catch (e) {
      issues = null;
      checkError = String(e);
    } finally {
      validating = false;
    }
  }

  // „Prüfen“ (PageBar): zeigt wieder die aktuelle Prüfung statt einer alten Ablehnung.
  async function runValidation() {
    issueFailure = null;
    await check();
  }

  function targetLink(t: ReasonTarget, contactId: string): { href: string; label: string } | null {
    // U6-005: nach dem Speichern der Firmendaten zurück zu diesem Angebot.
    if (t === "seller") return { href: `/settings/seller?next=/quotes/${id}`, label: "Firmendaten ergänzen" };
    if (t === "buyer") return { href: `/contacts/${contactId}`, label: "Kontakt ergänzen" };
    // ADR 0042 Nr. 13: Gründe im Entwurf selbst (Positionen, Daten) dort beheben.
    if (detail && canEditQuote(detail.quote)) return { href: `/quotes/${id}/edit`, label: "Entwurf bearbeiten" };
    return null;
  }

  async function issue() {
    if (!detail || busy) return;
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
    busy = true;
    issueFailure = null;
    try {
      await quotesIssue(detail.quote.id);
      flash(`Angebot ${detail.quote.quoteNumber} fertiggestellt.`);
      closePanels();
      await load();
    } catch (e) {
      const msg = String(e);
      if (isTechnicalError(msg)) {
        flash(msg, "error");
      } else {
        // V1-03: Gründe als Banner in der Karte „Prüfung“, Fokus dorthin.
        issueFailure = issueReasons(msg) ?? [msg];
        await tick();
        issueBannerEl?.focus();
      }
    } finally {
      busy = false;
    }
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
    if (busy) return;
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

  // V1-01: Versand laut E-Mail-Protokoll (der Server setzt beim Fertigstellen
  // `status = 'sent'`, ohne dass etwas verschickt wurde).
  let sentAtIso = $derived(lastSentAt(sendLog));
  // Nach dem Fertigstellen ist „Senden“ der nächste Schritt, bis ein Versand
  // protokolliert ist; danach „Annehmen“ (ADR 0041 Nr. 96, präzisiert Nr. 171).
  let sendIsMain = $derived(detail?.quote.status === "sent" && !sentAtIso);

  // ---- PageBar (U1-059): Hauptaktion = nächster Schritt; Nebenaktionen und
  // Gefahraktionen (abgesetzt) im Menü „Mehr“ (design.md §4.2). ----
  type Bar = { secondary: ActionMenuItem[]; more: ActionMenuItem[] };
  let bar = $derived.by((): Bar => {
    const q = detail?.quote;
    if (!q) return { secondary: [], more: [] };
    const secondary: ActionMenuItem[] = [];
    const more: ActionMenuItem[] = [];
    if (!q.lockedAt) {
      // ADR 0042 Nr. 13: offene Entwürfe lassen sich bearbeiten (wie Rechnungs-Entwürfe).
      if (canEditQuote(q)) secondary.push({ label: "Bearbeiten", href: `/quotes/${q.id}/edit` });
      // Während der Prüfung nicht gesperrt (Fokus bleibt); check() wartet.
      secondary.push({ label: validating ? "Prüfe …" : "Prüfen", onSelect: runValidation });
    } else {
      if (sendIsMain) secondary.push({ label: "Annehmen", onSelect: () => openPanel("accept"), disabled: busy });
      if (q.status === "sent") secondary.push({ label: "Ablehnen", onSelect: () => openPanel("reject"), disabled: busy });
      if (q.status === "converted" && q.convertedInvoiceId) {
        secondary.push({ label: "Zur Rechnung", href: `/invoices/${q.convertedInvoiceId}` });
      }
      // V1-18: gleiche Wörter wie bei der Rechnung („PDF öffnen“, „Senden“).
      secondary.push({ label: "PDF öffnen", onSelect: openQuotePdf });
      if (q.status !== "canceled" && q.status !== "rejected" && !sendIsMain) {
        secondary.push({ label: "Senden", href: `/quotes/${q.id}/send` });
      }
      more.push({ label: "Druck-PDF (mit AGB)", onSelect: openBundle });
    }
    if (q.status === "draft" || q.status === "sent" || q.status === "accepted") {
      more.push({
        label: q.lockedAt ? "Stornieren" : "Entwurf verwerfen",
        onSelect: () => openPanel("cancel"),
        danger: true,
        disabled: busy,
      });
    }
    return { secondary, more };
  });

  // ---- Positionen (U2-020): Kartenansicht nur, wenn die Tabelle nicht passt —
  // Schwelle je Tabelle nach Spaltenzahl statt global 820 px. Gemessen wird die
  // Breite der Karte (ResizeObserver), nicht der Viewport. ----
  let itemsBox = $state<HTMLDivElement | null>(null);
  let itemsWidth = $state(typeof window !== "undefined" ? window.innerWidth - 64 : 1024);
  $effect(() => {
    const el = itemsBox;
    if (!el || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => (itemsWidth = el.clientWidth));
    ro.observe(el);
    itemsWidth = el.clientWidth;
    return () => ro.disconnect();
  });
  // 6 Spalten (§19) passen ab rund 35rem, 8 Spalten (Regelbesteuerung) ab 45rem.
  let itemsAsCards = $derived(itemsWidth < (!isKlein ? 720 : 560));

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  // Plan W7: Anlagen und gebundene Rechtsdokumente als Download.
  function openAttachment(archiveEntryId: string) {
    saveArchiveFile(archiveEntryId);
  }

  // V4-08: Überschrift auch beim Laden und bei „nicht gefunden“.
  let pageTitle = $derived(detail ? `Angebot ${detail.quote.quoteNumber}` : notFound ? "Angebot nicht gefunden" : "Angebot");
  // Prüfergebnis als Statuszeile (4.1.3: „Prüfen“ aus der PageBar sagt das Ergebnis an).
  let checkText = $derived(
    validating
      ? "Wird geprüft …"
      : issues === null
        ? ""
        : issues.length === 0
          ? "Alle Pflichtangaben sind vorhanden — das Angebot kann fertiggestellt werden."
          : issues.length === 1
            ? "Vor dem Fertigstellen fehlt noch eine Angabe:"
            : `Vor dem Fertigstellen fehlen noch ${issues.length} Angaben:`,
  );
</script>

<PageBar
  back="/quotes"
  backLabel="Angebote"
  title={pageTitle}
  secondary={bar.secondary}
  more={bar.more}
>
  {#snippet actions()}
    {#if detail}
      {@const q = detail.quote}
      {#if !q.lockedAt && q.status === "draft"}
        <Button onclick={issue} ariaDisabled={busy}>{busy ? "Stelle fertig …" : "Fertigstellen"}</Button>
      {:else if sendIsMain}
        <Button href={`/quotes/${q.id}/send`}>Senden</Button>
      {:else if q.status === "sent"}
        <Button onclick={() => openPanel("accept")} ariaDisabled={busy}>Annehmen</Button>
      {:else if q.status === "accepted"}
        <Button href={`/quotes/${q.id}/convert`}>In Rechnung umwandeln</Button>
      {/if}
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error && !detail}
  <Banner title={notFound ? undefined : "Das Angebot lässt sich nicht laden."}>{error}</Banner>
{:else if detail}
  {@const q = detail.quote}
  {@const st = quoteStatusView(q)}
  <!-- Metazeile: Trennpunkt am Ende eines Teils, nie am Zeilenanfang (U1-065). -->
  <p class="sub">
    <span class="meta">{date(q.quoteDate)}<span aria-hidden="true"> ·</span></span>
    <span class="meta">
      <Badge tone={st.tone} strike={st.strike}>{st.label}</Badge><span aria-hidden="true"> ·</span>
    </span>
    {#if q.lockedAt && sentAtIso}
      <!-- V1-01: Versand aus dem E-Mail-Protokoll, getrennt vom Status. -->
      <span class="meta">versendet am {date(sentAtIso)}<span aria-hidden="true"> ·</span></span>
    {/if}
    <span class="meta">gültig bis {date(q.validUntil)}</span>
  </p>

  {#if canEditQuote(q)}
    <!-- Warteliste (8), ADR 0042 Nr. 16: aus dem Lesefeld des Entwurfs. -->
    <RestoreConflictBanner conflict={detail.restoreConflict} number={q.quoteNumber} doc="quote" where="detail" editHref={`/quotes/${q.id}/edit`} />
  {/if}

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
            class="kb-w-date"
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
      <div class="kb-actions actions">
        <Button variant="secondary" onclick={() => { showAccept = false; acceptError = ""; }} ariaDisabled={busy}>Abbrechen</Button>
        <Button onclick={accept} ariaDisabled={busy}>
          {busy ? "Speichere …" : "Annahme speichern"}
        </Button>
      </div>
    </section>
  {/if}

  {#if showReject}
    <section class="card accept-panel">
      <h2>Angebot ablehnen</h2>
      <label class="block">
        Ablehnungsgrund (optional)
        <textarea id="reject-reason" rows="2" bind:value={rejectReason} aria-describedby="reject-hint"></textarea>
        <span id="reject-hint" class="field-hint">Zum Beispiel: Kunde hat sich für einen Mitbewerber entschieden.</span>
      </label>
      <div class="kb-actions actions">
        <Button variant="secondary" onclick={() => (showReject = false)} ariaDisabled={busy}>Abbrechen</Button>
        <Button onclick={reject} ariaDisabled={busy}>
          {busy ? "Speichere …" : "Als abgelehnt markieren"}
        </Button>
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
          id="cancel-reason"
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
      <div class="kb-actions actions">
        <Button variant="secondary" onclick={() => { showCancel = false; cancelError = ""; }} ariaDisabled={busy}>Abbrechen</Button>
        <Button variant="danger" onclick={cancel} ariaDisabled={busy}>
          {busy ? (q.lockedAt ? "Storniere …" : "Verwerfe …") : q.lockedAt ? "Stornieren" : "Entwurf verwerfen"}
        </Button>
      </div>
    </section>
  {/if}

  {#if !q.lockedAt && q.status === "draft"}
    <!-- V1-03: Prüfung läuft beim Öffnen des Entwurfs; Ablehnungen beim Fertigstellen
         stehen hier als Banner mit dem Weg zur Korrektur. -->
    <section class="card" aria-labelledby="check-h">
      <h2 id="check-h">Prüfung</h2>
      <p class="check-line" class:ok={!issueFailure && issues !== null && issues.length === 0} role="status">{issueFailure ? "" : checkText}</p>
      {#if issueFailure}
        {@const targets = reasonTargets(issueFailure.map((text) => ({ text })))}
        <div class="issue-err" tabindex="-1" bind:this={issueBannerEl}>
          <Banner kind="error" title="Das Angebot wurde nicht fertiggestellt.">
            {#if issueFailure.length === 1}
              {issueFailure[0]}
            {:else}
              <ul class="reasons">
                {#each issueFailure as r (r)}<li>{r}</li>{/each}
              </ul>
            {/if}
            {#each targets as t (t)}
              {@const link = targetLink(t, q.contactId)}
              {#if link}<span class="fix-link"><a class="kb-link kb-target" href={link.href}>{link.label}</a></span>{/if}
            {/each}
          </Banner>
        </div>
      {:else if checkError}
        <p class="err">Die Prüfung ist fehlgeschlagen: {checkError}</p>
      {:else if issues && issues.length > 0}
        <ul class="issues">
          {#each issues as iss (iss.code + iss.message)}
            {@const link = targetLink(reasonTarget(iss.message, iss.code), q.contactId)}
            <li>
              {iss.message}
              {#if link}{" "}<a class="kb-link kb-target" href={link.href}>{link.label}</a>{/if}
            </li>
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
      {#if isKlein}
        <p class="klein-note">Kleinunternehmer (§19) — es wird keine Umsatzsteuer ausgewiesen.</p>
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
              · <a class="kb-link kb-target" href={`/invoices/${q.convertedInvoiceId}`}>zur Rechnung</a>
            {/if}
          </dd>
        {/if}
      </dl>
    </section>
  </div>

  <section class="card">
    <h2>Positionen</h2>
    <div class="items-box" bind:this={itemsBox}>
    <table class="items-table" class:cards={itemsAsCards}>
      <caption class="kb-visually-hidden">Positionen des Angebots {q.quoteNumber}</caption>
      <thead>
        <tr>
          <th>#</th>
          <th>Beschreibung</th>
          <th class="kb-num">Menge</th>
          <th>Einheit</th>
          <th class="kb-num">Einzelpreis</th>
          {#if !isKlein}
            <th class="kb-num">USt %</th>
            <th>USt-Art</th>
          {/if}
          <th class="kb-num">Netto</th>
        </tr>
      </thead>
      <tbody>
        {#each detail.items as it (it.id)}
          <tr>
            <td data-label="Pos.">{it.position}</td>
            <td class="cards-full desc" data-label="Beschreibung">{it.description}</td>
            <td class="kb-num" data-label="Menge">{formatQuantity(it.quantity)}</td>
            <td data-label="Einheit">{unitLabel(it.unitCode)}</td>
            <td class="kb-num" data-label="Einzelpreis">{euro(it.unitPriceCents)}</td>
            {#if !isKlein}
              <td class="kb-num" data-label="USt %">{percent(it.taxRatePercent)}</td>
              <td data-label="USt-Art">{it.taxCategoryCode}</td>
            {/if}
            <td class="kb-num" data-label="Netto">{euro(it.netAmountCents)}</td>
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr><td colspan={isKlein ? 5 : 7} class="sum-label">Netto</td>
            <td class="kb-num" data-label="Netto">{euro(q.netAmountCents)}</td></tr>
        {#if q.taxAmountCents !== 0}
          <tr><td colspan={isKlein ? 5 : 7} class="sum-label">USt</td>
              <td class="kb-num" data-label="USt">{euro(q.taxAmountCents)}</td></tr>
        {/if}
        <tr class="kb-sum"><td colspan={isKlein ? 5 : 7} class="sum-label">
              <strong>Brutto</strong></td>
            <td class="kb-num" data-label="Brutto"><strong>{euro(q.grossAmountCents)}</strong></td></tr>
      </tfoot>
    </table>
    </div>
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
              <td class="kb-card-head" data-label="Zeitpunkt">{dateTimeShort(e.createdAt)}</td>
              <td class="kb-card-status" data-label="Status">{#if e.status === "success"}<Badge tone="info">Versendet</Badge>{:else}<Badge tone="danger">Fehlgeschlagen</Badge>{/if}</td>
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
      <p class="muted hist-note">Vollständiges Protokoll: <a class="kb-link" href="/settings/mail-log">E-Mail-Protokoll</a>.</p>
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
  /* Teile der Metazeile brechen nur zwischen sich um; der Trennpunkt steht am Ende (U1-065). */
  .meta { display: inline-flex; align-items: center; white-space: nowrap; }
  /* V3-14: Das Leerzeichen vor dem Trennpunkt bleibt auch am Anfang eines Flex-Elements. */
  .meta > [aria-hidden="true"] { white-space: pre; }
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
  .check-line { margin: 0 0 var(--sp-2); }
  /* Leer bleibt die Zeile im Accessibility-Tree (Live-Region), nur ohne Abstand. */
  .check-line:empty { margin: 0; }
  .check-line.ok { color: var(--status-success-fg); }
  .issue-err { outline: none; }
  .issue-err:focus-visible { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); border-radius: var(--radius-control); }
  .reasons { margin: var(--sp-1) 0 0; padding-left: 1.25rem; }
  .fix-link { display: inline-block; margin: var(--sp-2) var(--sp-3) 0 0; }
  textarea { resize: vertical; width: 100%; box-sizing: border-box; }
  .actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 1rem; flex-wrap: wrap; }
  /* Labelspalte so breit wie das längste Label (U3-037). */
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); white-space: nowrap; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; border-bottom: 1px solid var(--line); }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  /* Beträge über `.kb-num` (V3-10, ADR 0042 Nr. 16); Summen-Bezeichnung rechts davor. */
  td.sum-label { text-align: right; }
  /* Beträge und Mengen in der Tabelle nicht umbrechen; die Beschreibung bricht. */
  .items-table td.kb-num { white-space: nowrap; }
  .items-table:not(.cards) td.desc { overflow-wrap: break-word; hyphens: auto; }
  .klein-note { color: var(--status-info-fg); font-size: var(--fs-sm); }
  .att { list-style: none; padding: 0; margin: 0; }
  .att li { display: flex; justify-content: space-between; align-items: center; gap: 0.75rem; padding: 0.4rem 0; border-bottom: 1px solid var(--line); flex-wrap: wrap; }
  .att-info { min-width: 0; overflow-wrap: anywhere; }
  .att-actions { display: flex; gap: 0.4rem; }
  /* .btn-sm entfernt — Buttons app-weit gleich groß (Manuel-Hardline 2026-05-26). */
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  .issues { padding-left: 1.25rem; }
  .issues li { color: var(--status-danger-fg); margin-bottom: 0.25rem; }
  .muted { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); }
  .hist-note { font-size: var(--fs-sm); margin-top: 0.4rem; }
  /* Kartenansicht (nur bis 820 px, tokens.css): Summenzeilen zeigen das Label per
     data-label an der Betragszelle, die colspan-Beschriftung entfällt; lange Werte
     brechen um. */
  @media screen and (max-width: 820px) {
    table.cards td.sum-label { display: none; }
    table.cards td { overflow-wrap: anywhere; }
  }
</style>
