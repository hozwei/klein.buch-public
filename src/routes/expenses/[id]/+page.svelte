<script lang="ts">
  import { todayBerlin } from "#lib/date.ts";
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import {
    expensesGet,
    expensesCancel,
    expensesSetPayment,
    expensesReceiptXmlText,
    paymentAccountsList,
    saveArchiveFile,
  } from "#lib/api.ts";
  import type { ExpenseDetail, AttachmentView, PaymentAccount } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import { expenseCategoryLabel } from "#lib/labels.ts";
  import AttachmentUpload from "#lib/AttachmentUpload.svelte";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { openXmlViewer } from "#lib/xmlViewerModal.svelte.ts";

  let detail = $state<ExpenseDetail | null>(null);
  let accounts = $state<PaymentAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let showCancel = $state(false);
  let cancelReason = $state("");
  let cancelError = $state("");
  let cancelReasonEl: HTMLInputElement | undefined = $state();
  let showPay = $state(false);
  let payDate = $state(todayBerlin());
  let payAccountId = $state<string>("");

  // PV1-A5: Roh-XML-Viewer wird global im Root-Layout gemountet
  // (siehe `XmlViewerDialog.svelte`). Lokal nur Lade-Flag, damit der Button
  // nicht doppelt klickbar ist.
  let xmlLoading = $state(false);

  let id = $derived(page.params.id ?? "");

  /** Beleg ist eine empfangene E-Rechnung mit archiviertem Original — Voraussetzung
   *  für „Roh-XML anzeigen". Coarse-Check über `sourceFormat`; die feine
   *  CII/UBL-Unterscheidung treffen wir erst im Viewer-Command. */
  let canShowRawXml = $derived(
    !!detail?.expense.receiptArchiveId &&
      (detail?.sourceFormat === "zugferd" ||
        detail?.sourceFormat === "xrechnung-cii" ||
        detail?.sourceFormat === "xrechnung-ubl"),
  );

  async function openRawXml(): Promise<void> {
    if (xmlLoading) return;
    xmlLoading = true;
    try {
      const payload = await expensesReceiptXmlText(id);
      if (!payload) {
        flash("Kein E-Rechnungs-Original im Archiv hinterlegt.", "error");
        return;
      }
      openXmlViewer(payload);
    } catch (e) {
      // Tamper-Detection schlägt hier durch — Domain-Error mit Hash-Mismatch.
      const msg = e instanceof Error ? e.message : String(e);
      const isTamper = msg.toLowerCase().includes("tamper");
      flash(
        isTamper
          ? "Beleg manipuliert: Hash stimmt nicht. Audit-Log gesetzt."
          : `Roh-XML konnte nicht gelesen werden: ${msg}`,
        "error",
      );
    } finally {
      xmlLoading = false;
    }
  }

  function accountLabel(accId: string | null): string {
    if (!accId) return "—";
    return accounts.find((a) => a.id === accId)?.label ?? "(unbekannt)";
  }

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await expensesGet(id);
      if (!detail) error = "Kosten nicht gefunden.";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    try {
      accounts = await paymentAccountsList(true);
    } catch {
      /* Konten optional für die Anzeige */
    }
    await load();
  });

  function startPay() {
    payDate = detail?.expense.paidDate ?? todayBerlin();
    payAccountId =
      detail?.expense.paidFromAccountId ??
      accounts.find((a) => a.isDefault === 1 && a.active === 1)?.id ??
      "";
    showPay = true;
  }

  // Übergabe F5 (R1-019): Zahldatum vor dem Belegdatum ist erlaubt, aber auffällig.
  const payBeforeExpense = $derived(
    !!payDate && !!detail?.expense.expenseDate && payDate < detail.expense.expenseDate,
  );

  async function markPaid() {
    busy = true;
    try {
      await expensesSetPayment({
        expenseId: id,
        paidDate: payDate || null,
        paidFromAccountId: payAccountId || null,
      });
      showPay = false;
      flash("Als bezahlt markiert.");
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function setUnpaid() {
    busy = true;
    try {
      await expensesSetPayment({ expenseId: id, paidDate: null, paidFromAccountId: null });
      flash("Wieder auf offen gesetzt.");
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function openCancel() {
    showCancel = true;
    cancelError = "";
    await tick();
    cancelReasonEl?.focus();
  }

  async function doCancel() {
    if (!cancelReason.trim()) {
      cancelError = "Bitte einen Grund für das Storno angeben.";
      cancelReasonEl?.focus();
      return;
    }
    cancelError = "";
    busy = true;
    try {
      await expensesCancel({ expenseId: id, reason: cancelReason.trim() });
      showCancel = false;
      cancelReason = "";
      flash("Kosten storniert.");
      await load();
    } catch (e) {
      flash("Storno fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  function onAttachmentsChanged(list: AttachmentView[]) {
    if (detail) detail.attachments = list;
  }

  // Plan W7: Beleg-Original (Fremddatei) als Download, hash-geprüft.
  function openReceipt(archiveId: string) {
    saveArchiveFile(archiveId);
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title={detail?.expense.expenseNumber}>
  {#snippet actions()}
    {#if canShowRawXml}
      <button
        type="button"
        class="btn-secondary btn-sm"
        onclick={openRawXml}
        aria-disabled={xmlLoading ? "true" : undefined}
        aria-busy={xmlLoading}
      >
        {xmlLoading ? "Lade Roh-XML …" : "Roh-XML anzeigen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if detail}
  {@const e = detail.expense}
  <p class="sub">
    {#if e.status === "canceled"}
      <Badge tone="neutral" strike>storniert</Badge>
    {:else}
      <Badge tone="success">erfasst</Badge>
    {/if}
  </p>

  {#if e.status === "canceled"}
    <p class="caveat">
      Diese Kosten wurden storniert{e.canceledReason ? ` — Grund: ${e.canceledReason}` : ""}.
      Stornierte Kosten zählen nicht für die Steuer.
    </p>
  {/if}

  <section class="card">
    <dl>
      <div><dt>Lieferant</dt><dd>{e.vendorNameSnapshot}{#if detail.vendor}{" "}<span class="muted">(Kontakt verknüpft)</span>{/if}</dd></div>
      <div><dt>Beleg-Datum</dt><dd>{date(e.expenseDate)}</dd></div>
      <div><dt>Bezahlt am</dt><dd>{e.paidDate ? date(e.paidDate) : "noch offen"}</dd></div>
      <div><dt>Kategorie</dt><dd>{expenseCategoryLabel(e.category)}</dd></div>
      <div><dt>Rechnungs-Nr. (Lieferant)</dt><dd>{e.vendorInvoiceNumber ?? "—"}</dd></div>
      <div><dt>Beschreibung</dt><dd>{e.description}</dd></div>
      <div><dt>Netto</dt><dd class="amt">{euro(e.netAmountCents)}</dd></div>
      <div><dt>USt</dt><dd class="amt">{euro(e.taxAmountCents)}</dd></div>
      <div><dt>Brutto (zählt fürs Finanzamt)</dt><dd class="amt"><strong>{euro(e.grossAmountCents)}</strong></dd></div>
      <div><dt>§13b Reverse-Charge</dt><dd>{e.reverseCharge13b === 1 ? "ja" : "nein"}</dd></div>
      {#if e.notes}<div><dt>Notiz</dt><dd class="pre">{e.notes}</dd></div>{/if}
    </dl>
  </section>

  {#if e.status !== "canceled"}
    <section class="card">
      <h2>Zahlung</h2>
      {#if showPay}
        <div class="pay-grid">
          <div class="field">
            <label for="pay-date">Bezahlt am<span class="req" aria-hidden="true">*</span></label>
            <input
              id="pay-date"
              class="kb-input"
              type="date"
              bind:value={payDate}
              max={todayBerlin()}
              required
              aria-required="true"
              aria-describedby={payBeforeExpense ? "pay-date-note" : undefined}
            />
          </div>
          <div class="field">
            <label for="pay-account">Zahlungs-Konto</label>
            <select id="pay-account" class="kb-input" bind:value={payAccountId}>
              <option value="">— keines —</option>
              {#each accounts.filter((a) => a.active === 1 || a.id === payAccountId) as a (a.id)}
                <option value={a.id}>{a.label}</option>
              {/each}
            </select>
          </div>
          <div class="row">
            <button type="button" class="btn-primary btn-sm" onclick={markPaid} disabled={busy || !payDate}>Zahlung speichern</button>
            <button type="button" class="btn-secondary btn-sm" onclick={() => (showPay = false)}>Abbrechen</button>
          </div>
          {#if payBeforeExpense}
            <p id="pay-date-note" class="note">
              Das Zahldatum liegt vor dem Belegdatum ({date(e.expenseDate)}), z. B. bei Vorkasse.
              Das ist erlaubt — prüfe nur kurz, ob beide Daten stimmen.
            </p>
          {/if}
        </div>
      {:else if e.paidDate}
        <p>Bezahlt am <strong>{date(e.paidDate)}</strong> · Konto: {accountLabel(e.paidFromAccountId)}</p>
        <div class="row">
          <button type="button" class="btn-secondary btn-sm" onclick={startPay} disabled={busy}>Zahlung korrigieren</button>
          <button type="button" class="btn-secondary btn-sm" onclick={setUnpaid} disabled={busy}>Als unbezahlt markieren</button>
        </div>
      {:else}
        <p class="muted">Noch nicht als bezahlt markiert (zählt erst mit dem Zahldatum für die Steuer).</p>
        <button type="button" class="btn-primary btn-sm" onclick={startPay} disabled={busy}>Als bezahlt markieren</button>
      {/if}
    </section>
  {/if}

  <section class="card">
    <h2>Beleg</h2>
    {#if e.receiptArchiveId}
      <div class="row">
        <button type="button" class="btn-secondary btn-sm" onclick={() => openReceipt(e.receiptArchiveId!)}>Beleg herunterladen</button>
      </div>
    {:else}
      <p class="muted">Kein primärer Beleg hinterlegt. Unter „Zusätzliche Anhänge“ kannst du ein Foto oder PDF ergänzen.</p>
    {/if}
  </section>

  {#if e.status !== "canceled"}
    <section class="card">
      <h2>Anlagevermögen</h2>
      {#if e.capitalizedAsAssetId}
        <p>
          Aus dieser Position wurde eine Anlage aktiviert.
          <a class="inline-link" href={`/assets/${e.capitalizedAsAssetId}`}>Zur Anlage <span aria-hidden="true">→</span></a>
        </p>
      {:else}
        <p class="muted">
          Größere Anschaffungen (z. B. Hardware), die du über mehrere Jahre
          abschreibst, kannst du als Anlage übernehmen — Klein.Buch schlägt die
          passende Abschreibung vor.
        </p>
        <a class="btn-secondary btn-sm" href={`/assets/new?expenseId=${e.id}`}>Als Anlage übernehmen</a>
      {/if}
    </section>
  {/if}

  <section class="card">
    <h2>Zusätzliche Anhänge</h2>
    <AttachmentUpload
      parentType="expense"
      parentId={e.id}
      attachments={detail.attachments}
      onchange={onAttachmentsChanged}
    />
  </section>

  {#if e.status !== "canceled"}
    <section class="card danger">
      <h2>Stornieren</h2>
      <p class="muted">
        Gespeicherte Kosten werden nicht gelöscht, sondern storniert — so verlangt
        es das Gesetz. Für eine Korrektur danach einfach neue Kosten erfassen.
      </p>
      {#if showCancel}
        <div class="cancel-form">
          <div class="field grow">
            <label for="cancel-reason">Storno-Grund<span class="req" aria-hidden="true">*</span></label>
            <input
              id="cancel-reason"
              class="kb-input"
              type="text"
              autocomplete="off"
              bind:this={cancelReasonEl}
              bind:value={cancelReason}
              oninput={() => (cancelError = "")}
              required
              aria-required="true"
              aria-invalid={cancelError ? "true" : undefined}
              aria-describedby={cancelError ? "cancel-reason-err" : "cancel-reason-hint"}
            />
            {#if cancelError}
              <p id="cancel-reason-err" class="msg err">{cancelError}</p>
            {:else}
              <p id="cancel-reason-hint" class="msg hint">z. B. „doppelt erfasst“ oder „falscher Betrag“</p>
            {/if}
          </div>
          <div class="row">
            <button type="button" class="btn-secondary btn-sm" onclick={() => (showCancel = false)}>Abbrechen</button>
            <button type="button" class="btn-danger btn-sm" onclick={doCancel} disabled={busy}>Storno bestätigen</button>
          </div>
        </div>
      {:else}
        <button type="button" class="btn-danger btn-sm" onclick={openCancel}>Kosten stornieren</button>
      {/if}
    </section>
  {/if}
{/if}

<style>
  .sub { margin: 0 0 1rem; }
  /* .card / .card.danger / .caveat / .kb-input: globale Definitionen aus tokens.css. */
  dl { display: grid; grid-template-columns: 1fr 1fr; gap: var(--sp-2) var(--sp-5); margin: 0; }
  dl div { display: flex; flex-direction: column; min-width: 0; }
  dt { font-size: var(--fs-xs); color: var(--text-muted); }
  dd { margin: 0; font-size: var(--fs-md); overflow-wrap: anywhere; }
  dd.amt { font-variant-numeric: tabular-nums; }
  dd.pre { white-space: pre-wrap; }
  @media (max-width: 600px) { dl { grid-template-columns: 1fr; } }
  .row { display: flex; gap: var(--sp-2); align-items: center; flex-wrap: wrap; }
  .pay-grid { display: flex; gap: var(--sp-4); align-items: end; flex-wrap: wrap; }
  .cancel-form { display: flex; gap: var(--sp-3); align-items: end; flex-wrap: wrap; }
  .field { display: flex; flex-direction: column; gap: var(--sp-1); min-width: 0; }
  .field.grow { flex: 1 1 16rem; }
  .field > label { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .msg { font-size: var(--fs-sm); margin: 0; line-height: 1.4; }
  .msg.err { color: var(--status-danger-fg); font-weight: 600; }
  .msg.hint { color: var(--text-muted); }
  .note {
    flex-basis: 100%; margin: 0; font-size: var(--fs-sm);
    color: var(--status-warning-fg); background: var(--status-warning-bg);
    border-left: 3px solid var(--status-warning-line); padding: var(--sp-1) var(--sp-2);
  }
  .inline-link { text-decoration: underline; text-underline-offset: 2px; }
  .muted { color: var(--text-muted); }
</style>
