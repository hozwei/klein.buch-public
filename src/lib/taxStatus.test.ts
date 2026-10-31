// ADR 0042 Nr. 1–6: Helfer des Steuerstatus in der Oberfläche (spiegeln
// domain::tax_status; der Server prüft unabhängig davon).

import { describe, expect, it, vi } from "vitest";
import {
  ANFANG,
  TaxStatusCache,
  bindingEnd,
  bindingUntilFor,
  kleinTodayOrLater,
  changeConfirmLabel,
  choosableReasons,
  consequenceLines,
  effectiveFromError,
  effectiveFromLabel,
  lastEntry,
  nextChangeAfter,
  nextJanFirst,
  invoiceTaxDate,
  quoteConversionHint,
  relevantTaxDate,
  statusAt,
  statusNoticeText,
  suggestChange,
  thresholdHintText,
} from "./taxStatus";
import type { TaxStatus, TaxStatusChange, TaxStatusReason, TaxThresholdHint } from "./types";

function entry(
  id: string,
  effectiveFrom: string,
  status: TaxStatus,
  reason: TaxStatusReason,
  extra: Partial<TaxStatusChange> = {},
): TaxStatusChange {
  return {
    id,
    effectiveFrom,
    status,
    reason,
    bindingUntil: null,
    triggerDocument: null,
    note: null,
    createdAt: "2026-01-01 00:00:00",
    revokedAt: null,
    revokeReason: null,
    ...extra,
  };
}

const verzicht2026 = entry("v", "2026-01-01", "regelbesteuerung", "verzicht", { bindingUntil: "2031-01-01" });

describe("relevantTaxDate", () => {
  it("Leistungsdatum vor Belegdatum vor heute (ADR 0042 Nr. 3)", () => {
    expect(relevantTaxDate("2026-07-14", "2026-07-20", "2026-10-27")).toBe("2026-07-14");
    expect(relevantTaxDate(null, "2026-07-20", "2026-10-27")).toBe("2026-07-20");
    expect(relevantTaxDate("", "", "2026-10-27")).toBe("2026-10-27");
  });

  it("ungültige oder halb getippte Daten fallen zurück", () => {
    expect(relevantTaxDate("2026-02-30", "2026-03-01", "2026-10-27")).toBe("2026-03-01");
    expect(relevantTaxDate("2026-0", undefined, "2026-10-27")).toBe("2026-10-27");
    expect(relevantTaxDate("2026-07-14T00:00:00", null, "2026-10-27")).toBe("2026-07-14");
  });
});

describe("invoiceTaxDate", () => {
  it("Leistungsdatum, sonst Ende des Zeitraums, sonst Rechnungsdatum (ADR 0042 Nr. 3/10)", () => {
    expect(invoiceTaxDate({ deliveryDate: "2026-07-14", invoiceDate: "2026-07-20" }, "2026-10-27")).toBe("2026-07-14");
    expect(
      invoiceTaxDate({ deliveryDate: null, servicePeriodEnd: "2026-12-31", invoiceDate: "2026-12-01" }, "2026-10-27"),
    ).toBe("2026-12-31");
    expect(invoiceTaxDate({ deliveryDate: null, servicePeriodEnd: null, invoiceDate: "2026-07-20" }, "2026-10-27")).toBe(
      "2026-07-20",
    );
    expect(invoiceTaxDate({}, "2026-10-27")).toBe("2026-10-27");
    expect(invoiceTaxDate({ servicePeriodEnd: "2026-13-01", invoiceDate: "2026-07-20" }, "2026-10-27")).toBe("2026-07-20");
  });
});

describe("statusAt", () => {
  it("ohne Eintrag §19", () => {
    expect(statusAt([], "2026-05-01")).toBe("kleinunternehmer");
  });

  it("Stichtag inklusiv, widerrufene Einträge zählen nicht", () => {
    const laufend = entry("l", "2026-07-15", "regelbesteuerung", "wegfall_laufend");
    expect(statusAt([laufend], "2026-07-14")).toBe("kleinunternehmer");
    expect(statusAt([laufend], "2026-07-15")).toBe("regelbesteuerung");
    const revoked = { ...laufend, revokedAt: "2026-08-01 10:00:00", revokeReason: "Irrtum" };
    expect(statusAt([revoked], "2026-07-20")).toBe("kleinunternehmer");
  });

  it("mehrfacher Wechsel, unsortierte Eingabe", () => {
    const list = [
      entry("r2", "2033-01-01", "regelbesteuerung", "wegfall_vorjahr"),
      entry("i", ANFANG, "regelbesteuerung", "initial"),
      entry("k", "2030-01-01", "kleinunternehmer", "rueckkehr"),
    ];
    expect(statusAt(list, "2029-12-31")).toBe("regelbesteuerung");
    expect(statusAt(list, "2030-01-01")).toBe("kleinunternehmer");
    expect(statusAt(list, "2033-06-01")).toBe("regelbesteuerung");
    expect(lastEntry(list)?.id).toBe("r2");
    expect(nextChangeAfter(list, "2031-05-05")?.id).toBe("r2");
    expect(nextChangeAfter(list, "2034-01-01")).toBeNull();
  });
});

describe("choosableReasons", () => {
  it("ohne Eintrag: Anfangsstand oder Wechsel zur Regelbesteuerung", () => {
    expect(choosableReasons([])).toEqual([
      "initial",
      "verzicht",
      "wegfall_vorjahr",
      "wegfall_laufend",
      "wegfall_gruendungsjahr",
    ]);
  });

  it("nach dem jüngsten Eintrag, nicht nach heute", () => {
    expect(choosableReasons([verzicht2026])).toEqual(["rueckkehr"]);
    const back = entry("k", "2031-01-01", "kleinunternehmer", "rueckkehr");
    expect(choosableReasons([verzicht2026, back])).not.toContain("rueckkehr");
    expect(choosableReasons([verzicht2026, back])).not.toContain("initial");
  });

  it("ein widerrufener jüngster Eintrag zählt nicht", () => {
    const revoked = { ...verzicht2026, revokedAt: "2026-02-01 08:00:00" };
    expect(choosableReasons([revoked])[0]).toBe("initial");
  });
});

describe("suggestChange", () => {
  const ctx = { today: "2026-10-27", entries: [] as TaxStatusChange[], earliestReturn: null, hints: [] as TaxThresholdHint[] };

  it("Verzicht: nächster 1. Januar; am 1. Januar selbst dieser Tag", () => {
    expect(suggestChange("verzicht", ctx).effectiveFrom).toBe("2027-01-01");
    expect(suggestChange("verzicht", { ...ctx, today: "2027-01-01" }).effectiveFrom).toBe("2027-01-01");
    expect(nextJanFirst("2026-12-31")).toBe("2027-01-01");
  });

  it("Wegfall Vorjahr: 1. Januar des laufenden Jahres", () => {
    expect(suggestChange("wegfall_vorjahr", ctx).effectiveFrom).toBe("2026-01-01");
  });

  it("Wegfall laufend: Datum und Beleg aus dem Grenzhinweis", () => {
    const hint: TaxThresholdHint = {
      kind: "laufend",
      year: 2026,
      turnoverCents: 10_050_000,
      limitCents: 10_000_000,
      exceeded: true,
      triggerDocument: "RE-2026-0042",
      suggestedEffectiveFrom: "2026-07-15",
      suggestedReason: "wegfall_laufend",
      presumed: false,
    };
    expect(suggestChange("wegfall_laufend", { ...ctx, hints: [hint] })).toEqual({
      effectiveFrom: "2026-07-15",
      triggerDocument: "RE-2026-0042",
    });
    expect(suggestChange("wegfall_laufend", ctx)).toEqual({ effectiveFrom: "", triggerDocument: "" });
  });

  it("Rückkehr: frühestmögliche Rückkehr laut Server", () => {
    expect(
      suggestChange("rueckkehr", { ...ctx, entries: [verzicht2026], earliestReturn: "2031-01-01" }).effectiveFrom,
    ).toBe("2031-01-01");
  });

  it("Vorschlag zum 1. Januar liegt nach dem jüngsten Eintrag", () => {
    const future = entry("v", "2027-01-01", "regelbesteuerung", "verzicht");
    expect(suggestChange("rueckkehr", { ...ctx, entries: [future] }).effectiveFrom).toBe("2028-01-01");
  });

  it("Anfangsstand ohne Datum", () => {
    expect(suggestChange("initial", ctx).effectiveFrom).toBe("");
  });
});

describe("effectiveFromError", () => {
  const today = "2026-10-27";

  it("Pflicht und gültiges Datum; Anfangsstand ohne Datum", () => {
    expect(effectiveFromError("verzicht", "", today)).toMatch(/Wirkungsdatum/);
    expect(effectiveFromError("verzicht", "2026-02-30", today)).toMatch(/gültiges Datum/);
    expect(effectiveFromError("verzicht", "1899-01-01", today)).toMatch(/gültiges Datum/);
    expect(effectiveFromError("initial", "", today)).toBe("");
  });

  it("Verzicht, Wegfall Vorjahr und Rückkehr nur zum 1. Januar", () => {
    for (const r of ["verzicht", "wegfall_vorjahr", "rueckkehr"] as const) {
      expect(effectiveFromError(r, "2026-03-01", today)).toMatch(/1\. Januar/);
    }
    expect(effectiveFromError("verzicht", "2027-01-01", today)).toBe("");
  });

  it("Wegfall laufend: beliebiger Tag, nicht in der Zukunft", () => {
    expect(effectiveFromError("wegfall_laufend", "2026-07-15", today)).toBe("");
    expect(effectiveFromError("wegfall_laufend", "2026-10-27", today)).toBe("");
    expect(effectiveFromError("wegfall_gruendungsjahr", "2026-10-28", today)).toMatch(/Zukunft/);
  });

  it("Wegfall Vorjahr nicht für ein künftiges Jahr", () => {
    expect(effectiveFromError("wegfall_vorjahr", "2027-01-01", today)).toMatch(/nicht im Voraus/);
    expect(effectiveFromError("wegfall_vorjahr", "2026-01-01", today)).toBe("");
  });
});

describe("Bindung", () => {
  it("fünf Kalenderjahre einschließlich des Wirkungsjahres (ADR 0042 Nr. 2)", () => {
    expect(bindingUntilFor("verzicht", "2026-01-01")).toBe("2031-01-01");
    expect(bindingEnd("2031-01-01")).toBe("2030-12-31");
  });

  it("nur beim Verzicht", () => {
    expect(bindingUntilFor("wegfall_vorjahr", "2026-01-01")).toBeNull();
    expect(bindingUntilFor("verzicht", "")).toBeNull();
    expect(bindingEnd(null)).toBeNull();
  });
});

describe("Texte", () => {
  it("Wirkungsdatum „seit Beginn“", () => {
    expect(effectiveFromLabel(ANFANG)).toBe("seit Beginn");
    expect(effectiveFromLabel("2026-07-15")).toBe("15.07.2026");
  });

  it("Knopf je Grund", () => {
    expect(changeConfirmLabel("verzicht")).toBe("Verzicht eintragen");
    expect(changeConfirmLabel("wegfall_laufend")).toBe("Wegfall eintragen");
    expect(changeConfirmLabel("rueckkehr")).toBe("Rückkehr eintragen");
    expect(changeConfirmLabel(null)).toBe("Wechsel eintragen");
  });

  it("Grenzhinweis nennt Beleg, Vorschlag und Vermutung", () => {
    const t = thresholdHintText({
      kind: "gruendungsjahr",
      year: 2026,
      turnoverCents: 2_600_000,
      limitCents: 2_500_000,
      exceeded: true,
      triggerDocument: "RE-2026-0007",
      suggestedEffectiveFrom: "2026-09-01",
      suggestedReason: "wegfall_gruendungsjahr",
      presumed: true,
    });
    expect(t).toContain("RE-2026-0007");
    expect(t).toContain("01.09.2026");
    expect(t).toContain("Gründungsjahr");
    expect(t).toMatch(/26\.000,00\s€/);
  });

  it("Grenze genau erreicht: kein Wegfall", () => {
    const t = thresholdHintText({
      kind: "vorjahr",
      year: 2025,
      turnoverCents: 2_500_000,
      limitCents: 2_500_000,
      exceeded: false,
      triggerDocument: null,
      suggestedEffectiveFrom: null,
      suggestedReason: "wegfall_vorjahr",
      presumed: false,
    });
    expect(t).toContain("erreicht damit genau");
    expect(t).toContain("gilt weiter");
  });

  it("Folgen als Zeilen; ohne Folgen leer", () => {
    expect(
      consequenceLines({
        firstDifference: null,
        affectedInvoices: [],
        affectedDrafts: [],
        recurringTemplates: [],
        capitalizedAssets: [],
        reexportYears: [],
      }),
    ).toEqual([]);
    const lines = consequenceLines({
      firstDifference: "2026-07-15",
      affectedInvoices: [
        {
          id: "1",
          number: "RE-2026-0042",
          documentDate: "2026-07-20",
          lockedAsKleinunternehmer: true,
          nowKleinunternehmer: false,
        },
      ],
      affectedDrafts: ["RE-2026-0050"],
      recurringTemplates: ["Wartung"],
      capitalizedAssets: ["AV-2026-0001"],
      reexportYears: [2026],
    });
    expect(lines).toHaveLength(5);
    expect(lines[0]).toContain("§ 31 Abs. 5 UStDV");
    expect(lines[0]).toContain("1 festgeschriebene Rechnung passt");
    expect(lines[4]).toContain("des Jahres 2026");
  });
});

describe("statusNoticeText", () => {
  const base = { day: "15.07.2026", dateLabel: "Leistungsdatum", noun: "auf der Rechnung" };

  it("§19 wie heute: Hinweis ohne Abweichung", () => {
    const t = statusNoticeText({ ...base, isKlein: true, todayKlein: true });
    expect(t?.differs).toBe(false);
    expect(t?.body).toContain("keine Umsatzsteuer");
  });

  it("Regelbesteuerung wie heute: kein Hinweis", () => {
    expect(statusNoticeText({ ...base, isKlein: false, todayKlein: false })).toBeNull();
  });

  it("Abweichung von heute in beide Richtungen", () => {
    const regel = statusNoticeText({ ...base, isKlein: false, todayKlein: true });
    expect(regel?.differs).toBe(true);
    expect(regel?.title).toContain("Regelbesteuerung");
    expect(regel?.body).toContain("19 %");
    const klein = statusNoticeText({ ...base, isKlein: true, todayKlein: false });
    expect(klein?.differs).toBe(true);
    expect(klein?.title).toContain("Kleinunternehmer");
  });
});

describe("quoteConversionHint", () => {
  it("nur bei unterschiedlichem Status", () => {
    expect(quoteConversionHint(true, true)).toBeNull();
    expect(quoteConversionHint(null, false)).toBeNull();
    expect(quoteConversionHint(true, false)).toContain("19 %");
    expect(quoteConversionHint(false, true)).toContain("ohne Umsatzsteuer");
  });
});

describe("TaxStatusCache", () => {
  it("eine Anfrage je Datum, gleichzeitige teilen sich das Ergebnis", async () => {
    const fetch = vi.fn((d: string) => Promise.resolve(d < "2026-07-15"));
    const c = new TaxStatusCache(fetch);
    expect(c.peek("2026-07-14")).toBeUndefined();
    const [a, b] = await Promise.all([c.get("2026-07-14"), c.get("2026-07-14")]);
    expect(a).toBe(true);
    expect(b).toBe(true);
    expect(await c.get("2026-07-15")).toBe(false);
    expect(c.peek("2026-07-14")).toBe(true);
    await c.get("2026-07-14");
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it("Fehler werden nicht gemerkt", async () => {
    let fail = true;
    const fetch = vi.fn(() => (fail ? Promise.reject(new Error("offline")) : Promise.resolve(true)));
    const c = new TaxStatusCache(fetch);
    await expect(c.get("2026-01-01")).rejects.toThrow("offline");
    fail = false;
    expect(await c.get("2026-01-01")).toBe(true);
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});

describe("kleinTodayOrLater", () => {
  it("heute §19 oder künftige Rückkehr", () => {
    expect(kleinTodayOrLater([], "2026-10-27")).toBe(true);
    expect(kleinTodayOrLater([verzicht2026], "2026-10-27")).toBe(false);
    const back = entry("k", "2031-01-01", "kleinunternehmer", "rueckkehr");
    expect(kleinTodayOrLater([verzicht2026, back], "2026-10-27")).toBe(true);
    expect(kleinTodayOrLater([verzicht2026, { ...back, revokedAt: "2026-10-01 09:00:00" }], "2026-10-27")).toBe(false);
  });
});
