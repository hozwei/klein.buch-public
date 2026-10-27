<script lang="ts">
  // Pflicht-Legende (U4, V2-18; ADR 0041 Nr. 165, ändert den Wortlaut von Nr. 36).
  //
  // Ein Wortlaut für alle Formulare: „Felder mit * sind Pflicht.“ Der Stern ist für
  // Screenreader verborgen wie hinter den Labels; vorgelesen wird „Felder mit Stern
  // sind Pflicht.“ (vorher „Felder mit sind Pflicht.“). Steht über dem Formular
  // (design.md §5 „Editoren“). Rohes Markup ohne Baustein: Klasse `.kb-req-legend`
  // mit demselben Inhalt.
  let { class: className = "" }: { class?: string } = $props();
</script>

<p class="kb-req-legend {className}">Felder mit <span class="kb-req-star" aria-hidden="true">*</span><span class="kb-visually-hidden">Stern</span> sind Pflicht.</p>
