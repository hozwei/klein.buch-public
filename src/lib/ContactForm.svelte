<script lang="ts">
  import { flash } from "#lib/toast.svelte.ts";
  import Banner from "#lib/Banner.svelte";
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
    busy = $bindable(false),
  }: {
    initial?: Partial<ContactInput>;
    submitLabel?: string;
    onsubmit: (input: ContactInput) => Promise<void>;
    formId?: string;
    showSubmit?: boolean;
    /** Läuft gerade das Speichern? Bindbar, damit die PageBar den Knopf mitführt (ADR 0041 Nr. 33). */
    busy?: boolean;
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

  // U1: Kontakte sind Daten Dritter → kein Autofill aus dem eigenen Profil
  // (autocomplete="off", Plan §4.2); Kennungen (USt-IdNr., Steuernummer,
  // IBAN, BIC) in Großbuchstaben ohne Rechtschreibprüfung, mit Formathilfe.
  const uid = randomId();
  const hid = (n: string) => `cf-${n}-${uid}`;
  const isDe = $derived((input.countryCode ?? "").trim().toUpperCase() === "DE");

  // U1 (ADR 0041 Nr. 22/38): Fehler am Feld statt als Toast — nach dem ersten
  // Absenden, danach live; Fokus auf das erste fehlerhafte Feld.
  // U4 (V4-04): Die Prüfung bildet ALLE Regeln von `domain::contact::validate` nach
  // (Pflichtfelder, Land aus 2 Buchstaben, deutsche PLZ mit 5 Ziffern, Format der
  // USt-IdNr., E-Mail, IBAN-Länge 15–34) — nur diese, keine strengeren (keine
  // IBAN-Prüfziffer, kein BIC-Format: der Server nimmt beides an). Lehnt der Server
  // trotzdem ab, ordnet `showServerError` die Gründe den Feldern zu; nicht zuordenbare
  // stehen als Banner über dem Formular, technische Fehler bleiben ein Toast.
  type FieldKey = "name" | "street" | "postalCode" | "city" | "countryCode" | "vatId" | "email" | "iban";
  const FIELD_ORDER: FieldKey[] = ["name", "street", "postalCode", "city", "countryCode", "vatId", "email", "iban"];
  const FIELD_IDS: Record<FieldKey, string> = {
    name: "name",
    street: "street",
    postalCode: "postal",
    city: "city",
    countryCode: "country",
    vatId: "vat-input",
    email: "email",
    iban: "iban-input",
  };

  /** Wie `domain::contact::is_valid_email` (local@domain.tld, TLD ≥ 2 Buchstaben oder IDN „xn--…“). */
  function isValidEmail(s: string): boolean {
    if (/\s/.test(s)) return false;
    const parts = s.split("@");
    if (parts.length !== 2) return false;
    const [local, domain] = parts;
    if (!local || !domain || !domain.includes(".")) return false;
    const tld = domain.slice(domain.lastIndexOf(".") + 1);
    const alphabetic = tld.length >= 2 && /^[A-Za-z]+$/.test(tld);
    const idn = tld.length > 4 && tld.toLowerCase().startsWith("xn--") && /^[A-Za-z0-9-]+$/.test(tld);
    return alphabetic || idn;
  }

  /** Wie `domain::contact::validate_vat_id`; null = gültig. */
  function vatIdError(raw: string): string | null {
    const cleaned = raw.replace(/\s/g, "").toUpperCase();
    const euMsg = "Eine USt-IdNr. beginnt mit dem Ländercode und hat danach mindestens 8 Zeichen, z. B. ATU12345678.";
    if (cleaned.length < 4) return euMsg;
    const prefix = cleaned.slice(0, 2);
    const rest = cleaned.slice(2);
    if (!/^[A-Z]{2}$/.test(prefix)) return euMsg;
    if (prefix === "DE") {
      return /^\d{9}$/.test(rest) ? null : "Eine deutsche USt-IdNr. besteht aus „DE“ und 9 Ziffern, z. B. DE123456789.";
    }
    return /^[A-Z0-9]{8,}$/.test(rest) ? null : euMsg;
  }

  function validate(v: ContactInput): Partial<Record<FieldKey, string>> {
    const out: Partial<Record<FieldKey, string>> = {};
    if (!v.name.trim()) out.name = "Bitte gib einen Namen ein.";
    if (!v.street.trim()) out.street = "Bitte gib Straße und Hausnummer ein.";
    const cc = v.countryCode.trim().toUpperCase();
    const plz = v.postalCode.trim();
    if (!plz) out.postalCode = "Bitte gib die PLZ ein.";
    if (!v.city.trim()) out.city = "Bitte gib die Stadt ein.";
    if (!cc) out.countryCode = "Bitte gib das Land als Kürzel ein, z. B. DE.";
    else if (!/^[A-Z]{2}$/.test(cc)) out.countryCode = "Gib das Land mit 2 Buchstaben an, z. B. DE oder AT.";
    else if (cc === "DE" && plz && !/^\d{5}$/.test(plz)) out.postalCode = "Eine deutsche PLZ hat 5 Ziffern, z. B. 10115.";
    const vat = (v.vatId ?? "").trim();
    if (vat) {
      const msg = vatIdError(vat);
      if (msg) out.vatId = msg;
    }
    const email = (v.email ?? "").trim();
    if (email && !isValidEmail(email)) out.email = "Gib die E-Mail-Adresse vollständig an, z. B. name@example.org.";
    const iban = (v.iban ?? "").trim().replace(/\s/g, "");
    if (iban && (iban.length < 15 || iban.length > 34)) {
      out.iban = `Die IBAN hat ${iban.length} Zeichen; gültig sind 15 bis 34 (in Deutschland 22).`;
    }
    return out;
  }

  /** Aktueller Wert eines Felds (für Server-Fehler, die nur bis zur nächsten Änderung gelten). */
  function fieldValue(k: FieldKey): string {
    return String(input[k] ?? "").trim();
  }

  let submitted = $state(false);
  let serverErrs = $state<Partial<Record<FieldKey, { value: string; msg: string }>>>({});
  let serverReasons = $state<string[]>([]);
  let serverBannerEl: HTMLDivElement | undefined = $state();

  const clientErrs: Partial<Record<FieldKey, string>> = $derived(submitted ? validate(input) : {});
  const errs: Partial<Record<FieldKey, string>> = $derived.by(() => {
    const out = { ...clientErrs };
    for (const k of FIELD_ORDER) {
      const s = serverErrs[k];
      if (s && !out[k] && fieldValue(k) === s.value) out[k] = s.msg;
    }
    return out;
  });
  let formEl: HTMLFormElement | undefined = $state();
  /** aria-describedby aus Hinweis und Fehler (Fehler zuerst). */
  const desc = (...ids: (string | false | undefined)[]) => ids.filter(Boolean).join(" ") || undefined;

  /**
   * Zerlegt eine Server-Meldung in einzelne Gründe: „; “-getrennt oder als Folge von
   * Sätzen („E-Mail-Adresse ist ungültig. IBAN ist ungültig (…).“). „z. B.“ und „Nr.“
   * trennen nicht.
   */
  function splitReasons(msg: string): string[] {
    const out: string[] = [];
    for (const part of msg.split(/;\s+/)) {
      let start = 0;
      const re = /[.!?]\s+(?=[A-ZÄÖÜ])/g;
      let m: RegExpExecArray | null;
      while ((m = re.exec(part))) {
        const before = part.slice(start, m.index + 1);
        if (/(?:\bz\.|\bz\. B\.|Nr\.|\bca\.|\bbzw\.)$/.test(before)) continue;
        out.push(before.trim());
        start = m.index + m[0].length;
      }
      const rest = part.slice(start).trim();
      if (rest) out.push(rest);
    }
    return out.filter(Boolean);
  }

  const REASON_FIELD: [FieldKey, RegExp][] = [
    ["name", /^Name\b/],
    ["street", /^Straße/],
    ["postalCode", /^PLZ\b/],
    ["city", /^(Stadt|Ort)\b/],
    ["countryCode", /^Land\b/],
    ["vatId", /^USt-IdNr/],
    ["email", /^E-Mail/],
    ["iban", /^IBAN\b/],
  ];

  async function focusFirstInvalid() {
    await tick();
    formEl?.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
  }

  /** Ordnet eine Ablehnung des Servers zu: Gründe am Feld, Übriges als Banner (ADR 0041 Nr. 22). */
  async function showServerError(err: unknown) {
    const msg = String(err);
    if (/^(Server nicht erreichbar|Fehler \d{3}$)/.test(msg)) {
      flash(msg, "error");
      return;
    }
    const fields: Partial<Record<FieldKey, { value: string; msg: string }>> = {};
    const rest: string[] = [];
    for (const reason of splitReasons(msg)) {
      const hit = REASON_FIELD.find(([, re]) => re.test(reason));
      if (hit && !fields[hit[0]]) fields[hit[0]] = { value: fieldValue(hit[0]), msg: reason };
      else rest.push(reason);
    }
    serverErrs = fields;
    serverReasons = rest;
    if (Object.keys(fields).length > 0) {
      await focusFirstInvalid();
    } else {
      await tick();
      serverBannerEl?.focus();
    }
  }

  async function handle(e: Event) {
    e.preventDefault();
    if (busy) return;
    submitted = true;
    serverErrs = {};
    serverReasons = [];
    if (Object.keys(validate(input)).length > 0) {
      await focusFirstInvalid();
      return;
    }
    busy = true;
    try {
      await onsubmit(input);
    } catch (err) {
      await showServerError(err);
    } finally {
      busy = false;
    }
  }
</script>

<form id={formId} class="kb-form-grid kb-sections" onsubmit={handle} novalidate bind:this={formEl}>
  {#if serverReasons.length > 0}
    <!-- U4 (V4-04): nicht zuordenbare Gründe des Servers bleiben stehen, bis erneut gespeichert wird. -->
    <div class="server-err kb-span-all" tabindex="-1" bind:this={serverBannerEl}>
      <Banner kind="error" title="Der Kontakt wurde nicht gespeichert.">
        {#if serverReasons.length === 1}
          {serverReasons[0]}
        {:else}
          <ul class="reasons">
            {#each serverReasons as r (r)}<li>{r}</li>{/each}
          </ul>
        {/if}
      </Banner>
    </div>
  {/if}
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
    <div class="f">
      <label for={hid("vat-input")}>USt-IdNr.</label>
      <input
        id={hid("vat-input")}
        type="text"
        bind:value={input.vatId}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-invalid={errs.vatId ? "true" : undefined}
        aria-describedby={desc(errs.vatId && hid("vat-err"), hid("vat"))} class="kb-w-medium"
      />
      {#if errs.vatId}<span id={hid("vat-err")} class="err">{errs.vatId}</span>{/if}
      <small id={hid("vat")} class="hint">Ländercode und Ziffern, z. B. DE123456789</small>
    </div>
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
        aria-describedby={hid("taxno")} class="kb-w-medium"
      />
      <small id={hid("taxno")} class="hint">Wie vom Finanzamt vergeben, z. B. 143/123/45678</small>
    </label>
  </fieldset>

  <fieldset>
    <legend>Kontakt</legend>
    <div class="f">
      <label for={hid("email")}>E-Mail</label>
      <input
        id={hid("email")}
        type="email"
        bind:value={input.email}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        inputmode="email"
        enterkeyhint="next"
        aria-invalid={errs.email ? "true" : undefined}
        aria-describedby={desc(errs.email && hid("email-err"))}
      />
      {#if errs.email}<span id={hid("email-err")} class="err">{errs.email}</span>{/if}
    </div>
    <label>
      Telefon
      <input type="tel" class="kb-w-medium" bind:value={input.phone} autocomplete="off" inputmode="tel" enterkeyhint="next" />
    </label>
  </fieldset>

  <fieldset>
    <legend>Bank</legend>
    <div class="f">
      <label for={hid("iban-input")}>IBAN</label>
      <input
        id={hid("iban-input")}
        type="text"
        bind:value={input.iban}
        autocomplete="off"
        autocapitalize="characters"
        spellcheck="false"
        inputmode="text"
        enterkeyhint="next"
        aria-invalid={errs.iban ? "true" : undefined}
        aria-describedby={desc(errs.iban && hid("iban-err"), hid("iban"))} class="kb-w-long"
      />
      {#if errs.iban}<span id={hid("iban-err")} class="err">{errs.iban}</span>{/if}
      <small id={hid("iban")} class="hint">z. B. DE89 3704 0044 0532 0130 00 (Leerzeichen erlaubt)</small>
    </div>
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
        aria-describedby={hid("bic")} class="kb-w-medium"
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

  <!-- U4 (V2-06, ADR 0041 Nr. 33): während des Speicherns aria-disabled statt disabled —
       der Knopf behält den Fokus, ein zweiter Klick bleibt wirkungslos (`handle`). -->
  {#if showSubmit}
    <button type="submit" class="btn-primary kb-span-all" aria-disabled={busy ? "true" : undefined} aria-busy={busy || undefined}>
      {busy ? "Speichert …" : submitLabel}
    </button>
  {:else}
    <!-- U1: Hauptaktion auch am Formularende (Handy; oben steht sie in der PageBar). -->
    <div class="kb-form-end kb-span-all"><button type="submit" class="btn-primary" aria-disabled={busy ? "true" : undefined} aria-busy={busy || undefined}>{busy ? "Speichert …" : submitLabel}</button></div>
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
  .server-err { outline: none; }
  .server-err:focus-visible { outline: var(--focus-width) solid var(--focus-ring); outline-offset: var(--focus-offset); border-radius: var(--radius-control); }
  .reasons { margin: var(--sp-1) 0 0; padding-left: 1.25rem; }
  input[type="text"], input[type="email"], input[type="tel"],
  select, textarea {
    display: block;
    width: 100%;
    margin-top: var(--sp-1);
    padding: 0.5rem 0.6rem;
    border: 1px solid var(--line-control);
    border-radius: var(--radius-control);
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
