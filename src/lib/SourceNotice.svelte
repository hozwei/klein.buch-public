<script lang="ts">
  // Quellangaben unter Anmeldung und Einrichtung (AGPL §13, Plan W12):
  // Version, Commit-Kurzform mit Link auf den exakten Quellstand bzw.
  // „Entwicklungs-Build, Quellstand unbekannt", dazu die Lizenzseiten unter
  // `/legal/`. Lädt `/api/public/info` selbst (ohne Sitzung).
  import { onMount } from "svelte";
  import { publicInfo, type PublicInfo } from "#lib/transport.ts";
  import { formatVersionDisplay } from "#lib/version.ts";
  import { DEV_SOURCE_TEXT, safeSourceLink, sourceLabel } from "#lib/sourceInfo.ts";

  let info = $state<PublicInfo | null>(null);
  let failed = $state(false);
  const link = $derived(safeSourceLink(info));

  onMount(() => {
    publicInfo()
      .then((p) => {
        info = p;
      })
      .catch(() => {
        failed = true;
      });
  });
</script>

{#if info || failed}
  <p class="source-notice">
    {#if info}
      Klein.Buch {formatVersionDisplay(info.version)} · freie Software ({info.licenseSpdx}) ·
      {#if link}
        <a href={link} target="_blank" rel="noopener noreferrer">Quellcode: {sourceLabel(info)}</a>
      {:else}
        {DEV_SOURCE_TEXT}
      {/if}
    {:else}
      Klein.Buch · freie Software (AGPL-3.0-or-later)
    {/if}
    ·
    <a href="/legal/" target="_blank" rel="noopener noreferrer" data-sveltekit-reload>Lizenzen</a>
  </p>
{/if}

<style>
  .source-notice {
    margin: 1.25rem 0 0;
    padding-top: 0.75rem;
    border-top: var(--rule);
    color: var(--text-muted);
    font-size: var(--type-caption);
    line-height: 1.5;
    text-align: center;
  }
  .source-notice a {
    color: var(--accent);
    text-decoration: underline;
  }
  @media (pointer: coarse) {
    .source-notice a { display: inline-flex; align-items: center; min-height: var(--target); }
  }
</style>
