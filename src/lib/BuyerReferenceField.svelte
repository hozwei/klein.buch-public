<script lang="ts">
  // Feld „Leitweg-ID“ (BT-10, Käuferreferenz) mit Formathilfe (ADR 0042 Nr. 12).
  // Gespeichert am Entwurf bzw. an der Abo-Vorlage; die Meldung (Länge,
  // Steuerzeichen) steht am Feld, ein abweichender Aufbau ist nur ein Hinweis —
  // BT-10 darf auch eine andere Käuferreferenz sein.
  import { buyerReferenceError, buyerReferenceFormatHint } from "#lib/serviceDate.ts";

  let {
    value = $bindable(""),
    id = "buyer-ref",
    showErrors = false,
    note = "",
    class: cls = "",
  }: {
    value?: string;
    id?: string;
    showErrors?: boolean;
    /** Zusatz unter der Formathilfe (z. B. „Gilt für jede erzeugte Rechnung.“). */
    note?: string;
    class?: string;
  } = $props();

  // Steuerzeichen und Überlänge sofort melden (eindeutig), nicht erst beim Speichern.
  const err = $derived(buyerReferenceError(value));
  const shownErr = $derived(err && (showErrors || value.trim()) ? err : "");
  const formatHint = $derived(buyerReferenceFormatHint(value));
</script>

<label class="bref {cls}" for={id}>
  <span>Leitweg-ID (optional)</span>
  <input
    {id}
    type="text"
    class="kb-w-medium"
    bind:value
    autocomplete="off"
    spellcheck="false"
    aria-invalid={shownErr ? "true" : undefined}
    aria-describedby={shownErr ? `${id}-err ${id}-hint` : `${id}-hint`}
  />
  {#if shownErr}<span id="{id}-err" class="field-err">{shownErr}</span>{/if}
  <span id="{id}-hint" class="field-hint">
    Nur bei öffentlichen Auftraggebern (Behörden): Die Leitweg-ID steht als Käuferreferenz in der
    E-Rechnung. Aufbau: Grobadressierung, optional Feinadressierung, zweistellige Prüfziffer, durch
    Bindestriche getrennt (z.&nbsp;B. 04011000-12345-67). Leer lassen, wenn der Auftraggeber keine
    Behörde ist.{#if formatHint}{" "}{formatHint}{/if}{#if note}{" "}{note}{/if}
  </span>
</label>

<style>
  .bref { display: flex; flex-direction: column; gap: 4px; font-size: var(--fs-sm); font-weight: 600; color: var(--text-muted); }
  .bref input {
    padding: 9px 11px; border: 1px solid var(--line-control); border-radius: var(--r-md);
    font: inherit; font-weight: 400; color: var(--text); background: var(--surface-card);
  }
  .field-err { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--status-danger-fg); }
  .field-hint { display: block; margin-top: 4px; font-size: var(--fs-xs); font-weight: 400; color: var(--text-muted); }
</style>
