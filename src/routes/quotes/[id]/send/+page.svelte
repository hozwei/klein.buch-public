<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import {
    quotesGet,
    mailAccountsList,
    mailQuotePreview,
    mailSendQuote,
    legalDocumentsList,
  } from "$lib/api";
  import type { QuoteDetail, MailAccount, LegalDocument } from "$lib/types";
  import { euro, date } from "$lib/format";
  import Banner from "$lib/Banner.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { flash } from "$lib/toast.svelte";

  let id = $derived($page.params.id ?? "");

  let detail = $state<QuoteDetail | null>(null);
  let accounts = $state<MailAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let accountId = $state("");
  let to = $state("");
  let subject = $state("");
  let body = $state("");
  let sending = $state(false);

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
    if (!accountId) {
      flash("Bitte ein Postfach wählen (oder unter Einstellungen einrichten).", "error");
      return;
    }
    if (!to.trim()) {
      flash("Bitte einen Empfänger angeben.", "error");
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
      setTimeout(() => goto(`/quotes/${id}`), 900);
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
      <button class="btn-primary" onclick={send} disabled={sending || accounts.length === 0 || !legalReady}>
        {sending ? "Sende …" : "Angebot senden"}
      </button>
    {/if}
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
  <p><a href={`/quotes/${id}`}>zurück</a></p>
{:else if detail}
  {@const q = detail.quote}
  <section class="card summary">
    <div>
      <strong>{q.quoteNumber}</strong> · {date(q.quoteDate)} · {euro(q.grossAmountCents)}
    </div>
    <div class="muted">
      Angehängt werden: <code>{q.quoteNumber}.pdf</code> · <code>AGB.pdf</code> · <code>Datenschutz.pdf</code>
    </div>
  </section>

  {#if !legalReady}
    <Banner kind="warning">
      Versand nicht möglich: Es fehlt
      {#if !hasAgb}eine aktive AGB-Version{/if}{#if !hasAgb && !hasPrivacy} und {/if}{#if !hasPrivacy}eine aktive Datenschutz-Version{/if}.
      Bitte unter <a href="/settings/legal">Einstellungen → AGB &amp; Datenschutz</a> hochladen und aktivieren.
    </Banner>
  {/if}

  {#if accounts.length === 0}
    <Banner kind="warning">
      Noch kein Postfach eingerichtet. Bitte zuerst unter
      <a href="/settings/mail">Einstellungen → E-Mail-Versand</a> einrichten.
    </Banner>
  {/if}

  <section class="card">
    <div class="grid">
      <label>
        Postfach
        <select bind:value={accountId}>
          {#each accounts as a (a.id)}
            <option value={a.id}>{a.label} — {a.fromEmail}{a.isDefault === 1 ? " ★" : ""}</option>
          {/each}
        </select>
      </label>
      <label>
        Empfänger
        <input type="email" bind:value={to} placeholder="kunde@example.com" />
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

  </section>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .summary { display: flex; flex-direction: column; gap: 0.25rem; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 0.75rem 1rem; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #374151; gap: 0.25rem; margin-bottom: 0.75rem; }
  label.full { width: 100%; }
  input, select, textarea {
    padding: 0.45rem 0.55rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem;
    font-family: inherit;
  }
  textarea { resize: vertical; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  code { background: #f3f4f6; padding: 0.05rem 0.3rem; border-radius: 3px; }
  .muted { color: #6b7280; }
</style>
