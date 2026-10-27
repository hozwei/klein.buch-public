<script lang="ts">
  // Inline-Banner für legitim PERSISTENTE Zustände, die kein flüchtiger Toast sein
  // dürfen: "… nicht gefunden", "Firmendaten fehlen", Versions-Inkompatibilität,
  // Server-Ablehnungen mit mehreren Gründen (ADR 0041 Nr. 22).
  // Bleibt im Seitenfluss stehen, bis der Zustand behoben ist.
  //
  // Für flüchtiges Aktions-Feedback (gespeichert, gesendet, fehlgeschlagen) gilt
  // weiterhin flash() / Toast — NICHT dieses Banner.
  //
  // Arten (U4: vollständig wie die Status-Rollen): error (role="alert"),
  // warning, info, success (role="status"). Der Text trägt die Aussage, Farbe und
  // Kante sind nur Zusatz (1.4.1); mit `title` beginnt der Text mit einer
  // fetten Kurzaussage.
  //
  // U4 (V2-21, ADR 0041 Nr. 166): einziger Baustein für Hinweisflächen — auch für
  // „Link-Banner“ wie „2 Kosten-Abos sind fällig – ansehen und buchen“: `href` +
  // `linkLabel` setzen einen unterstrichenen Link ans Ende (Ziel ≥ `--target`).
  // Links im Text erben die Statusfarbe (≥ 4,5:1 auf der Fläche) und sind
  // unterstrichen; Knöpfe im Banner (`actions`) stehen am Ende in `.kb-actions`.
  // Seiteneigene Flächen (`.info`, `.edit-hint`, `.due-banner`) und die alten
  // Klassen `.info-banner`/`.caveat`/`.danger-banner` ersetzen die Seiten hierdurch.
  import type { Snippet } from "svelte";

  let {
    kind = "error",
    title,
    href,
    linkLabel,
    id,
    children,
    actions,
  }: {
    kind?: "error" | "warning" | "info" | "success";
    /** Fette Kurzaussage vor dem Text („Rechnung nicht gefunden.“). */
    title?: string;
    /** Link am Ende des Banners (nächster Schritt), mit `linkLabel`. */
    href?: string;
    linkLabel?: string;
    /** Für `aria-describedby` von Feldern oder Knöpfen. */
    id?: string;
    children?: Snippet;
    /** Knöpfe am Ende (z. B. „Erneut laden“). */
    actions?: Snippet;
  } = $props();
</script>

<div class="banner {kind}" role={kind === "error" ? "alert" : "status"} {id}>
  {#if title}<strong class="banner-title">{title}</strong>{" "}{/if}{@render children?.()}
  {#if href && linkLabel}{" "}<a class="banner-link kb-link" {href}>{linkLabel}</a>{/if}
  {#if actions}<div class="kb-actions kb-start banner-actions">{@render actions()}</div>{/if}
</div>

<style>
  .banner {
    padding: var(--sp-3) var(--sp-4);
    border-radius: var(--radius-control);
    border: 1px solid transparent;
    font-size: var(--type-body);
    margin: var(--sp-2) 0 var(--sp-4);
    line-height: 1.45;
  }
  /* Rollen-Tokens; Kante links betont, Status nie nur über Farbe (Text trägt ihn). */
  .banner { border-left-width: 4px; overflow-wrap: anywhere; }
  .banner-title { font-weight: 700; }
  /* Links im Banner: Statusfarbe (≥ 4,5:1 auf der Fläche), immer unterstrichen. */
  .banner :global(a:not([class*="btn"])) { color: inherit; text-decoration: underline; text-underline-offset: 0.15em; }
  .banner-link { font-weight: 600; display: inline-flex; align-items: center; min-height: var(--target); }
  .banner-actions { margin-top: var(--sp-2); }
  .banner.error {
    background: var(--status-danger-bg);
    color: var(--status-danger-fg);
    border-color: var(--status-danger-line);
  }
  .banner.warning {
    background: var(--status-warning-bg);
    color: var(--status-warning-fg);
    border-color: var(--status-warning-line);
  }
  .banner.info {
    background: var(--status-info-bg);
    color: var(--status-info-fg);
    border-color: var(--status-info-line);
  }
  .banner.success {
    background: var(--status-success-bg);
    color: var(--status-success-fg);
    border-color: var(--status-success-line);
  }
</style>
