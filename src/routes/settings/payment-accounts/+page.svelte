<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import {
    paymentAccountsList,
    paymentAccountsCreate,
    paymentAccountsUpdate,
    paymentAccountsSetActive,
  } from "#lib/api.ts";
  import type { PaymentAccount, PaymentAccountInput, PaymentAccountType } from "#lib/types.ts";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import { iban as fmtIban } from "#lib/format.ts";
  import { flash } from "#lib/toast.svelte.ts";

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
      // Reines Lesen: Standardkonten legt der Server einmalig bei der
      // Einrichtung bzw. beim ersten Entsperren an (Lauf F, D-14).
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

  let labelError = $state("");
  let labelInput: HTMLInputElement | undefined = $state();
  let editLabelError = $state("");

  async function create() {
    labelError = "";
    if (!form.label.trim()) {
      labelError = "Bitte gib eine Bezeichnung ein, z. B. „Geschäftskonto“.";
      labelInput?.focus();
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
    editLabelError = "";
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
    editLabelError = "";
    if (!editForm.label.trim()) {
      editLabelError = "Bitte gib eine Bezeichnung ein.";
      document.getElementById(`pa-edit-label-${editId}`)?.focus();
      return;
    }
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
<p class="lead">
  Deine Konten für Zahlungen — Bankkonto, Bargeld, PayPal und so weiter. Du wählst
  sie bei Kosten und Privat-Buchungen aus. Konten werden nicht gelöscht, sondern
  deaktiviert. Genau ein Konto ist der <strong>Standard</strong>. Konten mit
  <strong>„Auf Beleg anzeigen“</strong> erscheinen auf Angeboten und Rechnungen
  (mehrere möglich) — Bankkonten zusätzlich als Überweisungsweg in der XRechnung.
</p>

<section class="card" aria-labelledby="pa-new-h">
  <h2 id="pa-new-h">Neues Konto</h2>
  <form class="grid" novalidate onsubmit={(e) => { e.preventDefault(); create(); }}>
    <div class="f">
      <label for="pa-label">Bezeichnung<span class="req" aria-hidden="true">*</span> <span class="kb-visually-hidden">(Pflicht)</span></label>
      <input
        id="pa-label"
        bind:this={labelInput}
        type="text"
        autocomplete="off"
        bind:value={form.label}
        required
        aria-required="true"
        aria-invalid={labelError ? "true" : undefined}
        aria-describedby={labelError ? "pa-label-err" : undefined}
        placeholder="z. B. Geschäftskonto Sparkasse"
      />
      {#if labelError}<span id="pa-label-err" class="err">{labelError}</span>{/if}
    </div>
    <div class="f">
      <label for="pa-type">Typ</label>
      <select id="pa-type" bind:value={form.accountType}>
        {#each TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
      </select>
    </div>
    <div class="f">
      <label for="pa-iban">IBAN (optional)</label>
      <input
        id="pa-iban"
        type="text"
        bind:value={form.iban}
        autocomplete="off"
        inputmode="text"
        autocapitalize="characters"
        spellcheck="false"
        aria-describedby="pa-iban-hint"
      />
      <span id="pa-iban-hint" class="hint">z. B. DE89 3704 0044 0532 0130 00 – Leerzeichen sind erlaubt</span>
    </div>
    <div class="f">
      <label for="pa-bic">BIC (optional)</label>
      <input
        id="pa-bic"
        type="text"
        bind:value={form.bic}
        autocomplete="off"
        inputmode="text"
        autocapitalize="characters"
        spellcheck="false"
        aria-describedby="pa-bic-hint"
      />
      <span id="pa-bic-hint" class="hint">8 oder 11 Zeichen, z. B. COBADEFFXXX</span>
    </div>
    <div class="f wide">
      <label for="pa-details">Zahlungs-Details (optional)</label>
      <input id="pa-details" type="text" autocomplete="off" bind:value={form.details} aria-describedby="pa-details-hint" />
      <span id="pa-details-hint" class="hint">z. B. paypal.me/deinname</span>
    </div>
    <Toggle bind:checked={form.isDefault} label="Als Standard-Konto" />
    <Toggle bind:checked={form.showOnInvoice} label="Auf Beleg anzeigen" />
    <div class="submit">
      <Button type="submit" disabled={busy}>{busy ? "Lege an …" : "Anlegen"}</Button>
    </div>
  </form>

</section>

<label class="chk show-inactive">
  <input type="checkbox" bind:checked={showInactive} onchange={reload} /> Inaktive anzeigen
</label>

{#if loading}
  <p class="muted" aria-busy="true">Lädt …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if accounts.length === 0}
  <p class="muted">Noch keine Konten. Lege oben unter „Neues Konto“ dein erstes an.</p>
{:else}
  <table class="cards kb-card-grid">
    <caption class="kb-visually-hidden">Deine Konten</caption>
    <thead>
      <tr><th>Bezeichnung</th><th class="kb-col-opt2">Typ</th><th>IBAN / BIC</th><th class="kb-col-opt1">Details</th><th>Auf Beleg</th><th>Standard</th><th>Status</th><th><span class="kb-visually-hidden">Aktionen</span></th></tr>
    </thead>
    <tbody>
      {#each accounts as a (a.id)}
        {#if editId === a.id}
          <tr class="edit-row">
            <td class="cards-full" data-label="Bezeichnung">
              <input
                id="pa-edit-label-{a.id}"
                type="text"
                autocomplete="off"
                bind:value={editForm.label}
                aria-label="Bezeichnung, Konto {a.label}"
                aria-required="true"
                aria-invalid={editLabelError ? "true" : undefined}
                aria-describedby={editLabelError ? `pa-edit-err-${a.id}` : undefined}
              />
              {#if editLabelError}<span id="pa-edit-err-{a.id}" class="err">{editLabelError}</span>{/if}
            </td>
            <td class="cards-full" data-label="Typ">
              <select bind:value={editForm.accountType} aria-label="Typ, Konto {a.label}">
                {#each TYPES as t (t.value)}<option value={t.value}>{t.label}</option>{/each}
              </select>
            </td>
            <td class="cards-full" data-label="IBAN / BIC">
              <input
                type="text"
                bind:value={editForm.iban}
                aria-label="IBAN, Konto {a.label}"
                autocomplete="off"
                inputmode="text"
                autocapitalize="characters"
                spellcheck="false"
              />
              <input
                class="bic"
                type="text"
                bind:value={editForm.bic}
                aria-label="BIC, Konto {a.label}"
                autocomplete="off"
                inputmode="text"
                autocapitalize="characters"
                spellcheck="false"
              />
            </td>
            <td class="cards-full" data-label="Details">
              <input type="text" autocomplete="off" bind:value={editForm.details} aria-label="Zahlungs-Details, Konto {a.label}" />
            </td>
            <td data-label="Auf Beleg">
              <label class="cell-chk">
                <input type="checkbox" bind:checked={editForm.showOnInvoice} />
                <span class="kb-visually-hidden">Auf Beleg anzeigen, Konto {a.label}</span>
              </label>
            </td>
            <td data-label="Standard">
              <label class="cell-chk">
                <input type="checkbox" bind:checked={editForm.isDefault} />
                <span class="kb-visually-hidden">Standard-Konto, Konto {a.label}</span>
              </label>
            </td>
            <td class="muted" data-label="Status">{a.active === 1 ? "aktiv" : "inaktiv"}</td>
            <td>
              <div class="row-actions">
                <Button onclick={saveEdit} disabled={busy} ariaLabel="Speichern, Konto {a.label}">Speichern</Button>
                <Button variant="secondary" onclick={() => (editId = null)} ariaLabel="Abbrechen, Konto {a.label}">Abbrechen</Button>
              </div>
            </td>
          </tr>
        {:else}
          <tr class:inactive={a.active !== 1}>
            <td data-label="Bezeichnung"><strong>{a.label}</strong></td>
            <td class="kb-col-opt2" data-label="Typ">{typeLabel(a.accountType)}</td>
            <td class="muted" data-label="IBAN / BIC">
              {#if a.iban || a.bic}
                {#if a.iban}<span class="mono">{fmtIban(a.iban)}</span>{/if}
                {#if a.bic}<span class="mono bic-show">{a.bic}</span>{/if}
              {:else}—{/if}
            </td>
            <td class="muted kb-col-opt1" data-label="Details">{a.details ?? "—"}</td>
            <td data-label="Auf Beleg">{#if a.showOnInvoice === 1}<Badge tone="success">ja</Badge>{:else}nein{/if}</td>
            <td data-label="Standard">{#if a.isDefault === 1}<Badge tone="primary">Standard</Badge>{:else}—{/if}</td>
            <td data-label="Status">{#if a.active === 1}<Badge tone="success">aktiv</Badge>{:else}<Badge>inaktiv</Badge>{/if}</td>
            <td>
              <div class="row-actions">
                <Button variant="secondary" onclick={() => startEdit(a)} disabled={busy} ariaLabel="Bearbeiten, Konto {a.label}">Bearbeiten</Button>
                <Button variant="secondary" onclick={() => toggleActive(a)} disabled={busy} ariaLabel="{a.active === 1 ? 'Deaktivieren' : 'Aktivieren'}, Konto {a.label}">
                  {a.active === 1 ? "Deaktivieren" : "Aktivieren"}
                </Button>
              </div>
            </td>
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
{/if}

<style>
  /* .card / .lead — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 2fr 1fr 1.5fr 1fr; gap: 0.75rem; align-items: start; }
  .grid .wide { grid-column: span 2; }
  .f { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .f > label { font-weight: 600; }
  .grid .submit { display: flex; align-items: flex-end; }
  label { display: flex; flex-direction: column; font-size: var(--fs-sm); font-weight: 600; color: var(--text); gap: 0.25rem; min-width: 0; }
  label.chk { flex-direction: row; align-items: center; gap: 0.6rem; font-weight: 400; }
  .show-inactive { margin: 0.5rem 0; min-height: var(--target); width: fit-content; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .hint { font-weight: 400; color: var(--text-muted); font-size: var(--fs-xs); }
  .err { font-weight: 600; color: var(--status-danger-fg); font-size: var(--fs-sm); display: block; }
  input:not([type="checkbox"]), select { padding: 0.4rem 0.5rem; border: 1px solid var(--line-control); border-radius: var(--r-sm); font: inherit; font-weight: 400; min-height: var(--target); background: var(--surface-card); }
  table { width: 100%; border-collapse: collapse; background: var(--surface-card); }
  th, td { text-align: left; font-size: var(--fs-sm); }
  tr.inactive td { color: var(--text-muted); }
  .edit-row input:not([type="checkbox"]), .edit-row select { width: 100%; }
  .edit-row input.bic { margin-top: 0.35rem; }
  .cell-chk { display: inline-flex; flex-direction: row; align-items: center; justify-content: center; min-width: var(--target); min-height: var(--target); }
  .mono { font-family: var(--font-mono); white-space: nowrap; }
  .bic-show { display: block; font-size: var(--fs-xs); }
  .row-actions { display: flex; flex-wrap: wrap; gap: 0.4rem; justify-content: flex-end; }
  .muted { color: var(--text-muted); }
  @media (max-width: 900px) {
    .grid { grid-template-columns: 1fr 1fr; }
  }
  @media (max-width: 600px) {
    .grid { grid-template-columns: 1fr; }
    .grid .wide { grid-column: auto; }
  }
  @media (max-width: 820px) {
    td { overflow-wrap: anywhere; }
    .mono { white-space: normal; overflow-wrap: anywhere; }
  }
</style>
