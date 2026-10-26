<script lang="ts">
  import { flash } from "#lib/toast.svelte.ts";
  import Toggle from "#lib/Toggle.svelte";
  import { randomId } from "#lib/browser.ts";
  import { tick } from "svelte";
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

  // U1: Kontakte sind Daten Dritter → kein Autofill aus dem eigenen Profil
  // (autocomplete="off", Plan §4.2); Kennungen (USt-IdNr., Steuernummer,
  // IBAN, BIC) in Großbuchstaben ohne Rechtschreibprüfung, mit Formathilfe.
  const uid = randomId();
  const hid = (n: string) => `cf-${n}-${uid}`;
  const isDe = $derived((input.countryCode ?? "").trim().toUpperCase() === "DE");

  // U1 (ADR 0041 Nr. 22): Pflichtfehler am Feld statt als Toast — nach dem ersten
  // Absenden, danach live; Fokus auf das erste fehlerhafte Feld.
  type ReqField = "name" | "street" | "postalCode" | "city" | "countryCode";
  function validate(v: ContactInput): Partial<Record<ReqField, string>> {
    const out: Partial<Record<ReqField, string>> = {};
    if (!v.name.trim()) out.name = "Bitte einen Namen eingeben.";
    if (!v.street.trim()) out.street = "Bitte Straße und Hausnummer eingeben.";
    if (!v.postalCode.trim()) out.postalCode = "Bitte die PLZ eingeben.";
    if (!v.city.trim()) out.city = "Bitte die Stadt eingeben.";
    if (!v.countryCode.trim()) out.countryCode = "Bitte das Land eingeben.";
    return out;
  }
  let submitted = $state(false);
  const errs: Partial<Record<ReqField, string>> = $derived(submitted ? validate(input) : {});
  let formEl: HTMLFormElement | undefined = $state();
  /** aria-describedby aus Hinweis und Fehler (Fehler zuerst). */
  const desc = (...ids: (string | false | undefined)[]) => ids.filter(Boolean).join(" ") || undefined;

  async function handle(e: Event) {
    e.preventDefault();
    submitted = true;
    if (Object.keys(validate(input)).length > 0) {
      await tick();
      formEl?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
      return;
    }
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

<form id={formId} class="kb-form-grid kb-sections" onsubmit={handle} novalidate bind:this={formEl}>
  <p class="legend-req kb-span-all">Felder mit <span class="req" aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
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
    <div class="f">
      <label for={hid("name")}>Name<span class="req" aria-hidden="true">*</span></label>
      <input id={hid("name")} type="text" bind:value={input.name} required aria-required="true" autocomplete="off" enterkeyhint="next"
        aria-invalid={errs.name ? "true" : undefined} aria-describedby={desc(errs.name && hid("name-err"))} />
      {#if errs.name}<span id={hid("name-err")} class="err">{errs.name}</span>{/if}
    </div>
    <label>
      Rechtsform
      <input
        type="text"
        bind:value={input.legalForm}
        autocomplete="off"
        enterkeyhint="next"
        aria-describedby={hid("legal")}
      />
      <small id={hid("legal")} class="hint">z. B. Einzelunternehmen, GmbH</small>
    </label>
  </fieldset>

  <fieldset>
    <legend>Adresse</legend>
    <div class="f">
      <label for={hid("street")}>Straße + Nr.<span class="req" aria-hidden="true">*</span></label>
      <input id={hid("street")} type="text" bind:value={input.street} required aria-required="true" autocomplete="off" enterkeyhint="next"
        aria-invalid={errs.street ? "true" : undefined} aria-describedby={desc(errs.street && hid("street-err"))} />
      {#if errs.street}<span id={hid("street-err")} class="err">{errs.street}</span>{/if}
    </div>
    <div class="row">
      <div class="f postal">
        <label for={hid("postal")}>PLZ<span class="req" aria-hidden="true">*</span></label>
        <input
          id={hid("postal")}
          type="text"
          bind:value={input.postalCode}
          required
          aria-required="true"
          aria-invalid={errs.postalCode ? "true" : undefined}
          autocomplete="off"
          inputmode={isDe ? "numeric" : "text"}
          enterkeyhint="next"
          aria-describedby={desc(errs.postalCode && hid("postal-err"), isDe && hid("plz"))}
        />
        {#if errs.postalCode}<span id={hid("postal-err")} class="err">{errs.postalCode}</span>{/if}
        {#if isDe}<small id={hid("plz")} class="hint">5 Ziffern</small>{/if}
      </div>
      <div class="f city">
        <label for={hid("city")}>Stadt<span class="req" aria-hidden="true">*</span></label>
        <input id={hid("city")} type="text" bind:value={input.city} required aria-required="true" autocomplete="off" enterkeyhint="next"
          aria-invalid={errs.city ? "true" : undefined} aria-describedby={desc(errs.city && hid("city-err"))} />
        {#if errs.city}<span id={hid("city-err")} class="err">{errs.city}</span>{/if}
      </div>
      <div class="f country">
        <label for={hid("country")}>Land<span class="req" aria-hidden="true">*</span></label>
        <input
          id={hid("country")}
          type="text"
          bind:value={input.countryCode}
          maxlength="2"
          required
          aria-required="true"
          autocomplete="off"
          autocapitalize="characters"
          spellcheck="false"
          enterkeyhint="next"
          aria-invalid={errs.countryCode ? "true" : undefined}
          aria-describedby={desc(errs.countryCode && hid("country-err"), hid("land"))}
        />
        {#if errs.countryCode}<span id={hid("country-err")} class="err">{errs.countryCode}</span>{/if}
        <small id={hid("land")} class="hint">2 Buchstaben, z. B. DE</small>
      </div>
    </div>
  </fieldset>

  <fieldset>
    <legend>Steuer</legend>
    <label>
      USt-IdNr.
      <input
        type="text"
        bind:value={input.vatId}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-describedby={hid("vat")}
      />
      <small id={hid("vat")} class="hint">Ländercode und Ziffern, z. B. DE123456789</small>
    </label>
    <label>
      Steuernummer
      <input
        type="text"
        bind:value={input.taxNumber}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-describedby={hid("taxno")}
      />
      <small id={hid("taxno")} class="hint">Wie vom Finanzamt vergeben, z. B. 143/123/45678</small>
    </label>
  </fieldset>

  <fieldset>
    <legend>Kontakt</legend>
    <label>
      E-Mail
      <input
        type="email"
        bind:value={input.email}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        inputmode="email"
        enterkeyhint="next"
      />
    </label>
    <label>
      Telefon
      <input type="tel" bind:value={input.phone} autocomplete="off" inputmode="tel" enterkeyhint="next" />
    </label>
  </fieldset>

  <fieldset>
    <legend>Bank</legend>
    <label>
      IBAN
      <input
        type="text"
        bind:value={input.iban}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-describedby={hid("iban")}
      />
      <small id={hid("iban")} class="hint">z. B. DE89 3704 0044 0532 0130 00 (Leerzeichen erlaubt)</small>
    </label>
    <label>
      BIC
      <input
        type="text"
        bind:value={input.bic}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-describedby={hid("bic")}
      />
      <small id={hid("bic")} class="hint">8 oder 11 Zeichen, z. B. COBADEFFXXX</small>
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
      <textarea rows="3" bind:value={input.notes} autocomplete="off"></textarea>
    </label>
  </fieldset>

  {#if showSubmit}
    <button type="submit" class="btn-primary kb-span-all" disabled={busy} aria-busy={busy}>
      {busy ? "Speichert …" : submitLabel}
    </button>
  {:else}
    <!-- U1: Hauptaktion auch am Formularende (Handy; oben steht sie in der PageBar). -->
    <div class="kb-form-end kb-span-all"><button type="submit" class="btn-primary" disabled={busy} aria-busy={busy}>{busy ? "Speichert …" : submitLabel}</button></div>
  {/if}
</form>

<style>
  /* U2: ab rund 37rem Breite zwei Spalten aus Fieldsets (Allgemein | Adresse, Steuer | Kontakt,
     Bank | Sonstiges; tokens.css .kb-form-grid), darunter einspaltig wie bisher. */
  form { max-width: 64rem; }
  form > button[type="submit"] { justify-self: start; }
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: var(--sp-4);
    margin: 0 0 var(--sp-4);
    min-width: 0;
  }
  legend { font-weight: 600; padding: 0 var(--sp-2); }
  label {
    display: block;
    margin-bottom: var(--sp-3);
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text);
  }
  .chk-wrap { margin-bottom: var(--sp-3); }
  /* U1: Pflichtfelder mit Fehler am Feld (Fehlertext außerhalb des Labels). */
  .f { margin-bottom: var(--sp-3); min-width: 0; }
  .f > label { margin-bottom: 0; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .legend-req { margin: 0 0 var(--sp-3); font-size: var(--fs-sm); color: var(--text-muted); }
  .err { display: block; margin-top: var(--sp-1); font-size: var(--fs-xs); font-weight: 600; color: var(--status-danger-fg); }
  input[type="text"], input[type="email"], input[type="tel"],
  select, textarea {
    display: block;
    width: 100%;
    margin-top: var(--sp-1);
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--line-control);
    border-radius: var(--r-sm);
    font: inherit;
    font-weight: 400;
    font-size: var(--fs-base);
    background: var(--surface-card);
    color: var(--text);
    box-sizing: border-box;
  }
  .hint {
    display: block;
    margin-top: var(--sp-1);
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--text-muted);
  }
  .row { display: flex; flex-wrap: wrap; gap: 0 var(--sp-3); }
  .postal { flex: 0 1 7rem; min-width: 6rem; }
  .city { flex: 1 1 10rem; }
  .country { flex: 0 1 7rem; min-width: 6rem; }
</style>
