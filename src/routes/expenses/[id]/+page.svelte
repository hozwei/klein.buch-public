<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import {
    expensesGet,
    expensesCancel,
    expensesSetPayment,
    expensesReceiptXmlText,
    paymentAccountsList,
    attachmentsOpen,
    attachmentsReveal,
  } from "$lib/api";
  import type { ExpenseDetail, AttachmentView, PaymentAccount } from "$lib/types";
  import { euro, date } from "$lib/format";
  import { expenseCategoryLabel } from "$lib/labels";
  import AttachmentUpload from "$lib/AttachmentUpload.svelte";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Badge from "$lib/Badge.svelte";
  import { flash } from "$lib/toast.svelte";
  import { openXmlViewer } from "$lib/xmlViewerModal.svelte";

  let detail = $state<ExpenseDetail | null>(null);
  let accounts = $state<PaymentAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let showCancel = $state(false);
  let cancelReason = $state("");
  let showPay = $state(false);
  let payDate = $state(new Date().toISOString().slice(0, 10));
  let payAccountId = $state<string>("");

  // PV1-A5: Roh-XML-Viewer wird global im Root-Layout gemountet
  // (siehe `XmlViewerDialog.svelte`). Lokal nur Lade-Flag, damit der Button
  // nicht doppelt klickbar ist.
  let xmlLoading = $state(false);

  let id = $derived($page.params.id ?? "");

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
    payDate = detail?.expense.paidDate ?? new Date().toISOString().slice(0, 10);
    payAccountId =
      detail?.expense.paidFromAccountId ??
      accounts.find((a) => a.isDefault === 1 && a.active === 1)?.id ??
      "";
    showPay = true;
  }

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

  async function doCancel() {
    if (!cancelReason.trim()) {
      flash("Storno-Grund ist erforderlich.", "error");
      return;
    }
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

  async function openReceipt(archiveId: string) {
    try {
      await attachmentsOpen(archiveId);
    } catch (e) {
      flash(String(e), "error");
    }
  }
  async function revealReceipt(archiveId: string) {
    try {
      await attachmentsReveal(archiveId);
    } catch (e) {
      flash(String(e), "error");
    }
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title={detail?.expense.expenseNumber}>
  {#snippet actions()}
    {#if canShowRawXml}
      <button
        type="button"
        class="btn-secondary btn-sm"
        onclick={openRawXml}
        disabled={xmlLoading}
      >
        {xmlLoading ? "Lade Roh-XML …" : "Roh-XML anzeigen"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
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
      <div><dt>Lieferant</dt><dd>{e.vendorNameSnapshot}{#if detail.vendor} <span class="muted">(Kontakt verknüpft)</span>{/if}</dd></div>
      <div><dt>Beleg-Datum</dt><dd>{date(e.expenseDate)}</dd></div>
      <div><dt>Bezahlt am</dt><dd>{e.paidDate ? date(e.paidDate) : "noch offen"}</dd></div>
      <div><dt>Kategorie</dt><dd>{expenseCategoryLabel(e.category)}</dd></div>
      <div><dt>Rechnungs-Nr. (Lieferant)</dt><dd>{e.vendorInvoiceNumber ?? "—"}</dd></div>
      <div><dt>Beschreibung</dt><dd>{e.description}</dd></div>
      <div><dt>Netto</dt><dd>{euro(e.netAmountCents)}</dd></div>
      <div><dt>USt</dt><dd>{euro(e.taxAmountCents)}</dd></div>
      <div><dt>Brutto (zählt fürs Finanzamt)</dt><dd><strong>{euro(e.grossAmountCents)}</strong></dd></div>
      <div><dt>§13b Reverse-Charge</dt><dd>{e.reverseCharge13b === 1 ? "ja" : "nein"}</dd></div>
      {#if e.notes}<div><dt>Notiz</dt><dd>{e.notes}</dd></div>{/if}
    </dl>
  </section>

  {#if e.status !== "canceled"}
    <section class="card">
      <h2>Zahlung</h2>
      {#if showPay}
        <div class="pay-grid">
          <label>
            Bezahlt am
            <input type="date" bind:value={payDate} max={new Date().toISOString().slice(0, 10)} />
          </label>
          <label>
            Konto
            <select bind:value={payAccountId}>
              <option value="">— keines —</option>
              {#each accounts.filter((a) => a.active === 1 || a.id === payAccountId) as a (a.id)}
                <option value={a.id}>{a.label}</option>
              {/each}
            </select>
          </label>
          <div class="row">
            <button class="btn-primary btn-sm" onclick={markPaid} disabled={busy || !payDate}>Speichern</button>
            <button class="btn-secondary btn-sm" onclick={() => (showPay = false)}>Abbrechen</button>
          </div>
        </div>
      {:else if e.paidDate}
        <p>Bezahlt am <strong>{date(e.paidDate)}</strong> · Konto: {accountLabel(e.paidFromAccountId)}</p>
        <div class="row">
          <button class="btn-secondary btn-sm" onclick={startPay} disabled={busy}>Zahlung korrigieren</button>
          <button class="btn-secondary btn-sm" onclick={setUnpaid} disabled={busy}>Als unbezahlt markieren</button>
        </div>
      {:else}
        <p class="muted">Noch nicht als bezahlt markiert (zählt erst mit dem Zahldatum für die Steuer).</p>
        <button class="btn-primary btn-sm" onclick={startPay} disabled={busy}>Als bezahlt markieren</button>
      {/if}
    </section>
  {/if}

  <section class="card">
    <h2>Beleg</h2>
    {#if e.receiptArchiveId}
      <div class="row">
        <button class="btn-secondary btn-sm" onclick={() => openReceipt(e.receiptArchiveId!)}>Beleg öffnen</button>
        <button class="btn-secondary btn-sm" onclick={() => revealReceipt(e.receiptArchiveId!)}>Im Ordner zeigen</button>
      </div>
    {:else}
      <p class="muted">Kein primärer Beleg hinterlegt.</p>
    {/if}
  </section>

  {#if e.status !== "canceled"}
    <section class="card">
      <h2>Anlagevermögen</h2>
      {#if e.capitalizedAsAssetId}
        <p>
          Aus dieser Position wurde eine Anlage aktiviert.
          <a href={`/assets/${e.capitalizedAsAssetId}`}>Zur Anlage →</a>
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
        <div class="row">
          <input type="text" bind:value={cancelReason} placeholder="Storno-Grund" />
          <button class="btn-danger btn-sm" onclick={doCancel} disabled={busy}>Storno bestätigen</button>
          <button class="btn-secondary btn-sm" onclick={() => (showCancel = false)}>Abbrechen</button>
        </div>
      {:else}
        <button class="btn-danger btn-sm" onclick={() => (showCancel = true)}>Kosten stornieren</button>
      {/if}
    </section>
  {/if}
{/if}

<style>
  .sub { margin: 0 0 1rem; }
  /* .card / .card.danger entfernt — globale Definitionen aus tokens.css. */
  dl { display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem 1.5rem; margin: 0; }
  dl div { display: flex; flex-direction: column; }
  dt { font-size: 0.78rem; color: #6b7280; }
  dd { margin: 0; font-size: 0.95rem; }
  .row { display: flex; gap: 0.5rem; align-items: center; flex-wrap: wrap; }
  input[type="text"] { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.9rem; font-family: inherit; min-width: 16rem; }
  .pay-grid { display: flex; gap: 1rem; align-items: end; flex-wrap: wrap; }
  .pay-grid label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  .pay-grid input, .pay-grid select { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.9rem; font-family: inherit; }
  /* .caveat entfernt — globale Definition aus tokens.css greift. */
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
