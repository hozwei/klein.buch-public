<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { safeNextPath } from "#lib/nextPath.ts";
  import PageBar from "#lib/PageBar.svelte";
  import RequiredLegend from "#lib/RequiredLegend.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Banner from "#lib/Banner.svelte";
  import Button from "#lib/Button.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import TaxStatusSection from "./TaxStatusSection.svelte";
  import {
    sellerProfileGet,
    sellerProfileUpsert,
    sellerOwnerGet,
    sellerOwnerSet,
    documentTermsGet,
    documentTermsSet,
  } from "#lib/api.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import type {
    SellerProfile,
    SellerProfileInput,
    TaxStatus,
  } from "#lib/types.ts";

  // ADR 0042 Nr. 1: Der Steuerstatus (§19) ist kein Feld der Firmendaten mehr,
  // sondern eine Historie mit Grund und Wirkungsdatum (Abschnitt „Steuerstatus“,
  // TaxStatusSection). Der Schalter und der §19-Wechseldialog (ADR 0041 Nr. 21/220/251)
  // sind entfallen; `seller_profile_upsert` ändert den Status nicht.
  let profile = $state<SellerProfile | null>(null);
  let loading = $state(true);
  /** U4 (V2-07): Nach einem Ladefehler kein leeres Formular (Speichern hätte die Firmendaten überschrieben). */
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  /** Nach dem ersten Speicherversuch: fehlende Angaben fürs Ausstellen am Feld zeigen. */
  let triedSave = $state(false);

  let input: SellerProfileInput = $state(blankInput());
  let ownerName = $state("");
  let quoteValidDays = $state<number | null>(30);
  let invoiceDueDays = $state<number | null>(14);
  let quoteDaysInput: QuantityInput | undefined = $state();
  let dueDaysInput: QuantityInput | undefined = $state();
  // U6-005 (V1-10): Rücksprung nach dem Speichern, z. B. zum Rechnungs-Editor
  // (`?next=/invoices/new`); nur Pfade innerhalb der App (`safeNextPath`).
  const nextPath = $derived(safeNextPath(page.url.searchParams.get("next")));

  // U1: Pflichtfelder mit Fehlermeldung am Feld statt Toast (3.3.1).
  type FieldKey = "name" | "street" | "postalCode" | "city" | "countryCode" | "email";
  let errors = $state<Partial<Record<FieldKey, string>>>({});
  const isDE = $derived(input.countryCode.trim().toUpperCase() === "DE");

  // U4 (V1-04, ADR 0041 Nr. 190): Telefon (XRechnung BR-DE-6) und Steuernummer/USt-IdNr.
  // (§14 Abs. 4 UStG, BR-CO-26) prüft der Server erst beim Ausstellen; speichern lassen sich die
  // Firmendaten auch ohne sie (eine neue Firma hat oft noch keine Steuernummer). Deshalb kein `*`
  // (das heißt „Pflicht zum Speichern“), sondern die Kennzeichnung „Pflicht für die E-Rechnung“ und
  // ein Hinweis am Feld, sobald gespeicherte oder abgeschickte Daten sie nicht enthalten.
  const phoneMissing = $derived(!(input.phone ?? "").trim());
  const taxIdMissing = $derived(!(input.taxNumber ?? "").trim() && !(input.vatId ?? "").trim());
  const showIssueHints = $derived(triedSave || profile !== null);

  function blankInput(): SellerProfileInput {
    return {
      name: "",
      legalForm: null,
      street: "",
      postalCode: "",
      city: "",
      countryCode: "DE",
      taxNumber: null,
      vatId: null,
      email: "",
      phone: null,
      iban: null,
      bic: null,
      logoFilename: null,
      defaultPdfTemplate: "default",
      defaultCurrency: "EUR",
    };
  }

  function fromProfile(p: SellerProfile): SellerProfileInput {
    return {
      name: p.name,
      legalForm: p.legalForm,
      street: p.street,
      postalCode: p.postalCode,
      city: p.city,
      countryCode: p.countryCode,
      taxNumber: p.taxNumber,
      vatId: p.vatId,
      email: p.email,
      phone: p.phone,
      iban: p.iban,
      bic: p.bic,
      logoFilename: p.logoFilename,
      defaultPdfTemplate: p.defaultPdfTemplate,
      defaultCurrency: p.defaultCurrency,
    };
  }

  async function load() {
    loading = true;
    loadError = null;
    try {
      profile = await sellerProfileGet();
      if (profile) input = fromProfile(profile);
      ownerName = (await sellerOwnerGet()) ?? "";
      const terms = await documentTermsGet();
      quoteValidDays = terms.quoteValidDays;
      invoiceDueDays = terms.invoiceDueDays;
    } catch (e) {
      loadError = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  /** Status heute aus dem Abschnitt „Steuerstatus“ (nach jedem Laden, auch nach einem
   *  Eintrag); bis dahin aus den Firmendaten. */
  let currentStatus = $state<TaxStatus | null>(null);
  const statusToday = $derived<TaxStatus>(
    currentStatus ?? (profile?.isKleinunternehmer === 0 ? "regelbesteuerung" : "kleinunternehmer"),
  );

  async function handleSubmit(e: Event) {
    e.preventDefault();
    if (saving) return;
    triedSave = true;

    const errs: Partial<Record<FieldKey, string>> = {};
    if (!input.name.trim()) errs.name = "Bitte gib den Namen deiner Firma ein.";
    if (!input.street.trim()) errs.street = "Bitte gib Straße und Hausnummer ein.";
    if (!input.postalCode.trim()) errs.postalCode = "Bitte gib die PLZ ein.";
    if (!input.city.trim()) errs.city = "Bitte gib den Ort ein.";
    if (!input.countryCode.trim()) errs.countryCode = "Bitte gib das Land als Kürzel ein, z. B. DE.";
    if (!input.email.trim()) errs.email = "Bitte gib deine E-Mail-Adresse ein.";
    errors = errs;
    const order: FieldKey[] = ["name", "street", "postalCode", "city", "countryCode", "email"];
    const first = order.find((k) => errs[k]);
    if (first) {
      document.getElementById(`seller-${first}`)?.focus();
      return;
    }
    if (quoteDaysInput && !quoteDaysInput.validate()) { quoteDaysInput.focus(); return; }
    if (dueDaysInput && !dueDaysInput.validate()) { dueDaysInput.focus(); return; }

    await save(input);
  }

  async function save(payload: SellerProfileInput) {
    if (saving) return;
    saving = true;
    let saved = false;
    try {
      profile = await sellerProfileUpsert(payload);
      await sellerOwnerSet(ownerName);
      // Leere Tage-Felder: bisherige Werte behalten (wie vorher bei 0 nicht möglich).
      await documentTermsSet(quoteValidDays ?? 30, invoiceDueDays ?? 14);
      input = fromProfile(profile);
      const missing = [
        phoneMissing ? "Telefon" : "",
        taxIdMissing ? "Steuernummer oder USt-IdNr." : "",
      ].filter(Boolean);
      if (missing.length > 0) {
        flash(
          `Firmendaten gespeichert. Für die E-Rechnung ${missing.length === 1 ? "fehlt" : "fehlen"} noch: ${missing.join(" und ")} — ohne ${missing.length === 1 ? "diese Angabe" : "diese Angaben"} kannst du keine Rechnung ausstellen.`,
          "warning",
        );
      } else {
        flash("Firmendaten gespeichert.");
      }
      saved = true;
    } catch (e) {
      flash(`Firmendaten speichern fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      saving = false;
    }
    // Rücksprung erst nach dem Speichern; ein Navigationsfehler ist kein Speicherfehler.
    if (saved && nextPath) await goto(nextPath);
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Meine Firmendaten">
  {#snippet actions()}
    {#if !loading && !loadError}
      <Button variant="secondary" href={nextPath ?? "/settings"}>Abbrechen</Button>
      <Button type="submit" form="seller-form" busy={saving}>{saving ? "Speichert …" : "Speichern"}</Button>
    {/if}
  {/snippet}
</PageBar>
<p class="lead">Diese Angaben stehen auf deinen Rechnungen und Angeboten.</p>

{#snippet err(k: FieldKey)}
  {#if errors[k]}<span id="seller-{k}-err" class="err">{errors[k]}</span>{/if}
{/snippet}

{#if loading}
  <LoadingState />
{:else if loadError}
  <Banner title="Die Firmendaten konnten nicht geladen werden.">{loadError}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else}
  <!-- U2: ab rund 37rem Breite zwei Spalten (Firma | Adresse, Steuer über die volle Breite,
       Kontakt | Standard-Einstellungen; tokens.css .kb-form-grid). -->
  <form id="seller-form" class="kb-form-grid kb-sections" onsubmit={handleSubmit} novalidate>
    <RequiredLegend class="kb-span-all" />
    <fieldset>
      <legend>Firma</legend>
      <div class="f">
        <label for="seller-name">Name<span class="req" aria-hidden="true">*</span></label>
        <input id="seller-name" type="text" bind:value={input.name} autocomplete="organization" required aria-required="true"
          aria-invalid={errors.name ? "true" : undefined} aria-describedby={errors.name ? "seller-name-err" : undefined} />
        {@render err("name")}
      </div>
      <div class="f">
        <label for="seller-legal">Rechtsform</label>
        <input id="seller-legal" type="text" bind:value={input.legalForm} autocomplete="off" placeholder="z. B. Einzelunternehmen, GmbH" />
      </div>
      <div class="f">
        <label for="seller-owner">Inhaber (Vor- und Nachname)</label>
        <input id="seller-owner" type="text" bind:value={ownerName} autocomplete="name" aria-describedby="seller-owner-hint" />
        <small id="seller-owner-hint" class="hint-inline">
          Bei Einzelunternehmern/Freiberuflern Pflicht — erscheint auf Angebot und
          Rechnung unter dem Firmennamen. Leer lassen, wenn der Name oben schon dein
          vollständiger Name ist.
        </small>
      </div>
    </fieldset>

    <fieldset>
      <legend>Adresse</legend>
      <div class="f">
        <label for="seller-street">Straße und Hausnummer<span class="req" aria-hidden="true">*</span></label>
        <input id="seller-street" type="text" bind:value={input.street} autocomplete="street-address" required aria-required="true"
          aria-invalid={errors.street ? "true" : undefined} aria-describedby={errors.street ? "seller-street-err" : undefined} />
        {@render err("street")}
      </div>
      <div class="row">
        <div class="f postal">
          <label for="seller-postalCode">PLZ<span class="req" aria-hidden="true">*</span></label>
          <input id="seller-postalCode" type="text" bind:value={input.postalCode} autocomplete="postal-code"
            inputmode={isDE ? "numeric" : "text"} required aria-required="true"
            aria-invalid={errors.postalCode ? "true" : undefined}
            aria-describedby={errors.postalCode ? "seller-postalCode-err" : isDE ? "seller-postal-hint" : undefined} />
          {#if errors.postalCode}{@render err("postalCode")}{:else if isDE}<small id="seller-postal-hint" class="hint-inline">5 Ziffern</small>{/if}
        </div>
        <div class="f city">
          <label for="seller-city">Ort<span class="req" aria-hidden="true">*</span></label>
          <input id="seller-city" type="text" bind:value={input.city} autocomplete="address-level2" required aria-required="true"
            aria-invalid={errors.city ? "true" : undefined} aria-describedby={errors.city ? "seller-city-err" : undefined} />
          {@render err("city")}
        </div>
        <div class="f country">
          <label for="seller-countryCode">Land<span class="req" aria-hidden="true">*</span></label>
          <input id="seller-countryCode" type="text" bind:value={input.countryCode} autocomplete="country" maxlength="2"
            autocapitalize="characters" spellcheck="false" required aria-required="true"
            aria-invalid={errors.countryCode ? "true" : undefined}
            aria-describedby={errors.countryCode ? "seller-countryCode-err" : "seller-country-hint"} />
          {#if errors.countryCode}{@render err("countryCode")}{:else}<small id="seller-country-hint" class="hint-inline">Kürzel, z. B. DE</small>{/if}
        </div>
      </div>
    </fieldset>

    <fieldset class="kb-span-all">
      <legend>Steuer</legend>
      <div class="kb-form-grid">
        <div class="f">
          <label for="seller-taxno">Steuernummer</label>
          <input id="seller-taxno" class="kb-w-medium" type="text" bind:value={input.taxNumber} autocomplete="off" inputmode="text"
            autocapitalize="characters" spellcheck="false"
            aria-describedby={showIssueHints && taxIdMissing ? "seller-taxno-hint seller-taxid-warn" : "seller-taxno-hint"} />
          <small id="seller-taxno-hint" class="hint-inline">
            Format wie vom Finanzamt, z. B. 132/456/7890. Steht auf jeder Rechnung. Noch keine
            vom Finanzamt? Dann leer lassen — du kannst die Firmendaten trotzdem speichern.
            Rechnungen schreiben geht erst, wenn hier eine Steuernummer oder eine USt-IdNr. steht.
          </small>
        </div>
        <div class="f">
          <label for="seller-vatid">USt-IdNr.</label>
          <input id="seller-vatid" class="kb-w-medium" type="text" bind:value={input.vatId} autocomplete="off" inputmode="text"
            autocapitalize="characters" spellcheck="false"
            aria-describedby={showIssueHints && taxIdMissing ? "seller-vatid-hint seller-taxid-warn" : "seller-vatid-hint"} />
          <small id="seller-vatid-hint" class="hint-inline">In Deutschland „DE“ und 9 Ziffern, z. B. DE123456789.</small>
        </div>
        {#if showIssueHints && taxIdMissing}
          <p id="seller-taxid-warn" class="warn kb-span-all">
            Noch fehlt: Steuernummer oder USt-IdNr. Ohne eine davon kannst du keine Rechnung ausstellen.
          </p>
        {/if}
      </div>

      <p class="p19">
        <strong>Steuerstatus heute:</strong>
        {statusToday === "regelbesteuerung" ? "Regelbesteuerung" : "Kleinunternehmer nach §19 UStG"}.
        Wechsel (Verzicht, Wegfall, Rückkehr) trägst du mit Grund und Wirkungsdatum im Abschnitt
        <a class="kb-link" href="#steuerstatus">Steuerstatus</a> unten ein.
      </p>
    </fieldset>

    <fieldset>
      <legend>Kontakt</legend>
      <div class="f">
        <label for="seller-email">E-Mail<span class="req" aria-hidden="true">*</span></label>
        <input id="seller-email" type="email" bind:value={input.email} autocomplete="email" autocapitalize="off" spellcheck="false"
          required aria-required="true"
          aria-invalid={errors.email ? "true" : undefined} aria-describedby={errors.email ? "seller-email-err" : undefined} />
        {@render err("email")}
      </div>
      <div class="f">
        <!-- U4 (V1-04): Pflicht fürs Ausstellen, nicht fürs Speichern — darum kein `*`. -->
        <label for="seller-phone">Telefon <span class="issue-req">(Pflicht für die E-Rechnung)</span></label>
        <input id="seller-phone" class="kb-w-medium" type="tel" bind:value={input.phone} autocomplete="tel"
          aria-describedby={showIssueHints && phoneMissing ? "seller-phone-warn seller-phone-hint" : "seller-phone-hint"} />
        {#if showIssueHints && phoneMissing}
          <span id="seller-phone-warn" class="warn">Noch fehlt die Telefonnummer. Ohne sie kannst du keine Rechnung ausstellen.</span>
        {/if}
        <small id="seller-phone-hint" class="hint-inline">
          z. B. +49 30 1234567. Die E-Rechnung (XRechnung) verlangt eine Telefonnummer des
          Verkäufers; speichern kannst du die Firmendaten auch ohne.
        </small>
      </div>
    </fieldset>

    <fieldset>
      <legend>Standard-Einstellungen</legend>
      <!-- U4 (V1-04): Währung als Auswahl statt Freitext — die E-Rechnung nimmt nur EUR an
           (`CurrencyUnsupported`); ein abweichender Altwert bleibt als Option sichtbar. -->
      <div class="f narrow">
        <label for="seller-currency">Währung</label>
        <select id="seller-currency" class="kb-input kb-w-medium" bind:value={input.defaultCurrency} aria-describedby="seller-currency-hint">
          <option value="EUR">Euro (EUR)</option>
          {#if input.defaultCurrency !== "EUR"}<option value={input.defaultCurrency}>{input.defaultCurrency}</option>{/if}
        </select>
        <small id="seller-currency-hint" class="hint-inline">Klein.Buch rechnet in Euro.</small>
      </div>
      <div class="days">
        <QuantityInput
          bind:this={quoteDaysInput}
          bind:value={quoteValidDays}
          fieldClass="kb-w-short"
          label="Angebot gültig"
          integer
          grouping={false}
          unit="Tage"
          noun="Anzahl Tage"
          max={3650}
          required
          hint="Füllt „Gültig bis“ beim neuen Angebot automatisch vor."
        />
        <QuantityInput
          bind:this={dueDaysInput}
          bind:value={invoiceDueDays}
          fieldClass="kb-w-short"
          label="Zahlungsziel Rechnung"
          integer
          grouping={false}
          unit="Tage"
          noun="Anzahl Tage"
          max={3650}
          required
          hint="Füllt „Fällig am“ bei der neuen Rechnung automatisch vor."
        />
      </div>
      <p class="kb-muted crossref">
        Bankverbindung und weitere Zahlwege pflegst du unter
        <a href="/settings/payment-accounts">Einstellungen → Konten</a>. Logo, Unterschrift
        und PDF-Vorlage unter <a href="/settings/pdf-templates">Einstellungen → Rechnungs-Layout</a>.
      </p>
    </fieldset>

    <!-- U1: Hauptaktion auch am Formularende (Handy, Daumen; PageBar klebt dort nicht). -->
    <div class="kb-form-end kb-span-all"><Button type="submit" busy={saving}>{saving ? "Speichert …" : "Speichern"}</Button></div>
  </form>

  <!-- ADR 0042 Nr. 1–6: eigener Abschnitt außerhalb des Formulars; Einträge gelten sofort
       (mit Rückfrage und Re-Auth), nicht erst mit „Speichern“. -->
  <TaxStatusSection onStatus={(s) => (currentStatus = s)} />
{/if}

<style>
  /* .lead / .caveat / .kb-dialog* — globale Definitionen aus tokens.css. */
  form { max-width: 64rem; }
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 1rem;
    margin: 0 0 1rem;
    min-width: 0;
  }
  legend { font-weight: 600; padding: 0 0.4rem; }
  .req { color: var(--status-danger-fg); margin-left: 2px; }
  .f { display: flex; flex-direction: column; gap: 0.25rem; margin-bottom: 0.75rem; min-width: 0; }
  .f > label { font-size: var(--fs-sm); font-weight: 600; color: var(--text); }
  .f.narrow { max-width: 10rem; }
  input[type="text"], input[type="email"], input[type="tel"] {
    width: 100%; padding: 0.45rem; border: 1px solid var(--line-control);
    border-radius: var(--r-sm); font: inherit; box-sizing: border-box;
    min-height: var(--target); background: var(--surface-card); color: var(--text);
  }
  .err { font-size: var(--fs-sm); font-weight: 600; color: var(--status-danger-fg); }
  .row { display: flex; gap: 0.75rem; flex-wrap: wrap; }
  .postal { flex: 0 0 7rem; }
  .city { flex: 1 1 12rem; }
  .country { flex: 0 0 7rem; }
  .days { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(14rem, 100%), 1fr)); gap: 0 0.75rem; }
  .warn { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--status-warning-fg); }
  .issue-req { font-weight: 400; color: var(--text-muted); }
  .hint-inline {
    display: block;
    font-size: var(--fs-sm);
    color: var(--text-muted);
  }
  .crossref { margin: 0.4rem 0 0; font-size: var(--fs-sm); }
  .crossref a { text-decoration: underline; }
  .p19 { max-width: 48rem; margin: 1rem 0 0; font-size: var(--fs-sm); }
</style>
