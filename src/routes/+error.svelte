<script lang="ts">
  // Fehlerseite der App (U4, V2-08; ADR 0041 Nr. 162). Der Server liefert für
  // unbekannte Pfade die SPA aus; ohne diese Datei zeigte SvelteKit „404 / Not Found“
  // auf Englisch, ohne Navigation und ohne Weg zurück. Muster wie jede Seite:
  // PageBar mit Zurück und Titel (auch Seitentitel, 2.4.2), darunter ein Banner mit
  // Ursache und nächstem Schritt. Das Handbuch hat eine eigene Fehlerseite
  // (help/+error.svelte) im Handbuch-Layout.
  import { page } from "$app/state";
  import PageBar from "#lib/PageBar.svelte";
  import Banner from "#lib/Banner.svelte";

  const notFound = $derived(page.status === 404);
</script>

<PageBar back="/" backLabel="Start" title={notFound ? "Seite nicht gefunden" : "Seite nicht verfügbar"} />

{#if notFound}
  <Banner kind="error" title="Diese Adresse gibt es in Klein.Buch nicht.">
    Vielleicht ist der Link veraltet oder falsch abgetippt. Wähle einen Bereich in der
    Navigation oder geh zurück zur <a class="kb-link" href="/">Startseite</a>.
  </Banner>
{:else}
  <Banner kind="error" title="Die Seite lässt sich gerade nicht anzeigen.">
    Lade die Seite neu. Bleibt der Fehler, geh zurück zur
    <a class="kb-link" href="/">Startseite</a> und versuche es dort erneut.
    {#if page.error?.message}<span class="detail">(Fehler {page.status}: {page.error.message})</span>{/if}
  </Banner>
{/if}

<style>
  .detail {
    display: block;
    margin-top: var(--sp-2);
    font-size: var(--type-small);
  }
</style>
