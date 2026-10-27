<script lang="ts">
  // Badge (DS-1, design.md §4.9). Zwei Arten:
  // - `kind="status"` (Standard): Zustand eines Belegs oder Objekts. `tone` steuert die
  //   Farbe, davor ein Punkt in Statusfarbe; strike = durchgestrichen (Storniert/Verworfen).
  //   neutral = Entwurf, primary = Festgeschrieben/Ausgestellt, info = Versendet,
  //   success = Bezahlt/Aktiv, warning = Teilbezahlt, danger = Überfällig,
  //   neutral + strike = Storniert. Wörter aus den Status-Helfern (invoiceStatus.ts …).
  // - `kind="tag"` (U4, V3-08): Merkmal oder Art (Storno-Beleg, Abo, §13b, Modus,
  //   „80 % betrieblich“, Standard). Immer neutral, ohne Punkt und mit kleiner Ecke statt
  //   Pille — der Punkt bedeutet allein Status (design.md §2 Nr. 3). `tone` gilt dort nicht.
  import type { Snippet } from "svelte";

  let {
    tone = "neutral",
    kind = "status",
    strike = false,
    children,
  }: {
    tone?: "neutral" | "primary" | "info" | "success" | "warning" | "danger";
    kind?: "status" | "tag";
    strike?: boolean;
    children?: Snippet;
  } = $props();

  const t = $derived(kind === "tag" ? "neutral" : tone);
</script>

<span
  class="badge"
  class:tag={kind === "tag"}
  class:neutral={t === "neutral"}
  class:primary={t === "primary"}
  class:info={t === "info"}
  class:success={t === "success"}
  class:warning={t === "warning"}
  class:danger={t === "danger"}
  class:strike
>
  {@render children?.()}
</span>

<style>
  .badge {
    display: inline-flex; align-items: center; gap: 6px;
    font-size: var(--type-caption); font-weight: 600; line-height: var(--lh-snug);
    padding: 3px 10px; border-radius: var(--radius-pill); white-space: nowrap;
  }
  .badge::before { content: ""; width: 6px; height: 6px; border-radius: 50%; background: currentColor; opacity: .85; }
  .badge.strike { text-decoration: line-through; }
  /* Merkmal/Art: ohne Punkt, kleine Ecke, gleiche Höhe wie ein Status-Badge. Lange
     Merkmale („Aus Paket: …“) brechen um statt aus der Zelle zu ragen. */
  .badge.tag { padding: 3px 8px; border-radius: var(--r-sm); white-space: normal; max-width: 100%; overflow-wrap: break-word; }
  .badge.tag::before { content: none; }

  /* Rollen-Tokens; Text auf Fläche jeweils ≥ 4,5:1 (design.md §3.3). */
  .neutral { background: var(--status-neutral-bg); color: var(--status-neutral-fg); }
  .primary { background: var(--status-primary-bg); color: var(--status-primary-fg); }
  .info    { background: var(--status-info-bg); color: var(--status-info-fg); }
  .success { background: var(--status-success-bg); color: var(--status-success-fg); }
  .warning { background: var(--status-warning-bg); color: var(--status-warning-fg); }
  .danger  { background: var(--status-danger-bg); color: var(--status-danger-fg); }
</style>
