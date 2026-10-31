<script lang="ts">
  // Steuerstatus am maßgeblichen Datum eines Belegs (ADR 0042 Nr. 3) als stehender
  // Hinweis über den Editoren (Rechnung, Angebot, Umwandlung, Abo-Vorlage).
  // Weicht der Status vom heutigen ab (Wechsel mitten im Jahr, Rückwirkung oder
  // eingetragener künftiger Wechsel), sagt der Hinweis das ausdrücklich.
  // UI-04: Scheitert die Abfrage, sagen das alle vier Editoren gleich (vorher nur
  // der Rechnungs-Editor); der Text folgt dem maßgeblichen Datum (`dateLabel`).
  import Banner from "#lib/Banner.svelte";
  import { date as fmtDate } from "#lib/format.ts";
  import { statusNoticeText } from "#lib/taxStatus.ts";

  let {
    isKlein,
    day,
    todayKlein,
    dateLabel,
    noun,
    error = null,
    checkedWhen = "beim Speichern",
  }: {
    /** §19 am Datum des Belegs (`null` = noch nicht geladen). */
    isKlein: boolean | null;
    /** Datum, zu dem `isKlein` gehört. */
    day: string | null;
    /** §19 heute (aus den Firmendaten). */
    todayKlein: boolean | null;
    /** „Leistungsdatum“, „Angebotsdatum“, „Ende des ersten Abrechnungszeitraums“ (nach „am“). */
    dateLabel: string;
    /** „auf der Rechnung“, „auf dem Angebot“, „auf den erzeugten Rechnungen“. */
    noun: string;
    /** Ladefehler der Abfrage (`taxStatusAtDate().error`) oder `null`. */
    error?: string | null;
    /** Wann der Server den Status prüft, z. B. „beim Speichern und Ausstellen“. */
    checkedWhen?: string;
  } = $props();

  const text = $derived(
    isKlein === null || !day ? null : statusNoticeText({ isKlein, day: fmtDate(day), todayKlein, dateLabel, noun }),
  );
</script>

{#if text}
  <Banner kind={text.differs ? "warning" : "info"} title={text.title}>
    {text.body} <a class="kb-link" href="/help/kleinunternehmer-regelung">Mehr im Handbuch</a>.
  </Banner>
{/if}
{#if error}
  <Banner kind="warning">
    Der Steuerstatus am {dateLabel} ließ sich nicht laden ({error}). Klein.Buch prüft ihn
    {checkedWhen}.
  </Banner>
{/if}
