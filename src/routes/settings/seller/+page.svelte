<script lang="ts">
  import { onMount } from "svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import Button from "#lib/Button.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { focusTrap } from "#lib/focusTrap.ts";
  import { portal } from "#lib/portal.ts";
  import { pushModal, popModal } from "#lib/modalStack.svelte.ts";
  import { date } from "#lib/format.ts";
  import {
    sellerProfileGet,
    sellerProfileUpsert,
    paragraph19Info,
    sellerOwnerGet,
    sellerOwnerSet,
    documentTermsGet,
    documentTermsSet,
  } from "#lib/api.ts";
  import { flash } from "#lib/toast.svelte.ts";
  import type {
    Paragraph19Info,
    SellerProfile,
    SellerProfileInput,
  } from "#lib/types.ts";

  let profile: SellerProfile | null = $state(null);
  let info: Paragraph19Info | null = $state(null);
  let loading = $state(true);
  let showWaiveDialog = $state(false);
  let pendingInput: SellerProfileInput | null = $state(null);

  let input: SellerProfileInput = $state(blankInput());
  let ownerName = $state("");
  let quoteValidDays = $state<number | null>(30);
  let invoiceDueDays = $state<number | null>(14);
  let quoteDaysInput: QuantityInput | undefined = $state();
  let dueDaysInput: QuantityInput | undefined = $state();

  // U1: Pflichtfelder mit Fehlermeldung am Feld statt Toast (3.3.1).
  type FieldKey = "name" | "street" | "postalCode" | "city" | "countryCode" | "email";
  let errors = $state<Partial<Record<FieldKey, string>>>({});
  const isDE = $derived(input.countryCode.trim().toUpperCase() === "DE");

  // R5-SUB4/U1: App-Shell inert, solange der §19-Dialog offen ist.
  $effect(() => {
    if (showWaiveDialog) {
      pushModal();
      return () => popModal();
    }
  });

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
      isKleinunternehmer: true, // §19-Default
      defaultPdfTemplate: "default",
      defaultCurrency: "EUR",
      confirmWaiveParagraph19: null,
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
      isKleinunternehmer: p.isKleinunternehmer === 1,
      defaultPdfTemplate: p.defaultPdfTemplate,
      defaultCurrency: p.defaultCurrency,
      confirmWaiveParagraph19: null,
    };
  }

  onMount(async () => {
    try {
      profile = await sellerProfileGet();
      info = await paragraph19Info();
      if (profile) input = fromProfile(profile);
      ownerName = (await sellerOwnerGet()) ?? "";
      const terms = await documentTermsGet();
      quoteValidDays = terms.quoteValidDays;
      invoiceDueDays = terms.invoiceDueDays;
    } catch (e) {
      flash(String(e), "error");
    } finally {
      loading = false;
    }
  });

  function isWaiveTransition(): boolean {
    return profile?.isKleinunternehmer === 1 && input.isKleinunternehmer === false;
  }

  async function handleSubmit(e: Event) {
    e.preventDefault();

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

    if (isWaiveTransition() && !input.confirmWaiveParagraph19) {
      // Modal-Dialog öffnen statt Backend abzulehnen.
      pendingInput = { ...input };
      showWaiveDialog = true;
      return;
    }
    await save(input);
  }

  async function save(payload: SellerProfileInput) {
    try {
      profile = await sellerProfileUpsert(payload);
      await sellerOwnerSet(ownerName);
      // Leere Tage-Felder: bisherige Werte behalten (wie vorher bei 0 nicht möglich).
      await documentTermsSet(quoteValidDays ?? 30, invoiceDueDays ?? 14);
      info = await paragraph19Info();
      input = fromProfile(profile);
      flash("Profil gespeichert.");
    } catch (e) {
      flash(String(e), "error");
    }
  }

  async function confirmWaive() {
    if (!pendingInput) return;
    const payload = { ...pendingInput, confirmWaiveParagraph19: true };
    showWaiveDialog = false;
    pendingInput = null;
    await save(payload);
  }

  function cancelWaive() {
    showWaiveDialog = false;
    pendingInput = null;
    // Toggle visuell zurücksetzen
    if (profile) input.isKleinunternehmer = profile.isKleinunternehmer === 1;
  }
</script>

<PageBar back="/settings" backLabel="Einstellungen" title="Meine Firmendaten">
  {#snippet actions()}
    {#if !loading}
      <Button variant="secondary" href="/settings">Abbrechen</Button>
      <Button type="submit" form="seller-form">Speichern</Button>
    {/if}
  {/snippet}
</PageBar>
<p class="lead">Diese Angaben stehen auf deinen Rechnungen und Angeboten.</p>

{#snippet err(k: FieldKey)}
  {#if errors[k]}<span id="seller-{k}-err" class="err">{errors[k]}</span>{/if}
{/snippet}

{#if loading}
  <p aria-busy="true">Lädt …</p>
{:else}
  <form id="seller-form" onsubmit={handleSubmit} novalidate>
    <p class="legend-req"><span class="req" aria-hidden="true">*</span> Pflichtfeld</p>
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

    <fieldset>
      <legend>Steuer</legend>
      <div class="f">
        <label for="seller-taxno">Steuernummer</label>
        <input id="seller-taxno" type="text" bind:value={input.taxNumber} autocomplete="off" inputmode="text"
          autocapitalize="characters" spellcheck="false" aria-describedby="seller-taxno-hint" />
        <small id="seller-taxno-hint" class="hint-inline">
          Format wie vom Finanzamt, z. B. 132/456/7890. Steht auf jeder Rechnung. Noch keine
          vom Finanzamt? Dann leer lassen — du kannst die Firmendaten trotzdem speichern.
          Rechnungen schreiben geht erst, wenn hier eine Steuernummer oder eine USt-IdNr. steht.
        </small>
      </div>
      <div class="f">
        <label for="seller-vatid">USt-IdNr.</label>
        <input id="seller-vatid" type="text" bind:value={input.vatId} autocomplete="off" inputmode="text"
          autocapitalize="characters" spellcheck="false" aria-describedby="seller-vatid-hint" />
        <small id="seller-vatid-hint" class="hint-inline">In Deutschland „DE“ und 9 Ziffern, z. B. DE123456789.</small>
      </div>

      <div class="paragraph19">
        <div class="p19-head">
          <Toggle
            bind:checked={input.isKleinunternehmer}
            label="Kleinunternehmer nach §19 UStG"
          />
          <HelpAnchor slug="kleinunternehmer-regelung" />
        </div>
        {#if input.isKleinunternehmer}
          <p class="hint">
            <strong>Eingeschaltet.</strong> Auf jeder Rechnung erscheint automatisch der
            Hinweis „{info?.hinweisText}“. Du weist keine Mehrwertsteuer aus — die
            MwSt-Felder sind gesperrt, damit dir kein Fehler passiert.
          </p>
        {:else}
          <p class="caveat">
            <strong>Mehrwertsteuer-Pflicht.</strong> Du weist auf deinen Rechnungen
            Mehrwertsteuer aus und meldest sie regelmäßig ans Finanzamt.{#if info?.returnDateAfterWaiver}{" "}
              Zurück zur Kleinunternehmer-Regelung kannst du frühestens am
              <strong>{date(info.returnDateAfterWaiver)}</strong> (du bist 5 Jahre gebunden).
            {/if}
          </p>
        {/if}
      </div>
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
        <label for="seller-phone">Telefon</label>
        <input id="seller-phone" type="tel" bind:value={input.phone} autocomplete="tel" aria-describedby="seller-phone-hint" />
        <small id="seller-phone-hint" class="hint-inline">
          z. B. +49 30 1234567. Pflicht für die E-Rechnung (XRechnung, Verkäufer-Kontakt):
          ohne Telefonnummer lässt sich keine Rechnung festschreiben.
        </small>
      </div>
    </fieldset>

    <fieldset>
      <legend>Standard-Einstellungen</legend>
      <div class="f narrow">
        <label for="seller-currency">Währung</label>
        <input id="seller-currency" type="text" bind:value={input.defaultCurrency} maxlength="3" autocomplete="off"
          autocapitalize="characters" spellcheck="false" aria-describedby="seller-currency-hint" />
        <small id="seller-currency-hint" class="hint-inline">ISO-Kürzel, z. B. EUR</small>
      </div>
      <div class="days">
        <QuantityInput
          bind:this={quoteDaysInput}
          bind:value={quoteValidDays}
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
    <div class="kb-form-end"><button type="submit" class="btn-primary">Speichern</button></div>
  </form>
{/if}

{#if showWaiveDialog}
  <!-- Gefahr-Dialog: kein Schließen über den Hintergrund, Startfokus „Abbrechen“,
       Escape bricht ab (ADR 0041 Nr. 11). Eigener Dialog statt confirmDialog,
       weil er zwei Fälle mit Zwischenüberschriften erklärt (R5-004, F8e). -->
  <div class="kb-backdrop" use:portal>
    <div
      class="kb-dialog waive"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="waive-title"
      aria-describedby="waive-lead"
      use:focusTrap={{ onEscape: cancelWaive }}
    >
      <header class="kb-dialog-head">
        <h2 id="waive-title">Zur Regelbesteuerung wechseln?</h2>
      </header>
      <div class="kb-dialog-body">
        <p id="waive-lead">
          Du schaltest die Kleinunternehmer-Regelung aus. Ab jetzt weisen neue Rechnungen
          <strong>Umsatzsteuer</strong> aus. Dafür gibt es zwei Gründe — lies den passenden Abschnitt.
        </p>

        <h3>Freiwilliger Verzicht (§19 Abs. 2 UStG)</h3>
        <p>
          An diese Entscheidung bist du <strong>5 Jahre lang gebunden</strong>. Zurück zur
          Kleinunternehmer-Regelung kannst du erst danach.
        </p>

        <h3>Gesetzlicher Wegfall durch die 100.000-€-Grenze</h3>
        <p>
          Überschreitet dein Umsatz im laufenden Jahr 100.000 €, endet die Regelung
          automatisch mit diesem Umsatz (§19 Abs. 1 UStG). Die 5-Jahres-Bindung gilt dann
          <strong>nicht</strong> — sie betrifft nur den freiwilligen Verzicht.
        </p>
        <ul>
          <li>
            Schalte um, <strong>bevor</strong> du die Rechnung ausstellst, mit der du die Grenze
            überschreitest. Ausgestellte Rechnungen behalten ihren Status; hast du sie noch mit
            §19-Hinweis ausgestellt, stornierst du sie und stellst sie mit Umsatzsteuer neu aus.
          </li>
          <li>
            Kosten vor dem Überschreiten bleiben ohne Vorsteuerabzug; Klein.Buch rechnet für das
            ganze Jahr mit Vorsteuer — das korrigierst du mit deiner Steuerberatung.
          </li>
          <li>Anlagen, die du vorher angeschafft hast, korrigierst du in diesem Fall nicht.</li>
        </ul>

        <h3>In beiden Fällen</h3>
        <ul>
          <li>Du weist auf allen neuen Rechnungen Mehrwertsteuer aus,</li>
          <li>schickst regelmäßig eine Umsatzsteuer-Voranmeldung ans Finanzamt</li>
          <li>und kümmerst dich um den Vorsteuerabzug.</li>
        </ul>
        <p>
          Klein.Buch unterstützt das nur teilweise: Es weist die Umsatzsteuer auf
          Rechnungen aus und teilt die EÜR-Ausfüllhilfe in netto und Umsatzsteuer
          auf. Die Umsatzsteuer-Voranmeldung, Zahlungen an und Erstattungen vom
          Finanzamt sowie die Umsatzsteuer auf Anlagenverkäufe bildet Klein.Buch
          nicht ab — das klärst du mit deiner Steuerberatung. Mehr im Handbuch,
          Kapitel <a href="/help/kleinunternehmer-regelung">Kleinunternehmer-Regelung</a>.
        </p>
      </div>
      <footer class="kb-dialog-foot">
        <Button variant="secondary" initialFocus onclick={cancelWaive}>Abbrechen</Button>
        <Button variant="danger" onclick={confirmWaive}>Zur Regelbesteuerung wechseln</Button>
      </footer>
    </div>
  </div>
{/if}

<style>
  /* .lead / .caveat / .kb-dialog* — globale Definitionen aus tokens.css. */
  form { max-width: 720px; }
  fieldset {
    border: 1px solid var(--line);
    border-radius: var(--r-md);
    padding: 1rem;
    margin: 0 0 1rem;
    min-width: 0;
  }
  legend { font-weight: 600; padding: 0 0.4rem; }
  .legend-req { font-size: var(--fs-sm); color: var(--text-muted); margin: 0 0 0.5rem; }
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
  .paragraph19 {
    margin-top: 1rem;
    padding: 0.75rem;
    background: var(--surface-sunken);
    border-radius: var(--r-sm);
  }
  .p19-head { display: flex; align-items: center; gap: 0.25rem; }
  .hint {
    background: var(--status-info-bg); color: var(--status-info-fg);
    padding: 0.6rem; border-radius: var(--r-sm);
    margin-top: 0.5rem; font-size: var(--fs-sm);
  }
  .hint-inline {
    display: block;
    font-size: var(--fs-sm);
    color: var(--text-muted);
  }
  .crossref { margin: 0.4rem 0 0; font-size: var(--fs-sm); }
  .crossref a, .waive a { text-decoration: underline; }
  .waive { --dialog-w: 40rem; }
  .waive h3 { font-size: var(--fs-md); margin: 1rem 0 0.25rem; }
  .waive ul { padding-left: 1.2rem; }
  .waive li { margin-bottom: 0.3rem; }
</style>
