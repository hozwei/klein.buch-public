<script lang="ts">
  // Rechnungs-Entwurf bearbeiten (U4, Warteliste (6); vorher `/invoices/new?edit=<id>`).
  // Gleicher Editor wie „Neue Rechnung“; die Nummer bleibt.
  import { page } from "$app/state";
  import InvoiceEditor from "../../InvoiceEditor.svelte";

  let id = $derived(page.params.id ?? "");
</script>

{#key id}
  <InvoiceEditor editId={id} />
{/key}
