<script lang="ts">
  // Start-Seite (G2-UX.2). Hero + KPI-Karten + Quick-Actions + Aufgaben/Letzte
  // Belege. Liest ausschließlich existierende API-Bindings (keine neuen Backend-
  // Commands). Lädt parallel via Promise.allSettled, zeigt Skeleton-Pulse bis
  // Daten da sind, fängt Einzel-Fehler je Karte ab.
  import PageBar from "$lib/PageBar.svelte";
  import {
    sellerProfileGet,
    paragraph19Info,
    fiscalYearOverview,
    euerComputeReport,
    invoicesList,
    expensesList,
    backupLogList,
    notificationsList,
    recurringInvoicesList,
  } from "$lib/api";
  import type { BackupLogEntry } from "$lib/api";
  import { euro, date as fmtDate } from "$lib/format";
  import type {
    SellerProfile,
    Paragraph19Info,
    FiscalYearOverview,
    EuerReport,
    InvoiceListItem,
    ExpenseListItem,
    AppNotification,
    RecurringInvoiceRow,
  } from "$lib/types";

  // ---- Loader-State ----------------------------------------------------------

  let seller = $state<SellerProfile | null>(null);
  let para19 = $state<Paragraph19Info | null>(null);
  let fyOverview = $state<FiscalYearOverview | null>(null);
  let fyErr = $state<string | null>(null); // wird im Hero als kompakter Hinweis gezeigt
  let euer = $state<EuerReport | null>(null);
  let euerErr = $state<string | null>(null);
  let invoices = $state<InvoiceListItem[] | null>(null);
  let invoicesErr = $state<string | null>(null);
  let expenses = $state<ExpenseListItem[] | null>(null);
  let expensesErr = $state<string | null>(null);
  let backupLog = $state<BackupLogEntry[] | null>(null);
  let backupErr = $state<string | null>(null);
  let notifications = $state<AppNotification[] | null>(null);
  let notifErr = $state<string | null>(null);
  let recurring = $state<RecurringInvoiceRow[] | null>(null);
  let recurringErr = $state<string | null>(null);

  let isRefreshing = $state(false);

  function errMsg(reason: unknown): string {
    if (typeof reason === "string") return reason;
    if (reason && typeof reason === "object" && "message" in reason) {
      return String((reason as { message: unknown }).message);
    }
    return String(reason);
  }

  async function loadAll() {
    // Setze alle States auf null → Skeleton.
    seller = null;
    para19 = null;
    fyOverview = null; fyErr = null;
    euer = null; euerErr = null;
    invoices = null; invoicesErr = null;
    expenses = null; expensesErr = null;
    backupLog = null; backupErr = null;
    notifications = null; notifErr = null;
    recurring = null; recurringErr = null;

    isRefreshing = true;

    // FY brauchen wir früh: das aktuelle GJ ist Input für die EÜR-Berechnung.
    let currentYear: number | null = null;
    try {
      fyOverview = await fiscalYearOverview();
      currentYear = fyOverview.currentYear;
    } catch (e) {
      fyErr = errMsg(e);
    }

    const results = await Promise.allSettled([
      sellerProfileGet(),
      paragraph19Info(),
      currentYear !== null ? euerComputeReport(currentYear) : Promise.reject("kein GJ"),
      invoicesList(),
      currentYear !== null
        ? expensesList({ fiscalYear: currentYear, includeCanceled: false })
        : expensesList({ includeCanceled: false }),
      backupLogList(20),
      notificationsList(false),
      recurringInvoicesList(false),
    ]);

    const [
      rSeller,
      rPara19,
      rEuer,
      rInvoices,
      rExpenses,
      rBackup,
      rNotif,
      rRecurring,
    ] = results;

    if (rSeller.status === "fulfilled") seller = rSeller.value;
    // Seller-Fehler still ignorieren → Hero fällt auf „Willkommen bei Klein.Buch" zurück.

    if (rPara19.status === "fulfilled") para19 = rPara19.value;
    // §19-Hinweis ist optional → Fehler still ignorieren (Stripe bleibt dann ohne Sub-Text).

    if (rEuer.status === "fulfilled") euer = rEuer.value;
    else euerErr = errMsg(rEuer.reason);

    if (rInvoices.status === "fulfilled") invoices = rInvoices.value;
    else invoicesErr = errMsg(rInvoices.reason);

    if (rExpenses.status === "fulfilled") expenses = rExpenses.value;
    else expensesErr = errMsg(rExpenses.reason);

    if (rBackup.status === "fulfilled") backupLog = rBackup.value;
    else backupErr = errMsg(rBackup.reason);

    if (rNotif.status === "fulfilled") notifications = rNotif.value;
    else notifErr = errMsg(rNotif.reason);

    if (rRecurring.status === "fulfilled") recurring = rRecurring.value;
    else recurringErr = errMsg(rRecurring.reason);

    isRefreshing = false;
  }

  $effect(() => {
    void loadAll();
  });

  // ---- Derived-Werte ---------------------------------------------------------

  const today = new Date();
  const todayIso = todayIsoLocal();

  function todayIsoLocal(): string {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  }

  const fmtHeroDate = new Intl.DateTimeFormat("de-DE", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
  });
  const heroDate = fmtHeroDate.format(today);

  function dayOfYear(d: Date): number {
    const start = new Date(d.getFullYear(), 0, 0);
    const diff = d.getTime() - start.getTime();
    return Math.floor(diff / 86_400_000);
  }

  function daysInYear(year: number): number {
    return ((year % 4 === 0 && year % 100 !== 0) || year % 400 === 0) ? 366 : 365;
  }

  const fyDay = dayOfYear(today);
  const fyDaysInYear = daysInYear(today.getFullYear());
  // Tag-Anzeige nur sinnvoll, wenn das offene GJ das laufende Kalenderjahr ist.
  // Bei offenem Vorjahr (currentYear < today.year) wäre „Tag X von 365" für
  // das Vorjahr irreführend → wir lassen die Tag-Zeile dann weg.
  const showFyDay = $derived(
    fyOverview !== null && fyOverview.currentYear === today.getFullYear(),
  );

  // §19-Modus aus seller_profile (Singleton; isKleinunternehmer ist 0|1).
  const isKlein = $derived(seller ? seller.isKleinunternehmer === 1 : null);

  // Offene Ausgangsrechnungen (status != canceled, kein Storno-Beleg, paid < gross).
  // received-Rechnungen liegen NICHT in invoices (Block 11 → expenses), daher
  // ist `invoicesList()` faktisch direction='issued'.
  const openInvoices = $derived(
    invoices?.filter(
      (i) =>
        i.status !== "canceled" &&
        i.status !== "draft" &&
        i.isStornoFor === null &&
        i.paidAmountCents < i.grossAmountCents,
    ) ?? [],
  );
  const openInvoicesCount = $derived(openInvoices.length);
  const openInvoicesTotalCents = $derived(
    openInvoices.reduce((sum, i) => sum + (i.grossAmountCents - i.paidAmountCents), 0),
  );

  // Überfällige Rechnungen (für Aufgaben-Liste). `dueDate` ist Iso8601, kann
  // YYYY-MM-DD oder YYYY-MM-DDTHH:MM:SSZ sein → für den lexikographischen
  // Vergleich auf den Datums-Teil zuschneiden.
  const overdueInvoices = $derived(
    openInvoices
      .filter((i) => i.dueDate !== null && i.dueDate.slice(0, 10) < todayIso)
      .map((i) => ({
        ...i,
        overdueDays: Math.max(
          1,
          Math.floor(
            (today.getTime() - new Date(((i.dueDate as string).slice(0, 10)) + "T00:00:00").getTime()) / 86_400_000,
          ),
        ),
      }))
      .sort((a, b) => b.overdueDays - a.overdueDays),
  );

  // Fällige Abo-Rechnungen.
  const recurringDue = $derived(
    recurring?.filter((r) => r.active === 1 && r.nextDueDate.slice(0, 10) <= todayIso) ?? [],
  );

  // Letzter erfolgreicher Off-Site-Backup. `backupLog === null` ⇒ noch nicht
  // geladen → backupAmpel bleibt null (kein „never"-False-Positive vor Load).
  type BackupAmpel = "green" | "yellow" | "red" | "never";
  const lastOffSiteBackup = $derived(
    backupLog === null
      ? null
      : (backupLog.find(
          (e) => e.status === "ok" && (e.targetKind === "directory" || e.targetKind === "sftp"),
        ) ?? null),
  );
  const backupDaysAgo = $derived(
    lastOffSiteBackup
      ? Math.floor(
          (today.getTime() - new Date(lastOffSiteBackup.createdAt).getTime()) / 86_400_000,
        )
      : null,
  );
  const backupAmpel: BackupAmpel | null = $derived(
    backupLog === null
      ? null
      : lastOffSiteBackup === null
        ? "never"
        : backupDaysAgo !== null && backupDaysAgo <= 7
          ? "green"
          : backupDaysAgo !== null && backupDaysAgo <= 14
            ? "yellow"
            : "red",
  );
  const backupLabel = $derived(
    backupDaysAgo === null
      ? "Noch nie"
      : backupDaysAgo === 0
        ? "Heute"
        : backupDaysAgo === 1
          ? "Gestern"
          : `vor ${backupDaysAgo} Tagen`,
  );

  // GJ-Abschluss-Erinnerung: Vorjahr noch nicht festgeschrieben?
  // currentYear ist das älteste offene GJ. Wenn currentYear < heutiges Jahr,
  // gibt es ein abschließbares Vorjahr.
  const overdueFy: number | null = $derived(
    fyOverview && fyOverview.currentYear < today.getFullYear()
      ? fyOverview.currentYear
      : null,
  );

  const notifCount = $derived(notifications?.length ?? 0);

  // Letzte Belege: Rechnungen (RE/ST) + Kosten (K) gemischt, neueste zuerst, max 5.
  type RecentEntry = {
    kind: "RE" | "ST" | "K";
    href: string;
    sortDate: string;
    dateIso: string;
    docNumber: string;
    counterparty: string;
    amountCents: number;
    statusKey: "open" | "paid" | "partial" | "canceled";
  };

  const recentEntries: RecentEntry[] = $derived.by(() => {
    const entries: RecentEntry[] = [];
    for (const i of invoices ?? []) {
      // Drafts ausblenden (kein „Beleg" im GoBD-Sinn).
      if (i.status === "draft") continue;
      const isStorno = i.isStornoFor !== null;
      const statusKey: RecentEntry["statusKey"] =
        i.status === "canceled"
          ? "canceled"
          : i.status === "paid"
            ? "paid"
            : i.status === "partially_paid"
              ? "partial"
              : "open";
      entries.push({
        kind: isStorno ? "ST" : "RE",
        href: `/invoices/${i.id}`,
        sortDate: i.invoiceDate,
        dateIso: i.invoiceDate,
        docNumber: i.invoiceNumber,
        counterparty: i.contactName,
        amountCents: i.grossAmountCents,
        statusKey,
      });
    }
    for (const e of expenses ?? []) {
      if (e.status === "canceled") {
        entries.push({
          kind: "K",
          href: `/expenses/${e.id}`,
          sortDate: e.expenseDate,
          dateIso: e.expenseDate,
          docNumber: e.expenseNumber,
          counterparty: e.vendorNameSnapshot,
          amountCents: e.grossAmountCents,
          statusKey: "canceled",
        });
        continue;
      }
      entries.push({
        kind: "K",
        href: `/expenses/${e.id}`,
        sortDate: e.expenseDate,
        dateIso: e.expenseDate,
        docNumber: e.expenseNumber,
        counterparty: e.vendorNameSnapshot,
        amountCents: e.grossAmountCents,
        statusKey: e.paidDate ? "paid" : "open",
      });
    }
    return entries.sort((a, b) => b.sortDate.localeCompare(a.sortDate)).slice(0, 5);
  });

  function statusLabel(s: RecentEntry["statusKey"]): string {
    return s === "paid"
      ? "bezahlt"
      : s === "partial"
        ? "teilgezahlt"
        : s === "canceled"
          ? "storniert"
          : "offen";
  }

  // Aufgaben-Liste: Anzahl Items (für Empty-State-Check).
  const taskTotal = $derived(
    overdueInvoices.length +
      recurringDue.length +
      (backupAmpel === "red" || backupAmpel === "never" ? 1 : 0) +
      (backupAmpel === "yellow" ? 1 : 0) +
      (overdueFy !== null ? 1 : 0) +
      (notifCount > 0 ? 1 : 0),
  );

  // Saldo-Farbe.
  const saldoIsNegative = $derived(euer !== null && euer.surplusCents < 0);

  // Begrüßung mit Vorname (alles vor erstem Leerzeichen) — sonst voller Name.
  const greetingName: string = $derived(
    seller?.name ? seller.name.split(" ")[0] : "",
  );
</script>

<PageBar title="Start">
  {#snippet actions()}
    <button
      type="button"
      class="btn-secondary btn-sm"
      onclick={() => void loadAll()}
      disabled={isRefreshing}
      aria-label="Daten aktualisieren"
    >
      {isRefreshing ? "Wird geladen …" : "Aktualisieren"}
    </button>
  {/snippet}
</PageBar>

<!-- Hero -->
<section class="hero">
  <h2 class="hero-greet">
    {#if seller === null && isRefreshing}
      <span class="sk sk-text" style="width: 18rem;">&nbsp;</span>
    {:else if greetingName}
      Willkommen, {greetingName}
    {:else}
      Willkommen bei Klein.Buch
    {/if}
  </h2>
  <p class="hero-meta">
    {heroDate}
    {#if fyOverview}
      <span class="hero-sep">·</span>
      GJ {fyOverview.currentYear}{#if showFyDay} — Tag {fyDay} von {fyDaysInYear}{:else} offen{/if}
    {:else if fyErr}
      <span class="hero-sep">·</span>
      <span class="hero-err">GJ konnte nicht geladen werden</span>
    {/if}
  </p>
  {#if isKlein === true}
    <p class="hero-stripe">
      §19-Modus aktiv — es wird keine Umsatzsteuer ausgewiesen.
      {#if para19}<br /><span class="hero-stripe-sub">{para19.hinweisText}</span>{/if}
    </p>
  {/if}
</section>

<!-- KPI-Karten -->
<section class="kpis" aria-label="Kennzahlen">
  <!-- Einnahmen GJ -->
  <article class="kpi" data-tone="neutral">
    <p class="kpi-label">Einnahmen GJ</p>
    {#if euer === null && euerErr === null}
      <p class="kpi-val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="kpi-val">{euro(euer.totalIncomeCents)}</p>
    {/if}
    <p class="kpi-sub">seit 01.01.</p>
  </article>

  <!-- Ausgaben GJ -->
  <article class="kpi" data-tone="neutral">
    <p class="kpi-label">Ausgaben GJ</p>
    {#if euer === null && euerErr === null}
      <p class="kpi-val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="kpi-val">{euro(euer.totalExpensesCents)}</p>
    {/if}
    <p class="kpi-sub">seit 01.01.</p>
  </article>

  <!-- Saldo -->
  <article class="kpi" data-tone={saldoIsNegative ? "danger" : "success"}>
    <p class="kpi-label">Saldo</p>
    {#if euer === null && euerErr === null}
      <p class="kpi-val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="kpi-val">{euro(euer.surplusCents)}</p>
    {/if}
    <p class="kpi-sub">vorläufige EÜR</p>
  </article>

  <!-- Offene Rechnungen -->
  <a class="kpi kpi-link" data-tone="neutral" href="/invoices">
    <p class="kpi-label">Offene Rechnungen</p>
    {#if invoices === null && invoicesErr === null}
      <p class="kpi-val sk sk-num">&nbsp;</p>
    {:else if invoicesErr}
      <p class="kpi-err">{invoicesErr}</p>
    {:else}
      <p class="kpi-val">{openInvoicesCount}</p>
      <p class="kpi-sub">{euro(openInvoicesTotalCents)} ausstehend</p>
    {/if}
  </a>

  <!-- Backup-Status -->
  <a
    class="kpi kpi-link"
    data-tone={
      backupAmpel === "green"
        ? "success"
        : backupAmpel === "yellow"
          ? "warning"
          : backupAmpel === "red" || backupAmpel === "never"
            ? "danger"
            : "neutral"
    }
    href="/settings/backup"
  >
    <p class="kpi-label">
      {#if backupAmpel !== null}
        <span class="dot dot-{backupAmpel}" aria-hidden="true"></span>
      {/if}
      Backup
    </p>
    {#if backupLog === null && backupErr === null}
      <p class="kpi-val sk sk-num">&nbsp;</p>
    {:else if backupErr}
      <p class="kpi-err">{backupErr}</p>
    {:else}
      <p class="kpi-val kpi-val-small">{backupLabel}</p>
      <p class="kpi-sub">
        {#if lastOffSiteBackup}
          letztes Off-Site-Backup
        {:else}
          noch kein Off-Site-Backup
        {/if}
      </p>
    {/if}
  </a>
</section>

<!-- Quick-Actions -->
<section class="quick" aria-label="Schnellzugriff">
  <a class="tile" href="/invoices/new">
    <span class="t-title">Rechnung schreiben</span>
    <span class="t-sub">Neue Rechnung an einen Kunden</span>
  </a>
  <a class="tile" href="/expenses/new">
    <span class="t-title">Kosten erfassen</span>
    <span class="t-sub">Etwas, das du fürs Geschäft bezahlt hast</span>
  </a>
  <a class="tile" href="/quotes/new">
    <span class="t-title">Angebot erstellen</span>
    <span class="t-sub">Kostenvoranschlag für einen Kunden</span>
  </a>
  <a class="tile" href="/contacts/new">
    <span class="t-title">Kontakt anlegen</span>
    <span class="t-sub">Neuer Kunde oder Lieferant</span>
  </a>
</section>

<!-- Zwei-Spalten: Aufgaben + Letzte Belege -->
<section class="cols">
  <!-- Aufgaben -->
  <article class="col card">
    <header class="col-head">
      <h3>Anstehende Aufgaben</h3>
    </header>

    {#if invoices === null && recurring === null && backupLog === null}
      <ul class="tasks">
        <li class="task sk sk-row">&nbsp;</li>
        <li class="task sk sk-row">&nbsp;</li>
        <li class="task sk sk-row">&nbsp;</li>
      </ul>
    {:else if taskTotal === 0}
      <p class="empty">Alles im grünen Bereich — nichts zu tun.</p>
    {:else}
      <ul class="tasks">
        {#each overdueInvoices.slice(0, 6) as inv (inv.id)}
          <li class="task">
            <a class="task-row" href="/invoices/{inv.id}">
              <span class="task-badge tb-danger">Überfällig</span>
              <span class="task-text">
                <strong>{inv.invoiceNumber}</strong> · {inv.contactName}
              </span>
              <span class="task-meta">
                {inv.overdueDays} {inv.overdueDays === 1 ? "Tag" : "Tage"} fällig
              </span>
            </a>
          </li>
        {/each}

        {#each recurringDue.slice(0, 3) as r (r.id)}
          <li class="task">
            <a class="task-row" href="/recurring-invoices/new?id={r.id}">
              <span class="task-badge tb-warning">Abo fällig</span>
              <span class="task-text">{r.label}</span>
              <span class="task-meta">{fmtDate(r.nextDueDate)}</span>
            </a>
          </li>
        {/each}

        {#if backupAmpel === "red" || backupAmpel === "never"}
          <li class="task">
            <div class="task-row">
              <span class="task-badge tb-danger">Backup</span>
              <span class="task-text">
                {#if backupAmpel === "never"}
                  Noch kein Off-Site-Backup vorhanden.
                {:else}
                  Letztes Off-Site-Backup {backupLabel}.
                {/if}
              </span>
              <a class="btn-secondary btn-sm" href="/settings/backup">Jetzt sichern</a>
            </div>
          </li>
        {:else if backupAmpel === "yellow"}
          <li class="task">
            <div class="task-row">
              <span class="task-badge tb-warning">Backup</span>
              <span class="task-text">Letztes Off-Site-Backup {backupLabel}.</span>
              <a class="btn-secondary btn-sm" href="/settings/backup">Jetzt sichern</a>
            </div>
          </li>
        {/if}

        {#if overdueFy !== null}
          <li class="task">
            <div class="task-row">
              <span class="task-badge tb-warning">GJ-Abschluss</span>
              <span class="task-text">Geschäftsjahr {overdueFy} noch nicht festgeschrieben.</span>
              <a class="btn-secondary btn-sm" href="/fiscal-year">Abschließen</a>
            </div>
          </li>
        {/if}

        {#if notifCount > 0}
          <li class="task">
            <a class="task-row" href="/notifications">
              <span class="task-badge tb-info">Hinweise</span>
              <span class="task-text">
                {notifCount} {notifCount === 1 ? "offener Hinweis" : "offene Hinweise"} in der Inbox.
              </span>
              <span class="task-meta">Ansehen →</span>
            </a>
          </li>
        {/if}
      </ul>
    {/if}

    {#if invoicesErr || recurringErr || backupErr || notifErr}
      <p class="card-err">
        Teilfehler beim Laden:
        {[invoicesErr, recurringErr, backupErr, notifErr].filter(Boolean).join(" · ")}
      </p>
    {/if}
  </article>

  <!-- Letzte Belege -->
  <article class="col card">
    <header class="col-head">
      <h3>Letzte Belege</h3>
    </header>

    {#if invoices === null && expenses === null}
      <ul class="recents">
        <li class="recent sk sk-row">&nbsp;</li>
        <li class="recent sk sk-row">&nbsp;</li>
        <li class="recent sk sk-row">&nbsp;</li>
        <li class="recent sk sk-row">&nbsp;</li>
      </ul>
    {:else if recentEntries.length === 0}
      <div class="empty">
        <p>Noch keine Belege — leg deine erste Rechnung an.</p>
        <a class="btn-primary btn-sm" href="/invoices/new">Rechnung schreiben</a>
      </div>
    {:else}
      <ul class="recents">
        {#each recentEntries as r (r.href + r.docNumber)}
          <li class="recent">
            <a class="recent-row" href={r.href}>
              <span class="recent-date">{fmtDate(r.dateIso)}</span>
              <span class="recent-badge rb-{r.kind.toLowerCase()}">{r.kind}</span>
              <span class="recent-num">{r.docNumber}</span>
              <span class="recent-party">{r.counterparty}</span>
              <span class="recent-amount">{euro(r.amountCents)}</span>
              <span class="recent-status">
                <span class="dot dot-{r.statusKey}" aria-hidden="true"></span>
                {statusLabel(r.statusKey)}
              </span>
            </a>
          </li>
        {/each}
      </ul>
    {/if}

    {#if invoicesErr || expensesErr}
      <p class="card-err">
        Teilfehler beim Laden:
        {[invoicesErr, expensesErr].filter(Boolean).join(" · ")}
      </p>
    {/if}
  </article>
</section>

<style>
  /* ----- Hero (G2-UX.3.3, Apple-Polish) -------------------------------------- */
  .hero {
    margin: 0 0 1.5rem;
    padding: 1.6rem 1.75rem;
    background: linear-gradient(135deg, var(--c-primary-50) 0%, var(--c-surface) 70%);
    border: 1px solid var(--c-primary-100);
    border-radius: var(--r-xl);
    box-shadow: var(--sh-md);
  }
  .hero-greet {
    margin: 0 0 0.4rem;
    /* Display-Font + große Schriftgröße + straffes Tracking — Apple-typische
       Hero-Headline. var(--font-display) greift global aus tokens.css, hier
       explizit gesetzt, weil die Headline ein h2 ist (Display nur an h1/h2/h3). */
    font-family: var(--font-display);
    font-size: var(--fs-3xl);
    letter-spacing: -0.024em;
    font-weight: 700;
    color: var(--c-text);
    line-height: 1.15;
  }
  .hero-meta {
    margin: 0;
    color: var(--c-text-muted);
    font-size: 0.95rem;
  }
  .hero-sep { margin: 0 0.45rem; color: var(--c-text-subtle); }
  .hero-err { color: var(--c-danger-700); }
  .hero-stripe {
    margin: 0.7rem 0 0;
    padding: 0.45rem 0.7rem;
    background: var(--c-primary-100);
    color: var(--c-primary-800);
    border-radius: var(--r-sm);
    font-size: 0.88rem;
    display: inline-block;
  }
  .hero-stripe-sub {
    color: var(--c-primary-700);
    font-size: 0.82rem;
    font-style: italic;
  }

  /* ----- KPIs ---------------------------------------------------------------- */
  .kpis {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 1rem;
    margin: 0 0 1.5rem;
  }
  .kpi {
    background: var(--c-surface);
    /* Hairline-Border statt solid grey — Schatten trägt die Card, nicht der Strich. */
    border: 1px solid rgba(16, 36, 44, 0.06);
    border-radius: var(--r-xl);
    padding: 1.1rem 1.25rem;
    box-shadow: var(--sh-md);
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
  }
  .kpi-link {
    text-decoration: none;
    color: inherit;
    /* Apple-Hover: die Karte hebt sich (Schatten + minimaler Lift). Kein
       Border-Color-Switch — wirkt nervöser als gewollt. */
    transition:
      box-shadow var(--t-base) var(--ease-apple),
      transform var(--t-base) var(--ease-apple);
    will-change: transform;
  }
  .kpi-link:hover {
    box-shadow: var(--sh-lg);
    transform: translateY(-1px);
  }
  .kpi-link:active { transform: translateY(0); box-shadow: var(--sh-md); }
  .kpi[data-tone="success"] .kpi-val { color: var(--c-success-700); }
  .kpi[data-tone="danger"] .kpi-val { color: var(--c-danger-700); }
  .kpi[data-tone="warning"] .kpi-val { color: var(--c-warning-700); }
  .kpi-label {
    margin: 0;
    font-size: 0.82rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--c-text-muted);
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 0.45rem;
  }
  .kpi-val {
    margin: 0.2rem 0 0.1rem;
    font-size: 1.7rem;
    font-weight: 700;
    color: var(--c-text);
    font-variant-numeric: tabular-nums;
  }
  .kpi-val-small { font-size: 1.2rem; }
  .kpi-sub {
    margin: 0;
    font-size: 0.82rem;
    color: var(--c-text-muted);
  }
  .kpi-err {
    margin: 0.2rem 0;
    font-size: 0.82rem;
    color: var(--c-danger-700);
  }

  /* ----- Quick-Actions ------------------------------------------------------- */
  .quick {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: 1rem;
    margin: 0 0 1.5rem;
  }
  .tile {
    display: flex; flex-direction: column; gap: 0.3rem;
    background: var(--c-surface);
    border: 1px solid rgba(16, 36, 44, 0.06);
    border-radius: var(--r-xl);
    padding: 1.15rem 1.25rem;
    text-decoration: none;
    box-shadow: var(--sh-md);
    transition:
      box-shadow var(--t-base) var(--ease-apple),
      transform var(--t-base) var(--ease-apple);
    will-change: transform;
  }
  .tile:hover { box-shadow: var(--sh-lg); transform: translateY(-1px); }
  .tile:active { transform: translateY(0); box-shadow: var(--sh-md); }
  .t-title { font-weight: 600; color: var(--c-text); font-size: 1rem; }
  .t-sub { color: var(--c-text-muted); font-size: 0.85rem; }

  /* ----- Zwei-Spalten -------------------------------------------------------- */
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
  }
  @media (max-width: 1023px) {
    .cols { grid-template-columns: 1fr; }
  }
  /* R5-011: lokale `.card`-Re-Definition entfernt — globale aus tokens.css
     greift (Memory `feedback_sweep_local_overrides`: svelte-check meldet
     solche Doppelungen NICHT, daher manueller Sweep nötig). */
  .col-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    /* Hairline rgba — innerer Trenner statt harter grauer Strich. */
    border-bottom: 1px solid rgba(16, 36, 44, 0.08);
    padding-bottom: 0.65rem;
    margin-bottom: 0.7rem;
  }
  .col-head h3 { margin: 0; font-size: 1rem; font-weight: 700; color: var(--c-text); }
  .empty {
    text-align: center;
    color: var(--c-text-muted);
    padding: 1.5rem 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
    align-items: center;
  }
  .empty p { margin: 0; }
  .card-err {
    margin: 0.7rem 0 0;
    font-size: 0.82rem;
    color: var(--c-danger-700);
  }

  /* ----- Tasks --------------------------------------------------------------- */
  .tasks { list-style: none; padding: 0; margin: 0; }
  /* Hairlines zwischen Items — macOS-Settings-Look. */
  .task + .task { border-top: 1px solid rgba(16, 36, 44, 0.08); }
  .task-row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.7rem;
    padding: 0.6rem 0.2rem;
    text-decoration: none;
    color: var(--c-text);
  }
  a.task-row:hover { background: var(--c-surface-2); }
  .task-badge {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 2px 8px;
    border-radius: var(--r-pill);
    white-space: nowrap;
  }
  .tb-danger { background: var(--c-danger-50); color: var(--c-danger-700); }
  .tb-warning { background: var(--c-warning-50); color: var(--c-warning-700); }
  .tb-info { background: var(--c-primary-50); color: var(--c-primary-700); }
  .task-text { font-size: 0.92rem; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .task-meta { font-size: 0.82rem; color: var(--c-text-muted); white-space: nowrap; }

  /* ----- Recent-Belege ------------------------------------------------------- */
  .recents { list-style: none; padding: 0; margin: 0; }
  .recent + .recent { border-top: 1px solid rgba(16, 36, 44, 0.08); }
  .recent-row {
    display: grid;
    grid-template-columns: auto auto 1fr 1fr auto auto;
    align-items: center;
    gap: 0.6rem;
    padding: 0.55rem 0.2rem;
    text-decoration: none;
    color: var(--c-text);
    font-size: 0.92rem;
  }
  .recent-row:hover { background: var(--c-surface-2); }
  .recent-date { font-size: 0.82rem; color: var(--c-text-muted); white-space: nowrap; min-width: 5.5rem; }
  .recent-badge {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 7px;
    border-radius: var(--r-sm);
    background: var(--c-primary-50);
    color: var(--c-primary-700);
    min-width: 1.6rem;
    text-align: center;
  }
  .rb-st { background: var(--c-danger-50); color: var(--c-danger-700); }
  .rb-k { background: var(--c-warning-50); color: var(--c-warning-700); }
  .recent-num { font-weight: 600; font-variant-numeric: tabular-nums; white-space: nowrap; }
  .recent-party { color: var(--c-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0; }
  .recent-amount { font-variant-numeric: tabular-nums; font-weight: 600; white-space: nowrap; }
  .recent-status { font-size: 0.8rem; color: var(--c-text-muted); display: inline-flex; align-items: center; gap: 0.3rem; white-space: nowrap; }

  @media (max-width: 720px) {
    .recent-row { grid-template-columns: 1fr auto; row-gap: 0.2rem; }
    .recent-date { grid-column: 1; }
    .recent-badge { grid-column: 2; justify-self: end; }
    .recent-num { grid-column: 1 / span 2; }
    .recent-party { grid-column: 1 / span 2; }
    .recent-amount { grid-column: 1; }
    .recent-status { grid-column: 2; justify-self: end; }
  }

  /* ----- Dots ---------------------------------------------------------------- */
  .dot {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c-text-subtle);
  }
  .dot-green, .dot-paid { background: var(--c-success-500); }
  .dot-yellow, .dot-partial { background: var(--c-warning-500); }
  .dot-red, .dot-canceled { background: var(--c-danger-500); }
  .dot-never { background: var(--c-danger-500); }
  .dot-open { background: var(--c-text-subtle); }

  /* ----- Skeleton ------------------------------------------------------------ */
  .sk {
    background: linear-gradient(90deg, var(--c-bg) 0%, var(--c-border) 50%, var(--c-bg) 100%);
    background-size: 200% 100%;
    animation: sk-pulse 1.2s ease-in-out infinite;
    border-radius: var(--r-sm);
    display: inline-block;
    color: transparent;
  }
  .sk-text { height: 1.5rem; }
  .sk-num { height: 1.7rem; width: 6rem; }
  .sk-row { display: block; height: 2rem; margin: 0.4rem 0; border-radius: var(--r-sm); }
  @keyframes sk-pulse {
    0% { background-position: 200% 0; }
    100% { background-position: -200% 0; }
  }

  /* R5-011: lokales `.btn-sm` entfernt — Manuel-Hardline 2026-05-26: alle
     Buttons app-weit gleich groß. tokens.css hält `.btn-sm` absichtlich
     no-op; Task-Row-CTAs erben jetzt die Standard-Button-Größe. */
</style>
