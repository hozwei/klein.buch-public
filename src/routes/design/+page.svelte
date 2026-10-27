<script lang="ts">
  // In-App-Design-Galerie (DS-1, U1). Zeigt die echten Svelte-Komponenten gerendert,
  // damit das Look-and-Feel im laufenden Programm geprüft werden kann.
  // Reine Doku-Seite, nicht in der Navigation verlinkt: Aufruf via /design.
  // U1: PageBar, neue Bausteine (MoneyInput, QuantityInput, Toast, Dialog),
  // Tabelle mit Überschrift und Kartenansicht; bei 390 px ohne Seitenüberlauf.
  // U4 (V3-19): nach design.md §4/§9 — „Abbrechen“ als secondary vor der einen primary,
  // Badges vollständig (Status mit Punkt, Merkmale ohne Punkt), Datumsfeld `kb-w-date`,
  // Vorlage „Standard“, Toast mit Aktion über `flash(text, "ok", { action })`, Belegnummern
  // über DocNo, Linkliste, Aufstellung mit Summe, Banner; Abstände nur aus der 4-px-Skala.
  import Button from "#lib/Button.svelte";
  import Card from "#lib/Card.svelte";
  import Badge from "#lib/Badge.svelte";
  import Banner from "#lib/Banner.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import FormField from "#lib/FormField.svelte";
  import Table from "#lib/Table.svelte";
  import PageBar from "#lib/PageBar.svelte";
  import MoneyInput from "#lib/MoneyInput.svelte";
  import QuantityInput from "#lib/QuantityInput.svelte";
  import { flash } from "#lib/toast.svelte.ts";
  import { confirmDialog } from "#lib/confirm.svelte.ts";
  import { euro, percent } from "#lib/format.ts";
  import { PDF_TEMPLATE_NAMES, pdfTemplateDefaultOptionLabel, pdfTemplateLabel } from "#lib/labels.ts";

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
            title: "Rechnung ausstellen?",
            body: "Danach ist die Rechnung festgeschrieben; Änderungen gehen nur noch per Storno.",
            confirmLabel: "Rechnung ausstellen",
          },
    );
    dialogResult = ok ? "Bestätigt." : "Abgebrochen – nichts geändert.";
  }

  const rows = [
    { nr: "RE-2025-0042", kunde: "Mustermann GmbH", datum: "14.05.2025", cents: 119000, tone: "success", status: "Bezahlt", strike: false },
    { nr: "RE-2025-0041", kunde: "Bäckerei Huber", datum: "02.05.2025", cents: 42000, tone: "danger", status: "Überfällig", strike: false },
    { nr: "RE-2025-0040", kunde: "Kanzlei Berger und Partner Rechtsanwälte mbB", datum: "28.04.2025", cents: 238000, tone: "primary", status: "Ausgestellt", strike: false },
    { nr: "ST-2025-0003", kunde: "Bäckerei Huber", datum: "20.04.2025", cents: -42000, tone: "primary", status: "Ausgestellt", strike: false },
    { nr: null, kunde: "Studio Nord", datum: "—", cents: 85000, tone: "neutral", status: "Entwurf", strike: false },
  ] as const;
</script>

<PageBar title="Musterseite Design-System" />

<p class="lead">
  Zentrale Tokens und wiederverwendbare Bausteine. Diese Seite rendert die echten Komponenten
  zur Sicht- und Funktionsprüfung.
</p>

<h2 class="sec">Knöpfe</h2>
<Card>
  <p class="note">Formularende: „Abbrechen“ zuerst, die eine Hauptaktion zuletzt.</p>
  <div class="kb-actions kb-start">
    <Button variant="secondary">Abbrechen</Button>
    <Button variant="primary">Rechnung ausstellen</Button>
  </div>
  <p class="note">Nebenaktionen: secondary; ghost nur neben einer sichtbaren Aktion derselben Gruppe.</p>
  <div class="kb-actions kb-start">
    <Button variant="secondary">PDF öffnen</Button>
    <Button variant="ghost">Abhaken</Button>
  </div>
  <p class="note">Gefahr und gesperrt (der Grund steht daneben):</p>
  <div class="kb-actions kb-start">
    <Button variant="danger">Stornieren</Button>
    <Button variant="secondary" disabled ariaDescribedby="design-disabled-why">Senden</Button>
  </div>
  <p id="design-disabled-why" class="note">Senden ist gesperrt: Es ist noch kein Postfach eingerichtet.</p>
</Card>

<h2 class="sec">Badges</h2>
<Card>
  <p class="note">Status (Punkt in Statusfarbe, design.md §4.9):</p>
  <div class="row">
    <Badge tone="neutral">Entwurf</Badge>
    <Badge tone="primary">Ausgestellt</Badge>
    <Badge tone="primary">Festgeschrieben</Badge>
    <Badge tone="primary">Fertiggestellt</Badge>
    <Badge tone="info">Versendet</Badge>
    <Badge tone="success">Bezahlt</Badge>
    <Badge tone="success">Angenommen</Badge>
    <Badge tone="success">Aktiv</Badge>
    <Badge tone="warning">Teilbezahlt</Badge>
    <Badge tone="warning">Unbezahlt</Badge>
    <Badge tone="danger">Überfällig</Badge>
    <Badge tone="danger">Abgelehnt</Badge>
    <Badge tone="neutral" strike>Storniert</Badge>
    <Badge tone="neutral" strike>Verworfen</Badge>
  </div>
  <p class="note">Merkmal oder Art (neutral, ohne Punkt):</p>
  <div class="row">
    <Badge kind="tag">Storno-Beleg</Badge>
    <Badge kind="tag">Abo</Badge>
    <Badge kind="tag">§13b</Badge>
    <Badge kind="tag">Nur Entwurf</Badge>
    <Badge kind="tag">{percent(80)} betrieblich</Badge>
    <Badge kind="tag">Standard</Badge>
  </div>
</Card>

<h2 class="sec">Formularfelder</h2>
<Card>
  <div class="kb-form-grid">
    <!-- R5-016: FormField gibt inputId/describedBy/invalid/required als Snippet-
         Param raus — der Slot bindet sie ans <input>, damit Screen-Reader das
         Label, den Hinweis und den Fehler verlässlich vorlesen. -->
    <FormField label="Kunde">
      {#snippet children({ inputId })}
        <input id={inputId} class="kb-input kb-w-long" autocomplete="off" value="Mustermann GmbH" />
      {/snippet}
    </FormField>
    <FormField
      label="Rechnungsdatum"
      required
      error={datum ? "" : "Bitte ein Rechnungsdatum angeben."}
    >
      {#snippet children({ inputId, describedBy, invalid, required })}
        <input
          id={inputId}
          type="date"
          aria-describedby={describedBy}
          aria-invalid={invalid || undefined}
          aria-required={required || undefined}
          class="kb-input kb-w-date"
          bind:value={datum}
        />
      {/snippet}
    </FormField>
    <FormField label="PDF-Vorlage">
      {#snippet children({ inputId })}
        <select id={inputId} class="kb-input kb-w-medium">
          <option value="">{pdfTemplateDefaultOptionLabel("default")}</option>
          {#each PDF_TEMPLATE_NAMES as name (name)}
            <option value={name}>{pdfTemplateLabel(name)}</option>
          {/each}
        </select>
      {/snippet}
    </FormField>
  </div>
</Card>

<h2 class="sec">Betrag und Menge</h2>
<Card>
  <p class="note">
    Komma oder Punkt als Dezimalzeichen, Tausenderpunkt erlaubt; beim Verlassen erscheint der
    erkannte Wert im deutschen Format. Probier z.&nbsp;B. <code>1.234</code>, <code>12.5</code>,
    <code>0,125</code> oder <code>-3</code>.
  </p>
  <div class="kb-form-grid">
    <MoneyInput
      bind:cents={betrag}
      fieldClass="kb-w-amount"
      label="Betrag (brutto)"
      hint="Gemäß §19 UStG wird keine Umsatzsteuer ausgewiesen."
    />
    <QuantityInput bind:value={menge} fieldClass="kb-w-short" label="Menge" maxFractionDigits={4} />
    <QuantityInput bind:value={km} fieldClass="kb-w-short" label="Strecke" unit="km" noun="Strecke" maxFractionDigits={1} />
    <QuantityInput bind:value={tage} fieldClass="kb-w-short" label="Zahlungsziel" unit="Tage" noun="Zahl" integer />
  </div>
  <p class="note" aria-live="polite">
    Gebundene Werte: {betrag === null ? "—" : euro(betrag)} · Menge {menge ?? "—"} ·
    {km ?? "—"} km · {tage ?? "—"} Tage
  </p>
</Card>

<h2 class="sec">Meldungen und Dialoge</h2>
<Banner kind="info">Banner für Zustände, die bleiben, bis sie behoben sind.</Banner>
<Banner kind="warning" href="/settings/backup" linkLabel="Zu den Backups">Kein externes Backup-Ziel eingerichtet.</Banner>
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
        flash("Paket archiviert.", "ok", {
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
  <div class="kb-metric"><p class="lbl">Saldo</p><div class="val ok">{euro(1454000)}</div></div>
  <div class="kb-metric"><p class="lbl">Saldo Vorjahr</p><div class="val neg">{euro(-280090)}</div></div>
</div>

<h2 class="sec">Linkliste</h2>
<ul class="kb-linklist">
  <li><a href="/design"><span class="kb-ll-title">Meine Firmendaten</span><span class="kb-ll-sub">Name, Adresse, Steuernummer</span></a></li>
  <li><a href="/design"><span class="kb-ll-title">Konten</span><span class="kb-ll-sub">Bankkonto, Bargeld und weitere Zahlungswege</span></a></li>
  <li><a href="/design"><span class="kb-ll-title">Backups</span><span class="kb-ll-sub">Automatische Sicherungen und Ziele</span></a></li>
</ul>

<h2 class="sec">Aufstellung mit Summe</h2>
<Card>
  <dl class="kb-ledger ledger-demo">
    <div><dt>Bezahlte Rechnungen</dt><dd>{euro(2418000)}</dd></div>
    <div><dt>abzgl. Erstattungen aus Stornos</dt><dd>{euro(-42000)}</dd></div>
    <div class="kb-sum"><dt>Summe Einnahmen</dt><dd>{euro(2376000)}</dd></div>
  </dl>
</Card>

<h2 class="sec">Tabelle</h2>
<Table caption="Beispielrechnungen" cards>
  <thead>
    <tr><th>Nummer</th><th>Kunde</th><th>Datum</th><th class="kb-num">Brutto</th><th>Status</th></tr>
  </thead>
  <tbody>
    {#each rows as r (r.nr ?? r.kunde)}
      <tr>
        <td data-label="Nummer">
          <span class="nr">
            <DocNo value={r.nr} href={r.nr ? "/design" : undefined} placeholder="Entwurf" />
            {#if r.nr?.startsWith("ST-")}<Badge kind="tag">Storno-Beleg</Badge>{/if}
          </span>
        </td>
        <td data-label="Kunde">{r.kunde}</td>
        <td data-label="Datum" class="kb-muted">{r.datum}</td>
        <td data-label="Brutto" class="kb-num">{euro(r.cents)}</td>
        <td data-label="Status"><Badge tone={r.tone} strike={r.strike}>{r.status}</Badge></td>
      </tr>
    {/each}
  </tbody>
  <tfoot>
    <tr class="kb-sum">
      <td colspan="3">Summe</td>
      <td data-label="Summe" class="kb-num">{euro(rows.reduce((s, r) => s + r.cents, 0))}</td>
      <td></td>
    </tr>
  </tfoot>
</Table>

<style>
  .sec { font-size: var(--type-heading); font-weight: 600; margin: var(--sp-6) 0 var(--sp-3); }
  .row { display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: center; }
  .note { font-size: var(--type-small); color: var(--text-muted); margin: var(--sp-3) 0 var(--sp-2); overflow-wrap: anywhere; }
  .note:first-child { margin-top: 0; }
  .note code { font-family: var(--font-mono); }
  .metrics { display: grid; grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr)); gap: var(--sp-3); }
  .val.ok { color: var(--status-success-fg); }
  .val.neg { color: var(--status-danger-fg); }
  .ledger-demo { max-width: 40rem; }
  .nr { display: inline-flex; align-items: center; gap: var(--sp-2); flex-wrap: wrap; }
  @media screen and (max-width: 820px) {
    /* Karten: „Summe“ steht im data-label der Betragszelle. */
    tfoot td:first-child { display: none; }
  }
</style>
