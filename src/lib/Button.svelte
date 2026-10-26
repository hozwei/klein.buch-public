<script lang="ts">
  // Wiederverwendbarer Button (DS-1). variant steuert die Farbe; size ist
  // absichtlich wirkungslos (alle Buttons gleich groß, Manuel 2026-05-26).
  // Mit href wird ein <a> im Button-Look gerendert (Navigation = Link).
  // Trefferfläche mindestens --target (24 px, im Drawer-Band/Touch 44 px).
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
    id,
    ariaLabel,
    ariaDescribedby,
    initialFocus = false,
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
    id?: string;
    /** Zugänglicher Name, falls der sichtbare Text allein nicht reicht (muss ihn enthalten). */
    ariaLabel?: string;
    ariaDescribedby?: string;
    /** Startfokus in einem Dialog mit `use:focusTrap` (setzt `data-initial-focus`). */
    initialFocus?: boolean;
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
    {id}
    aria-label={ariaLabel}
    aria-describedby={ariaDescribedby}
    data-initial-focus={initialFocus ? "" : undefined}
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
    {id}
    aria-label={ariaLabel}
    aria-describedby={ariaDescribedby}
    data-initial-focus={initialFocus ? "" : undefined}
    {title}
    {form}
    {onclick}
  >
    {@render children?.()}
  </button>
{/if}

<style>
  /* Specs identisch zu tokens.css .btn-primary/.btn-secondary/.btn-ghost/.btn-danger:
     9px 16px, Schrift geerbt, line-height 1.2 — Button.svelte und rohe
     <button class="btn-primary"> sind gleich hoch. Farben nur über Rollen-Tokens. */
  .btn {
    font: inherit; font-weight: 600; line-height: 1.2;
    border-radius: var(--r-md); padding: 9px 16px; border: 1px solid transparent;
    min-height: var(--target); min-width: var(--target);
    cursor: pointer; display: inline-flex; align-items: center; justify-content: center; gap: 8px;
    text-decoration: none; transition: background var(--t-fast), border-color var(--t-fast);
  }
  .btn:active { transform: translateY(1px); }

  .primary { background: var(--accent); color: var(--text-on-accent); border-color: var(--accent); }
  .secondary { background: var(--surface-card); color: var(--accent-on-soft); border-color: var(--line-control); }
  .ghost { background: transparent; color: var(--text-muted); }
  .danger { background: var(--surface-card); color: var(--status-danger-fg); border-color: var(--status-danger-line); }

  @media (hover: hover) {
    .primary:hover { background: var(--accent-hover); border-color: var(--accent-hover); }
    .secondary:hover { background: var(--surface-hover); border-color: var(--c-primary-300); }
    .ghost:hover { background: var(--surface-page); color: var(--text); }
    .danger:hover { background: var(--status-danger-bg); }
    a.btn:hover { text-decoration: none; }
  }

  .btn:disabled, .btn.disabled { opacity: .5; cursor: not-allowed; pointer-events: none; }
</style>
