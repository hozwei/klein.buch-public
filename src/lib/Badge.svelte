<script lang="ts">
  // Status-Badge (DS-1). tone steuert die Farbe; strike = durchgestrichen (Storno).
  // Konvention Klein.Buch:
  //   neutral = Entwurf, primary = Festgeschrieben, info = Versendet,
  //   success = Bezahlt, warning = Teilbezahlt, danger = Überfällig,
  //   neutral + strike = Storniert.
  import type { Snippet } from "svelte";

  let {
    tone = "neutral",
    strike = false,
    children,
  }: {
    tone?: "neutral" | "primary" | "info" | "success" | "warning" | "danger";
    strike?: boolean;
    children?: Snippet;
  } = $props();
</script>

<span
  class="badge"
  class:neutral={tone === "neutral"}
  class:primary={tone === "primary"}
  class:info={tone === "info"}
  class:success={tone === "success"}
  class:warning={tone === "warning"}
  class:danger={tone === "danger"}
  class:strike
>
  {@render children?.()}
</span>

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: var(--fs-xs); font-weight: 600;
    padding: 3px 10px; border-radius: var(--r-pill); white-space: nowrap;
  }
  .badge::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: currentColor; opacity: .85; }
  .badge.strike { text-decoration: line-through; }

  /* Rollen-Tokens; Text auf Fläche jeweils ≥ 4,5:1 (design.md §3.2). */
  .neutral { background: var(--status-neutral-bg); color: var(--status-neutral-fg); }
  .primary { background: var(--c-primary-100); color: var(--c-primary-800); }
  .info    { background: var(--status-info-bg); color: var(--status-info-fg); }
  .success { background: var(--status-success-bg); color: var(--status-success-fg); }
  .warning { background: var(--status-warning-bg); color: var(--status-warning-fg); }
  .danger  { background: var(--status-danger-bg); color: var(--status-danger-fg); }
</style>
