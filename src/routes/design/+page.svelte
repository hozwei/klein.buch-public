<script lang="ts">
  // In-App-Design-Galerie (DS-1). Zeigt die echten Svelte-Komponenten gerendert,
  // damit das Look-and-Feel im laufenden Programm geprüft werden kann.
  // Reine Doku-Seite, nicht in der Navigation verlinkt: Aufruf via /design.
  import Button from "$lib/Button.svelte";
  import Card from "$lib/Card.svelte";
  import Badge from "$lib/Badge.svelte";
  import FormField from "$lib/FormField.svelte";
  import Table from "$lib/Table.svelte";

  let datum = $state("");
</script>

<h1 class="title">Design-System</h1>
<p class="lead">DS-1: zentrale Tokens + wiederverwendbare Komponenten. Diese Seite rendert die echten Komponenten zur Sicht- und Funktionsprüfung.</p>

<h2 class="sec">Buttons</h2>
<Card>
  <div class="row">
    <Button variant="primary">Festschreiben</Button>
    <Button variant="secondary">Als PDF</Button>
    <Button variant="ghost">Abbrechen</Button>
    <Button variant="danger">Stornieren</Button>
    <Button variant="primary" size="sm">Speichern</Button>
    <Button variant="primary" disabled>Gesperrt</Button>
  </div>
</Card>

<h2 class="sec">Status-Badges</h2>
<Card>
  <div class="row">
    <Badge tone="neutral">Entwurf</Badge>
    <Badge tone="info">Versendet</Badge>
    <Badge tone="primary">Festgeschrieben</Badge>
    <Badge tone="success">Bezahlt</Badge>
    <Badge tone="warning">Teilbezahlt</Badge>
    <Badge tone="danger">Überfällig</Badge>
    <Badge tone="neutral" strike>Storniert</Badge>
  </div>
</Card>

<h2 class="sec">Formularfelder</h2>
<Card>
  <div class="grid2">
    <!-- R5-016: FormField gibt inputId/describedBy/invalid/required als Snippet-
         Param raus — der Slot bindet sie ans <input>, damit Screen-Reader das
         Label, den Hinweis und den Fehler verlässlich vorlesen. -->
    <FormField label="Kunde">
      {#snippet children({ inputId })}
        <input id={inputId} class="kb-input" value="Mustermann GmbH" />
      {/snippet}
    </FormField>
    <FormField
      label="Betrag (brutto)"
      hint="Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
    >
      {#snippet children({ inputId, describedBy })}
        <input
          id={inputId}
          aria-describedby={describedBy}
          class="kb-input kb-num"
          value="1.190,00 €"
        />
      {/snippet}
    </FormField>
    <FormField
      label="Rechnungsdatum"
      required
      error={datum ? "" : "Pflichtfeld — bitte ein Datum angeben."}
    >
      {#snippet children({ inputId, describedBy, invalid, required })}
        <input
          id={inputId}
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          aria-required={required || undefined}
          class="kb-input"
          bind:value={datum}
          placeholder="TT.MM.JJJJ"
        />
      {/snippet}
    </FormField>
    <FormField label="PDF-Vorlage">
      {#snippet children({ inputId })}
        <select id={inputId} class="kb-input">
          <option>Modern</option>
          <option>Klassisch</option>
          <option>Minimal</option>
        </select>
      {/snippet}
    </FormField>
  </div>
</Card>

<h2 class="sec">Kennzahlen</h2>
<div class="metrics">
  <div class="kb-metric"><p class="lbl">Einnahmen 2025</p><div class="val">24.180 €</div></div>
  <div class="kb-metric"><p class="lbl">Ausgaben 2025</p><div class="val">9.640 €</div></div>
  <div class="kb-metric"><p class="lbl">Überschuss</p><div class="val" style="color:var(--c-success-700)">14.540 €</div></div>
  <div class="kb-metric"><p class="lbl">Offene Posten</p><div class="val">2.380 €</div></div>
</div>

<h2 class="sec">Tabelle</h2>
<Table>
  <thead>
    <tr><th>Nr.</th><th>Kunde</th><th>Datum</th><th class="kb-num">Betrag</th><th>Status</th></tr>
  </thead>
  <tbody>
    <tr><td>RE-2025-0042</td><td>Mustermann GmbH</td><td class="kb-muted">14.05.2025</td><td class="kb-num">1.190,00 €</td><td><Badge tone="success">Bezahlt</Badge></td></tr>
    <tr><td>RE-2025-0041</td><td>Bäckerei Huber</td><td class="kb-muted">02.05.2025</td><td class="kb-num">420,00 €</td><td><Badge tone="danger">Überfällig</Badge></td></tr>
    <tr><td>RE-2025-0040</td><td>Kanzlei Berger</td><td class="kb-muted">28.04.2025</td><td class="kb-num">2.380,00 €</td><td><Badge tone="primary">Festgeschrieben</Badge></td></tr>
    <tr><td class="kb-muted">—</td><td>Studio Nord</td><td class="kb-muted">Entwurf</td><td class="kb-num">850,00 €</td><td><Badge tone="neutral">Entwurf</Badge></td></tr>
  </tbody>
</Table>

<style>
  .title { font-size: var(--fs-3xl); font-weight: 700; letter-spacing: -.02em; margin: 0 0 4px; }
  .lead { color: var(--c-text-muted); margin: 0 0 8px; max-width: 70ch; }
  .sec { font-size: var(--fs-md); font-weight: 600; margin: 28px 0 12px; }
  .row { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 0 20px; }
  .metrics { display: grid; grid-template-columns: repeat(4, minmax(0,1fr)); gap: 12px; }
  @media (max-width: 760px) { .grid2, .metrics { grid-template-columns: 1fr 1fr; } }
</style>
