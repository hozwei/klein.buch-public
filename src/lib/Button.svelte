<script lang="ts">
  // Wiederverwendbarer Button (DS-1). variant steuert die Farbe, size die Größe.
  // Mit href wird ein <a> im Button-Look gerendert (z. B. Navigations-Aktionen).
  import type { Snippet } from "svelte";

  let {
    variant = "primary",
    size = "md",
    type = "button",
    disabled = false,
    href,
    onclick,
    title,
    form,
    children,
  }: {
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "md" | "sm";
    type?: "button" | "submit" | "reset";
    disabled?: boolean;
    href?: string;
    onclick?: (e: MouseEvent) => void;
    title?: string;
    form?: string;
    children?: Snippet;
  } = $props();
</script>

{#if href}
  <!-- R5-001: Disabled-Anchor wirklich inert machen: kein href (kein Routing),
       tabindex="-1" (kein Tab-Focus), aria-disabled (SR-Ansage), onclick
       absorbieren (Enter/Space würde sonst noch SvelteKit-Routing triggern). -->
  <a
    class="btn"
    class:primary={variant === "primary"}
    class:secondary={variant === "secondary"}
    class:ghost={variant === "ghost"}
    class:danger={variant === "danger"}
    class:sm={size === "sm"}
    class:disabled
    href={disabled ? undefined : href}
    aria-disabled={disabled ? "true" : undefined}
    tabindex={disabled ? -1 : undefined}
    {title}
    onclick={disabled ? (e) => e.preventDefault() : onclick}
  >
    {@render children?.()}
  </a>
{:else}
  <button
    class="btn"
    class:primary={variant === "primary"}
    class:secondary={variant === "secondary"}
    class:ghost={variant === "ghost"}
    class:danger={variant === "danger"}
    class:sm={size === "sm"}
    {type}
    {disabled}
    {title}
    {form}
    {onclick}
  >
    {@render children?.()}
  </button>
{/if}

<style>
  /* Specs IDENTISCH zu tokens.css .btn-primary/.btn-secondary/.btn-ghost/.btn-danger
     (Zeile 173–192 + 310–313 in tokens.css). Damit haben Button.svelte-Buttons und
     raw <button class="btn-primary"> exakt dieselbe Höhe, Padding, Schrift,
     line-height — egal wo sie im UI auftauchen. Default = 9px 16px / fs inherit /
     line-height 1.2. .sm = 6px 12px / fs-sm — gleich wie globale .btn-sm in
     tokens.css. */
  .btn {
    font: inherit; font-weight: 600; line-height: 1.2;
    border-radius: var(--r-md); padding: 9px 16px; border: 1px solid transparent;
    cursor: pointer; display: inline-flex; align-items: center; justify-content: center; gap: 8px;
    text-decoration: none; transition: background .12s, border-color .12s, box-shadow .12s, transform .04s;
  }
  .btn:active { transform: translateY(1px); }
  /* `size="sm"` ist absichtlich no-op: alle Buttons in der App sind gleich groß
     (Manuel-Hardline 2026-05-26). Die .sm-Klasse bleibt am Markup für
     Backwards-Kompat. */

  .primary { background: var(--c-primary-600); color: #fff; box-shadow: var(--sh-sm); }
  .primary:hover { background: var(--c-primary-700); }

  .secondary { background: var(--c-surface); color: var(--c-primary-700); border-color: var(--c-border-strong); }
  .secondary:hover { background: var(--c-primary-50); border-color: var(--c-primary-300); }

  .ghost { background: transparent; color: var(--c-text-muted); }
  .ghost:hover { background: var(--c-bg); color: var(--c-text); }

  .danger { background: var(--c-surface); color: var(--c-danger-700); border-color: #efc6c9; }
  .danger:hover { background: var(--c-danger-50); }

  .btn:disabled, .btn.disabled { opacity: .5; cursor: not-allowed; pointer-events: none; }
  a.btn:hover { text-decoration: none; }
</style>
