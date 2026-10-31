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
  import type { ActionMenuItem } from "#lib/ActionMenu.svelte";
  import { euro, date } from "#lib/format.ts";
  import { expenseCategoryLabel } from "#lib/labels.ts";
  import AttachmentUpload from "#lib/AttachmentUpload.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Badge from "#lib/Badge.svelte";
  import { expenseStatusView } from "../expenseStatus.ts";
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
  let payError = $state("");
  let payDateEl: HTMLInputElement | undefined = $state();
  let notFound = $state(false);

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
    notFound = false;
    try {
      detail = await expensesGet(id);
      if (!detail) {
        notFound = true;
        error = "Kosten nicht gefunden.";
      }
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

  async function startPay() {
    if (busy) return;
    payDate = detail?.expense.paidDate?.slice(0, 10) ?? todayBerlin();
    payAccountId =
      detail?.expense.paidFromAccountId ??
      accounts.find((a) => a.isDefault === 1 && a.active === 1)?.id ??
      "";
    payError = "";
    showPay = true;
    await tick();
    payDateEl?.focus();
  }

  // Übergabe F5 (R1-019): Zahldatum vor dem Belegdatum ist erlaubt, aber auffällig.
  const payBeforeExpense = $derived(
    !!payDate && !!detail?.expense.expenseDate && payDate < detail.expense.expenseDate,
  );

  async function markPaid() {
    if (busy) return;
    if (!payDate) {
      payError = "Bitte das Zahldatum angeben.";
      payDateEl?.focus();
      return;
    }
    payError = "";
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

  // U4 (V2-12, design.md §5): umkehrbar — ohne Rückfrage, dafür „Rückgängig“ im Toast.
  async function setUnpaid() {
    if (busy || !detail) return;
    const prev = {
      paidDate: detail.expense.paidDate,
      paidFromAccountId: detail.expense.paidFromAccountId,
    };
    busy = true;
    try {
      await expensesSetPayment({ expenseId: id, paidDate: null, paidFromAccountId: null });
      flash("Wieder auf offen gesetzt.", "ok", undefined, {
        action: { label: "Rückgängig", run: () => void restorePayment(prev) },
      });
      await load();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function restorePayment(prev: { paidDate: string | null; paidFromAccountId: string | null }) {
    if (busy) return;
    busy = true;
    try {
      await expensesSetPayment({ expenseId: id, paidDate: prev.paidDate, paidFromAccountId: prev.paidFromAccountId });
      flash("Zahlung wiederhergestellt.");
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
    if (busy) return;
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

  // U4 (V2-20, design.md §5 „Detailseiten“, ADR 0041 Nr. 96): Hauptaktion = nächster
  // Schritt („Als bezahlt markieren“ solange offen; nach einem Storno „Korrigiert neu
  // erfassen“), Nebenaktionen als `secondary`, Stornieren im Menü „Mehr“ unten. Der
  // Eintrag öffnet den Storno-Bereich mit Pflichtgrund.
  let isCanceled = $derived(detail?.expense.status === "canceled");
  let barSecondary = $derived.by<ActionMenuItem[]>(() => {
    if (!detail) return [];
    const items: ActionMenuItem[] = [];
    if (canShowRawXml) {
      items.push({ label: xmlLoading ? "Roh-XML wird geladen …" : "Roh-XML anzeigen", onSelect: () => void openRawXml() });
    }
    // V1-22: Werte übernehmen (neue Kosten, das Original bleibt unverändert).
    if (!isCanceled) items.push({ label: "Als Vorlage für neue Kosten", href: `/expenses/new?from=${detail.expense.id}` });
    return items;
  });
  let barMore = $derived<ActionMenuItem[]>(
    detail && !isCanceled ? [{ label: "Stornieren", onSelect: () => void openCancel(), danger: true }] : [],
  );
</script>

<PageBar
  back="/expenses"
  backLabel="Kosten"
  title={detail ? `Kosten ${detail.expense.expenseNumber}` : notFound ? "Kosten nicht gefunden" : "Kosten"}
  secondary={barSecondary}
  more={barMore}
>
  {#snippet actions()}
    {#if detail && isCanceled}
      <Button variant="primary" href={`/expenses/new?from=${detail.expense.id}`}>Korrigiert neu erfassen</Button>
    {:else if detail && !detail.expense.paidDate && !showPay}
      <Button variant="primary" onclick={() => void startPay()} ariaDisabled={busy}>Als bezahlt markieren</Button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted" role="status">Wird geladen …</p>
{:else if error}
  <Banner>{error}{#if notFound}{" "}<a class="kb-link" href="/expenses">Zur Kostenliste</a>{/if}</Banner>
{:else if detail}
  {@const e = detail.expense}
  <!-- U4 (V3-09): Status aus expenseStatus.ts, wie in der Liste. -->
  {@const st = expenseStatusView(e)}
  <p class="sub">
    <Badge tone={st.tone} strike={st.strike}>{st.label}</Badge>
  </p>

  {#if e.status === "canceled"}
    <Banner kind="warning">
      Diese Kosten wurden storniert{e.canceledReason ? ` — Grund: ${e.canceledReason}` : ""}.
      Stornierte Kosten zählen nicht für die Steuer. Mit „Korrigiert neu erfassen“ legst du
      neue Kosten mit diesen Werten an.
    </Banner>
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
              class="kb-input kb-w-date"
              type="date"
              bind:this={payDateEl}
              bind:value={payDate}
              oninput={() => (payError = "")}
              max={todayBerlin()}
              required
              aria-required="true"
              aria-invalid={payError ? "true" : undefined}
              aria-describedby={[payError ? "pay-date-err" : "", payBeforeExpense ? "pay-date-note" : ""].filter(Boolean).join(" ") || undefined}
            />
            {#if payError}<p id="pay-date-err" class="msg err">{payError}</p>{/if}
          </div>
          <div class="field">
            <label for="pay-account">Zahlungs-Konto</label>
            <select id="pay-account" class="kb-input kb-w-medium" bind:value={payAccountId}>
              <option value="">— keines —</option>
              {#each accounts.filter((a) => a.active === 1 || a.id === payAccountId) as a (a.id)}
                <option value={a.id}>{a.label}</option>
              {/each}
            </select>
          </div>
          <!-- design.md §5: „Abbrechen“ zuerst, Hauptaktion zuletzt. -->
          <div class="row">
            <button type="button" class="btn-secondary btn-sm" onclick={() => (showPay = false)}>Abbrechen</button>
            <button type="button" class="btn-primary btn-sm" onclick={markPaid} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>Zahlung speichern</button>
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
          <button type="button" class="btn-secondary btn-sm" onclick={startPay} aria-disabled={busy ? "true" : undefined}>Zahlung korrigieren</button>
          <button type="button" class="btn-secondary btn-sm" onclick={setUnpaid} aria-disabled={busy ? "true" : undefined}>Als unbezahlt markieren</button>
        </div>
      {:else}
        <!-- Hauptaktion steht in der PageBar (V2-20); hier dieselbe Aktion als `secondary`
             im Zusammenhang (Handy: PageBar klebt nicht). -->
        <p class="muted">Noch nicht als bezahlt markiert (zählt erst mit dem Zahldatum für die Steuer).</p>
        <button type="button" class="btn-secondary btn-sm" onclick={startPay} aria-disabled={busy ? "true" : undefined}>Als bezahlt markieren</button>
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
      <!-- U6-012: Ein Anhang ersetzt den beim Erfassen fehlenden Beleg; die Liste kennt
           Anhänge nicht und zeigt weiter „Kein Beleg“ (eigenes Lesefeld nötig). -->
      {#if detail.attachments.length > 0}
        <p class="muted">Beim Erfassen wurde kein Beleg hochgeladen; deine Anhänge stehen unter „Zusätzliche Anhänge“.</p>
      {:else}
        <p class="muted">Kein primärer Beleg hinterlegt. Unter „Zusätzliche Anhänge“ kannst du ein Foto oder PDF ergänzen.</p>
      {/if}
    {/if}
  </section>

  {#if e.status !== "canceled"}
    <section class="card">
      <h2>Anlagevermögen</h2>
      {#if e.capitalizedAsAssetId}
        <p>
          Diese Kosten sind als Anlage übernommen.
          <a class="kb-link kb-target" href={`/assets/${e.capitalizedAsAssetId}`}>Zur Anschaffung <span aria-hidden="true">→</span></a>
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

  <!-- U4 (V2-20): geöffnet über „Mehr“ → „Stornieren“; GoBD: Storno mit Pflichtgrund. -->
  {#if e.status !== "canceled" && showCancel}
    <section class="card danger" aria-labelledby="exp-cancel-h">
      <h2 id="exp-cancel-h">Stornieren</h2>
      <p class="muted">
        Festgeschriebene Kosten werden nicht gelöscht, sondern storniert — so verlangt
        es das Gesetz. Danach kannst du sie mit „Korrigiert neu erfassen“ neu anlegen.
      </p>
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
            <button type="button" class="btn-danger btn-sm" onclick={doCancel} aria-disabled={busy ? "true" : undefined} aria-busy={busy}>Kosten stornieren</button>
          </div>
      </div>
    </section>
  {/if}
{/if}

<style>
  .sub { margin: 0 0 1rem; }
  /* .card / .card.danger / .kb-input: globale Definitionen aus tokens.css; Hinweise über Banner (V3-18). */
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
  .muted { color: var(--text-muted); }
</style>
