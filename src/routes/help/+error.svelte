<script lang="ts">
  // U2-Sichtprüfung: Fehlerseite des Handbuchs (z. B. toter Hilfe-Anker). Sie steht
  // innerhalb von help/+layout.svelte, damit Kapitelliste, Suche und „Zur App“
  // erreichbar bleiben — vorher zeigte SvelteKit nur „404“ ohne Weg zurück.
  import { page } from "$app/state";

  let notFound = $derived(page.status === 404);
</script>

<header class="entry-header">
  <h1>{notFound ? "Seite nicht gefunden" : "Seite nicht verfügbar"}</h1>
  <p class="meta">Handbuch · Fehler {page.status}</p>
</header>

<!-- U7 (R2-04): Bei 404 fester Text, nie Teile der Adresse (Inhalts-Spoofing). -->
<p>
  {notFound
    ? "Diese Handbuch-Seite gibt es nicht. Vielleicht ist der Link veraltet."
    : (page.error?.message ?? "Diese Handbuch-Seite lässt sich nicht anzeigen.")}
</p>
<p>
  Wähle ein Kapitel aus der Liste, suche im Handbuch oder beginne bei
  <a href="/help/willkommen">Willkommen in Klein.Buch</a>.
</p>

<style>
  .entry-header {
    margin-bottom: 1.5rem;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid var(--line);
  }
  h1 {
    font-size: var(--fs-2xl);
    margin: 0 0 0.5rem;
    letter-spacing: -0.01em;
  }
  .meta {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    font-weight: 600;
  }
  p {
    max-width: 42rem;
    line-height: 1.6;
  }
  /* Link im Fließtext: unterstrichen (ADR 0041 Nr. 31). */
  a {
    text-decoration: underline;
    text-underline-offset: 2px;
  }
</style>
