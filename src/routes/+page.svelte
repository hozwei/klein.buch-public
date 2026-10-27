<script lang="ts">
  import {
    APP_TIME_ZONE,
    dayOfYearBerlin,
    daysBetweenIso,
    daysInYear,
    todayBerlin,
    yearBerlin,
  } from "#lib/date.ts";
  import { dayClock } from "#lib/dayClock.svelte.ts";
  // Start-Seite (G2-UX.2). Sachstand + Kennzahlen + Schnellzugriff + Aufgaben/Letzte
  // Belege. Liest ausschließlich existierende API-Bindings (keine neuen Backend-
  // Commands). Lädt parallel via Promise.allSettled, zeigt Platzhalter bis
  // Daten da sind, fängt Einzel-Fehler je Karte ab.
  // U4: Kennzahlen als `.kb-metric` ohne Anheben, Schnellzugriff als Liste statt
  // gleicher Kacheln (design.md §8.1), Sachstand statt Begrüßung (U1-064), Backup-
  // Stand aus den Server-Feldern (Warteliste 13), „Letzte Belege“ im gemeinsamen
  // Raster (U3-039). Korrekturrunde V3: Datum in --type-heading (keine Hero-Fläche),
  // Kennzahlen ohne Umbruch und auf dem Handy zweispaltig, Schnellzugriff als
  // .kb-linklist, Status und Belegart nur über <Badge> mit den Wörtern der
  // Status-Helfer (invoiceStatus.ts, expenseStatus.ts), Platzhalter ohne Animation.
  import PageBar from "#lib/PageBar.svelte";
  import DocNo from "#lib/DocNo.svelte";
  import Banner from "#lib/Banner.svelte";
  import Badge from "#lib/Badge.svelte";
  import { invoiceStatusView } from "./invoices/invoiceStatus.ts";
  import type { StatusView } from "./invoices/invoiceStatus.ts";
  import { expenseStatusView } from "./expenses/expenseStatus.ts";
  import {
    sellerProfileGet,
    paragraph19Info,
    fiscalYearOverview,
    euerComputeReport,
    invoicesList,
    expensesList,
    backupGetSettings,
    notificationsList,
    recurringInvoicesList,
  } from "#lib/api.ts";
  import type { BackupSettings } from "#lib/api.ts";
  import { euro, date as fmtDate } from "#lib/format.ts";
  import { withinTerm } from "#lib/recurringChoice.ts";
  import type {
    SellerProfile,
    Paragraph19Info,
    FiscalYearOverview,
    EuerReport,
    InvoiceListItem,
    ExpenseListItem,
    AppNotification,
    RecurringInvoiceRow,
  } from "#lib/types.ts";

  // ---- Loader-State ----------------------------------------------------------

  let seller = $state<SellerProfile | null>(null);
  // U4 (V1-10): Firmendaten-Hinweis nur nach einer erfolgreichen Antwort (null = kein
  // Profil); ein Ladefehler zeigt ihn nicht.
  let sellerState = $state<"loading" | "ok" | "error">("loading");
  let para19 = $state<Paragraph19Info | null>(null);
  let fyOverview = $state<FiscalYearOverview | null>(null);
  let fyErr = $state<string | null>(null); // wird im Hero als kompakter Hinweis gezeigt
  let euer = $state<EuerReport | null>(null);
  let euerErr = $state<string | null>(null);
  let invoices = $state<InvoiceListItem[] | null>(null);
  let invoicesErr = $state<string | null>(null);
  let expenses = $state<ExpenseListItem[] | null>(null);
  let expensesErr = $state<string | null>(null);
  // Plan W7 / U4 (Warteliste 13, Übergabe F3): Stand der Off-Site-Sicherung vom Server —
  // Status (missing | failing | stale | ok) und Alter der ältesten der je Ziel jüngsten
  // Kopien in Berliner Tagen (ADR 0041 Nr. 86); null = nicht geladen.
  let backupSettings = $state<BackupSettings | null>(null);
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
    // Kein Lesen von Zustand vor dem ersten await: loadAll läuft im $effect unten, jede
    // gelesene Variable würde ihn erneut auslösen (Schutz vor Doppelklick im Menüeintrag).
    // Setze alle States auf null → Skeleton.
    seller = null;
    sellerState = "loading";
    para19 = null;
    fyOverview = null; fyErr = null;
    euer = null; euerErr = null;
    invoices = null; invoicesErr = null;
    expenses = null; expensesErr = null;
    backupSettings = null; backupErr = null;
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
      notificationsList(false),
      recurringInvoicesList(false),
      backupGetSettings(),
    ]);

    const [
      rSeller,
      rPara19,
      rEuer,
      rInvoices,
      rExpenses,
      rNotif,
      rRecurring,
      rBackupSettings,
    ] = results;

    if (rSeller.status === "fulfilled") {
      seller = rSeller.value;
      sellerState = "ok";
    } else {
      // Seller-Fehler still ignorieren → Kopfzeile ohne Firmenname, kein Firmendaten-Hinweis.
      sellerState = "error";
    }

    if (rPara19.status === "fulfilled") para19 = rPara19.value;
    // §19-Hinweis ist optional → Fehler still ignorieren (Stripe bleibt dann ohne Sub-Text).

    if (rEuer.status === "fulfilled") euer = rEuer.value;
    else euerErr = errMsg(rEuer.reason);

    if (rInvoices.status === "fulfilled") invoices = rInvoices.value;
    else invoicesErr = errMsg(rInvoices.reason);

    if (rExpenses.status === "fulfilled") expenses = rExpenses.value;
    else expensesErr = errMsg(rExpenses.reason);


    if (rNotif.status === "fulfilled") notifications = rNotif.value;
    else notifErr = errMsg(rNotif.reason);

    if (rBackupSettings.status === "fulfilled") backupSettings = rBackupSettings.value;
    else backupErr = errMsg(rBackupSettings.reason);

    if (rRecurring.status === "fulfilled") recurring = rRecurring.value;
    else recurringErr = errMsg(rRecurring.reason);

    isRefreshing = false;
  }

  $effect(() => {
    void loadAll();
  });

  // ---- Derived-Werte ---------------------------------------------------------

  // Datum und Jahr in Europe/Berlin, nicht in der Zeitzone des Geräts (W1.1);
  // reaktiv über Mitternacht (D-27, lib/dayClock.svelte.ts).
  const today = $derived(dayClock.now);
  const todayIso = $derived(todayBerlin(today));
  const currentYear = $derived(yearBerlin(today));

  const fmtHeroDate = new Intl.DateTimeFormat("de-DE", {
    weekday: "long",
    day: "numeric",
    month: "long",
    year: "numeric",
    timeZone: APP_TIME_ZONE,
  });
  const heroDate = $derived(fmtHeroDate.format(today));

  const fyDay = $derived(dayOfYearBerlin(today));
  const fyDaysInYear = $derived(daysInYear(currentYear));
  // Tag-Anzeige nur sinnvoll, wenn das offene GJ das laufende Kalenderjahr ist.
  // Bei offenem Vorjahr (currentYear < today.year) wäre „Tag X von 365" für
  // das Vorjahr irreführend → wir lassen die Tag-Zeile dann weg.
  const showFyDay = $derived(
    fyOverview !== null && fyOverview.currentYear === currentYear,
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
        // Kalendertage in Europe/Berlin, nicht in der Zeitzone des Geräts (D-15).
        overdueDays: Math.max(1, daysBetweenIso((i.dueDate as string).slice(0, 10), todayIso) || 1),
      }))
      .sort((a, b) => b.overdueDays - a.overdueDays),
  );

  // Fällige Abo-Rechnungen (nur innerhalb der Laufzeit, F1d).
  const recurringDue = $derived(
    recurring?.filter(
      (r) => r.active === 1 && withinTerm(r) && r.nextDueDate.slice(0, 10) <= todayIso,
    ) ?? [],
  );

  // Off-Site-Sicherung (U4, Warteliste 13): Status und Alter rechnet der Server
  // (Berliner Kalendertage, wie die Regel „Backup überfällig“); die Seite zeigt nur an.
  const offsiteStatus = $derived(backupSettings?.offsiteStatus ?? null);
  const offsiteAgeDays = $derived(backupSettings?.offsiteAgeDays ?? null);
  const backupTone = $derived<"success" | "warning" | "danger" | "neutral">(
    offsiteStatus === "ok"
      ? "success"
      : offsiteStatus === "stale"
        ? "warning"
        : offsiteStatus === "failing" || offsiteStatus === "missing"
          ? "danger"
          : "neutral",
  );
  function ageText(n: number): string {
    return n === 0 ? "heute" : n === 1 ? "gestern" : `vor ${n} Tagen`;
  }
  // Kennzahl „Backup“: Wert und Zusatzzeile.
  const backupValue = $derived(
    offsiteStatus === "missing"
      ? "Kein Ziel"
      : offsiteAgeDays === null
        ? "Noch keine Kopie"
        : ageText(offsiteAgeDays).replace(/^./, (c) => c.toUpperCase()),
  );
  const backupSub = $derived(
    offsiteStatus === "missing"
      ? "kein externes Backup-Ziel"
      : offsiteStatus === "failing"
        ? "letzter Lauf fehlgeschlagen"
        : offsiteAgeDays === null
          ? "ein externes Ziel ohne Kopie"
          : (backupSettings?.targets.length ?? 0) > 1
            ? "Stand der externen Kopien"
            : "letzte externe Kopie",
  );
  // Aufgabe „Backup“ mit klarem Text je Zustand (Übergabe F3: Wortlaut bei `stale`).
  const backupTask = $derived.by<{ tone: "danger" | "warning"; text: string; cta: string } | null>(() => {
    if (offsiteStatus === "missing") {
      return {
        tone: "danger",
        text: "Kein externes Backup-Ziel eingerichtet. Richte eines ein, damit eine Kopie außerhalb des Servers liegt.",
        cta: "Ziel einrichten",
      };
    }
    if (offsiteStatus === "failing") {
      return {
        tone: "danger",
        text:
          "Ein externes Backup-Ziel ist beim letzten Backup fehlgeschlagen." +
          (offsiteAgeDays !== null ? ` Letzte erfolgreiche externe Kopie: ${ageText(offsiteAgeDays)}.` : ""),
        cta: "Ziele prüfen",
      };
    }
    if (offsiteStatus === "stale") {
      return {
        tone: "warning",
        text:
          offsiteAgeDays === null
            ? "Ein externes Backup-Ziel hat noch keine Kopie. Erstelle ein Backup, damit es eine bekommt."
            : `Die externe Kopie ist ${offsiteAgeDays} ${offsiteAgeDays === 1 ? "Tag" : "Tage"} alt. Erstelle ein Backup; kommen die täglichen Backups nicht an, prüfe die Ziele. Nach einer Wiederherstellung kopiert Klein.Buch erst nach der Freigabe wieder extern.`,
        cta: "Zu den Backups",
      };
    }
    return null;
  });

  // GJ-Abschluss-Erinnerung: Vorjahr noch nicht festgeschrieben?
  // currentYear ist das älteste offene GJ. Wenn currentYear < heutiges Jahr,
  // gibt es ein abschließbares Vorjahr.
  const overdueFy: number | null = $derived(
    fyOverview && fyOverview.currentYear < currentYear
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
    /** Badge-Wort und -Ton aus dem Status-Helfer des Bereichs (V3-07). */
    status: StatusView;
  };

  const recentEntries: RecentEntry[] = $derived.by(() => {
    const entries: RecentEntry[] = [];
    for (const i of invoices ?? []) {
      // Drafts ausblenden (kein „Beleg" im GoBD-Sinn).
      if (i.status === "draft") continue;
      const isStorno = i.isStornoFor !== null;
      // V3-07: dieselben Wörter wie in der Rechnungsliste (invoiceStatusView); die
      // Belegart „Storno-Beleg“ steht als Merkmal davor (Spalte „Art“).
      entries.push({
        kind: isStorno ? "ST" : "RE",
        href: `/invoices/${i.id}`,
        sortDate: i.invoiceDate,
        dateIso: i.invoiceDate,
        docNumber: i.invoiceNumber,
        counterparty: i.contactName,
        amountCents: i.grossAmountCents,
        status: invoiceStatusView(i, todayIso),
      });
    }
    for (const e of expenses ?? []) {
      entries.push({
        kind: "K",
        href: `/expenses/${e.id}`,
        sortDate: e.expenseDate,
        dateIso: e.expenseDate,
        docNumber: e.expenseNumber,
        counterparty: e.vendorNameSnapshot,
        amountCents: e.grossAmountCents,
        status: expenseStatusView(e),
      });
    }
    return entries.sort((a, b) => b.sortDate.localeCompare(a.sortDate)).slice(0, 5);
  });

  const KIND_LABEL: Record<RecentEntry["kind"], string> = {
    RE: "Rechnung",
    ST: "Storno-Beleg",
    K: "Kosten",
  };

  // U4 (V1-10): Firmendaten, die zum Ausstellen der ersten Rechnung fehlen — dieselben
  // Pflichtangaben, die der Server beim Ausstellen prüft (§14 UStG, XRechnung BR-DE-6:
  // Name, Anschrift, Telefon, Steuernummer oder USt-IdNr.). Nur Anzeige.
  const sellerTask = $derived.by<{ text: string; cta: string } | null>(() => {
    if (sellerState !== "ok") return null;
    if (!seller) {
      return {
        text: "Deine Firmendaten fehlen noch — ohne sie kannst du keine Rechnung ausstellen.",
        cta: "Firmendaten ausfüllen",
      };
    }
    const missing: string[] = [];
    if (!seller.name.trim()) missing.push("Firmenname");
    if (!seller.street.trim() || !seller.postalCode.trim() || !seller.city.trim()) missing.push("Anschrift");
    if (!seller.phone?.trim()) missing.push("Telefon");
    if (!seller.taxNumber?.trim() && !seller.vatId?.trim()) missing.push("Steuernummer oder USt-IdNr.");
    if (missing.length === 0) return null;
    return {
      text: `In deinen Firmendaten fehlt für das Ausstellen von Rechnungen: ${missing.join(", ")}.`,
      cta: "Firmendaten ergänzen",
    };
  });

  // Aufgaben-Liste: Anzahl Items (für Empty-State-Check).
  const taskTotal = $derived(
    (sellerTask ? 1 : 0) +
      overdueInvoices.length +
      recurringDue.length +
      (backupTask ? 1 : 0) +
      (overdueFy !== null ? 1 : 0) +
      (notifCount > 0 ? 1 : 0),
  );

  // Saldo-Farbe.
  const saldoIsNegative = $derived(euer !== null && euer.surplusCents < 0);

  // Kopfzeile der Seite (U1-064, design.md §8.1): Sachstand statt Begrüßung.
  const fyText = $derived(
    fyOverview
      ? `Geschäftsjahr ${fyOverview.currentYear}` +
          (showFyDay ? `, Tag ${fyDay} von ${fyDaysInYear}` : ", noch nicht abgeschlossen")
      : null,
  );
</script>

<!-- U4 (design.md §4.2): „Aktualisieren“ als Nebenaktion, bis 600 px im Menü „Mehr“. -->
<!-- V2-06: nicht `disabled` während des Ladens (der Knopf behielte sonst den Fokus nicht);
     ein zweiter Klick bleibt wirkungslos. -->
<PageBar
  title="Start"
  secondary={[
    {
      label: isRefreshing ? "Wird geladen …" : "Aktualisieren",
      onSelect: () => {
        if (!isRefreshing) void loadAll();
      },
    },
  ]}
>
  {#snippet actions()}
    <!-- Hauptaktion der Startseite, auf Mobil ganz oben erreichbar (U1, §8.5). -->
    <a class="btn-primary" href="/invoices/new">Rechnung schreiben</a>
  {/snippet}
</PageBar>

<!-- Sachstand (U1-064): Datum und Geschäftsjahr statt Begrüßung. Trennpunkt steht am Ende
     eines Teils, nie am Zeilenanfang (U1-065). -->
<!-- U4 (V4-10): Das Datum ist keine Abschnittsüberschrift (sonst wären die Karten darunter
     Unterabschnitte des Datums) — Text in --type-heading (V3-03: nicht größer als der
     Seitentitel, keine Hero-Fläche); Abschnitte sind h2 (design.md §2).
     V2-02: eine dauerhafte Statuszeile sagt das Laden an; die Platzhalter tragen aria-busy. -->
<p class="kb-visually-hidden" role="status">{isRefreshing ? "Wird geladen …" : ""}</p>
<section class="hero" aria-label="Sachstand">
  <p class="hero-date">{heroDate}</p>
  <p class="hero-meta">
    {#if fyText}
      <span class="hero-part">{fyText}{#if seller?.name}<span aria-hidden="true">{" "}·</span>{/if}</span>
    {:else if fyErr}
      <span class="hero-part hero-err">Geschäftsjahr konnte nicht geladen werden{#if seller?.name}<span aria-hidden="true">{" "}·</span>{/if}</span>
    {/if}
    {#if seller?.name}{" "}<span class="hero-part">{seller.name}</span>{/if}
  </p>
  {#if isKlein === true}
    <p class="hero-stripe">
      <strong>Kleinunternehmer (§19 UStG):</strong>{" "}
      {para19 ? `Auf deinen Rechnungen steht „${para19.hinweisText}“` : "Es wird keine Umsatzsteuer ausgewiesen."}
    </p>
  {/if}
</section>

<!-- U4 (V1-10, design.md §4.10): Solange Pflichtangaben fehlen, führt die Übersicht zu den
     Firmendaten — oben als Banner und als erste Aufgabe. -->
{#if sellerTask}
  <Banner kind="warning" href="/settings/seller?next=/" linkLabel={sellerTask.cta}>
    {sellerTask.text}
  </Banner>
{/if}

<!-- Kennzahlen (design.md §4.4): .kb-metric — Haarlinie, flacher Schatten, kein Anheben. -->
<section class="kpis" aria-label="Kennzahlen">
  <article class="kpi kb-metric">
    <p class="lbl">Einnahmen GJ</p>
    {#if euer === null && euerErr === null}
      <p class="val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="val">{euro(euer.totalIncomeCents)}</p>
    {/if}
    <p class="kpi-sub">seit 01.01.</p>
  </article>

  <article class="kpi kb-metric">
    <p class="lbl">Ausgaben GJ</p>
    {#if euer === null && euerErr === null}
      <p class="val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="val">{euro(euer.totalExpensesCents)}</p>
    {/if}
    <p class="kpi-sub">seit 01.01.</p>
  </article>

  <article class="kpi kb-metric" data-tone={saldoIsNegative ? "danger" : "success"}>
    <p class="lbl">Saldo</p>
    {#if euer === null && euerErr === null}
      <p class="val sk sk-num">&nbsp;</p>
    {:else if euerErr}
      <p class="kpi-err">{euerErr}</p>
    {:else if euer}
      <p class="val">{euro(euer.surplusCents)}</p>
    {/if}
    <p class="kpi-sub">vorläufige EÜR</p>
  </article>

  <a class="kpi kb-metric kpi-link" href="/invoices">
    <p class="lbl">Offene Rechnungen</p>
    {#if invoices === null && invoicesErr === null}
      <p class="val sk sk-num">&nbsp;</p>
    {:else if invoicesErr}
      <p class="kpi-err">{invoicesErr}</p>
    {:else}
      <p class="val">{openInvoicesCount}</p>
      <p class="kpi-sub">{euro(openInvoicesTotalCents)} ausstehend</p>
    {/if}
  </a>

  <a class="kpi kb-metric kpi-link kpi-backup" data-tone={backupTone} href="/settings/backup">
    <p class="lbl">
      {#if backupSettings !== null}<span class="dot dot-{backupTone}" aria-hidden="true"></span>{/if}
      Backup
    </p>
    {#if backupSettings === null && backupErr === null}
      <p class="val sk sk-num">&nbsp;</p>
    {:else if backupErr}
      <p class="kpi-err">{backupErr}</p>
    {:else}
      <p class="val val-small">{backupValue}</p>
      <p class="kpi-sub">{backupSub}</p>
    {/if}
  </a>
</section>

<!-- Schnellzugriff: eine Liste mit Lineatur statt vier gleicher Kacheln (design.md §8.1),
     globaler Baustein .kb-linklist wie der Einstellungen-Hub (V3-04). -->
<nav class="quick" aria-label="Schnellzugriff">
  <ul class="kb-linklist">
    <li>
      <a href="/invoices/new">
        <span class="kb-ll-title">Rechnung schreiben</span>
        <span class="kb-ll-sub">Neue Rechnung an einen Kunden</span>
      </a>
    </li>
    <li>
      <a href="/expenses/new">
        <span class="kb-ll-title">Kosten erfassen</span>
        <span class="kb-ll-sub">Etwas, das du fürs Geschäft bezahlt hast</span>
      </a>
    </li>
    <li>
      <a href="/quotes/new">
        <span class="kb-ll-title">Angebot erstellen</span>
        <span class="kb-ll-sub">Kostenvoranschlag für einen Kunden</span>
      </a>
    </li>
    <li>
      <a href="/contacts/new">
        <span class="kb-ll-title">Kontakt anlegen</span>
        <span class="kb-ll-sub">Neuer Kunde oder Lieferant</span>
      </a>
    </li>
  </ul>
</nav>

<!-- Zwei Spalten: Aufgaben + Letzte Belege -->
<section class="cols">
  <!-- Aufgaben -->
  <article class="col card" aria-labelledby="tasks-h">
    <header class="col-head">
      <h2 id="tasks-h">Anstehende Aufgaben</h2>
    </header>

    {#if invoices === null && recurring === null && backupSettings === null}
      <ul class="tasks" aria-busy="true">
        <li class="task sk sk-row">&nbsp;</li>
        <li class="task sk sk-row">&nbsp;</li>
        <li class="task sk sk-row">&nbsp;</li>
      </ul>
    {:else if taskTotal === 0}
      <p class="empty">Nichts zu tun: keine überfälligen Rechnungen, fälligen Abos oder offenen Hinweise.</p>
    {:else}
      <!-- U7 (R3-07, design.md §4.9): Die Art der Aufgabe ist ein Merkmal (Tag ohne
           Punkt); Status-Badge nur „Überfällig“. Die Dringlichkeit trägt der Text. -->
      <ul class="tasks">
        {#if sellerTask}
          <li class="task">
            <div class="task-row">
              <Badge kind="tag">Firmendaten</Badge>
              <span class="task-text">{sellerTask.text}</span>
              <a class="btn-secondary" href="/settings/seller?next=/">{sellerTask.cta}</a>
            </div>
          </li>
        {/if}

        {#each overdueInvoices.slice(0, 6) as inv (inv.id)}
          <li class="task">
            <a class="task-row" href="/invoices/{inv.id}">
              <Badge tone="danger">Überfällig</Badge>
              <span class="task-text">
                <DocNo value={inv.invoiceNumber} /> · {inv.contactName}
              </span>
              <span class="task-meta">
                {inv.overdueDays} {inv.overdueDays === 1 ? "Tag" : "Tage"} überfällig
              </span>
            </a>
          </li>
        {/each}

        {#each recurringDue.slice(0, 3) as r (r.id)}
          <li class="task">
            <a class="task-row" href="/recurring-invoices/new?id={r.id}">
              <Badge kind="tag">Abo fällig</Badge>
              <span class="task-text">{r.label}</span>
              <span class="task-meta">{fmtDate(r.nextDueDate)}</span>
            </a>
          </li>
        {/each}

        {#if backupTask}
          <li class="task">
            <div class="task-row">
              <Badge kind="tag">Backup</Badge>
              <span class="task-text">{backupTask.text}</span>
              <a class="btn-secondary" href="/settings/backup">{backupTask.cta}</a>
            </div>
          </li>
        {/if}

        {#if overdueFy !== null}
          <li class="task">
            <div class="task-row">
              <Badge kind="tag">GJ-Abschluss</Badge>
              <span class="task-text">Geschäftsjahr {overdueFy} noch nicht festgeschrieben.</span>
              <a class="btn-secondary" href="/fiscal-year">Abschließen</a>
            </div>
          </li>
        {/if}

        {#if notifCount > 0}
          <li class="task">
            <a class="task-row" href="/notifications">
              <Badge kind="tag">Hinweise</Badge>
              <span class="task-text">
                {notifCount} {notifCount === 1 ? "offener Hinweis" : "offene Hinweise"}.
              </span>
              <span class="task-meta">Ansehen <span aria-hidden="true">→</span></span>
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

  <!-- Letzte Belege: alle Zeilen in einem gemeinsamen Raster (U3-039, CSS subgrid). -->
  <article class="col card" aria-labelledby="recent-h">
    <header class="col-head">
      <h2 id="recent-h">Letzte Belege</h2>
    </header>

    {#if invoices === null && expenses === null}
      <ul class="recents-sk" aria-busy="true">
        <li class="sk sk-row">&nbsp;</li>
        <li class="sk sk-row">&nbsp;</li>
        <li class="sk sk-row">&nbsp;</li>
        <li class="sk sk-row">&nbsp;</li>
      </ul>
    {:else if recentEntries.length === 0}
      <!-- V2-09: dieselbe Beschriftung wie die Hauptaktion der PageBar, als secondary. -->
      <div class="empty">
        <p>Noch keine Belege — leg deine erste Rechnung an.</p>
        <a class="btn-secondary" href="/invoices/new">Rechnung schreiben</a>
      </div>
    {:else}
      <ul class="recents">
        {#each recentEntries as r (r.href + r.docNumber)}
          <li class="recent">
            <a class="recent-row" href={r.href}>
              <span class="recent-date">{fmtDate(r.dateIso)}</span>
              <!-- V2-05/V3-07: Belegart als Merkmal (Badge ohne Punkt); vorgelesen als Wort. -->
              <span class="recent-badge"><Badge kind="tag"><span aria-hidden="true">{r.kind}</span><span class="kb-visually-hidden">{KIND_LABEL[r.kind]}</span></Badge></span>
              <span class="recent-num"><DocNo value={r.docNumber} muted={r.status.strike} /></span>
              <span class="recent-party">{r.counterparty}</span>
              <span class="recent-amount">{euro(r.amountCents)}</span>
              <span class="recent-status"><Badge tone={r.status.tone} strike={r.status.strike}>{r.status.label}</Badge></span>
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
  /* ----- Sachstand (U1-064, U4) ---------------------------------------------
     Flach, kompakt: Datum in --type-heading (V3-03: kleiner als der Seitentitel, keine
     Hero-Fläche), Petrol-Kante links als einziges Akzentdetail (kein Verlauf, kein Schatten). */
  .hero {
    margin: 0 0 var(--sp-5);
    padding: var(--sp-4) var(--sp-5);
    background: var(--surface-card);
    border: var(--rule);
    border-left: 4px solid var(--accent);
    border-radius: var(--radius-card);
  }
  @media (max-width: 600px) {
    .hero { padding: var(--sp-3) var(--sp-4); margin-bottom: var(--sp-4); }
  }
  .hero-date {
    margin: 0 0 var(--sp-1);
    font-family: var(--font-display);
    font-size: var(--type-heading);
    line-height: var(--lh-snug);
    font-weight: 600;
    color: var(--text);
  }
  .hero-meta {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--type-body);
    display: flex;
    flex-wrap: wrap;
    column-gap: 0.35em;
  }
  .hero-part { overflow-wrap: break-word; min-width: 0; }
  .hero-err { color: var(--status-danger-fg); }
  .hero-stripe {
    margin: var(--sp-3) 0 0;
    padding: var(--sp-1) var(--sp-3);
    background: var(--status-primary-bg);
    color: var(--status-primary-fg);
    border-radius: var(--radius-control);
    font-size: var(--type-small);
    line-height: var(--lh-snug);
    display: inline-block;
    max-width: 100%;
  }

  /* ----- Kennzahlen ---------------------------------------------------------- */
  /* V3-01/V3-02: feste Spalten (die Kachel misst ihre Breite, tokens.css .kb-metric);
     Handy zweispaltig statt fünf gestapelter Karten, Backup über die volle Breite. */
  .kpis {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--sp-3);
    margin: 0 0 var(--sp-5);
  }
  .kpi { display: flex; flex-direction: column; gap: 2px; min-width: 0; }
  .kpi .lbl { display: flex; align-items: center; gap: var(--sp-2); }
  .kpi .val { margin: 2px 0; }
  /* Text statt Zahl („Noch keine Kopie“): darf umbrechen. */
  .kpi .val-small { font-size: var(--type-heading); white-space: normal; overflow-wrap: break-word; }
  .kpi[data-tone="success"] .val { color: var(--status-success-fg); }
  .kpi[data-tone="danger"] .val { color: var(--status-danger-fg); }
  .kpi[data-tone="warning"] .val { color: var(--status-warning-fg); }
  .kpi-link { text-decoration: none; color: inherit; transition: background var(--t-base) var(--ease-apple), border-color var(--t-base) var(--ease-apple); }
  /* Zeigen ändert nur die Fläche (kein Anheben, design.md §8.1); Fokus bleibt der Ring. */
  @media (hover: hover) {
    .kpi-link:hover { background: var(--surface-hover); border-color: var(--line-hover); }
  }
  .kpi-sub { margin: 0; font-size: var(--type-small); color: var(--text-muted); }
  .kpi-err { margin: 2px 0; font-size: var(--type-small); color: var(--status-danger-fg); }

  /* ----- Schnellzugriff (.kb-linklist, tokens.css) --------------------------- */
  .quick { margin: 0 0 var(--sp-5); }
  .quick .kb-linklist { grid-template-columns: 1fr; }

  /* U2-Sichtprüfung: keine einzelne Kachel allein in der letzten Zeile. Feste
     Spaltenzahlen nach der Breite des Inhaltsbereichs (<main>, Container „kb-main“). */
  @container kb-main (width < 36rem) {
    .kpi.kb-metric { padding: var(--sp-3); }
    .kpi-backup { grid-column: 1 / -1; }
  }
  @container kb-main (width >= 30rem) {
    .quick .kb-linklist { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  @container kb-main (width >= 36rem) {
    .kpis { grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--sp-4); }
  }
  @container kb-main (width >= 44rem) {
    .quick .kb-linklist { grid-template-columns: repeat(4, minmax(0, 1fr)); }
  }
  @container kb-main (width >= 66rem) {
    .kpis { grid-template-columns: repeat(5, minmax(0, 1fr)); }
  }

  /* ----- Zwei Spalten -------------------------------------------------------- */
  .cols {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--sp-4);
  }
  @media (max-width: 1023px) {
    .cols { grid-template-columns: 1fr; }
  }
  /* Zeilen richten sich nach der Breite der Karte, nicht des Fensters (U1). */
  .col { container-type: inline-size; min-width: 0; }
  .col-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    border-bottom: var(--rule);
    padding-bottom: var(--sp-2);
    margin-bottom: var(--sp-2);
  }
  .col-head h2 { margin: 0; font-size: var(--type-heading); line-height: var(--lh-snug); color: var(--text); }
  .empty {
    color: var(--text-muted);
    padding: var(--sp-4) var(--sp-2);
    display: flex;
    flex-direction: column;
    gap: var(--sp-3);
    align-items: flex-start;
    margin: 0;
  }
  .empty p { margin: 0; }
  .card-err {
    margin: var(--sp-3) 0 0;
    font-size: var(--type-small);
    color: var(--status-danger-fg);
  }

  /* ----- Aufgaben ------------------------------------------------------------ */
  .tasks { list-style: none; padding: 0; margin: 0; }
  .task + .task { border-top: var(--rule); }
  .task-row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: var(--sp-3);
    padding: var(--sp-2) 2px;
    /* Trefferfläche ganze Zeile ≥ --target (44 px im Drawer-Band, U1). */
    min-height: var(--target);
    text-decoration: none;
    color: var(--text);
  }
  @media (hover: hover) {
    a.task-row:hover { background: var(--surface-row-hover); }
  }
  /* Schmale Spalte: Badge und Zusatz über bzw. unter den Text, damit der Text
     umbrechen kann statt abgeschnitten zu werden (1.4.12). */
  @container (max-width: 30rem) {
    .task-row { grid-template-columns: auto 1fr; }
    .task-text { grid-column: 1 / -1; }
    .task-meta, .task-row > .btn-secondary { grid-column: 1 / -1; justify-self: start; }
  }
  /* Umbrechen statt abschneiden (1.4.12 Textabstände, U1). */
  .task-text { min-width: 0; overflow-wrap: break-word; }
  .task-meta { font-size: var(--type-small); color: var(--text-muted); white-space: nowrap; }

  /* ----- Letzte Belege (U3-039) ---------------------------------------------
     Ein Raster für alle Zeilen: Liste = Grid mit sechs Spalten, jede Zeile und ihr
     Link übernehmen die Spalten per subgrid — Datum, Art, Nummer, Partner, Betrag
     und Status stehen in Fluchten. Schmale Karte: zweispaltig gestapelt. */
  .recents {
    list-style: none; padding: 0; margin: 0;
    display: grid;
    grid-template-columns: auto auto auto minmax(0, 1fr) auto auto;
    column-gap: var(--sp-3);
  }
  .recent { display: grid; grid-column: 1 / -1; grid-template-columns: subgrid; }
  .recent + .recent { border-top: var(--rule); }
  .recent-row {
    display: grid;
    grid-column: 1 / -1;
    grid-template-columns: subgrid;
    align-items: center;
    padding: var(--sp-2) 2px;
    min-height: var(--target);
    text-decoration: none;
    color: var(--text);
  }
  @media (hover: hover) {
    .recent-row:hover { background: var(--surface-row-hover); }
  }
  .recent-date { font-size: var(--type-small); color: var(--text-muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
  /* U4 (V2-05/V3-07): Belegart ist kein Status — Badge kind="tag" für RE, ST und K. */
  .recent-badge { justify-self: start; }
  .recent-num { white-space: nowrap; color: var(--text); }
  /* Umbrechen statt abschneiden (1.4.12), Namen nur an Leerzeichen (U3). */
  .recent-party { color: var(--text-muted); overflow-wrap: break-word; min-width: 0; }
  .recent-amount { font-variant-numeric: tabular-nums; font-weight: 600; white-space: nowrap; text-align: right; }
  .recent-status { justify-self: start; white-space: nowrap; }

  @container (max-width: 36rem) {
    .recents { display: block; }
    .recent { display: block; }
    .recent-row { grid-template-columns: 1fr auto; column-gap: var(--sp-3); row-gap: 2px; }
    .recent-date { grid-column: 1; }
    .recent-badge { grid-column: 2; justify-self: end; }
    .recent-num { grid-column: 1 / span 2; }
    .recent-party { grid-column: 1 / span 2; }
    .recent-amount { grid-column: 1; text-align: left; }
    .recent-status { grid-column: 2; justify-self: end; }
  }

  /* ----- Punkte (Status mit Farbe UND Text daneben) -------------------------- */
  .dot {
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-subtle);
    flex: none;
  }
  .dot-success { background: var(--status-success-fg); }
  .dot-warning { background: var(--status-warning-fg); }
  .dot-danger { background: var(--status-danger-fg); }
  .dot-neutral { background: var(--text-subtle); }

  /* ----- Platzhalter beim Laden ---------------------------------------------
     Statische Flächen, nur damit das Layout nicht springt (design.md §5 „Laden“);
     keine Endlos-Animation (V3-06), die Ansage trägt die Statuszeile (role="status"). */
  .recents-sk { list-style: none; padding: 0; margin: 0; }
  .sk {
    background: var(--surface-page);
    border-radius: var(--r-sm);
    display: inline-block;
    color: transparent;
  }
  .sk-num { height: 1.7rem; width: 6rem; }
  .sk-row { display: block; height: 2rem; margin: var(--sp-2) 0; }
</style>
