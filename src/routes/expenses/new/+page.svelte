<script lang="ts">
  import { onMount } from "svelte";
  import PageBar from "$lib/PageBar.svelte";
  import { goto } from "$app/navigation";
  import { contactsList, paymentAccountsList, expensesCreate } from "$lib/api";
  import type { Contact, PaymentAccount, ExpenseCategory, ExpenseInputDto } from "$lib/types";
  import { EXPENSE_CATEGORIES } from "$lib/labels";
  import { euro } from "$lib/format";
  import { flash } from "$lib/toast.svelte";

  let vendors = $state<Contact[]>([]);
  let accounts = $state<PaymentAccount[]>([]);
  let busy = $state(false);

  // Formularfelder
  let expenseDate = $state(new Date().toISOString().slice(0, 10));
  let paidDate = $state<string>(new Date().toISOString().slice(0, 10));
  let notPaidYet = $state(false);
  let vendorContactId = $state<string>("");
  let vendorName = $state("");
  let vendorInvoiceNumber = $state("");
  let category = $state<ExpenseCategory>("office");
  let description = $state("");
  let netEuros = $state<number>(0);
  // USt-Satz: feste Sätze rechnen den Betrag automatisch; "custom" = manuell
  // (für cent-genaue Übernahme vom Beleg, falls der Lieferant abweichend rundet).
  let vatRate = $state<"19" | "7" | "0" | "custom">("19");
  let taxEuros = $state<number>(0);
  let paidFromAccountId = $state<string>("");
  let reverseCharge13b = $state(false);
  let notes = $state("");
  let receiptFile = $state<File | null>(null);

  // Kein zukünftiges Datum erlaubt (Beleg-/Zahldatum); Backend erzwingt es auch.
  const maxDate = new Date().toISOString().slice(0, 10);

  let grossCents = $derived(Math.round(netEuros * 100) + Math.round(taxEuros * 100));

  // Bei festem Satz USt aus Netto berechnen (kaufmännisch auf Cent gerundet).
  // Liest bewusst NICHT taxEuros → keine Effekt-Schleife.
  $effect(() => {
    if (vatRate !== "custom") {
      const rate = Number(vatRate);
      taxEuros = Math.round(netEuros * rate) / 100;
    }
  });

  onMount(async () => {
    try {
      const all = await contactsList(false);
      vendors = all.filter((c) => c.contactType === "vendor" || c.contactType === "both");
      accounts = await paymentAccountsList(false);
      const def = accounts.find((a) => a.isDefault === 1);
      if (def) paidFromAccountId = def.id;
    } catch (e) {
      flash("Daten konnten nicht geladen werden: " + String(e), "error");
    }
  });

  // Bei Auswahl eines Lieferanten-Kontakts den Snapshot-Namen vorbefüllen.
  function onVendorSelect() {
    const c = vendors.find((v) => v.id === vendorContactId);
    if (c) vendorName = c.name;
  }

  async function save() {
    if (!vendorName.trim()) {
      flash("Lieferant (Name) ist erforderlich.", "error");
      return;
    }
    if (!description.trim()) {
      flash("Beschreibung ist erforderlich.", "error");
      return;
    }
    if (grossCents <= 0) {
      flash("Betrag muss größer als 0 sein.", "error");
      return;
    }
    busy = true;
    try {
      let receiptBytes: number[] | null = null;
      let receiptFilename: string | null = null;
      if (receiptFile) {
        const buf = await receiptFile.arrayBuffer();
        receiptBytes = Array.from(new Uint8Array(buf));
        receiptFilename = receiptFile.name;
      }
      const input: ExpenseInputDto = {
        expenseDate,
        paidDate: notPaidYet ? null : paidDate || null,
        paidFromAccountId: paidFromAccountId || null,
        vendorContactId: vendorContactId || null,
        vendorName: vendorName.trim(),
        vendorInvoiceNumber: vendorInvoiceNumber.trim() || null,
        category,
        description: description.trim(),
        netAmountCents: Math.round(netEuros * 100),
        taxAmountCents: Math.round(taxEuros * 100),
        grossAmountCents: grossCents,
        currencyCode: "EUR",
        reverseCharge13b,
        notes: notes.trim() || null,
      };
      const detail = await expensesCreate({
        input,
        fiscalYear: null, // Backend leitet aus dem Beleg-Datum ab
        receiptBytes,
        receiptFilename,
      });
      flash("Kosten erfasst: " + detail.expense.expenseNumber);
      await goto(`/expenses/${detail.expense.id}`);
    } catch (e) {
      flash("Speichern fehlgeschlagen: " + String(e), "error");
    } finally {
      busy = false;
    }
  }
</script>

<PageBar back="/expenses" backLabel="Kosten" title="Neue Kosten erfassen">
  {#snippet actions()}
    <a class="btn-secondary" href="/expenses">Abbrechen</a>
    <button class="btn-primary" onclick={save} disabled={busy}>
      {busy ? "Speichere …" : "Kosten speichern"}
    </button>
  {/snippet}
</PageBar>
<p class="caveat">
  Einmal gespeichert lässt sich eine Kostenposition <strong>nicht mehr ändern</strong>
  — das schreibt das Gesetz so vor. Vertippt? Dann stornierst du sie und legst
  sie neu an. Also vorher kurz prüfen.
</p>

<section class="card">
  <div class="grid">
    <label>
      Beleg-Datum
      <input type="date" bind:value={expenseDate} max={maxDate} />
    </label>
    <label>
      Bezahlt am <span class="lbl-hint">(zählt fürs Finanzamt)</span>
      <input type="date" bind:value={paidDate} max={maxDate} disabled={notPaidYet} />
      <span class="chk-inline">
        <input type="checkbox" bind:checked={notPaidYet} /> noch nicht bezahlt
      </span>
    </label>
    <label>
      Lieferant (Kontakt)
      <select bind:value={vendorContactId} onchange={onVendorSelect}>
        <option value="">— ohne Kontakt (Freitext) —</option>
        {#each vendors as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
      </select>
    </label>
    <label>
      Lieferanten-Name
      <input type="text" bind:value={vendorName} placeholder="Name des Lieferanten" />
    </label>
    <label>
      Rechnungs-Nr. des Lieferanten
      <input type="text" bind:value={vendorInvoiceNumber} />
    </label>
    <label>
      Kategorie (EÜR)
      <select bind:value={category}>
        {#each EXPENSE_CATEGORIES as c}<option value={c.value}>{c.label}</option>{/each}
      </select>
    </label>
    <label class="span2">
      Beschreibung
      <input type="text" bind:value={description} placeholder="Wofür wurde gezahlt?" />
    </label>
    <label>
      Netto (€)
      <input type="number" step="0.01" min="0" bind:value={netEuros} />
    </label>
    <label>
      USt-Satz
      <select bind:value={vatRate}>
        <option value="19">19 % (Regelsatz)</option>
        <option value="7">7 % (ermäßigt)</option>
        <option value="0">0 % (steuerfrei / §13b / Kleinunternehmer)</option>
        <option value="custom">individuell (Betrag vom Beleg)</option>
      </select>
    </label>
    <label>
      USt-Betrag (€)
      <input
        type="number"
        step="0.01"
        min="0"
        bind:value={taxEuros}
        readonly={vatRate !== "custom"}
        class={vatRate !== "custom" ? "ro" : ""}
      />
    </label>
    <label>
      Brutto (€)
      <input type="text" value={euro(grossCents)} readonly class="ro" />
    </label>
    <p class="hint span2">
      Der Beleg ist maßgeblich: weicht die ausgewiesene USt vom berechneten Wert
      ab (Lieferanten-Rundung), wähle „individuell" und übernimm den USt-Betrag
      exakt vom Beleg.
    </p>
    <label>
      Zahlungs-Konto
      <select bind:value={paidFromAccountId}>
        <option value="">— keines —</option>
        {#each accounts as a (a.id)}<option value={a.id}>{a.label}</option>{/each}
      </select>
    </label>
    <label class="chk span2">
      <input
        type="checkbox"
        bind:checked={reverseCharge13b}
        onchange={() => { if (reverseCharge13b) vatRate = "0"; }}
      />
      §13b Reverse-Charge (Steuerschuldnerschaft des Leistungsempfängers)
    </label>
    {#if reverseCharge13b}
      <p class="hint span2">
        §13b ist hier nur ein Hinweis-Flag — es findet <strong>keine</strong>
        automatische USt-Berechnung statt. Trage Netto/USt so ein, wie es für
        deine Buchung korrekt ist (im Zweifel Steuerberater fragen).
      </p>
    {/if}
    <label class="span2">
      Beleg (PDF/Bild, optional)
      <input
        type="file"
        accept="application/pdf,image/*"
        onchange={(e) => (receiptFile = (e.currentTarget as HTMLInputElement).files?.[0] ?? null)}
      />
    </label>
    <label class="span2">
      Notiz (optional)
      <textarea rows="2" bind:value={notes}></textarea>
    </label>
  </div>

</section>

<style>
  /* .caveat / .card entfernt — globale Definitionen aus tokens.css. */
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 1rem; }
  .span2 { grid-column: 1 / -1; }
  label { display: flex; flex-direction: column; font-size: 0.85rem; color: #4b5563; gap: 0.25rem; }
  .lbl-hint { font-weight: 400; color: #9ca3af; font-size: 0.78rem; }
  label.chk { flex-direction: row; align-items: center; gap: 0.5rem; }
  .chk-inline { display: inline-flex; align-items: center; gap: 0.3rem; font-size: 0.78rem; color: #6b7280; margin-top: 0.2rem; }
  input, select, textarea { padding: 0.45rem 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font-size: 0.95rem; font-family: inherit; }
  input.ro { background: #f3f4f6; font-weight: 600; }
  .hint { font-size: 0.82rem; color: #3730a3; background: #eef2ff; border: 1px solid #c7d2fe; padding: 0.5rem 0.75rem; border-radius: 4px; margin: 0; }
  /* Pre-DS-Style-Block entfernt (G2-UX.3.x Konsistenz-Fix): tokens.css greift. */
  button:disabled { opacity: 0.6; cursor: not-allowed; }
</style>
