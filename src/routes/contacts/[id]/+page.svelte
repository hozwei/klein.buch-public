<script lang="ts">
  // U4 (V2-12): Archivieren ohne Rückfrage, mit „Rückgängig“ im Toast; die Seite bleibt stehen.
  // U4 (V4-08/V2-07): Titel auch beim Laden und bei „nicht gefunden“, Ladefehler mit „Erneut laden“.
  // U4 (V2-06/V2-11): „Änderungen speichern“ während des Speicherns ariaDisabled („Speichert …“).
  import { page } from "$app/state";
  import { onMount } from "svelte";
  import {
    contactsGet,
    contactsUpdate,
    contactsArchive,
    contactsUnarchive,
    contactsAnonymize,
    contactsAnonymizeCheck,
    dsgvoExport,
    saveDownload,
  } from "#lib/api.ts";
  import ContactForm from "#lib/ContactForm.svelte";
  import Banner from "#lib/Banner.svelte";
  import HelpAnchor from "#lib/HelpAnchor.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import LoadingState from "#lib/LoadingState.svelte";
  import Button from "#lib/Button.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import type { Contact, ContactInput } from "#lib/types.ts";

  // `[id]`-Route garantiert, dass id definiert ist — `page.params` ist
  // generisch `Record<string, string | undefined>`.
  let id = $derived(page.params.id as string);
  let contact: Contact | null = $state(null);
  let loading = $state(true);
  let error: string | null = $state(null);
  /** U4 (V4-08): „nicht gefunden“ als eigener Zustand mit Titel und Weg zurück. */
  let notFound = $state(false);
  let saving = $state(false);

  async function load() {
    loading = true;
    error = null;
    notFound = false;
    try {
      contact = await contactsGet(id);
      if (!contact) notFound = true;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  onMount(load);

  async function handleSubmit(input: ContactInput) {
    contact = await contactsUpdate(id, input);
    flash("Kontakt gespeichert.");
  }

  let archiving = $state(false);

  /**
   * Archivieren ist umkehrbar (Reaktivieren): keine Rückfrage, dafür „Rückgängig“ im Toast
   * (design.md §5, U4 V2-12). Die Seite bleibt stehen und zeigt den Archiv-Hinweis. Die ID
   * wird festgehalten — „Rückgängig“ wirkt auch nach einem Seitenwechsel auf diesen Kontakt.
   */
  async function setArchived(archive: boolean, undo = false, cid: string = id) {
    if (archiving) return;
    archiving = true;
    try {
      if (archive) await contactsArchive(cid);
      else await contactsUnarchive(cid);
      const fresh = await contactsGet(cid);
      if (cid === id) contact = fresh;
      const msg = archive ? "Kontakt archiviert." : "Kontakt reaktiviert.";
      if (undo) flash(msg);
      else flash(msg, "ok", { action: { label: "Rückgängig", run: () => void setArchived(!archive, true, cid) } });
    } catch (e) {
      flash(`Kontakt ${archive ? "archivieren" : "reaktivieren"} fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      archiving = false;
    }
  }

  let exporting = $state(false);

  async function handleDsgvoExport() {
    if (exporting) return;
    if (
      !(await confirmDialog({
        title: "DSGVO-Auskunft erstellen?",
        body: "Erzeugt ein ZIP mit allen gespeicherten Daten zu diesem Kontakt (lesbares PDF, maschinenlesbares JSON und die archivierten Originaldateien) zum Herunterladen. Vor einer Herausgabe an die Person bitte prüfen — Angaben Dritter beachten (Art. 15 Abs. 4 DSGVO). Keine Rechtsberatung.",
        confirmLabel: "Auskunft erstellen",
      }))
    )
      return;
    exporting = true;
    try {
      const res = await dsgvoExport(id);
      if (res.download) saveDownload(res.download);
      // U4 (V2-17): Einzahl/Mehrzahl statt „1 Rechnungen“.
      const n = (k: number, one: string, many: string) => `${k} ${k === 1 ? one : many}`;
      flash(
        `Auskunft erstellt: ${res.fileName} — ${n(res.invoiceCount, "Rechnung", "Rechnungen")}, ` +
          `${n(res.quoteCount, "Angebot", "Angebote")}, ${res.expenseCount} Kosten, ` +
          `${res.bundledDocumentCount} von ${n(res.documentCount, "Dokument", "Dokumenten")} beigelegt.`,
      );
    } catch (e) {
      flash(`DSGVO-Auskunft fehlgeschlagen: ${String(e)}`, "error");
    } finally {
      exporting = false;
    }
  }

  let anonymizing = $state(false);

  async function handleAnonymize() {
    if (anonymizing) return;
    let check;
    try {
      check = await contactsAnonymizeCheck(id);
    } catch (e) {
      flash(`Anonymisierung prüfen fehlgeschlagen: ${e instanceof Error ? e.message : String(e)}`, "error");
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
          "Diese Aktion ist unwiderruflich. Die personenbezogenen Kontaktdaten werden überschrieben " +
          "(Name durch Platzhalter ersetzt, alle übrigen Felder gelöscht) und der Kontakt archiviert. " +
          `Gesetzlich aufbewahrungspflichtige Belege bleiben erhalten: ${check.lockedInvoices === 1 ? "1 Rechnung" : `${check.lockedInvoices} Rechnungen`} ` +
          `und ${check.lockedQuotes === 1 ? "1 Angebot" : `${check.lockedQuotes} Angebote`} behalten ihren damaligen Empfänger-Stand (§147 AO, 10 Jahre). ` +
          "Ebenfalls ersetzt werden: der Empfänger in verworfenen Rechnungs- und " +
          "Angebots-Entwürfen (Nummer, Datum, Betrag und Positionen bleiben), " +
          "Bezeichnung und Notiz der Abo-Vorlagen an diesen Kontakt sowie die Bezeichnung seiner " +
          "Kosten-Abos; diese Abos werden pausiert. " +
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
      flash(`Anonymisieren fehlgeschlagen: ${String(e)}`, "error");
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

<!-- U4 (U1-059, design.md §4.2): Hauptaktion „Speichern“, Nebenaktion „DSGVO-Auskunft“
     (bis 600 px im Menü), Archivieren/Reaktivieren und Anonymisieren im Menü „Mehr“ —
     Gefahraktionen unten abgesetzt. Die früheren title-Tooltips stehen als sichtbare
     Zusatzzeile im Menü bzw. im Bestätigungsdialog (ADR 0041 Nr. 47). -->
<PageBar
  back="/contacts"
  backLabel="Kontakte"
  title={contact?.name ?? (notFound ? "Kontakt nicht gefunden" : "Kontakt")}
  docTitle={notFound ? "Kontakt nicht gefunden" : "Kontakt"}
  secondary={contact && contact.anonymizedAt == null
    ? [{ label: exporting ? "Erstelle Auskunft …" : "DSGVO-Auskunft", onSelect: handleDsgvoExport, disabled: exporting }]
    : []}
  more={contact && contact.anonymizedAt == null
    ? [
        contact.archived === 1
          ? { label: "Reaktivieren", onSelect: () => setArchived(false), disabled: archiving }
          : { label: "Archivieren", onSelect: () => setArchived(true), danger: true, disabled: archiving, hint: "Danach nicht mehr in Auswahllisten; Belege bleiben, Reaktivieren jederzeit möglich." },
        {
          label: anonymizing ? "Anonymisiere …" : "Anonymisieren (DSGVO)",
          onSelect: handleAnonymize,
          danger: true,
          disabled: anonymizing,
          hint: "Löscht die Kontaktdaten; festgeschriebene Belege bleiben erhalten.",
        },
      ]
    : []}
>
  {#snippet actions()}
    {#if contact && contact.anonymizedAt == null}
      <Button variant="primary" type="submit" form="contact-form" busy={saving}>
        {saving ? "Speichert …" : "Änderungen speichern"}
      </Button>
    {/if}
    <HelpAnchor slug="dsgvo-auskunft-und-anonymisierung" />
  {/snippet}
</PageBar>

{#if loading}
  <LoadingState />
{:else if notFound}
  <Banner title="Kontakt nicht gefunden." href="/contacts" linkLabel="Zur Kontaktliste">
    Unter dieser Adresse gibt es keinen Kontakt.
  </Banner>
{:else if error}
  <Banner title="Der Kontakt konnte nicht geladen werden.">{error}
    {#snippet actions()}<Button variant="secondary" onclick={load}>Erneut laden</Button>{/snippet}
  </Banner>
{:else if contact}
  {#if contact.anonymizedAt != null}
    <Banner kind="info">
      Dieser Kontakt wurde anonymisiert (DSGVO Art. 17). Die personenbezogenen
      Kontaktdaten wurden gelöscht; festgeschriebene Rechnungen und Angebote behalten
      ihren damaligen Empfänger-Stand und bleiben gesetzlich erhalten (§147 AO).
    </Banner>
  {:else}
    {#if contact.archived === 1}
      <Banner kind="info">
        Dieser Kontakt ist archiviert und steht nicht in Auswahllisten. Bestehende Rechnungen und
        Angebote bleiben erhalten; reaktivieren kannst du ihn im Menü „Mehr“.
      </Banner>
    {/if}
    <ContactForm
      formId="contact-form"
      showSubmit={false}
      initial={toInput(contact)}
      submitLabel="Änderungen speichern"
      onsubmit={handleSubmit}
      bind:busy={saving}
    />
  {/if}
{/if}

