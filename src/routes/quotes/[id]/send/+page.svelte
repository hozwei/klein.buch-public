<script lang="ts">
  import { onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import {
    quotesGet,
    mailAccountsList,
    mailQuotePreview,
    mailSendQuote,
    legalDocumentsList,
  } from "#lib/api.ts";
  import type { QuoteDetail, MailAccount, LegalDocument } from "#lib/types.ts";
  import { euro, date } from "#lib/format.ts";
  import Banner from "#lib/Banner.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import { flash } from "#lib/toast.svelte.ts";

  let id = $derived(page.params.id ?? "");

  let detail = $state<QuoteDetail | null>(null);
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

  // Hinterlegte Adresse wie im Server (commands::mail): die des Kontakts. Ein
  // anderer Empfänger verlangt beim Senden eine Bestätigung (Daten-Passwort + App-Code).
  let storedRecipient = $derived((detail?.buyer?.email ?? "").trim());
  let recipientDiffers = $derived(
    !!to.trim() && to.trim().toLowerCase() !== storedRecipient.toLowerCase(),
  );
  let errAccount = $derived(accountId ? "" : "Bitte ein Postfach wählen.");
  let errTo = $derived(to.trim() ? "" : "Bitte einen Empfänger angeben.");

  let hasAgb = $state(false);
  let hasPrivacy = $state(false);
  let legalReady = $derived(hasAgb && hasPrivacy);

  async function load() {
    loading = true;
    error = null;
    try {
      detail = await quotesGet(id);
      if (!detail) {
        error = "Angebot nicht gefunden.";
        return;
      }
      const q = detail.quote;
      if (!q.lockedAt) {
        error = "Dieses Angebot ist noch nicht fertiggestellt — bitte stelle es zuerst im Angebot fertig.";
        return;
      }
      if (q.status === "canceled" || q.status === "rejected") {
        error = "Ein storniertes oder abgelehntes Angebot kann nicht versendet werden.";
        return;
      }
      to = detail.buyer?.email ?? "";

      accounts = await mailAccountsList();
      const def = accounts.find((a) => a.isDefault === 1) ?? accounts[0];
      accountId = def?.id ?? "";

      const legal: LegalDocument[] = await legalDocumentsList();
      hasAgb = legal.some((d) => d.docType === "agb" && d.isActive === 1);
      hasPrivacy = legal.some((d) => d.docType === "privacy" && d.isActive === 1);

      const preview = await mailQuotePreview(id);
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
    if (!legalReady) {
      flash("Es fehlen aktive AGB und Datenschutz — siehe Einstellungen → AGB & Datenschutz.", "error");
      return;
    }
    sending = true;
    try {
      const res = await mailSendQuote({
        accountId,
        quoteId: id,
        to: to.trim(),
        subject: subject || null,
        body: body || null,
      });
      flash(`Gesendet an ${res.to} (${res.attachmentCount} Anhänge).`, "ok");
      if (res.warning) sendWarning = res.warning;
      else setTimeout(() => goto(`/quotes/${id}`), 900);
    } catch (e) {
      flash("Versand fehlgeschlagen: " + String(e), "error");
    } finally {
      sending = false;
    }
  }
</script>

<PageBar back={`/quotes/${id}`} backLabel="Angebot" title="Angebot versenden">
  {#snippet actions()}
    {#if detail && !error}
      <a class="btn-secondary" href={`/quotes/${id}`}>Abbrechen</a>
      <button type="submit" form="send-form" class="btn-primary" disabled={sending || accounts.length === 0 || !legalReady}>
        {sending ? "Sende …" : "Angebot senden"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
  <p><a class="btn-secondary" href={`/quotes/${id}`}>Zurück zum Angebot</a></p>
{:else if detail}
  {@const q = detail.quote}
  {#if sendWarning}
    <Banner kind="warning">
      {sendWarning}
      <a class="txt-link" href="/settings/mail-log">Versandprotokoll öffnen</a> ·
      <a class="txt-link" href={`/quotes/${id}`}>zum Angebot</a>
    </Banner>
  {/if}
  <section class="card summary">
    <div>
      <strong class="doc-no">{q.quoteNumber}</strong> · {date(q.quoteDate)} · <span class="num">{euro(q.grossAmountCents)}</span>
    </div>
    <div class="muted">
      Angehängt werden: <code>{q.quoteNumber}.pdf</code> · <code>AGB.pdf</code> · <code>Datenschutz.pdf</code>
    </div>
  </section>

  {#if !legalReady}
    <Banner kind="warning">
      Versand nicht möglich: Es fehlt
      {#if !hasAgb}eine aktive AGB-Version{/if}{#if !hasAgb && !hasPrivacy}{" und "}{/if}{#if !hasPrivacy}eine aktive Datenschutz-Version{/if}.
      Bitte unter <a class="txt-link" href="/settings/legal">Einstellungen → AGB &amp; Datenschutz</a> hochladen und aktivieren.
    </Banner>
  {/if}

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
