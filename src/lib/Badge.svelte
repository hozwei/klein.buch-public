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

  .neutral { background: #eef1f3; color: #51616b; }
  .primary { background: var(--c-primary-100); color: var(--c-primary-800); }
  .info    { background: var(--c-info-50); color: var(--c-info-700); }
  .success { background: var(--c-success-50); color: var(--c-success-700); }
  .warning { background: var(--c-warning-50); color: var(--c-warning-700); }
  .danger  { background: var(--c-danger-50); color: var(--c-danger-700); }
</style>
