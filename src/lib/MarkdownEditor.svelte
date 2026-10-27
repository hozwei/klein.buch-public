<script lang="ts">
  // Leichter Markdown-Editor (Block P2b+): Textfeld + Formatierungs-Toolbar.
  // KEIN schwergewichtiges WYSIWYG — die Buttons fügen nur die Markdown-Zeichen
  // an der Cursor-/Auswahl-Position ein. Die exakte Darstellung liefert die
  // Live-Vorschau daneben + die PDF-Vorschau.
  //
  // U1: sichtbares Label (Standard „Beschreibung (Markdown)“, per `label`
  // änderbar oder mit `labelHidden` nur für Screenreader), Werkzeugleiste
  // als benannte Gruppe mit Trefferflächen ≥ --target, Rahmen ≥ 3:1 und
  // sichtbarer Fokusring im Textfeld.
  // U4 (U2-018): Werkzeugleiste mit Klartext („Überschrift“, „Fett“ …) statt
  // Kürzeln mit Tooltip — auf Touch gibt es kein Hover.
  import { tick } from "svelte";
  import { randomId } from "#lib/browser.ts";

  let {
    value = $bindable(""),
    rows = 14,
    placeholder = "",
    label = "Beschreibung (Markdown)",
    labelHidden = false,
    id,
  }: {
    value?: string;
    rows?: number;
    placeholder?: string;
    label?: string;
    labelHidden?: boolean;
    id?: string;
  } = $props();

  const uid = randomId();
  const fieldId = $derived(id ?? `md-${uid}`);

  let el: HTMLTextAreaElement;

  // Wendet eine Transformation auf die aktuelle Auswahl an und stellt danach
  // eine sinnvolle Cursor-/Auswahl-Position wieder her.
  async function apply(
    transform: (sel: string) => { text: string; selStart: number; selEnd: number },
  ) {
    const start = el?.selectionStart ?? value.length;
    const end = el?.selectionEnd ?? value.length;
    const before = value.slice(0, start);
    const sel = value.slice(start, end);
    const after = value.slice(end);
    const r = transform(sel);
    value = before + r.text + after;
    await tick();
    el.focus();
    el.selectionStart = start + r.selStart;
    el.selectionEnd = start + r.selEnd;
  }

  function wrap(mark: string, ph: string) {
    apply((sel) => {
      const inner = sel || ph;
      return { text: `${mark}${inner}${mark}`, selStart: mark.length, selEnd: mark.length + inner.length };
    });
  }

  function prefixLines(prefix: string, ph: string) {
    apply((sel) => {
      const src = sel || ph;
      const text = src
        .split("\n")
        .map((l) => prefix + l)
        .join("\n");
      return { text, selStart: prefix.length, selEnd: text.length };
    });
  }

</script>

<div class="md-wrap">
<label class="md-label" class:kb-visually-hidden={labelHidden} for={fieldId}>{label}</label>
<div class="md-editor">
  <div class="md-toolbar" role="group" aria-label="Formatierung">
    <!-- U4 (U2-018): sichtbare Klartext-Beschriftung statt „H1/H2/F/K“ mit Tooltip;
         Name = sichtbarer Text, Symbole davor sind Dekoration. -->
    <button type="button" onclick={() => prefixLines("# ", "Überschrift")}>Überschrift</button>
    <button type="button" onclick={() => prefixLines("## ", "Zwischentitel")}>Zwischentitel</button>
    <span class="sep" aria-hidden="true"></span>
    <button type="button" class="b" onclick={() => wrap("**", "fett")}>Fett</button>
    <button type="button" class="i" onclick={() => wrap("*", "kursiv")}>Kursiv</button>
    <span class="sep" aria-hidden="true"></span>
    <button type="button" onclick={() => prefixLines("- ", "Punkt")}><span class="sym" aria-hidden="true">•</span>Aufzählung</button>
    <button type="button" onclick={() => prefixLines("1. ", "Punkt")}><span class="sym" aria-hidden="true">1.</span>Nummerierung</button>
  </div>
  <textarea id={fieldId} bind:this={el} bind:value {rows} {placeholder} spellcheck="true"></textarea>
</div>
</div>

<style>
  .md-wrap { min-width: 0; }
  .md-label {
    display: block;
    font-size: var(--fs-sm);
    font-weight: 600;
    margin: 0 0 var(--sp-1);
  }
  .md-editor {
    border: 1px solid var(--line-control);
    border-radius: var(--radius-control);
    overflow: hidden;
    background: var(--surface-card);
  }
  .md-toolbar {
    display: flex;
    align-items: center;
    gap: var(--sp-1);
    flex-wrap: wrap;
    padding: var(--sp-1);
    background: var(--surface-sunken);
  }
  .md-toolbar button {
    min-width: var(--target);
    min-height: max(var(--target), 28px);
    font: inherit;
    font-size: var(--fs-sm);
    padding: 0 var(--sp-2);
    border: 1px solid var(--line-control);
    border-radius: var(--r-sm);
    background: var(--surface-card);
    color: var(--text);
    cursor: pointer;
  }
  @media (hover: hover) {
    .md-toolbar button:hover {
      background: var(--surface-hover);
      border-color: var(--line-hover);
    }
  }
  .md-toolbar button.b { font-weight: 700; }
  .md-toolbar button.i { font-style: italic; }
  .md-toolbar .sym { margin-right: var(--sp-1); color: var(--text-muted); }
  .md-toolbar .sep {
    width: 1px;
    align-self: stretch;
    background: var(--line);
    margin: 2px 3px;
  }
  /* Obere Kante = Grenze zur Werkzeugleiste (≥ 3:1); Fokusring nach innen,
     weil .md-editor überstehende Ringe abschneidet. */
  textarea {
    display: block;
    width: 100%;
    box-sizing: border-box;
    border: 0;
    border-top: 1px solid var(--line-control);
    border-radius: 0;
    padding: 10px 12px;
    resize: vertical;
    font-family: var(--font-mono, monospace);
    font-size: var(--fs-base);
    background: var(--surface-card);
    color: var(--text);
  }
  .md-editor textarea:focus,
  .md-editor textarea:focus-visible {
    outline: var(--focus-width) solid var(--focus-ring);
    outline-offset: var(--focus-offset-inset);
  }
</style>
