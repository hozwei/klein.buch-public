<script lang="ts">
  // Wiederverwendbarer Button (DS-1). variant steuert die Farbe; size ist
  // absichtlich wirkungslos (alle Buttons gleich groß, Manuel 2026-05-26).
  // Mit href wird ein <a> im Button-Look gerendert (Navigation = Link).
  // Höhe --control-h (40 px, im Drawer-Band 44 px) wie Text- und Auswahlfelder (U4).
  //
  // Varianten (design.md §4.1): höchstens EINE `primary` je Bereich (PageBar,
  // Dialog, Formular, Karte); `secondary` für jede weitere Aktion mit eigenem
  // Ergebnis; `ghost` nur für leichte Nebenaktionen neben einer sichtbaren
  // primary/secondary derselben Gruppe (Abhaken, Zuklappen), nie allein und nie in
  // der PageBar; `danger` für Gefahraktionen (Stornieren, Verwerfen).
  //
  // `ariaDisabled` (ADR 0041 Nr. 33): gesperrt, aber fokussierbar (z. B. während
  // eines Ladevorgangs, damit der Fokus nicht verloren geht); Klicks werden
  // verschluckt. Den Grund zeigt die Seite sichtbar und verknüpft ihn per
  // `ariaDescribedby`.
  //
  // `busy` (U4, V2-06/V4-03): Aktion läuft. Wie `ariaDisabled` (fokussierbar,
  // Klick und Absenden ohne Wirkung) plus `aria-busy`; der Knopf behält den Fokus,
  // Tastatur- und Screenreader-Nutzer verlieren ihre Stelle nicht. Natives
  // `disabled` nur für dauerhaft gesperrte Aktionen mit sichtbarem Grund.
  import type { Snippet } from "svelte";

  let {
    variant = "primary",
    size = "md",
    type = "button",
    disabled = false,
    ariaDisabled = false,
    busy = false,
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
    /** Gesperrt, aber fokussierbar (aria-disabled); Klick ohne Wirkung. */
    ariaDisabled?: boolean;
    /** Aktion läuft: aria-disabled + aria-busy, Klick ohne Wirkung, Fokus bleibt. */
    busy?: boolean;
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

  const soft = $derived(ariaDisabled || busy);

  function handleClick(e: MouseEvent) {
    if (soft) {
      e.preventDefault();
      return;
    }
    onclick?.(e);
  }
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
    class:disabled={disabled || soft}
    href={disabled || soft ? undefined : href}
    role={soft && !disabled ? "link" : undefined}
    aria-disabled={disabled || soft ? "true" : undefined}
    aria-busy={busy && !disabled ? "true" : undefined}
    tabindex={disabled ? -1 : soft ? 0 : undefined}
    {id}
    aria-label={ariaLabel}
    aria-describedby={ariaDescribedby}
    data-initial-focus={initialFocus ? "" : undefined}
    {title}
    onclick={disabled ? (e) => e.preventDefault() : handleClick}
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
    class:soft-disabled={soft && !disabled}
    class:busy={busy && !disabled}
    {type}
    {disabled}
    {id}
    aria-disabled={soft && !disabled ? "true" : undefined}
    aria-busy={busy && !disabled ? "true" : undefined}
    aria-label={ariaLabel}
    aria-describedby={ariaDescribedby}
    data-initial-focus={initialFocus ? "" : undefined}
    {title}
    {form}
    onclick={handleClick}
  >
    {@render children?.()}
  </button>
{/if}

<style>
  /* Specs identisch zu tokens.css .btn-primary/.btn-secondary/.btn-ghost/.btn-danger:
     Höhe --control-h, Innenabstand 8/16 px, Schrift geerbt, line-height --lh-tight —
     Button.svelte und rohe <button class="btn-primary"> sind gleich hoch. Farben nur
     über Rollen-Tokens. */
  .btn {
    font: inherit; font-weight: 600; line-height: var(--lh-tight);
    border-radius: var(--radius-control); padding: var(--sp-2) var(--sp-4); border: 1px solid transparent;
    min-height: var(--control-h); min-width: var(--target);
    cursor: pointer; display: inline-flex; align-items: center; justify-content: center; gap: var(--sp-2);
    text-decoration: none;
    /* V3-21: Übergang wie die globalen .btn-* (tokens.css), kein Eindrücken beim Klick. */
    transition: background var(--t-base) var(--ease-apple), border-color var(--t-base) var(--ease-apple);
  }

  .primary { background: var(--accent); color: var(--text-on-accent); border-color: var(--accent); }
  .secondary { background: var(--surface-card); color: var(--accent-on-soft); border-color: var(--line-control); }
  .ghost { background: transparent; color: var(--text-muted); }
  .danger { background: var(--surface-card); color: var(--status-danger-fg); border-color: var(--status-danger-line); }

  @media (hover: hover) {
    .primary:not(.soft-disabled):hover { background: var(--accent-hover); border-color: var(--accent-hover); }
    .secondary:not(.soft-disabled):hover { background: var(--surface-hover); border-color: var(--line-hover); }
    .ghost:not(.soft-disabled):hover { background: var(--surface-page); color: var(--text); }
    .danger:not(.soft-disabled):hover { background: var(--status-danger-bg); }
    a.btn:hover { text-decoration: none; }
  }

  .btn:disabled, .btn.disabled { opacity: .5; cursor: not-allowed; pointer-events: none; }
  /* aria-disabled: bleibt fokussierbar und zeigt den Fokusring, darum ohne pointer-events: none. */
  .btn.soft-disabled { opacity: .5; cursor: not-allowed; }
  .btn.busy { cursor: progress; }
</style>
