<script lang="ts" module>
  /** Belegnummern aller Nummernkreise (`domain::numbering`): AN, RE, ST, KO, PV, AV. */
  export const DOC_NO_PATTERN = /\b(?:AN|RE|ST|KO|PV|AV)-\d{4}-\d{4,}\b/g;

  /**
   * Zerlegt einen Text in Teile und markiert Belegnummern, z. B. für Titel
   * („Entwurf RE-2026-0001 bearbeiten“): `{#each splitDocNumbers(t) as p}`.
   */
  export function splitDocNumbers(text: string): { text: string; docNo: boolean }[] {
    const parts: { text: string; docNo: boolean }[] = [];
    let last = 0;
    for (const m of text.matchAll(DOC_NO_PATTERN)) {
      const at = m.index ?? 0;
      if (at > last) parts.push({ text: text.slice(last, at), docNo: false });
      parts.push({ text: m[0], docNo: true });
      last = at + m[0].length;
    }
    if (last < text.length) parts.push({ text: text.slice(last), docNo: false });
    return parts;
  }
</script>

<script lang="ts">
  // Belegnummer (U4, U1-061/U3-030; design.md §4.6): markantes Monospace-Element,
  // nie umbrechend; als Link immer gleich (feine Unterstreichung, Trefferfläche
  // ≥ --target). Stil global in tokens.css (`.kb-docno`), damit auch Druck und
  // Seiten ohne Baustein gleich aussehen.
  //
  //   <DocNo value={inv.docNumber} href={`/invoices/${inv.id}`} />
  //   <DocNo value={r.docNumber} muted />            (storniert/abgegangen)
  //   <DocNo value={nr} href={…} inline />            (Link mitten im Satz, 2.5.8 „inline“)
  //   <DocNo value={null} placeholder="Entwurf" />    (noch ohne Nummer)
  let {
    value,
    href,
    muted = false,
    inline = false,
    placeholder = "—",
    ariaLabel,
  }: {
    /** Belegnummer; leer = `placeholder` (ohne Monospace). */
    value: string | null | undefined;
    /** Ziel, wenn die Nummer ein Link ist. */
    href?: string;
    /** Gedämpft (storniert, abgegangen, verworfen). Status trägt ein Badge daneben. */
    muted?: boolean;
    /** Link im Fließtext: ohne Mindesthöhe (Ausnahme „inline“, 2.5.8). */
    inline?: boolean;
    /** Anzeige ohne Nummer (Entwurf). */
    placeholder?: string;
    /** Zugänglicher Name, wenn die Nummer allein nicht reicht (muss sie enthalten,
     *  z. B. „RE-2026-0042 öffnen“). */
    ariaLabel?: string;
  } = $props();
</script>

{#if !value}
  <span class="kb-docno-empty">{placeholder}</span>
{:else if href}
  <a class="kb-docno" class:kb-docno-muted={muted} class:kb-docno-inline={inline} {href} aria-label={ariaLabel}
    >{value}</a
  >
{:else}
  <span class="kb-docno" class:kb-docno-muted={muted}>{value}</span>
{/if}

<style>
  .kb-docno-empty {
    color: var(--text-muted);
  }
</style>
