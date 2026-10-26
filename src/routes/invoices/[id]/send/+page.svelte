<script lang="ts">
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    invoicesGet,
    mailAccountsList,
    mailInvoicePreview,
    mailSendInvoice,
  } from "#lib/api.ts";
  import type { Invoice, InvoiceDetail, MailAccount } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  let id = $derived(page.params.id ?? "");

  let detail = $state<InvoiceDetail | null>(null);
  let accounts = $state<MailAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let accountId = $state("");
  let to = $state("");
  let subject = $state("");
  let body = $state("");
  let sending = $state(false);
  // Nach dem ersten Sende-Versuch zeigen die Felder ihre Meldungen.
  let submitted = $state(false);
  // Versand gelungen, aber Protokolleintrag fehlt (R4-S2-EMAILLOG): Hinweis
  // auf der Seite statt Fehler-Toast, kein automatischer Rücksprung.
  let sendWarning = $state<string | null>(null);
  // Storno-Beleg: Bezug aufs Original (Nummer, Datum, Link).
  let original = $state<Invoice | null>(null);

  // Hinterlegte Adresse wie im Server (commands::mail::resolve_recipient):
  // Käufer-Snapshot der Rechnung, sonst die Adresse des Kontakts. Ein anderer
  // Empfänger verlangt beim Senden eine Bestätigung (Daten-Passwort + App-Code).
  let storedRecipient = $derived(
    (detail?.invoice.buyerEmail ?? detail?.buyer?.email ?? "").trim(),
  );
  let recipientDiffers = $derived(
    !!to.trim() && to.trim().toLowerCase() !== storedRecipient.toLowerCase(),
  );
  let errAccount = $derived(accountId ? "" : "Bitte ein Postfach wählen.");
  let errTo = $derived(to.trim() ? "" : "Bitte einen Empfänger angeben.");
  let isStorno = $derived(!!detail?.invoice.isStornoFor);

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await invoicesGet(id);
      if (!detail) {
        error = "Rechnung nicht gefunden.";
        return;
      }
      const inv = detail.invoice;
      if (!inv.lockedAt || !inv.pdfArchiveId) {
        error = "Diese Rechnung ist noch nicht ausgestellt — bitte sie zuerst ausstellen.";
        return;
      }
      // Empfänger-Default: Buyer-Snapshot der Rechnung, sonst Kontakt-Mail.
      to = inv.buyerEmail ?? detail.buyer?.email ?? "";
      if (inv.isStornoFor) {
        try {
          original = (await invoicesGet(inv.isStornoFor))?.invoice ?? null;
        } catch {
          original = null;
        }
      }

      accounts = await mailAccountsList();
      const def = accounts.find((a) => a.isDefault === 1) ?? accounts[0];
      accountId = def?.id ?? "";

      // Betreff + Body aus dem invoice-de-Template.
      const preview = await mailInvoicePreview(id);
      subject = preview.subject;
      body = preview.body;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function send() {
    submitted = true;
    if (errAccount || errTo) {
      await tick();
      document.querySelector<HTMLElement>('#send-form [aria-invalid="true"]')?.focus();
      return;
    }
    sending = true;
    try {
      const res = await mailSendInvoice({
        accountId,
        invoiceId: id,
        to: to.trim(),
        subject: subject || null,
        body: body || null,
      });
      flash(`Gesendet an ${res.to} (${res.attachmentCount} Anhang).`, "ok");
      // R4-S2-EMAILLOG: Versand gelungen, Protokolleintrag fehlt.
      if (res.warning) sendWarning = res.warning;
      else setTimeout(() => goto(`/invoices/${id}`), 900);
    } catch (e) {
      flash("Versand fehlgeschlagen: " + String(e), "error");
    } finally {
      sending = false;
    }
  }
</script>

<PageBar back={`/invoices/${id}`} backLabel={isStorno ? "Stornorechnung" : "Rechnung"} title={isStorno ? "Stornorechnung versenden" : "Rechnung versenden"}>
  {#snippet actions()}
    {#if detail && !error}
      <a class="btn-secondary" href={`/invoices/${id}`}>Abbrechen</a>
      <button type="submit" form="send-form" class="btn-primary" disabled={sending || accounts.length === 0}>
        {sending ? "Sende …" : "Senden"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
  <p><a class="btn-secondary" href={`/invoices/${id}`}>Zurück zur Rechnung</a></p>
{:else if detail}
  {@const inv = detail.invoice}
  {#if sendWarning}
    <Banner kind="warning">
      {sendWarning}
      <a class="txt-link" href="/settings/mail-log">Versandprotokoll öffnen</a> ·
      <a class="txt-link" href={`/invoices/${id}`}>zur Rechnung</a>
    </Banner>
  {/if}
  <section class="card summary">
    <div>
      {#if isStorno}Stornorechnung{/if}
      <strong class="doc-no">{inv.invoiceNumber}</strong> · {date(inv.invoiceDate)} · <span class="num">{euro(inv.grossAmountCents)}</span>
    </div>
    {#if isStorno}
      <div>
        Storniert die Rechnung
        {#if original}
          <a class="txt-link doc-no" href={`/invoices/${inv.isStornoFor}`}>{original.invoiceNumber}</a> vom {date(original.invoiceDate)}.
        {:else}
          <a class="txt-link" href={`/invoices/${inv.isStornoFor}`}>(Original öffnen)</a>.
        {/if}
      </div>
    {/if}
    <div class="muted">
      Die Rechnung wird automatisch als PDF angehängt: <code>{inv.invoiceNumber}.pdf</code>
    </div>
  </section>

  {#if accounts.length === 0}
    <Banner kind="warning">
      Noch kein Postfach eingerichtet. Bitte zuerst unter
      <a class="txt-link" href="/settings/mail">Einstellungen → E-Mail-Versand</a> einrichten.
    </Banner>
  {/if}

  <form id="send-form" class="card" onsubmit={(e) => { e.preventDefault(); send(); }} novalidate>
    <div class="grid">
      <label>
        <span>Postfach <span class="req" aria-hidden="true">*</span></span>
        <select
          bind:value={accountId}
          required
          aria-invalid={submitted && errAccount ? "true" : undefined}
          aria-describedby={submitted && errAccount ? "send-account-err" : undefined}
        >
          {#each accounts as a (a.id)}
            <option value={a.id}>{a.label} — {a.fromEmail}{a.isDefault === 1 ? " (Standard)" : ""}</option>
          {/each}
        </select>
        {#if submitted && errAccount}<span id="send-account-err" class="field-err">{errAccount}</span>{/if}
      </label>
      <label>
        <span>Empfänger <span class="req" aria-hidden="true">*</span></span>
        <input
          type="email"
          inputmode="email"
          autocomplete="off"
          autocapitalize="off"
          spellcheck="false"
          bind:value={to}
          required
          aria-invalid={submitted && errTo ? "true" : undefined}
          aria-describedby={submitted && errTo ? "send-to-err" : recipientDiffers ? "send-to-differs" : undefined}
        />
        {#if submitted && errTo}
          <span id="send-to-err" class="field-err">{errTo}</span>
        {:else if recipientDiffers}
          <span id="send-to-differs" class="field-warn">
            {#if storedRecipient}
              Weicht von der hinterlegten Adresse {storedRecipient} ab.
            {:else}
              Beim Kontakt ist keine E-Mail-Adresse hinterlegt.
            {/if}
            Beim Senden fragt Klein.Buch zur Bestätigung Daten-Passwort und App-Code ab.
          </span>
        {/if}
      </label>
    </div>
    <label class="full">
      Betreff
      <input type="text" bind:value={subject} />
    </label>
    <label class="full">
      Nachricht (kannst du anpassen)
      <textarea rows="14" bind:value={body}></textarea>
    </label>
  </form>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .summary { display: flex; flex-direction: column; gap: 0.25rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 0.75rem 1rem; }
  label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); gap: 0.25rem; margin-bottom: 0.75rem; min-width: 0; }
  label.full { width: 100%; }
  input, select, textarea {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text);
  }
  textarea { resize: vertical; }
  .req { color: var(--status-danger-fg); }
  .field-err { font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-warn {
    font-size: var(--fs-xs); font-weight: 400; color: var(--status-warning-fg);
    background: var(--status-warning-bg); border-left: 3px solid var(--status-warning-line);
    padding: 4px 8px; border-radius: var(--r-sm); overflow-wrap: anywhere;
  }
  .txt-link { text-decoration: underline; text-underline-offset: 2px; }
  .doc-no { font-family: var(--font-mono); }
  .num { font-variant-numeric: tabular-nums; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  code { background: var(--surface-sunken); padding: 0.05rem 0.3rem; border-radius: 3px; overflow-wrap: anywhere; }
  .muted { color: var(--text-muted); }
</style>
