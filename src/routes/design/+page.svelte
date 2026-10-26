<script lang="ts">
  // In-App-Design-Galerie (DS-1, U1). Zeigt die echten Svelte-Komponenten gerendert,
  // damit das Look-and-Feel im laufenden Programm geprüft werden kann.
  // Reine Doku-Seite, nicht in der Navigation verlinkt: Aufruf via /design.
  // U1: PageBar, neue Bausteine (MoneyInput, QuantityInput, Toast, Dialog),
  // Tabelle mit Überschrift und Kartenansicht; bei 390 px ohne Seitenüberlauf.
  import Button from "#lib/Button.svelte";
  import Card from "#lib/Card.svelte";
  import Badge from "#lib/Badge.svelte";
  import FormField from "#lib/FormField.svelte";
  import Table from "#lib/Table.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { euro, percent } from "#lib/format.ts";

  let datum = $state("");
  let betrag = $state<number | null>(119000);
  let menge = $state<number | null>(2.5);
  let km = $state<number | null>(null);
  let tage = $state<number | null>(14);
  let dialogResult = $state("");

  async function showDialog(danger: boolean) {
    const ok = await confirmDialog(
      danger
        ? {
            title: "Rechnung stornieren?",
            body: "Es entsteht ein Storno-Beleg mit eigener Nummer. Das lässt sich nicht rückgängig machen.",
            confirmLabel: "Stornieren",
            danger: true,
          }
        : {
            title: "Entwurf speichern?",
            body: "Der Entwurf bleibt änderbar, bis du ihn festschreibst.",
            confirmLabel: "Speichern",
          },
    );
    dialogResult = ok ? "Bestätigt." : "Abgebrochen.";
  }

  const rows = [
    { nr: "RE-2025-0042", kunde: "Mustermann GmbH", datum: "14.05.2025", cents: 119000, tone: "success", status: "Bezahlt" },
    { nr: "RE-2025-0041", kunde: "Bäckerei Huber", datum: "02.05.2025", cents: 42000, tone: "danger", status: "Überfällig" },
    { nr: "RE-2025-0040", kunde: "Kanzlei Berger und Partner Rechtsanwälte mbB", datum: "28.04.2025", cents: 238000, tone: "primary", status: "Festgeschrieben" },
    { nr: "—", kunde: "Studio Nord", datum: "Entwurf", cents: 85000, tone: "neutral", status: "Entwurf" },
  ] as const;
</script>

<PageBar title="Musterseite Design-System" />

<p class="lead">
  Zentrale Tokens und wiederverwendbare Bausteine. Diese Seite rendert die echten Komponenten
  zur Sicht- und Funktionsprüfung.
</p>

<h2 class="sec">Buttons</h2>
<Card>
  <div class="row">
    <Button variant="primary">Festschreiben</Button>
    <Button variant="secondary">Als PDF</Button>
    <Button variant="ghost">Abbrechen</Button>
    <Button variant="danger">Stornieren</Button>
    <Button variant="primary" disabled ariaDescribedby="design-disabled-why">Gesperrt</Button>
  </div>
  <p id="design-disabled-why" class="note">Deaktivierte Buttons erklären sichtbar, warum: hier nur ein Beispiel.</p>
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
        <input id={inputId} class="kb-input" autocomplete="off" value="Mustermann GmbH" />
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
          type="date"
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          aria-required={required || undefined}
          class="kb-input"
          bind:value={datum}
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

<h2 class="sec">Betrag und Menge</h2>
<Card>
  <p class="note">
    Komma oder Punkt als Dezimalzeichen, Tausenderpunkt erlaubt; beim Verlassen erscheint der
    erkannte Wert im deutschen Format. Probier z. B. <code>1.234</code>, <code>12.5</code>,
    <code>0,125</code> oder <code>-3</code>.
  </p>
  <div class="grid2">
    <MoneyInput
      bind:cents={betrag}
      label="Betrag (brutto)"
      hint="Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
    />
    <QuantityInput bind:value={menge} label="Menge" maxFractionDigits={4} />
    <QuantityInput bind:value={km} label="Strecke" unit="km" noun="Strecke" maxFractionDigits={1} />
    <QuantityInput bind:value={tage} label="Zahlungsziel" unit="Tage" noun="Zahl" integer />
  </div>
  <p class="note" aria-live="polite">
    Gebundene Werte: {betrag === null ? "—" : euro(betrag)} · Menge {menge ?? "—"} ·
    {km ?? "—"} km · {tage ?? "—"} Tage
  </p>
</Card>

<h2 class="sec">Meldungen und Dialoge</h2>
<Card>
  <div class="row">
    <Button variant="secondary" onclick={() => flash("Gespeichert.")}>Erfolg zeigen</Button>
    <Button
      variant="secondary"
      onclick={() => flash("Speichern fehlgeschlagen: Server nicht erreichbar.", "error")}
    >
      Fehler zeigen
    </Button>
    <Button
      variant="secondary"
      onclick={() =>
        flash("Paket archiviert.", "ok", undefined, {
          action: { label: "Rückgängig", run: () => flash("Rückgängig gemacht.") },
        })}
    >
      Meldung mit Aktion
    </Button>
  </div>
  <p class="note">
    Erfolgsmeldungen verschwinden nach einigen Sekunden; Fehler und Meldungen mit Aktion bleiben
    stehen, bis du sie schließt.
  </p>
  <div class="row">
    <Button variant="secondary" onclick={() => showDialog(false)}>Dialog öffnen</Button>
    <Button variant="danger" onclick={() => showDialog(true)}>Gefahr-Dialog öffnen</Button>
    {#if dialogResult}<span class="note" role="status">{dialogResult}</span>{/if}
  </div>
</Card>

<h2 class="sec">Kennzahlen</h2>
<div class="metrics">
  <div class="kb-metric"><p class="lbl">Einnahmen 2025</p><div class="val">{euro(2418000)}</div></div>
  <div class="kb-metric"><p class="lbl">Ausgaben 2025</p><div class="val">{euro(964000)}</div></div>
  <div class="kb-metric"><p class="lbl">Überschuss</p><div class="val ok">{euro(1454000)}</div></div>
  <div class="kb-metric"><p class="lbl">Quote</p><div class="val">{percent(60.1)}</div></div>
</div>

<h2 class="sec">Tabelle</h2>
<Table caption="Beispielrechnungen" cards>
  <thead>
    <tr><th>Nr.</th><th>Kunde</th><th>Datum</th><th class="kb-num">Betrag</th><th>Status</th></tr>
  </thead>
  <tbody>
    {#each rows as r (r.nr + r.kunde)}
      <tr>
        <td data-label="Nr." class="docno">{r.nr}</td>
        <td data-label="Kunde">{r.kunde}</td>
        <td data-label="Datum" class="kb-muted">{r.datum}</td>
        <td data-label="Betrag" class="kb-num">{euro(r.cents)}</td>
        <td data-label="Status"><Badge tone={r.tone}>{r.status}</Badge></td>
      </tr>
    {/each}
  </tbody>
</Table>

<style>
  .lead { color: var(--c-text-muted); margin: 0 0 8px; max-width: 70ch; }
  .sec { font-size: var(--fs-md); font-weight: 600; margin: 28px 0 12px; }
  .row { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; }
  .note { font-size: var(--fs-sm); color: var(--text-muted); margin: 10px 0; overflow-wrap: anywhere; }
  .note code { font-family: var(--font-mono); }
  .grid2 { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 20px; align-items: start; }
  .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr)); gap: 12px; }
  .metrics .val { overflow-wrap: anywhere; }
  .val.ok { color: var(--status-success-fg); }
  .docno { font-family: var(--font-mono); white-space: nowrap; }
  @media (max-width: 600px) { .grid2 { grid-template-columns: minmax(0, 1fr); } }
</style>
