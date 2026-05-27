<script lang="ts">
  import { onMount } from "svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Toggle from "$lib/Toggle.svelte";
  import {
    sellerProfileGet,
    sellerProfileUpsert,
    paragraph19Info,
    sellerOwnerGet,
    sellerOwnerSet,
    documentTermsGet,
    documentTermsSet,
  } from "$lib/api";
  import { flash } from "$lib/toast.svelte";
  import type {
    Paragraph19Info,
    SellerProfile,
    SellerProfileInput,
  } from "$lib/types";

  let profile: SellerProfile | null = $state(null);
  let info: Paragraph19Info | null = $state(null);
  let loading = $state(true);
  let showWaiveDialog = $state(false);
  let pendingInput: SellerProfileInput | null = $state(null);

  let input: SellerProfileInput = $state(blankInput());
  let ownerName = $state("");
  let quoteValidDays = $state(30);
  let invoiceDueDays = $state(14);

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

    if (!input.name.trim()) { flash("Name ist Pflicht.", "error"); return; }
    if (!input.street.trim()) { flash("Straße ist Pflicht.", "error"); return; }
    if (!input.postalCode.trim()) { flash("PLZ ist Pflicht.", "error"); return; }
    if (!input.city.trim()) { flash("Stadt ist Pflicht.", "error"); return; }
    if (!input.countryCode.trim()) { flash("Land ist Pflicht.", "error"); return; }
    if (!input.email.trim()) { flash("E-Mail ist Pflicht.", "error"); return; }

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
      await documentTermsSet(quoteValidDays, invoiceDueDays);
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
      <a class="btn-secondary" href="/settings">Abbrechen</a>
      <button type="submit" form="seller-form" class="btn-primary">Speichern</button>
    {/if}
  {/snippet}
</PageBar>
<p class="lead">Diese Angaben stehen auf deinen Rechnungen und Angeboten.</p>

{#if loading}
  <p>Lade …</p>
{:else}
  <form id="seller-form" onsubmit={handleSubmit} novalidate>
    <fieldset>
      <legend>Firma</legend>
      <label>Name *<input type="text" bind:value={input.name} required /></label>
      <label>Rechtsform<input type="text" bind:value={input.legalForm} placeholder="z. B. Einzelunternehmen, GmbH" /></label>
      <label>Inhaber (Vor- und Nachname)
        <input type="text" bind:value={ownerName} placeholder="z. B. Manuel Schmid" />
        <small class="hint-inline">
          Bei Einzelunternehmern/Freiberuflern Pflicht — erscheint auf Angebot und
          Rechnung unter dem Firmennamen. Leer lassen, wenn der Name oben schon dein
          vollständiger Name ist.
        </small>
      </label>
    </fieldset>

    <fieldset>
      <legend>Adresse</legend>
      <label>Straße + Nr. *<input type="text" bind:value={input.street} required /></label>
      <div class="row">
        <label class="postal">PLZ *<input type="text" bind:value={input.postalCode} required /></label>
        <label class="city">Stadt *<input type="text" bind:value={input.city} required /></label>
        <label class="country">Land *<input type="text" bind:value={input.countryCode} maxlength="2" required /></label>
      </div>
    </fieldset>

    <fieldset>
      <legend>Steuer</legend>
      <label>
        Steuernummer
        <input type="text" bind:value={input.taxNumber} placeholder="z. B. 132/456/7890" />
        <small class="hint-inline">
          Steht auf jeder Rechnung. Noch keine vom Finanzamt? Dann leer lassen —
          du kannst die Firmendaten trotzdem speichern. Rechnungen schreiben geht
          erst, wenn hier eine Steuernummer oder eine USt-IdNr. steht.
        </small>
      </label>
      <label>USt-IdNr.<input type="text" bind:value={input.vatId} placeholder="DE123456789" /></label>

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
            Hinweis „{info?.hinweisText}". Du weist keine Mehrwertsteuer aus — die
            MwSt-Felder sind gesperrt, damit dir kein Fehler passiert.
          </p>
        {:else}
          <p class="caveat">
            <strong>Mehrwertsteuer-Pflicht.</strong> Du weist auf deinen Rechnungen
            Mehrwertsteuer aus und meldest sie regelmäßig ans Finanzamt.{#if info?.returnDateAfterWaiver}
              Zurück zur Kleinunternehmer-Regelung kannst du frühestens am
              <strong>{info.returnDateAfterWaiver}</strong> (du bist 5 Jahre gebunden).
            {/if}
          </p>
        {/if}
      </div>
    </fieldset>

    <fieldset>
      <legend>Kontakt</legend>
      <label>E-Mail *<input type="email" bind:value={input.email} required /></label>
      <label>Telefon<input type="tel" bind:value={input.phone} /></label>
    </fieldset>

    <fieldset>
      <legend>Standard-Einstellungen</legend>
      <label>Währung
        <input type="text" bind:value={input.defaultCurrency} maxlength="3" />
      </label>
      <label>Angebot gültig (Tage)
        <input type="number" min="0" bind:value={quoteValidDays} />
        <small class="hint-inline">Füllt „Gültig bis" beim neuen Angebot automatisch vor.</small>
      </label>
      <label>Zahlungsziel Rechnung (Tage)
        <input type="number" min="0" bind:value={invoiceDueDays} />
        <small class="hint-inline">Füllt „Fällig am" bei der neuen Rechnung automatisch vor.</small>
      </label>
      <p class="kb-muted" style="margin:0.4rem 0 0;font-size:var(--fs-sm)">
        Bankverbindung und weitere Zahlwege pflegst du unter Einstellungen →
        Konten. Logo, Unterschrift und PDF-Layout unter Einstellungen →
        Rechnungs-Layout.
      </p>
    </fieldset>

  </form>
{/if}

{#if showWaiveDialog}
  <div
    class="modal-backdrop"
    role="presentation"
    onclick={cancelWaive}
    onkeydown={(e) => e.key === "Escape" && cancelWaive()}
  >
    <div
      class="modal"
      role="dialog"
      aria-modal="true"
      aria-labelledby="waive-title"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <h2 id="waive-title">Kleinunternehmer-Regelung aufgeben?</h2>
      <p>
        Du wechselst von der Kleinunternehmer-Regelung zur <strong>normalen Besteuerung
        mit Mehrwertsteuer</strong>.
      </p>
      <p>
        Achtung: An diese Entscheidung bist du <strong>5 Jahre lang gebunden</strong>
        (§19 Abs. 2 UStG). Zurück zur Kleinunternehmer-Regelung kannst du erst danach.
      </p>
      <p>Danach musst du:</p>
      <ul>
        <li>auf allen Rechnungen Mehrwertsteuer ausweisen,</li>
        <li>regelmäßig eine Umsatzsteuer-Voranmeldung ans Finanzamt schicken,</li>
        <li>dich um den Vorsteuerabzug kümmern.</li>
      </ul>
      <div class="modal-actions">
        <button type="button" class="secondary" onclick={cancelWaive}>Abbrechen</button>
        <button type="button" class="danger" onclick={confirmWaive}>
          Ich verstehe die 5-Jahres-Bindung
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* .lead entfernt — globale Definition aus tokens.css. */
  form { max-width: 720px; }
  fieldset {
    border: 1px solid #e5e7eb;
    border-radius: 4px;
    padding: 1rem;
    margin: 0 0 1rem;
  }
  legend { font-weight: 600; padding: 0 0.4rem; }
  label { display: block; margin-bottom: 0.6rem; font-size: 0.9rem; color: #374151; }
  input[type="text"], input[type="email"], input[type="tel"] {
    width: 100%; padding: 0.45rem; border: 1px solid #d1d5db;
    border-radius: 4px; font-size: 0.95rem; box-sizing: border-box;
  }
  .row { display: flex; gap: 0.75rem; }
  .postal { flex: 0 0 7rem; }
  .city { flex: 1; }
  .country { flex: 0 0 5rem; }
  /* Lokale Pre-DS-Klassen .secondary/.danger an DS-Tokens angeglichen
     (G2-UX.3.x Konsistenz-Fix): gleiche Specs wie globale .btn-* aus tokens.css.
     Bare <button> = primary (Default-Action der Seite), .secondary = neutral,
     .danger = Solid-Rot (für den Verzicht-Bestätigen-Dialog). */
  button {
    padding: 9px 16px;
    border: 1px solid var(--c-primary-600);
    background: var(--c-primary-600);
    color: #fff;
    border-radius: var(--r-md);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
    transition: background var(--t-base) var(--ease-apple), border-color var(--t-base) var(--ease-apple);
  }
  button:hover { background: var(--c-primary-700); border-color: var(--c-primary-700); }
  button.secondary {
    background: var(--c-surface);
    color: var(--c-primary-700);
    border-color: var(--c-border-strong);
  }
  button.secondary:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }
  button.danger {
    background: var(--c-danger-500);
    border-color: var(--c-danger-500);
    color: #fff;
  }
  button.danger:hover { background: var(--c-danger-700); border-color: var(--c-danger-700); }
  .paragraph19 {
    margin-top: 1rem;
    padding: 0.75rem;
    background: #f9fafb;
    border-radius: 4px;
  }
  .p19-head { display: flex; align-items: center; gap: 0.25rem; }
  .hint {
    background: #dbeafe; color: #1e40af;
    padding: 0.6rem; border-radius: 4px;
    margin-top: 0.5rem; font-size: 0.88rem;
  }
  /* .warn entfernt — Markup nutzt jetzt das globale .caveat aus tokens.css. */
  .hint-inline {
    display: block;
    font-size: 0.8rem;
    color: #6b7280;
    margin-top: 0.2rem;
  }
  .modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    display: flex; align-items: center; justify-content: center;
    z-index: 999;
  }
  .modal {
    background: #fff;
    border-radius: 8px;
    padding: 1.5rem;
    max-width: 520px;
    box-shadow: 0 10px 30px rgba(0,0,0,0.2);
  }
  .modal h2 { margin-top: 0; color: #b91c1c; }
  .modal-actions {
    display: flex; gap: 0.5rem; justify-content: flex-end;
    margin-top: 1rem;
  }
</style>
