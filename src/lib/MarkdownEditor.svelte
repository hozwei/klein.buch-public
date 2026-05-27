<script lang="ts">
  // Leichter Markdown-Editor (Block P2b+): Textfeld + Formatierungs-Toolbar.
  // KEIN schwergewichtiges WYSIWYG — die Buttons fügen nur die Markdown-Zeichen
  // an der Cursor-/Auswahl-Position ein. Die exakte Darstellung liefert die
  // Live-Vorschau daneben + die PDF-Vorschau.
  import { tick } from "svelte";

  let {
    value = $bindable(""),
    rows = 14,
    placeholder = "",
  }: { value?: string; rows?: number; placeholder?: string } = $props();

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

<div class="md-editor">
  <div class="md-toolbar">
    <button type="button" title="Überschrift" onclick={() => prefixLines("# ", "Überschrift")}>H1</button>
    <button type="button" title="Unterüberschrift" onclick={() => prefixLines("## ", "Überschrift")}>H2</button>
    <span class="sep"></span>
    <button type="button" title="Fett" class="b" onclick={() => wrap("**", "fett")}>F</button>
    <button type="button" title="Kursiv" class="i" onclick={() => wrap("*", "kursiv")}>K</button>
    <span class="sep"></span>
    <button type="button" title="Aufzählung" onclick={() => prefixLines("- ", "Punkt")}>• Liste</button>
    <button type="button" title="Nummerierte Liste" onclick={() => prefixLines("1. ", "Punkt")}>1. Liste</button>
  </div>
  <textarea bind:this={el} bind:value {rows} {placeholder}></textarea>
</div>

<style>
  .md-editor {
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-md);
    overflow: hidden;
    background: var(--c-surface);
  }
  .md-toolbar {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    flex-wrap: wrap;
    padding: 5px 6px;
    background: var(--c-surface-2, #eef1f3);
    border-bottom: 1px solid var(--c-border);
  }
  .md-toolbar button {
    font-size: 0.8rem;
    padding: 3px 9px;
    border: 1px solid var(--c-border-strong);
    border-radius: var(--r-sm);
    background: var(--c-surface);
    color: var(--c-text);
    cursor: pointer;
  }
  .md-toolbar button:hover {
    background: var(--c-primary-50);
    border-color: var(--c-primary-300);
  }
  .md-toolbar button.b { font-weight: 700; }
  .md-toolbar button.i { font-style: italic; }
  .md-toolbar .sep {
    width: 1px;
    align-self: stretch;
    background: var(--c-border);
    margin: 2px 3px;
  }
  textarea {
    display: block;
    width: 100%;
    box-sizing: border-box;
    border: none;
    padding: 10px 12px;
    resize: vertical;
    font-family: var(--font-mono, monospace);
    font-size: 0.9rem;
    background: var(--c-surface);
    color: var(--c-text);
  }
  textarea:focus {
    outline: none;
  }
</style>
