<script lang="ts">
  import { page } from "$app/stores";
  import { goto } from "$app/navigation";
  import { onMount } from "svelte";
  import {
    contactsGet,
    contactsUpdate,
    contactsArchive,
    contactsUnarchive,
    contactsAnonymize,
    contactsAnonymizeCheck,
    dsgvoExport,
  } from "$lib/api";
  import ContactForm from "$lib/ContactForm.svelte";
  import Banner from "$lib/Banner.svelte";
  import HelpAnchor from "$lib/HelpAnchor.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import { flash } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import type { Contact, ContactInput } from "$lib/types";

  // `[id]`-Route garantiert, dass id definiert ist — `$page.params` ist
  // generisch `Record<string, string | undefined>`.
  let id = $derived($page.params.id as string);
  let contact: Contact | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);

  onMount(async () => {
    try {
      contact = await contactsGet(id);
      if (!contact) error = `Kontakt ${id} nicht gefunden.`;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  });

  async function handleSubmit(input: ContactInput) {
    contact = await contactsUpdate(id, input);
    flash("Gespeichert.");
  }

  async function handleArchive() {
    if (
      !(await confirmDialog({
        title: "Diesen Kontakt archivieren?",
        body: "Bestehende Rechnungen und Angebote bleiben erhalten.",
        confirmLabel: "Archivieren",
        danger: true,
      }))
    )
      return;
    await contactsArchive(id);
    await goto("/contacts");
  }

  async function handleUnarchive() {
    await contactsUnarchive(id);
    contact = await contactsGet(id);
  }

  let exporting = $state(false);

  async function handleDsgvoExport() {
    if (
      !(await confirmDialog({
        title: "DSGVO-Auskunft erstellen?",
        body: "Erzeugt ein ZIP mit allen gespeicherten Daten zu diesem Kontakt (lesbares PDF, maschinenlesbares JSON und die archivierten Originaldateien) und öffnet den Ordner. Vor einer Herausgabe an die Person bitte prüfen — Angaben Dritter beachten (Art. 15 Abs. 4 DSGVO). Keine Rechtsberatung.",
        confirmLabel: "Auskunft erstellen",
      }))
    )
      return;
    exporting = true;
    try {
      const res = await dsgvoExport(id);
      flash(
        `Auskunft erstellt: ${res.fileName} — ${res.invoiceCount} Rechnungen, ` +
          `${res.quoteCount} Angebote, ${res.expenseCount} Kosten, ` +
          `${res.bundledDocumentCount}/${res.documentCount} Dokumente beigelegt.`,
      );
    } catch (e) {
      flash(String(e), "error");
    } finally {
      exporting = false;
    }
  }

  let anonymizing = $state(false);

  async function handleAnonymize() {
    let check;
    try {
      check = await contactsAnonymizeCheck(id);
    } catch (e) {
      flash(String(e), "error");
      return;
    }
    if (!check.canAnonymize) {
      flash(check.blocker ?? "Anonymisierung derzeit nicht möglich.", "error");
      return;
    }
    if (
      !(await confirmDialog({
        title: "Kontakt anonymisieren (DSGVO Art. 17)?",
        body:
          "Diese Aktion ist unwiderruflich. Die personenbezogenen Stammdaten werden überschrieben " +
          "(Name durch Platzhalter ersetzt, alle übrigen Felder gelöscht) und der Kontakt archiviert. " +
          `Gesetzlich aufbewahrungspflichtige Belege bleiben erhalten: ${check.lockedInvoices} Rechnung(en) ` +
          `und ${check.lockedQuotes} Angebot(e) behalten ihren damaligen Empfänger-Stand (§147 AO, 10 Jahre). ` +
          "Keine Rechtsberatung.",
        confirmLabel: "Anonymisieren",
        danger: true,
      }))
    )
      return;
    anonymizing = true;
    try {
      contact = await contactsAnonymize(id);
      flash("Kontakt anonymisiert. Festgeschriebene Belege bleiben erhalten.");
    } catch (e) {
      flash(String(e), "error");
    } finally {
      anonymizing = false;
    }
  }

  function toInput(c: Contact): Partial<ContactInput> {
    return {
      contactType: (c.contactType as ContactInput["contactType"]) ?? "customer",
      name: c.name,
      legalForm: c.legalForm,
      vatId: c.vatId,
      taxNumber: c.taxNumber,
      street: c.street ?? "",
      postalCode: c.postalCode ?? "",
      city: c.city ?? "",
      countryCode: c.countryCode,
      email: c.email,
      phone: c.phone,
      iban: c.iban,
      bic: c.bic,
      acceptsEinvoice: c.acceptsEinvoice === 1,
      notes: c.notes,
    };
  }
</script>

<PageBar back="/contacts" backLabel="Kontakte" title={contact?.name}>
  {#snippet actions()}
    {#if contact && contact.anonymizedAt == null}
      <Button
        variant="secondary"
        onclick={handleDsgvoExport}
        disabled={exporting}
        title="Alle gespeicherten Daten zu diesem Kontakt als Auskunft nach Art. 15 DSGVO exportieren"
      >
        {exporting ? "Erstelle …" : "DSGVO-Auskunft"}
      </Button>
      <Button
        variant="danger"
        onclick={handleAnonymize}
        disabled={anonymizing}
        title="Personenbezogene Stammdaten löschen (DSGVO Art. 17). Festgeschriebene Belege bleiben gesetzlich erhalten."
      >
        {anonymizing ? "Anonymisiere …" : "Anonymisieren (DSGVO)"}
      </Button>
      {#if contact.archived === 1}
        <Button variant="secondary" onclick={handleUnarchive}>Reaktivieren</Button>
      {:else}
        <Button variant="danger" onclick={handleArchive}>Archivieren</Button>
      {/if}
      <Button variant="primary" type="submit" form="contact-form">Speichern</Button>
    {/if}
    <HelpAnchor slug="dsgvo-auskunft-und-anonymisierung" />
  {/snippet}
</PageBar>

{#if loading}
  <p class="muted">Lade …</p>
{:else if error}
  <Banner>{error}</Banner>
{:else if contact}
  {#if contact.anonymizedAt != null}
    <Banner kind="info">
      Dieser Kontakt wurde anonymisiert (DSGVO Art. 17). Die personenbezogenen
      Stammdaten wurden gelöscht; festgeschriebene Rechnungen und Angebote behalten
      ihren damaligen Empfänger-Stand und bleiben gesetzlich erhalten (§147 AO).
    </Banner>
  {:else}
    {#if contact.archived === 1}
      <Banner kind="info">
        Dieser Kontakt ist archiviert. Bestehende Rechnungen und Angebote bleiben erhalten.
      </Banner>
    {/if}
    <ContactForm
      formId="contact-form"
      showSubmit={false}
      initial={toInput(contact)}
      submitLabel="Änderungen speichern"
      onsubmit={handleSubmit}
    />
  {/if}
{/if}

<style>
  .muted { color: var(--c-text-muted); }
</style>
