<script lang="ts">
  // U4 (V2-11/V2-06): Hauptaktion nach dem Ergebnis benannt („Kontakt anlegen“), „Abbrechen“
  // davor; während des Speicherns `ariaDisabled` und „Speichert …“ (Zustand aus ContactForm).
  import { goto } from "$app/navigation";
  import { contactsCreate } from "#lib/api.ts";
  import ContactForm from "#lib/ContactForm.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import Button from "#lib/Button.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import type { ContactInput } from "#lib/types.ts";

  let saving = $state(false);

  async function handleSubmit(input: ContactInput) {
    const c = await contactsCreate(input);
    flash("Kontakt angelegt.");
    await goto(`/contacts/${c.id}`);
  }
</script>

<PageBar back="/contacts" backLabel="Kontakte" title="Neuer Kontakt">
  {#snippet actions()}
    <Button variant="secondary" href="/contacts">Abbrechen</Button>
    <Button variant="primary" type="submit" form="contact-form" busy={saving}>
      {saving ? "Speichert …" : "Kontakt anlegen"}
    </Button>
  {/snippet}
</PageBar>

<ContactForm formId="contact-form" showSubmit={false} submitLabel="Kontakt anlegen" onsubmit={handleSubmit} bind:busy={saving} />
