<script lang="ts">
  import { goto } from "$app/navigation";
  import { contactsCreate } from "$lib/api";
  import ContactForm from "$lib/ContactForm.svelte";
  import PageBar from "$lib/PageBar.svelte";
  import Button from "$lib/Button.svelte";
  import type { ContactInput } from "$lib/types";

  async function handleSubmit(input: ContactInput) {
    const c = await contactsCreate(input);
    await goto(`/contacts/${c.id}`);
  }
</script>

<PageBar back="/contacts" backLabel="Kontakte" title="Neuer Kontakt">
  {#snippet actions()}
    <Button variant="secondary" href="/contacts">Abbrechen</Button>
    <Button variant="primary" type="submit" form="contact-form">Anlegen</Button>
  {/snippet}
</PageBar>

<ContactForm formId="contact-form" showSubmit={false} submitLabel="Anlegen" onsubmit={handleSubmit} />
