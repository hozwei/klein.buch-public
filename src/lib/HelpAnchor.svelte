<script lang="ts">
  // G2-DOC.4-A — Kontext-Hilfe-Anker.
  //
  // Mini-Button neben einer Überschrift / einem Pflicht-Button, der via
  // `goto("/help/<slug>")` direkt in das passende Kapitel des Handbuchs
  // springt. Bewusst klein, ohne Text — die Optik ist die etablierte
  // `?`-Konvention.
  //
  // Props:
  //   slug    — Front-Matter-`slug` einer Handbuch-Seite. Pflicht. Wird
  //             zur Build-Zeit von `handbook_anchors_test` gegen den
  //             Resource-Index geprüft (siehe docs/architecture/handbook.md).
  //   heading — Optional. Tiefer Sprung auf einen H2/H3-Anker innerhalb
  //             der Seite. Wird über denselben Slugifier (`slugifyHeading`)
  //             gejagt wie der Markdown-Renderer in `$lib/handbook/render`,
  //             damit der Hash identisch zur Heading-ID ist.
  //   label   — Optional. Überschreibt den Tooltip-Text. Ohne Override
  //             wird der Handbuch-Titel aus dem Index gezogen, mit Fallback
  //             auf den Slug (falls der Eintrag zur Laufzeit fehlt — sollte
  //             durch den Verify-Test nicht passieren, ist aber harmlos).
  //
  // Bewusst NICHT:
  // - neue JS-Dependency (Icon kommt als Inline-SVG)
  // - History-Pollution (`replaceState`): wir wollen, dass der Nutzer mit
  //   dem Browser-Zurück aus dem Handbuch zurückkommt.
  // - Tab-Trap / Modal-Verhalten: der Button öffnet eine normale Route,
  //   keine Overlay-Hilfe.

  import { goto } from "$app/navigation";
  import { getEntry } from "$lib/handbook";
  import { slugifyHeading } from "$lib/handbook/render";

  let {
    slug,
    heading,
    label,
  }: {
    slug: string;
    heading?: string;
    label?: string;
  } = $props();

  const handbookTitle = $derived(getEntry(slug)?.title);
  const tooltipText = $derived(label ?? handbookTitle ?? slug);

  const target = $derived(
    heading ? `/help/${slug}#${slugifyHeading(heading)}` : `/help/${slug}`,
  );

  function open(e: MouseEvent) {
    e.preventDefault();
    void goto(target);
  }
</script>

<button
  type="button"
  class="help-anchor"
  aria-label={`Hilfe öffnen: ${tooltipText}`}
  title={`Hilfe: ${tooltipText}`}
  onclick={open}
>
  <svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
    <circle
      cx="8"
      cy="8"
      r="7"
      fill="none"
      stroke="currentColor"
      stroke-width="1.4"
    />
    <path
      d="M5.9 6.3a2.2 2.2 0 0 1 4.3 0c0 .9-.5 1.4-1.2 1.9-.6.5-1 .8-1 1.5v.3"
      fill="none"
      stroke="currentColor"
      stroke-width="1.4"
      stroke-linecap="round"
    />
    <circle cx="8" cy="12.2" r=".9" fill="currentColor" />
  </svg>
</button>

<style>
  .help-anchor {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    margin-left: 6px;
    border: 0;
    background: transparent;
    color: var(--c-primary-600);
    border-radius: var(--r-pill);
    cursor: pointer;
    line-height: 0;
    vertical-align: middle;
    transition:
      background 0.12s,
      color 0.12s;
  }
  .help-anchor:hover {
    background: var(--c-primary-50);
    color: var(--c-primary-700);
  }
  .help-anchor:focus-visible {
    outline: 2px solid var(--c-primary-500);
    outline-offset: 2px;
  }
</style>
