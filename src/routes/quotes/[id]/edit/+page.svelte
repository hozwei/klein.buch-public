<script lang="ts">
  // Angebots-Entwurf bearbeiten (ADR 0042 Nr. 13). Gleicher Editor wie „Neues
  // Angebot“; die Nummer bleibt.
  import { page } from "$app/state";
  import QuoteEditor from "../../QuoteEditor.svelte";

  let id = $derived(page.params.id ?? "");
</script>

{#key id}
  <QuoteEditor editId={id} />
{/key}
