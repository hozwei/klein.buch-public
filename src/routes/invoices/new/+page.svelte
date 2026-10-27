<script lang="ts">
  // Neue Rechnung. Der Editor ist ein gemeinsamer Baustein mit „Entwurf bearbeiten“
  // (`/invoices/[id]/edit`, U4 Warteliste (6)); alte Links `?edit=<id>` leitet
  // `+page.ts` dorthin weiter.
  import InvoiceEditor from "../InvoiceEditor.svelte";
</script>

<InvoiceEditor editId={null} />
