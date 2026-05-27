<script lang="ts">
  // Dünner Tabellen-Wrapper (DS-1): umschließt die Tabelle in einer Karte.
  // Inhalt (<thead>/<tbody>) kommt als Slot; nutzt die globale .kb-table aus tokens.css.
  import type { Snippet } from "svelte";

  let { children }: { children?: Snippet } = $props();
</script>

<div class="table-wrap">
  <table class="kb-table">
    {@render children?.()}
  </table>
</div>

<style>
  .table-wrap {
    background: var(--c-surface);
    border: 1px solid var(--c-border);
    border-radius: var(--r-lg);
    box-shadow: var(--sh-sm);
    overflow: hidden;
  }
</style>
