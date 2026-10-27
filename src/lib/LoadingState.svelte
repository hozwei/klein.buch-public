<script lang="ts">
  // Ladezustand (U4, V2-02/V4-07; ADR 0041 Nr. 164; design.md §5 „Leer, Laden, Fehler“).
  //
  // Ein Muster für alle Seiten: Text „Wird geladen …“ in einer höflichen
  // Statusmeldung (`role="status"`, 4.1.3) — Screenreader hören, dass etwas lädt.
  // `aria-busy` gehört NICHT an diesen Text (bei `role="status"` unterdrückt es die
  // Ansage), sondern an den Bereich, der nachlädt (z. B. `<Table busy>` oder
  // `aria-busy` an der Karte), und nur, solange er sichtbar Inhalt hat.
  //
  // Rohes Markup ohne Baustein: `<p class="kb-loading" role="status">Wird geladen …</p>`
  // (gleiche Optik über tokens.css).
  let {
    text = "Wird geladen …",
    class: className = "",
  }: {
    /** Abweichender Text nur, wenn er mehr sagt („Vorschau wird berechnet …“). */
    text?: string;
    class?: string;
  } = $props();
</script>

<p class="kb-loading {className}" role="status">{text}</p>
