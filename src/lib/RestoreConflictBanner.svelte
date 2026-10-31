<script lang="ts">
  // Hinweis zur Restore-Vormerkung eines offenen Entwurfs (ADR 0042 Nr. 16,
  // Warteliste 8) für Editor und Detailseite von Rechnung und Angebot. Quelle ist
  // das Lesefeld `restoreConflict` des Entwurfs (keine Listenabfrage mehr).
  //
  // - `flagged`: Nummer war vor der Wiederherstellung festgeschrieben; Ausstellen
  //   bzw. Fertigstellen erst nach dem Abgleich unter Einstellungen → Backups.
  // - `confirmed`: Abgleich bestätigt; im Editor steht VOR dem Speichern, dass
  //   eine Änderung ihn wieder vormerkt.
  import Banner from "#lib/Banner.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import { finalizeVerb, type RestoreDoc } from "#lib/restoreConflict.ts";
  import type { RestoreConflictState } from "#lib/types.ts";

  let {
    conflict,
    number,
    doc,
    where,
    editHref,
  }: {
    conflict: RestoreConflictState | null | undefined;
    number: string;
    doc: RestoreDoc;
    where: "editor" | "detail";
    /** Detailseite: Weg zu „Entwurf bearbeiten“ (fehlt z. B. beim Storno-Entwurf). */
    editHref?: string;
  } = $props();

  const v = $derived(finalizeVerb(doc));
  const BACKUP_HREF = "/settings/backup#bk-numbering-h";
</script>

{#if conflict === "flagged"}
  <Banner kind="warning" title="Nach der Wiederherstellung vorgemerkt.">
    Die Nummer <DocNo value={number} inline /> war vor der Wiederherstellung schon festgeschrieben.
    {#if where === "editor"}
      Pass den Entwurf an das versandte Original an und bestätige danach den Abgleich unter
      <a class="kb-link" href={BACKUP_HREF}>Einstellungen → Backups</a> (Nummernkreise) — erst dann
      lässt er sich {v.inf}. Jede spätere Änderung verlangt einen neuen Abgleich.
    {:else}
      {#if editHref}
        Weicht der Entwurf vom versandten Original ab, pass ihn über
        <a class="kb-link" href={editHref}>Entwurf bearbeiten</a> an.
      {/if}
      {v.noun} geht erst, wenn du unter
      <a class="kb-link" href={BACKUP_HREF}>Einstellungen → Backups</a>
      (Nummernkreise) bestätigt hast, dass der Entwurf dem versandten Original entspricht.
    {/if}
  </Banner>
{:else if conflict === "confirmed"}
  {#if where === "editor"}
    <Banner kind="warning" title="Speichern hebt den Abgleich auf.">
      Du hast nach der Wiederherstellung bestätigt, dass der Entwurf <DocNo value={number} inline /> dem
      versandten Original entspricht. Speicherst du Änderungen, ist er wieder vorgemerkt: Bestätige den
      Abgleich dann unter <a class="kb-link" href={BACKUP_HREF}>Einstellungen → Backups</a>
      (Nummernkreise) erneut, bevor du ihn {v.you}.
    </Banner>
  {:else}
    <Banner kind="info" title="Abgleich nach der Wiederherstellung bestätigt.">
      Der Entwurf <DocNo value={number} inline /> entspricht laut deiner Bestätigung dem versandten
      Original und lässt sich {v.inf}. Jede Änderung am Entwurf verlangt einen neuen Abgleich.
    </Banner>
  {/if}
{/if}
