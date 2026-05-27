<script lang="ts">
  import { flash } from "$lib/toast.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import type { ContactInput, ContactType } from "./types";

  let {
    initial,
    submitLabel = "Speichern",
    onsubmit,
    formId,
    showSubmit = true,
  }: {
    initial?: Partial<ContactInput>;
    submitLabel?: string;
    onsubmit: (input: ContactInput) => Promise<void>;
    formId?: string;
    showSubmit?: boolean;
  } = $props();

  // Initial-Werte werden EINMAL bei Mount übernommen — danach editierbar.
  // Extraktion in eine Funktion, damit Svelte 5 die `initial`-Reads nicht
  // als "state_referenced_locally" markiert.
  function buildInitial(src: Partial<ContactInput> | undefined): ContactInput {
    return {
      contactType: (src?.contactType as ContactType) ?? "customer",
      name: src?.name ?? "",
      legalForm: src?.legalForm ?? null,
      vatId: src?.vatId ?? null,
      taxNumber: src?.taxNumber ?? null,
      street: src?.street ?? "",
      postalCode: src?.postalCode ?? "",
      city: src?.city ?? "",
      countryCode: src?.countryCode ?? "DE",
      email: src?.email ?? null,
      phone: src?.phone ?? null,
      iban: src?.iban ?? null,
      bic: src?.bic ?? null,
      acceptsEinvoice: src?.acceptsEinvoice ?? true,
      notes: src?.notes ?? null,
    };
  }

  // Bewusst Einmal-Snapshot von `initial` beim Mount; Form ist danach lokal
  // editierbar und resynced nicht auf Prop-Changes (das wäre für ein
  // Edit-Form falsch — der User würde seine Edits verlieren).
  // svelte-ignore state_referenced_locally
  let input: ContactInput = $state(buildInitial(initial));

  let busy = $state(false);

  async function handle(e: Event) {
    e.preventDefault();
    if (!input.name.trim()) { flash("Name ist Pflicht.", "error"); return; }
    if (!input.street.trim()) { flash("Straße ist Pflicht.", "error"); return; }
    if (!input.postalCode.trim()) { flash("PLZ ist Pflicht.", "error"); return; }
    if (!input.city.trim()) { flash("Stadt ist Pflicht.", "error"); return; }
    if (!input.countryCode.trim()) { flash("Land ist Pflicht.", "error"); return; }
    busy = true;
    try {
      await onsubmit(input);
    } catch (e) {
      flash(String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<form id={formId} onsubmit={handle} novalidate>
  <fieldset>
    <legend>Allgemein</legend>
    <label>
      Typ
      <select bind:value={input.contactType}>
        <option value="customer">Kunde</option>
        <option value="vendor">Lieferant</option>
        <option value="both">Kunde + Lieferant</option>
        <option value="partner">Partner</option>
      </select>
    </label>
    <label>
      Name *
      <input type="text" bind:value={input.name} required />
    </label>
    <label>
      Rechtsform
      <input type="text" bind:value={input.legalForm} placeholder="z. B. Einzelunternehmen, GmbH" />
    </label>
  </fieldset>

  <fieldset>
    <legend>Adresse</legend>
    <label>
      Straße + Nr. *
      <input type="text" bind:value={input.street} required />
    </label>
    <div class="row">
      <label class="postal">
        PLZ *
        <input type="text" bind:value={input.postalCode} required />
      </label>
      <label class="city">
        Stadt *
        <input type="text" bind:value={input.city} required />
      </label>
      <label class="country">
        Land *
        <input type="text" bind:value={input.countryCode} maxlength="2" required />
      </label>
    </div>
  </fieldset>

  <fieldset>
    <legend>Steuer</legend>
    <label>
      USt-IdNr. (z. B. DE123456789)
      <input type="text" bind:value={input.vatId} />
    </label>
    <label>
      Steuernummer
      <input type="text" bind:value={input.taxNumber} />
    </label>
  </fieldset>

  <fieldset>
    <legend>Kontakt</legend>
    <label>
      E-Mail
      <input type="email" bind:value={input.email} />
    </label>
    <label>
      Telefon
      <input type="tel" bind:value={input.phone} />
    </label>
  </fieldset>

  <fieldset>
    <legend>Bank</legend>
    <label>
      IBAN
      <input type="text" bind:value={input.iban} />
    </label>
    <label>
      BIC
      <input type="text" bind:value={input.bic} />
    </label>
  </fieldset>

  <fieldset>
    <legend>Sonstiges</legend>
    <div class="chk-wrap">
      <Toggle
        bind:checked={input.acceptsEinvoice}
        label="Kann elektronische Rechnungen (E-Rechnung) empfangen"
      />
    </div>
    <label>
      Notizen
      <textarea rows="3" bind:value={input.notes}></textarea>
    </label>
  </fieldset>

  {#if showSubmit}
    <button type="submit" disabled={busy}>{busy ? "…" : submitLabel}</button>
  {/if}
</form>

<style>
  form { max-width: 720px; }
  fieldset {
    border: 1px solid #e5e7eb;
    border-radius: 4px;
    padding: 1rem;
    margin: 0 0 1rem;
  }
  legend { font-weight: 600; padding: 0 0.4rem; }
  label {
    display: block;
    margin-bottom: 0.6rem;
    font-size: 0.9rem;
    color: #374151;
  }
  .chk-wrap { margin-bottom: 0.6rem; }
  input[type="text"], input[type="email"], input[type="tel"],
  select, textarea {
    width: 100%;
    padding: 0.45rem;
    border: 1px solid #d1d5db;
    border-radius: 4px;
    font-size: 0.95rem;
    box-sizing: border-box;
  }
  .row { display: flex; gap: 0.75rem; }
  .postal { flex: 0 0 7rem; }
  .city { flex: 1; }
  .country { flex: 0 0 5rem; }
  button {
    /* R5-012: Tailwind-Blau (#2563eb) durch Petrol-Marke ersetzt
       (Memory `feedback_design_direction`). */
    background: var(--c-primary-600);
    color: #fff;
    border: 0;
    padding: 0.6rem 1.2rem;
    border-radius: 4px;
    font-size: 1rem;
    cursor: pointer;
  }
  button:hover:not([disabled]) { background: var(--c-primary-700); }
  button[disabled] { opacity: 0.5; cursor: wait; }
</style>
