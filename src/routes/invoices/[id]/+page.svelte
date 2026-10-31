<script lang="ts">
  import { openPdfPreview } from "#lib/pdfPreviewModal.svelte.ts";
  import { pdfTemplateLabel, unitLabel } from "#lib/labels.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    invoicesGet,
    invoicesValidateDraft,
    invoicesLockAndIssue,
    invoicesVoidDraft,
    invoicesReversePayment,
    archiveUrl,
    saveArchiveFile,
    emailLogFor,
    paymentAccountsList,
  } from "#lib/api.ts";
  import type {
    Invoice,
    InvoiceDetail,
    InvoicePayment,
    ValidationIssueDto,
    EmailLogEntry,
    PaymentAccount,
  } from "#lib/types.ts";
  import { euro, date, dateTime, dateTimeShort, percent } from "#lib/format.ts";
  import { formatQuantity } from "#lib/money.ts";
  import { todayBerlin } from "#lib/date.ts";
  import { invoiceTaxDate } from "#lib/taxStatus.ts";
  import { buyerReferenceError, normalizeBuyerReference, serviceSummary } from "#lib/serviceDate.ts";
  import BuyerReferenceField from "#lib/BuyerReferenceField.svelte";
  import { taxStatusAtDate } from "#lib/taxStatusAt.svelte.ts";
  import { dayClock } from "#lib/dayClock.svelte.ts";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import Button from "#lib/Button.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import ActionMenu, { type ActionMenuItem } from "#lib/ActionMenu.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { voidReasonError } from "#lib/voidDraft.ts";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { portal } from "#lib/portal.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { REAUTH_CANCELED } from "#lib/transport.ts";
  import { focusLost } from "#lib/focusAfterRemoval.ts";
  import RestoreConflictBanner from "#lib/RestoreConflictBanner.svelte";
  import {
    accountText,
    cleanReverseReason,
    isReasonRejection,
    paymentAccountId,
    paymentLines,
    REVERSE_REASON_MAX_CHARS,
    reverseReasonError,
  } from "../paymentHistory.ts";
  import {
    docKind,
    invoiceStatusView,
    isTechnicalError,
    issueReasons,
    lastSentAt,
    reasonTarget,
    reasonTargets,
    type ReasonTarget,
  } from "../invoiceStatus.ts";

  let detail = $state<InvoiceDetail | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let notFound = $state(false);
  // Prüfung des Entwurfs (nur lesend, V1-03): läuft beim Öffnen eines Entwurfs von
  // selbst und erscheint in der Karte „Ausstellen“; „Prüfen“ wiederholt sie.
  let issues = $state<ValidationIssueDto[] | null>(null);
  let checkError = $state("");
  let validating = $state(false);
  let issuing = $state(false);
  // Ablehnung beim Ausstellen: Gründe als stehendes Banner (ADR 0041 Nr. 22).
  let issueFailure = $state<string[] | null>(null);
  let issueBannerEl: HTMLDivElement | undefined = $state();
  // ADR 0042 Nr. 12: Leitweg-ID mit dem am Entwurf gespeicherten Wert vorbelegt.
  // Ein geänderter Wert gilt beim Ausstellen; leer gilt der gespeicherte, sonst „N/A“.
  let buyerReference = $state("");
  let issueTried = $state(false);
  // Leitweg-ID, die beim Ausstellen in die E-Rechnung geht (Feld vor gespeichertem Wert).
  let effectiveBuyerReference = $derived(
    normalizeBuyerReference(buyerReference) ?? normalizeBuyerReference(detail?.invoice.buyerReference),
  );
  let buyerReferenceNote = $derived.by(() => {
    const stored = normalizeBuyerReference(detail?.invoice.buyerReference);
    if (!stored) return "";
    return normalizeBuyerReference(buyerReference)
      ? "Am Entwurf gespeichert; eine Änderung hier gilt für diese E-Rechnung."
      : `Leer gilt die am Entwurf gespeicherte Leitweg-ID „${stored}“. Entfernen lässt sie sich über „Entwurf bearbeiten“.`;
  });
  let sendLog = $state<EmailLogEntry[]>([]);
  // Lauf F3b: Entwurf verwerfen (Grund ≥ 10 Zeichen, Re-Auth im Server).
  let showVoid = $state(false);
  let voidReason = $state("");
  let voiding = $state(false);
  // Feldfehler am Grund (statt Toast), erst nach dem ersten Versuch.
  let voidError = $state("");
  let voidReasonEl: HTMLTextAreaElement | undefined = $state();
  // Storno-Bezug: Original bzw. aufhebender Storno-Beleg (Nummer, Datum, Link).
  let relatedOriginal = $state<Invoice | null>(null);
  let relatedStorno = $state<Invoice | null>(null);
  // Warteliste (7): Konto je Zahlung (auch deaktivierte Konten).
  let accounts = $state<PaymentAccount[]>([]);

  let id = $derived(page.params.id ?? "");
  // Heute in Europe/Berlin, reaktiv über Mitternacht (V2-03: „Überfällig“ wie auf Start).
  const todayIso = $derived(todayBerlin(dayClock.now));

  // ADR 0042 Nr. 3 (V1-R06): Ein Entwurf zeigt den Steuerstatus seines Datums
  // (Leistungsdatum, sonst Ende des Leistungszeitraums, sonst Rechnungsdatum) wie
  // der Editor; der gespeicherte Wert ist bis zum Festschreiben nur vorläufig.
  // Festgeschriebene Belege zeigen den eingefrorenen Wert, Storno-Belege den des
  // Originals, verworfene Entwürfe den gespeicherten.
  const draftTaxDate = $derived.by(() => {
    const i = detail?.invoice;
    if (!i || i.lockedAt || i.voidedAt || i.isStornoFor) return "";
    return invoiceTaxDate(i, todayIso);
  });
  const draftTaxAt = taxStatusAtDate(() => draftTaxDate);
  let isKlein = $derived(
    draftTaxDate && draftTaxAt.date === draftTaxDate && draftTaxAt.isKlein !== null
      ? draftTaxAt.isKlein
      : detail?.invoice.isKleinunternehmer !== 0,
  );

  async function load() {
    loading = true;
    error = null;
    notFound = false;
    try {
      detail = await invoicesGet(id);
      if (!detail) {
        notFound = true;
        error = "Rechnung nicht gefunden.";
      } else {
        buyerReference = detail.invoice.buyerReference ?? "";
        issueTried = false;
        await loadRelated(detail);
      }
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
    if (detail && !detail.invoice.lockedAt && !detail.invoice.voidedAt && !detail.invoice.isStornoFor) await check();
  }

  // Neu lesen ohne Ladezustand (nach „Zahlung stornieren“): die Seite bleibt stehen,
  // nur Rechnung, Zahlungen und Summen ändern sich.
  async function refresh() {
    try {
      const d = await invoicesGet(id);
      if (d) {
        detail = d;
        await loadRelated(d);
      }
    } catch {
      // Der Stand bleibt; die Meldung der eigentlichen Aktion steht schon.
    }
  }

  // Nur zur Anzeige; fehlt der Bezug, bleibt der Hinweis ohne Nummer.
  async function loadRelated(d: InvoiceDetail) {
    relatedOriginal = null;
    relatedStorno = null;
    try {
      if (d.invoice.isStornoFor) relatedOriginal = (await invoicesGet(d.invoice.isStornoFor))?.invoice ?? null;
      if (d.invoice.canceledByStornoId) relatedStorno = (await invoicesGet(d.invoice.canceledByStornoId))?.invoice ?? null;
      // ADR 0042 Nr. 8: auch nach einer vollen Gegenbuchung (bezahlt 0 €) zeigt die
      // Historie Konten — also laden, sobald es Buchungen gibt.
      if (d.invoice.paidAmountCents > 0 || (d.payments ?? []).length > 0) accounts = await paymentAccountsList(true);
    } catch {
      // Bezug ist Zusatzinformation — Fehler hier nicht hochziehen.
    }
  }

  onMount(load);

  /** Prüft den Entwurf (nur lesend); Ergebnis und Fehler stehen in der Karte „Ausstellen“. */
  async function check() {
    if (!detail || validating) return;
    validating = true;
    checkError = "";
    try {
      issues = await invoicesValidateDraft(detail.invoice.id);
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

  // Plan W7: das archivierte PDF kommt hash-geprüft über `/files/archive/{id}`.
  function openPdf() {
    const id = detail?.invoice.pdfArchiveId;
    if (!detail || !id) return;
    openPdfPreview(archiveUrl(id), docTitle(detail.invoice), "");
  }

  function downloadPdf() {
    const id = detail?.invoice.pdfArchiveId;
    if (id) saveArchiveFile(id);
  }

  async function lockAndIssue() {
    if (!detail || issuing) return;
    // Meldung am Feld Leitweg-ID statt Rückfrage (ADR 0041, Fehler am Feld).
    issueTried = true;
    if (buyerReferenceError(buyerReference)) {
      await tick();
      document.getElementById("buyer-ref")?.focus();
      return;
    }
    const ref = effectiveBuyerReference;
    if (
      !(await confirmDialog({
        title: "Rechnung jetzt ausstellen?",
        bullets: [
          "Es wird eine fertige PDF- und E-Rechnung erzeugt und gespeichert.",
          "Danach lässt sich die Rechnung nicht mehr ändern (gesetzlich vorgeschrieben).",
          "Korrigieren geht dann nur noch über einen Storno-Beleg.",
          ...(ref ? [`Leitweg-ID in der E-Rechnung: ${ref}`] : []),
        ],
        confirmLabel: "Ausstellen",
      }))
    )
      return;
    issuing = true;
    issueFailure = null;
    try {
      // ADR 0042 Nr. 12: ohne Wert gilt die am Entwurf gespeicherte Leitweg-ID.
      await invoicesLockAndIssue(detail.invoice.id, normalizeBuyerReference(buyerReference));
      flash(`Rechnung ${detail.invoice.invoiceNumber} ausgestellt.`);
      await load();
    } catch (e) {
      const msg = String(e);
      if (isTechnicalError(msg)) {
        flash(msg, "error");
      } else {
        // V1-03: Gründe als Banner in der Karte „Ausstellen“, Fokus dorthin.
        issueFailure = issueReasons(msg) ?? [msg];
        await tick();
        issueBannerEl?.focus();
      }
    } finally {
      issuing = false;
    }
  }

  // Bereich „Entwurf verwerfen“ öffnen (aus dem Menü „Mehr“): Fokus auf den Grund.
  async function openVoid() {
    showVoid = true;
    await tick();
    voidReasonEl?.focus();
  }

  // Verwerfen eines nummerierten Entwurfs (W14-032): kein Löschen, die Nummer
  // bleibt verbraucht; der Server verlangt Daten-Passwort und App-Code
  // (Re-Auth-Dialog über transport.ts).
  async function voidDraft() {
    if (!detail || voiding) return;
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

  function docTitle(inv: Invoice): string {
    return `${docKind(inv)} ${inv.invoiceNumber}`;
  }

  type Tone = "neutral" | "primary" | "info" | "success" | "warning" | "danger";

  function valLabel(s: string | null): string {
    switch (s) {
      case "passed":  return "In Ordnung";
      case "warning": return "Mit Hinweisen";
      case "failed":  return "Fehler";
      default:        return "—";
    }
  }
  function valTone(s: string | null): Tone {
    return s === "passed" ? "success" : s === "warning" ? "warning" : s === "failed" ? "danger" : "neutral";
  }

  // Wo sich ein Grund beheben lässt (V1-03): Firmendaten, Kontakt oder Entwurf.
  function targetLink(t: ReasonTarget, inv: Invoice): { href: string; label: string } | null {
    // U6-005: nach dem Speichern der Firmendaten zurück zu dieser Rechnung.
    if (t === "seller") return { href: `/settings/seller?next=/invoices/${inv.id}`, label: "Firmendaten ergänzen" };
    if (t === "buyer") return { href: `/contacts/${inv.contactId}`, label: "Kontakt ergänzen" };
    if (!inv.isStornoFor) return { href: `/invoices/${inv.id}/edit`, label: "Entwurf bearbeiten" };
    return null;
  }

  // ---- Versand: erfolgreicher Versand laut Rechnung oder E-Mail-Protokoll ----
  let sentAtIso = $derived(detail?.invoice.sentAt ?? lastSentAt(sendLog));

  // „+ Zahlung“ nur, solange etwas offen ist (bezahlt/storniert/Storno-Beleg: nein).
  let canPay = $derived.by(() => {
    const inv = detail?.invoice;
    return !!inv && !!inv.lockedAt && inv.status !== "canceled" && inv.status !== "paid" && !inv.isStornoFor;
  });
  // V1-07: ausgestellt, aber noch nicht versendet → „Senden“ ist der nächste Schritt
  // (auch beim Storno-Beleg); danach „+ Zahlung“, solange etwas offen ist.
  let canSend = $derived(!!detail?.invoice.lockedAt && !!detail?.invoice.pdfArchiveId && detail?.invoice.status !== "canceled");
  let sendIsMain = $derived(canSend && !sentAtIso);

  // ---- PageBar (U1-059, ADR 0041 Nr. 96): Hauptaktion = nächster Schritt;
  // Nebenaktionen und Gefahraktionen (abgesetzt) im Menü „Mehr“ (design.md §4.2). ----
  type Bar = { secondary: ActionMenuItem[]; more: ActionMenuItem[] };
  let bar = $derived.by((): Bar => {
    const inv = detail?.invoice;
    if (!inv || inv.voidedAt) return { secondary: [], more: [] };
    if (!inv.lockedAt) {
      const secondary: ActionMenuItem[] = [];
      // U7 (R1-04): Storno-Entwurf ohne „Bearbeiten“/„Prüfen“ — Ergebnis stünde in der
      // Karte „Ausstellen“, die es für ihn nicht gibt.
      if (!inv.isStornoFor) {
        secondary.push({ label: "Bearbeiten", href: `/invoices/${inv.id}/edit` });
        // Während der Prüfung nicht gesperrt (Fokus bleibt, ADR 0041 Nr. 33); check() wartet.
        secondary.push({ label: validating ? "Prüfe …" : "Prüfen", onSelect: runValidation });
      }
      return {
        secondary,
        more: [{ label: "Entwurf verwerfen", onSelect: openVoid, danger: true, disabled: voiding || issuing }],
      };
    }
    const secondary: ActionMenuItem[] = [];
    const more: ActionMenuItem[] = [];
    if (sendIsMain && canPay) secondary.push({ label: "+ Zahlung", href: `/invoices/${inv.id}/payment` });
    if (inv.pdfArchiveId) {
      secondary.push({ label: "PDF öffnen", onSelect: openPdf });
      if (canSend && !sendIsMain) secondary.push({ label: "Senden", href: `/invoices/${inv.id}/send` });
      more.push({ label: "PDF herunterladen", onSelect: downloadPdf });
    }
    if (inv.status !== "canceled" && !inv.isStornoFor) {
      more.push({ label: "Stornieren", href: `/invoices/${inv.id}/cancel`, danger: true });
    }
    return { secondary, more };
  });

  // ---- Zahlungen (Warteliste (7); Zahlungsbuch, ADR 0042 Nr. 7–9) ----
  let payments = $derived(paymentLines(detail?.payments ?? []));
  let paymentsHeadingEl: HTMLHeadingElement | undefined = $state();

  // „Zahlung stornieren …“ (ADR 0042 Nr. 8): Gefahr-Dialog mit Grund, Re-Auth im
  // Server. Der Dialog hält eine Kopie der Zahlung, damit ein Neuladen ihn nicht leert.
  let showReverse = $state(false);
  let reverseTarget = $state<InvoicePayment | null>(null);
  let reverseReason = $state("");
  let reverseTried = $state(false);
  let reverseFieldError = $state("");
  let reverseServerError = $state<string | null>(null);
  let reversing = $state(false);
  const reverseReasonShown = $derived(
    reverseFieldError || (reverseTried ? (reverseReasonError(reverseReason) ?? "") : ""),
  );
  const reverseYear = $derived(reverseTarget ? reverseTarget.paidDate.slice(0, 4) : "");

  $effect(() => {
    if (showReverse) {
      pushModal();
      return () => popModal();
    }
  });

  function paymentLabel(p: InvoicePayment): string {
    return `Zahlung vom ${date(p.paidDate)} über ${euro(p.amountCents)}`;
  }

  function reverseItems(p: InvoicePayment): ActionMenuItem[] {
    return [
      {
        label: "Zahlung stornieren …",
        onSelect: () => openReverse(p),
        danger: true,
        disabled: !p.reversible || reversing,
        hint: p.reversible ? undefined : (p.notReversibleReason ?? undefined),
      },
    ];
  }

  function openReverse(p: InvoicePayment) {
    if (!p.reversible) return;
    reverseTarget = { ...p };
    reverseReason = "";
    reverseTried = false;
    reverseFieldError = "";
    reverseServerError = null;
    showReverse = true;
  }

  function closeReverse() {
    showReverse = false;
  }

  async function confirmReverse() {
    if (reversing || !detail || !reverseTarget) return;
    reverseTried = true;
    reverseFieldError = "";
    if (reverseReasonError(reverseReason)) {
      document.getElementById("rp-reason")?.focus();
      return;
    }
    const target = reverseTarget;
    const invoiceId = detail.invoice.id;
    reversing = true;
    // Wie beim Widerruf des Steuerstatus: Dialog zu, solange der Re-Auth-Dialog fragt;
    // bei einer Ablehnung öffnet er wieder mit Banner bzw. Fehler am Feld.
    showReverse = false;
    try {
      await invoicesReversePayment({ invoiceId, paymentId: target.id, reason: cleanReverseReason(reverseReason) });
      await refresh();
      flash(`${paymentLabel(target)} storniert. Ist Geld eingegangen, erfasse die Zahlung jetzt richtig neu.`, "ok", {
        action: {
          label: "Zahlung neu erfassen",
          run: () => void goto(`/invoices/${invoiceId}/payment?ersetzt=${encodeURIComponent(target.id)}`),
        },
      });
      // Die Zeilenaktion ist mit der Gegenbuchung weg — Fokus nie auf <body> (ADR 0041 Nr. 33).
      await tick();
      if (focusLost()) paymentsHeadingEl?.focus();
    } catch (e) {
      if (e === REAUTH_CANCELED) {
        flash(String(e), "info");
      } else {
        const msg = String(e);
        if (isReasonRejection(msg)) reverseFieldError = msg;
        else reverseServerError = msg;
        // Stand neu lesen (z. B. Jahr inzwischen abgeschlossen); der Dialog behält seine Kopie.
        await refresh();
        showReverse = true;
        await tick();
        if (reverseFieldError) document.getElementById("rp-reason")?.focus();
      }
    } finally {
      reversing = false;
    }
  }

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

  // V4-08: Überschrift auch beim Laden und bei „nicht gefunden“.
  let pageTitle = $derived(detail ? docTitle(detail.invoice) : notFound ? "Rechnung nicht gefunden" : "Rechnung");
  // Prüfergebnis als Statuszeile (4.1.3: „Prüfen“ aus der PageBar sagt das Ergebnis an).
  let checkText = $derived(
    validating
      ? "Wird geprüft …"
      : issues === null
        ? ""
        : issues.length === 0
          ? "Alle Pflichtangaben sind vorhanden — die Rechnung kann ausgestellt werden."
          : issues.length === 1
            ? "Vor dem Ausstellen fehlt noch eine Angabe:"
            : `Vor dem Ausstellen fehlen noch ${issues.length} Angaben:`,
  );
</script>

<PageBar
  back="/invoices"
  backLabel="Rechnungen"
  title={pageTitle}
  secondary={bar.secondary}
  more={bar.more}
>
  {#snippet actions()}
    {#if detail && !detail.invoice.voidedAt}
      {#if !detail.invoice.lockedAt && detail.invoice.isStornoFor}
        <!-- U7 (R1-04): Ein Storno-Entwurf wird über „Stornieren“ der Original-Rechnung
             abgeschlossen (der Server lehnt „Ausstellen“ hier ab). -->
        <Button href={`/invoices/${detail.invoice.isStornoFor}/cancel`}>Weiter zum Stornieren</Button>
      {:else if !detail.invoice.lockedAt}
        <Button onclick={lockAndIssue} ariaDisabled={issuing}>
          {issuing ? "Stelle aus …" : "Rechnung ausstellen"}
        </Button>
      {:else if sendIsMain}
        <Button href={`/invoices/${detail.invoice.id}/send`}>Senden</Button>
      {:else if canPay}
        <Button href={`/invoices/${detail.invoice.id}/payment`}>+ Zahlung</Button>
      {/if}
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error && !detail}
  <Banner title={notFound ? undefined : "Die Rechnung lässt sich nicht laden."}>{error}</Banner>
{:else if detail}
  {@const inv = detail.invoice}
  {@const st = invoiceStatusView(inv, todayIso)}
  <!-- Metazeile: Trennpunkt am Ende eines Teils, nie am Zeilenanfang (U1-065). -->
  <p class="sub">
    <span class="meta">{date(inv.invoiceDate)}<span class="sep" aria-hidden="true"> ·</span></span>
    <span class="meta">
      <Badge tone={st.tone} strike={st.strike}>{st.label}</Badge>{#if inv.lockedAt && sentAtIso}<span class="sep" aria-hidden="true"> ·</span>{/if}
    </span>
    {#if inv.lockedAt && sentAtIso}
      <span class="meta">versendet am {date(sentAtIso)}</span>
    {/if}
  </p>

  {#if inv.isStornoFor}
    <Banner kind="info">
      Dieser Storno-Beleg storniert
      {#if relatedOriginal}
        die Rechnung <DocNo value={relatedOriginal.invoiceNumber} href={`/invoices/${inv.isStornoFor}`} inline />
        vom {date(relatedOriginal.invoiceDate)}.
      {:else}
        <a class="kb-link" href={`/invoices/${inv.isStornoFor}`}>die ursprüngliche Rechnung</a>.
      {/if}
      {#if inv.lockedAt && !sentAtIso}{" "}Schick ihn dem Kunden mit „Senden“.{/if}
      {#if !inv.lockedAt && !inv.voidedAt}
        {" "}Er ist noch ein Entwurf und wird über „Stornieren“ der Original-Rechnung abgeschlossen:
        <a class="kb-link kb-target" href={`/invoices/${inv.isStornoFor}/cancel`}>Weiter zum Stornieren</a>.
      {/if}
    </Banner>
  {/if}
  {#if inv.canceledByStornoId}
    <Banner kind="warning">
      Diese Rechnung ist aufgehoben durch
      {#if relatedStorno}
        den Storno-Beleg <DocNo value={relatedStorno.invoiceNumber} href={`/invoices/${inv.canceledByStornoId}`} inline />
        vom {date(relatedStorno.invoiceDate)}.
      {:else}
        <a class="kb-link" href={`/invoices/${inv.canceledByStornoId}`}>einen Storno-Beleg</a>.
      {/if}
    </Banner>
  {/if}
  {#if inv.lockedAt && inv.paymentNote && canPay}
    <!-- V1-08: Der Bezahlt-Hinweis ist nur Text auf dem Beleg; die EÜR zählt gebuchte Zahlungen. -->
    <Banner kind="info" title="Bezahlt-Hinweis ohne Buchung.">
      Auf dem Beleg steht „{inv.paymentNote}“, gebucht ist noch nichts. Erfasse die Zahlung über
      <a class="kb-link" href={`/invoices/${inv.id}/payment`}>+ Zahlung</a>, damit die Rechnung als bezahlt
      gilt und in der EÜR zählt.
    </Banner>
  {/if}
  {#if !inv.lockedAt && !inv.voidedAt}
    <!-- Warteliste (8), ADR 0042 Nr. 16: aus dem Lesefeld des Entwurfs (keine Listenabfrage). -->
    <RestoreConflictBanner
      conflict={detail.restoreConflict}
      number={inv.invoiceNumber}
      doc="invoice"
      where="detail"
      editHref={inv.isStornoFor ? undefined : `/invoices/${inv.id}/edit`}
    />
  {/if}

  {#if inv.voidedAt}
    <section class="card">
      <h2>Verworfen</h2>
      <p>
        Am {dateTime(inv.voidedAt)} verworfen. Der Entwurf wurde nie ausgestellt; die Nummer
        <DocNo value={inv.invoiceNumber} inline /> bleibt vergeben und wird nicht erneut benutzt.
      </p>
      {#if inv.voidReason}<p><strong>Grund:</strong> {inv.voidReason}</p>{/if}
    </section>
  {/if}

  {#if showVoid && !inv.lockedAt && !inv.voidedAt}
    <section class="card danger">
      <h2>Entwurf verwerfen</h2>
      <p class="muted">
        Der Entwurf wird nicht gelöscht: Zeitpunkt und Grund stehen daran und im Protokoll, die
        Nummer <DocNo value={inv.invoiceNumber} inline /> bleibt vergeben. Danach lässt er sich weder
        ändern noch ausstellen. Klein.Buch fragt zur Bestätigung Daten-Passwort und App-Code ab.
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
      <div class="kb-actions void-actions">
        <Button variant="secondary" onclick={() => { showVoid = false; voidError = ""; }} ariaDisabled={voiding}>Abbrechen</Button>
        <Button variant="danger" onclick={voidDraft} ariaDisabled={voiding}>
          {voiding ? "Verwerfe …" : "Entwurf verwerfen"}
        </Button>
      </div>
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
      {#if isKlein}
        <p class="klein-note">Kleinunternehmer (§19) — es wird keine Umsatzsteuer ausgewiesen.</p>
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
        <!-- ADR 0042 Nr. 10/12: Leistungsdatum oder -zeitraum, Leitweg-ID (BT-10). -->
        <dt>{serviceSummary(inv).label}</dt><dd>{serviceSummary(inv).text}</dd>
        <dt>Fällig</dt><dd>{date(inv.dueDate)}</dd>
        {#if inv.buyerReference}
          <dt>Leitweg-ID</dt><dd>{inv.buyerReference}</dd>
        {/if}
        <dt>PDF-Vorlage</dt><dd>{pdfTemplateLabel(inv.pdfTemplate)}</dd>
        {#if inv.paymentNote}
          <!-- V1-08: Bezahlt-Hinweis (reiner Text auf dem Beleg, keine Buchung). -->
          <dt>Bezahlt-Hinweis</dt><dd>{inv.paymentNote}</dd>
        {/if}
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
    <div class="items-box" bind:this={itemsBox}>
      <table class="items-table" class:cards={itemsAsCards}>
        <caption class="kb-visually-hidden">Positionen der {docTitle(inv)}</caption>
        <thead>
          <tr>
            <th>#</th>
            <th>Beschreibung</th>
            <th class="right">Menge</th>
            <th>Einheit</th>
            <th class="right">Einzelpreis</th>
            {#if !isKlein}
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
              <td class="cards-full desc" data-label="Beschreibung">{it.description}</td>
              <td class="right" data-label="Menge">{formatQuantity(it.quantity)}</td>
              <td data-label="Einheit">{unitLabel(it.unitCode)}</td>
              <td class="right" data-label="Einzelpreis">{euro(it.unitPriceCents)}</td>
              {#if !isKlein}
                <td class="right" data-label="USt %">{percent(it.taxRatePercent)}</td>
                <td data-label="USt-Art">{it.taxCategoryCode}</td>
              {/if}
              <td class="right" data-label="Netto">{euro(it.netAmountCents)}</td>
            </tr>
          {/each}
        </tbody>
        <tfoot>
          <tr><td colspan={isKlein ? 5 : 7} class="right sum-label">Netto</td>
              <td class="right" data-label="Netto">{euro(inv.netAmountCents)}</td></tr>
          {#if inv.taxAmountCents !== 0}
            <tr><td colspan={isKlein ? 5 : 7} class="right sum-label">USt</td>
                <td class="right" data-label="USt">{euro(inv.taxAmountCents)}</td></tr>
          {/if}
          <tr class="kb-sum"><td colspan={isKlein ? 5 : 7} class="right sum-label">
                <strong>Brutto</strong></td>
              <td class="right" data-label="Brutto"><strong>{euro(inv.grossAmountCents)}</strong></td></tr>
          {#if inv.paidAmountCents > 0 || payments.length > 0}
            <tr><td colspan={isKlein ? 5 : 7} class="right sum-label">Bezahlt</td>
                <td class="right" data-label="Bezahlt">{euro(inv.paidAmountCents)}</td></tr>
            <tr><td colspan={isKlein ? 5 : 7} class="right sum-label">Offen</td>
                <td class="right" data-label="Offen">{euro(inv.grossAmountCents - inv.paidAmountCents)}</td></tr>
          {/if}
        </tfoot>
      </table>
    </div>
  </section>

  {#if !inv.lockedAt && !inv.voidedAt && !inv.isStornoFor}
    <!-- V1-06/V1-03: Karte „Ausstellen“ unter den Positionen — Prüfung, Leitweg-ID (BT-10,
         vom Entwurf vorbelegt, ADR 0042 Nr. 12) und die Hauptaktion am Ende. -->
    <section class="card issue-card" aria-labelledby="issue-h">
      <h2 id="issue-h">Ausstellen</h2>
      <p class="check-line" class:ok={!issueFailure && issues !== null && issues.length === 0} role="status">{issueFailure ? "" : checkText}</p>
      {#if issueFailure}
        <!-- Ablehnung bleibt stehen, bis erneut geprüft oder ausgestellt wird (ADR 0041 Nr. 22). -->
        {@const targets = reasonTargets(issueFailure.map((text) => ({ text })))}
        <div class="issue-err" tabindex="-1" bind:this={issueBannerEl}>
          <Banner kind="error" title="Die Rechnung wurde nicht ausgestellt.">
            {#if issueFailure.length === 1}
              {issueFailure[0]}
            {:else}
              <ul class="reasons">
                {#each issueFailure as r (r)}<li>{r}</li>{/each}
              </ul>
            {/if}
            {#each targets as t (t)}
              {@const link = targetLink(t, inv)}
              {#if link}<span class="fix-link"><a class="kb-link kb-target" href={link.href}>{link.label}</a></span>{/if}
            {/each}
          </Banner>
        </div>
      {:else}
        {#if checkError}
          <p class="err">Die Prüfung ist fehlgeschlagen: {checkError}</p>
        {:else if issues && issues.length > 0}
          <ul class="issues">
            {#each issues as iss (iss.code + iss.message)}
              {@const link = targetLink(reasonTarget(iss.message, iss.code), inv)}
              <li>
                {iss.message}
                {#if link}{" "}<a class="kb-link kb-target" href={link.href}>{link.label}</a>{/if}
              </li>
            {/each}
          </ul>
        {/if}
      {/if}
      <!-- ADR 0042 Nr. 12: am Entwurf gespeicherte Leitweg-ID vorbelegt; ein geänderter Wert gilt für diese E-Rechnung. -->
      <div class="ref-field">
        <BuyerReferenceField bind:value={buyerReference} id="buyer-ref" showErrors={issueTried} note={buyerReferenceNote} />
      </div>
      <!-- U1/V1-06: Hauptaktion am Ende (Handy, Daumen; PageBar klebt dort nicht). -->
      <div class="kb-form-end">
        <Button onclick={lockAndIssue} ariaDisabled={issuing}>
          {issuing ? "Stelle aus …" : "Rechnung ausstellen"}
        </Button>
      </div>
    </section>
  {/if}

  {#if inv.paidAmountCents > 0 || payments.length > 0}
    <!-- ADR 0042 Nr. 7–9: Zahlungsbuch. Die Karte bleibt, solange es Buchungen gibt —
         auch nach einer vollen Gegenbuchung (bezahlt 0 €); nichts verschwindet. -->
    <section class="card" aria-labelledby="pay-h">
      <h2 id="pay-h" tabindex="-1" bind:this={paymentsHeadingEl}>Zahlungen</h2>
      {#if payments.length === 0}
        <!-- Altbestand ohne Buchung im Zahlungsbuch (vor 0034 nicht vorgesehen): nur Summe. -->
        <p>
          {euro(inv.paidAmountCents)} am {date(inv.paidAt)}, Konto: {accountText(inv.paidToAccountId, accounts)}.
        </p>
      {:else}
        <table class="cards pay-table">
          <caption class="kb-visually-hidden">Zahlungen zur {docTitle(inv)}</caption>
          <thead>
            <tr>
              <th>Zahldatum</th>
              <th>Buchung</th>
              <th class="right">Betrag</th>
              <th>Konto</th>
              <th>Notiz bzw. Grund</th>
              <th><span class="kb-visually-hidden">Aktion</span></th>
            </tr>
          </thead>
          <tbody>
            {#each payments as line (line.entry.id)}
              {@const p = line.entry}
              <tr class:reversed={!!line.reversedBy} class:reversal={p.kind === "reversal"}>
                <td data-label="Zahldatum" class="kb-nowrap">{date(p.paidDate)}</td>
                <td data-label="Buchung">
                  {#if p.kind === "reversal"}
                    <!-- Gegenbuchung: als solche benannt, mit Verweis — nicht nur über Farbe. -->
                    <Badge kind="tag">Gegenbuchung</Badge>
                    <small class="pay-sub">
                      Storno der Zahlung vom {date(line.reverses?.paidDate ?? p.paidDate)}{#if line.reverses}{" "}über {euro(line.reverses.amountCents)}{/if},
                      erfasst am {dateTimeShort(p.createdAt)}
                    </small>
                  {:else}
                    Zahlung
                    {#if line.reversedBy}
                      <Badge strike>Storniert</Badge>
                      <small class="pay-sub">durch Gegenbuchung vom {dateTimeShort(line.reversedBy.createdAt)}</small>
                    {/if}
                  {/if}
                </td>
                <td class="right kb-nowrap" data-label="Betrag">
                  {#if line.reversedBy}<s>{euro(p.amountCents)}</s>{:else}{euro(p.amountCents)}{/if}
                </td>
                <td data-label="Konto">{accountText(paymentAccountId(p, inv), accounts)}</td>
                <td data-label={p.kind === "reversal" ? "Grund" : "Notiz"} class="pay-text">
                  {#if p.kind === "reversal"}<span class="kb-visually-hidden">Grund: </span>{p.reason ?? "—"}{:else}{p.note ?? "—"}{/if}
                </td>
                <td data-label="Aktion">
                  {#if p.kind === "payment" && !line.reversedBy && inv.lockedAt}
                    <ActionMenu ariaLabel={`Mehr zur ${paymentLabel(p)}`} items={reverseItems(p)} />
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
        <p class="muted hist-note">
          Gebuchte Zahlungen werden nie geändert oder gelöscht. Eine falsch erfasste Zahlung stornierst du
          über „Mehr“ → „Zahlung stornieren …“ und erfasst sie danach richtig neu.
          <HelpAnchor
            slug="zahlung-erfassen"
            heading="Falsch erfasste Zahlung stornieren"
            label="Falsch erfasste Zahlung stornieren"
          />
        </p>
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
              <td data-label="Status">{#if e.status === "success"}<Badge tone="info">Versendet</Badge>{:else}<Badge tone="danger">Fehlgeschlagen</Badge>{/if}</td>
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

{#if showReverse && reverseTarget && detail}
  {@const rp = reverseTarget}
  <!-- Gefahr-Dialog „Zahlung stornieren“ (ADR 0042 Nr. 8, ADR 0041 Nr. 11): kein Schließen
       über den Hintergrund, Startfokus „Abbrechen“, Escape bricht ab. -->
  <div class="kb-backdrop" use:portal>
    <div
      class="kb-dialog rp-dialog"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="rp-title"
      aria-describedby="rp-lead"
      use:focusTrap={{ onEscape: closeReverse }}
    >
      <header class="kb-dialog-head">
        <h2 id="rp-title">Zahlung stornieren?</h2>
      </header>
      <div class="kb-dialog-body">
        <p id="rp-lead">
          Klein.Buch bucht eine Gegenbuchung über {euro(-rp.amountCents)} mit Datum und Konto dieser
          Zahlung. Sie wirkt am ursprünglichen Zahldatum, die EÜR {reverseYear} ändert sich also. Die
          Zahlung bleibt sichtbar und gilt als storniert; ist Geld eingegangen, erfasse danach die richtige Zahlung.
        </p>
        <dl class="rp-facts">
          <dt>Rechnung</dt><dd><DocNo value={detail.invoice.invoiceNumber} inline /></dd>
          <dt>Betrag</dt><dd class="kb-nowrap">{euro(rp.amountCents)}</dd>
          <dt>Zahldatum</dt><dd>{date(rp.paidDate)}</dd>
          <dt>Konto</dt><dd>{accountText(paymentAccountId(rp, detail.invoice), accounts)}</dd>
          {#if rp.note}<dt>Notiz</dt><dd>{rp.note}</dd>{/if}
        </dl>

        {#if reverseServerError}
          <Banner title="Nicht storniert.">{reverseServerError}</Banner>
        {/if}

        <p class="rp-note">
          Nur für Fehlbuchungen: Das Geld ist nicht, mit anderem Betrag, an einem anderen Tag oder auf
          einem anderen Konto eingegangen. Hast du dem Kunden Geld zurückgezahlt, storniere stattdessen
          die Rechnung; die Erstattung zählt am Tag der Rückzahlung.
        </p>

        <div class="rp-field">
          <label for="rp-reason">Grund<span class="req" aria-hidden="true">*</span><span class="kb-visually-hidden">(Pflicht)</span></label>
          <textarea
            id="rp-reason"
            class="kb-input"
            rows="3"
            maxlength={REVERSE_REASON_MAX_CHARS}
            bind:value={reverseReason}
            oninput={() => {
              reverseFieldError = "";
            }}
            required
            aria-required="true"
            aria-invalid={reverseReasonShown ? "true" : undefined}
            aria-describedby={reverseReasonShown ? "rp-reason-err rp-reason-hint" : "rp-reason-hint"}
          ></textarea>
          {#if reverseReasonShown}<span id="rp-reason-err" class="field-err">{reverseReasonShown}</span>{/if}
          <small id="rp-reason-hint" class="field-hint">
            Mindestens 10 Zeichen, z. B. „Betrag falsch erfasst, laut Kontoauszug 450,00 €“. Ohne Namen von Personen.
          </small>
        </div>
        <p class="rp-auth">Klein.Buch fragt zur Bestätigung Daten-Passwort und App-Code ab.</p>
      </div>
      <footer class="kb-dialog-foot">
        <Button variant="secondary" initialFocus onclick={closeReverse}>Abbrechen</Button>
        <Button variant="danger" onclick={confirmReverse} ariaDisabled={reversing}>Zahlung stornieren</Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  .sub { color: var(--text-muted); margin: 0 0 1rem; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  /* Teile der Metazeile brechen nur zwischen sich um; der Trennpunkt steht am Ende. */
  .meta { display: inline-flex; align-items: center; white-space: nowrap; }
  /* V3-14: Das Leerzeichen vor dem Trennpunkt bleibt auch am Anfang eines Flex-Elements. */
  .meta > .sep { white-space: pre; }
  .ref-field { margin-top: var(--sp-3); }
  .check-line { margin: 0 0 var(--sp-2); }
  /* Leer bleibt die Zeile im Accessibility-Tree (Live-Region), nur ohne Abstand. */
  .check-line:empty { margin: 0; }
  .check-line.ok { color: var(--status-success-fg); }
  .issue-err { outline: none; }
  .issue-err:focus-visible { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); border-radius: var(--radius-control); }
  .reasons { margin: var(--sp-1) 0 0; padding-left: 1.25rem; }
  .fix-link { display: inline-block; margin: var(--sp-2) var(--sp-3) 0 0; }
  .cols { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 0.75rem; }
  /* .card entfernt — globale Definition aus tokens.css. */
  /* Labelspalte so breit wie das längste Label, nie getrennt (U3-037). */
  dl { display: grid; grid-template-columns: max-content minmax(0, 1fr); gap: 0.25rem 0.75rem; }
  dt { color: var(--text-muted); font-size: var(--fs-sm); white-space: nowrap; }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 0.4rem; text-align: left; border-bottom: 1px solid var(--line); }
  th { background: var(--surface-sunken); font-weight: 600; font-size: var(--fs-sm); }
  .right { text-align: right; font-variant-numeric: tabular-nums; }
  /* Beträge und Mengen in der Tabelle nicht umbrechen; die Beschreibung bricht. */
  .items-table td.right { white-space: nowrap; }
  .items-table:not(.cards) td.desc { overflow-wrap: break-word; hyphens: auto; }
  .klein-note { color: var(--status-info-fg); font-size: var(--fs-sm); }
  .issues { padding-left: 1.25rem; margin: 0 0 var(--sp-2); }
  .issues li { color: var(--status-danger-fg); margin-bottom: 0.25rem; }
  .muted { color: var(--text-muted); }
  .err { color: var(--status-danger-fg); }
  .hist-note { font-size: var(--fs-sm); margin-top: 0.4rem; }
  .req { color: var(--status-danger-fg); }
  .field-err { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--status-danger-fg); }
  .field-hint { margin: 4px 0 0; font-size: var(--fs-xs); color: var(--text-muted); }
  .void-reason { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); margin-bottom: 4px; }
  .void-text {
    font: inherit; width: 100%; box-sizing: border-box; resize: vertical;
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    background: var(--surface-card);
  }
  .void-actions { margin-top: 0.5rem; }
  /* Zahlungsbuch (ADR 0042 Nr. 7–9): Storniert/Gegenbuchung stehen als Text (Badge,
     Unterzeile), die gedämpfte Farbe ist nur Zusatz (1.4.1). */
  .pay-sub { display: block; color: var(--text-muted); font-size: var(--fs-sm); }
  .pay-table tr.reversed td, .pay-table tr.reversal td { color: var(--text-muted); }
  .pay-table td.pay-text { overflow-wrap: anywhere; }
  #pay-h { outline: none; }
  #pay-h:focus-visible { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); }
  .rp-dialog { --dialog-w: 36rem; }
  .rp-facts { margin: 0.75rem 0; }
  .rp-note { font-size: var(--fs-sm); margin: 0.75rem 0; }
  .rp-auth { font-size: var(--fs-sm); color: var(--text-muted); margin: 0.5rem 0 0; }
  .rp-field { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .rp-field > label { font-size: var(--fs-sm); font-weight: 600; }
  .rp-field textarea { width: 100%; box-sizing: border-box; min-height: var(--target); font: inherit; resize: vertical; }
  /* Kartenansicht (nur bis 820 px, tokens.css): Summenzeilen zeigen das Label per
     data-label an der Betragszelle, die colspan-Beschriftung entfällt; lange Werte
     brechen um. */
  @media screen and (max-width: 820px) {
    table.cards td.sum-label { display: none; }
    table.cards td { overflow-wrap: anywhere; }
  }
</style>
