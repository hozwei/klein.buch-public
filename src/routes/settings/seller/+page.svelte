<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { safeNextPath } from "#lib/nextPath.ts";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import RequiredLegend from "#lib/RequiredLegend.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Toggle from "#lib/Toggle.svelte";
  import Banner from "#lib/Banner.svelte";
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
  /** U4 (V2-07): Nach einem Ladefehler kein leeres Formular (Speichern hätte die Firmendaten überschrieben). */
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  /** Nach dem ersten Speicherversuch: fehlende Angaben fürs Ausstellen am Feld zeigen. */
  let triedSave = $state(false);
  let showWaiveDialog = $state(false);
  let pendingInput: SellerProfileInput | null = $state(null);

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

  async function load() {
    loading = true;
    loadError = null;
    try {
      profile = await sellerProfileGet();
      info = await paragraph19Info();
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

  function isWaiveTransition(): boolean {
    return profile?.isKleinunternehmer === 1 && input.isKleinunternehmer === false;
  }

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

    if (isWaiveTransition() && !input.confirmWaiveParagraph19) {
      // Modal-Dialog öffnen statt Backend abzulehnen.
      pendingInput = { ...input };
      showWaiveDialog = true;
      return;
    }
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
      info = await paragraph19Info();
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
            Hinweis „{info?.hinweisText}“. Du weist keine Umsatzsteuer aus — Rechnungen
            und Angebote blenden die USt-Felder aus, damit dir kein Fehler passiert.
          </p>
        {:else}
          <!-- U6-002/U7 (R1-02): Gespeichert ist nur das Datum des Wechsels, nicht sein Grund
               (freiwilliger Verzicht oder Wegfall durch eine Umsatzgrenze, ADR 0041 Nr. 21/220).
               Deshalb die Fälle nennen; der Server lässt die Rückkehr in allen erst ab diesem
               Datum zu. -->
          <p class="caveat">
            <strong>Regelbesteuerung.</strong> Du weist auf deinen Rechnungen
            Umsatzsteuer aus und meldest sie regelmäßig ans Finanzamt.{#if info?.returnDateAfterWaiver}{" "}
              Hast du freiwillig verzichtet, bist du mindestens 5 Kalenderjahre gebunden: Zurück
              zur Kleinunternehmer-Regelung kannst du frühestens am
              <strong>{date(info.returnDateAfterWaiver)}</strong>. Beim Wegfall durch eine
              Umsatzgrenze (über 25.000 € im Vorjahr oder über 100.000 € im laufenden Jahr) gilt
              diese Bindung nicht; Klein.Buch kennt den Grund des Wechsels aber nicht und lässt
              die Rückkehr erst ab diesem Datum zu.
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
{/if}

{#if showWaiveDialog}
  <!-- Gefahr-Dialog: kein Schließen über den Hintergrund, Startfokus „Abbrechen“,
       Escape bricht ab (ADR 0041 Nr. 11). Eigener Dialog statt confirmDialog,
       weil er drei Fälle mit Zwischenüberschriften erklärt (R5-004, F8e, U7 R1-02). -->
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
          Du schaltest die Kleinunternehmer-Regelung aus. Neue Rechnungen weisen dann
          <strong>Umsatzsteuer</strong> aus. Für den Wechsel gibt es drei Gründe mit
          unterschiedlichen Folgen — lies den passenden Abschnitt. Die Hinweise ersetzen keine
          Steuerberatung.
        </p>

        <h3>Freiwilliger Verzicht (§ 19 Abs. 3 UStG)</h3>
        <p>
          Der Verzicht gilt ab Beginn des Kalenderjahres, für das du ihn erklärst, und bindet
          dich <strong>mindestens 5 Kalenderjahre</strong>. Zurück zur Kleinunternehmer-Regelung
          kannst du erst danach. Hast du in diesem Jahr schon Rechnungen mit §19-Hinweis
          ausgestellt, klärst du mit deiner Steuerberatung, wie du mit ihnen umgehst. Willst du
          erst ab dem nächsten Jahr verzichten, schalte erst im neuen Jahr um.
        </p>

        <h3>Wegfall, weil dein Vorjahresumsatz über 25.000 € lag</h3>
        <p>
          Lag dein Gesamtumsatz im Vorjahr über 25.000 €, gilt die Regelung ab dem 1. Januar
          nicht mehr (§ 19 Abs. 1 UStG). Eine 5-Jahres-Bindung gibt es dabei
          <strong>nicht</strong>. Schalte um, bevor du die erste Rechnung des Jahres ausstellst.
        </p>

        <h3>Wegfall, weil dein Umsatz in diesem Jahr 100.000 € überschreitet</h3>
        <p>
          Überschreitet dein Umsatz im laufenden Jahr 100.000 €, endet die Regelung mit dem
          Umsatz, der die Grenze überschreitet (§ 19 Abs. 1 UStG). Auch hier gilt
          <strong>keine</strong> 5-Jahres-Bindung.
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
          <li>
            Die Anschaffungskosten von Anlagen, die du vorher angeschafft hast, korrigierst du in
            diesem Fall nicht.
          </li>
        </ul>

        <h3>In allen Fällen</h3>
        <ul>
          <li>Du weist auf allen neuen Rechnungen Umsatzsteuer aus,</li>
          <li>schickst regelmäßig eine Umsatzsteuer-Voranmeldung ans Finanzamt</li>
          <li>und kümmerst dich um den Vorsteuerabzug.</li>
        </ul>
        <p>
          Nutzt du Anlagen weiter, die du als Kleinunternehmer angeschafft hast, kann der
          Wechsel eine Berichtigung der Vorsteuer nach § 15a UStG auslösen (der Übergang gilt
          als Änderung der Verhältnisse). Klein.Buch berechnet sie nicht; kläre das mit deiner
          Steuerberatung.
        </p>
        <p>
          Klein.Buch unterstützt das nur teilweise: Es weist die Umsatzsteuer auf
          Rechnungen aus und teilt die EÜR-Ausfüllhilfe in netto und Umsatzsteuer
          auf. Die Umsatzsteuer-Voranmeldung, Zahlungen an und Erstattungen vom
          Finanzamt sowie die Umsatzsteuer auf Anlagenverkäufe bildet Klein.Buch
          nicht ab — das klärst du mit deiner Steuerberatung. Klein.Buch kennt den Grund des
          Wechsels nicht und lässt eine Rückkehr zur Kleinunternehmer-Regelung in allen drei
          Fällen erst nach Ablauf der 5-Jahres-Frist zu. Mehr im Handbuch, Kapitel
          <a href="/help/kleinunternehmer-regelung">Kleinunternehmer-Regelung</a>.
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
  .paragraph19 {
    max-width: 48rem; /* U2: lesbare Zeilenlänge im breiten Steuer-Kasten */
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
  .warn { display: block; font-size: var(--fs-sm); font-weight: 600; color: var(--status-warning-fg); }
  .issue-req { font-weight: 400; color: var(--text-muted); }
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
