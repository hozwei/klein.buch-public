<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import {
    paymentAccountsEnsureDefaults,
    paymentAccountsList,
    paymentAccountsCreate,
    paymentAccountsUpdate,
    paymentAccountsSetActive,
  } from "$lib/api";
  import type { PaymentAccount, PaymentAccountInput, PaymentAccountType } from "$lib/types";
  import Banner from "$lib/Banner.svelte";
  import { flash } from "$lib/toast.svelte";

  let accounts = $state<PaymentAccount[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let showInactive = $state(false);

  const TYPES: { value: PaymentAccountType; label: string }[] = [
    { value: "bank", label: "Bankkonto" },
    { value: "cash", label: "Bargeld-Kasse" },
    { value: "paypal", label: "PayPal" },
    { value: "stripe", label: "Stripe" },
    { value: "other", label: "Sonstiges" },
  ];

  function typeLabel(t: string): string {
    return TYPES.find((x) => x.value === t)?.label ?? t;
  }

  // Neu-Formular
  let form = $state<PaymentAccountInput>({
    label: "",
    accountType: "bank",
    iban: null,
    bic: null,
    isDefault: false,
    showOnInvoice: false,
    details: null,
  });

  // Inline-Edit
  let editId = $state<string | null>(null);
  let editForm = $state<PaymentAccountInput>({
    label: "",
    accountType: "bank",
    iban: null,
    bic: null,
    isDefault: false,
    showOnInvoice: false,
    details: null,
  });

  async function load() {
    loading = true;
    error = null;
    try {
      // Beim ersten Öffnen Standard-Konten anlegen (idempotent).
      await paymentAccountsEnsureDefaults();
      accounts = await paymentAccountsList(showInactive);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function reload() {
    accounts = await paymentAccountsList(showInactive);
  }

  async function create() {
    if (!form.label.trim()) {
      flash("Bezeichnung fehlt.", "error");
      return;
    }
    busy = true;
    try {
      await paymentAccountsCreate({
        ...form,
        iban: form.iban?.trim() || null,
        bic: form.bic?.trim() || null,
        details: form.details?.trim() || null,
      });
      form = {
        label: "",
        accountType: "bank",
        iban: null,
        bic: null,
        isDefault: false,
        showOnInvoice: false,
        details: null,
      };
      flash("Konto angelegt.");
      await reload();
    } catch (e) {
      flash("Anlegen fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  function startEdit(a: PaymentAccount) {
    editId = a.id;
    editForm = {
      label: a.label,
      accountType: a.accountType as PaymentAccountType,
      iban: a.iban,
      bic: a.bic,
      isDefault: a.isDefault === 1,
      showOnInvoice: a.showOnInvoice === 1,
      details: a.details,
    };
  }

  async function saveEdit() {
    if (!editId) return;
    busy = true;
    try {
      await paymentAccountsUpdate(editId, {
        ...editForm,
        iban: editForm.iban?.trim() || null,
        bic: editForm.bic?.trim() || null,
        details: editForm.details?.trim() || null,
      });
      editId = null;
      flash("Konto gespeichert.");
      await reload();
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }

  async function toggleActive(a: PaymentAccount) {
    busy = true;
    try {
      await paymentAccountsSetActive(a.id, a.active !== 1);
      flash(a.active === 1 ? "Konto deaktiviert." : "Konto aktiviert.");
      await reload();
    } catch (e) {
      flash("Fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Konten (Bank & Bargeld)" />
<p class="muted">
  Deine Konten für Zahlungen — Bankkonto, Bargeld, PayPal und so weiter. Du wählst
  sie bei Kosten und Privat-Buchungen aus. Konten werden nicht gelöscht, sondern
  deaktiviert. Genau ein Konto ist der <strong>Standard</strong>. Konten mit
  <strong>„Auf Beleg anzeigen"</strong> erscheinen auf Angeboten und Rechnungen
  (mehrere möglich) — Bankkonten zusätzlich als Überweisungsweg in der XRechnung.
</p>

<section class="card">
  <h2>Neues Konto</h2>
  <div class="grid">
    <label>
      Bezeichnung
      <input type="text" bind:value={form.label} placeholder="z. B. Geschäftskonto Sparkasse" />
    </label>
    <label>
      Typ
      <select bind:value={form.accountType}>
        {#each TYPES as t}<option value={t.value}>{t.label}</option>{/each}
      </select>
    </label>
    <label>
      IBAN (optional)
      <input type="text" bind:value={form.iban} placeholder="DE…" />
    </label>
    <label>
      BIC (optional)
      <input type="text" bind:value={form.bic} />
    </label>
    <label>
      Zahlungs-Details (optional)
      <input type="text" bind:value={form.details} placeholder="z. B. paypal.me/…" />
    </label>
    <Toggle bind:checked={form.isDefault} label="Als Standard-Konto" />
    <Toggle bind:checked={form.showOnInvoice} label="Auf Beleg anzeigen" />
    <button class="btn-primary" onclick={create} disabled={busy || !form.label.trim()}>
      {busy ? "…" : "Anlegen"}
    </button>
  </div>
</section>

<label class="chk show-inactive">
  <input type="checkbox" bind:checked={showInactive} onchange={reload} /> Inaktive anzeigen
</label>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if accounts.length === 0}
  <p class="muted">Keine Konten.</p>
{:else}
  <table>
    <thead>
      <tr><th>Bezeichnung</th><th>Typ</th><th>IBAN</th><th>Details</th><th>Auf Beleg</th><th>Standard</th><th>Status</th><th></th></tr>
    </thead>
    <tbody>
      {#each accounts as a (a.id)}
        {#if editId === a.id}
          <tr class="edit-row">
            <td><input type="text" bind:value={editForm.label} /></td>
            <td>
              <select bind:value={editForm.accountType}>
                {#each TYPES as t}<option value={t.value}>{t.label}</option>{/each}
              </select>
            </td>
            <td><input type="text" bind:value={editForm.iban} placeholder="IBAN" /></td>
            <td><input type="text" bind:value={editForm.details} placeholder="z. B. PayPal-Link" /></td>
            <td><input type="checkbox" bind:checked={editForm.showOnInvoice} /></td>
            <td><input type="checkbox" bind:checked={editForm.isDefault} /></td>
            <td class="muted">{a.active === 1 ? "aktiv" : "inaktiv"}</td>
            <td class="row-actions">
              <button class="btn-primary btn-sm" onclick={saveEdit} disabled={busy}>Speichern</button>
              <button class="btn-secondary btn-sm" onclick={() => (editId = null)}>Abbrechen</button>
            </td>
          </tr>
        {:else}
          <tr class={a.active !== 1 ? "inactive" : ""}>
            <td>{a.label}</td>
            <td>{typeLabel(a.accountType)}</td>
            <td class="muted">{a.iban ?? "—"}</td>
            <td class="muted">{a.details ?? "—"}</td>
            <td>{#if a.showOnInvoice === 1}<span class="badge ok">✓</span>{:else}—{/if}</td>
            <td>{#if a.isDefault === 1}<span class="badge ok">Standard</span>{/if}</td>
            <td>{#if a.active === 1}<span class="badge ok">aktiv</span>{:else}<span class="badge muted-badge">inaktiv</span>{/if}</td>
            <td class="row-actions">
              <button class="btn-secondary btn-sm" onclick={() => startEdit(a)} disabled={busy}>Bearbeiten</button>
              <button class="btn-secondary btn-sm" onclick={() => toggleActive(a)} disabled={busy}>
                {a.active === 1 ? "Deaktivieren" : "Aktivieren"}
              </button>
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
{/if}

<style>
  /* .card entfernt — globale Definition aus tokens.css. */
  .grid { display: grid; grid-template-columns: 2fr 1fr 1.5fr 1fr; gap: 0.75rem; align-items: end; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  label.chk { flex-direction: row; align-items: center; gap: 0.4rem; }
  .show-inactive { margin: 0.5rem 0; font-size: 0.85rem; }
  input, select { padding: 0.4rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  table { width: 100%; border-collapse: collapse; background: #fff; }
  th, td { padding: 0.5rem; text-align: left; border-bottom: 1px solid #e5e7eb; font-size: 0.9rem; }
  th { background: #f3f4f6; font-weight: 600; font-size: 0.8rem; }
  tr.inactive { opacity: 0.55; }
  .row-actions { display: flex; gap: 0.4rem; justify-content: flex-end; }
  .badge { padding: 0.1rem 0.5rem; border-radius: 4px; font-size: 0.75rem; }
  .badge.ok { background: #d1fae5; color: #065f46; }
  .muted-badge { background: #e5e7eb; color: #374151; }
  /* R5-003: button/btn-Override-Selektor entfernt — schlug globale tokens.css-
     `.btn-primary`/`.btn-secondary` und Manuel-Hardline „alle Buttons app-weit
     gleich groß" (2026-05-26). Edit-Row-Actions erben jetzt korrekt 9px 16px
     aus tokens.css statt 8px 16px (Border 0 statt 1px). */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
  .muted { color: #6b7280; }
</style>
