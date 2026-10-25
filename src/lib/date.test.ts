import { afterAll, beforeAll, describe, expect, it } from "vitest";
import {
  addDaysIso,
  berlinDateOf,
  dayOfYearBerlin,
  daysBetweenIso,
  daysFromTodayBerlin,
  daysInYear,
  daysUntilBerlin,
  isIsoDate,
  parseTimestamp,
  todayBerlin,
  yearBerlin,
  yearOfIsoDate,
  msUntilNextBerlinMidnight,
} from "./date";

// Läuft mit TZ=UTC (package.json "test"), damit die Prozess-Zeitzone nicht hilft.
describe("todayBerlin", () => {
  it("Silvester 23:30 UTC ist in Berlin schon Neujahr", () => {
    const now = new Date("2026-12-31T23:30:00Z");
    expect(todayBerlin(now)).toBe("2027-01-01");
    expect(yearBerlin(now)).toBe(2027);
  });

  it("Sommerzeit: 21:59 UTC ist noch derselbe Tag, 22:00 UTC der nächste", () => {
    expect(todayBerlin(new Date("2026-06-30T21:59:00Z"))).toBe("2026-06-30");
    expect(todayBerlin(new Date("2026-06-30T22:00:00Z"))).toBe("2026-07-01");
  });

  it("Winterzeit: 22:59 UTC ist noch derselbe Tag, 23:00 UTC der nächste", () => {
    expect(todayBerlin(new Date("2026-11-30T22:59:00Z"))).toBe("2026-11-30");
    expect(todayBerlin(new Date("2026-11-30T23:00:00Z"))).toBe("2026-12-01");
  });
});

describe("Kalenderrechnung", () => {
  it("addDaysIso über Monats- und Jahresgrenzen", () => {
    expect(addDaysIso("2026-12-25", 14)).toBe("2027-01-08");
    expect(addDaysIso("2028-02-28", 1)).toBe("2028-02-29");
    expect(addDaysIso("2026-03-29", 1)).toBe("2026-03-30");
    expect(addDaysIso("kein-datum", 3)).toBe("kein-datum");
  });

  it("daysFromTodayBerlin rechnet ab dem Berliner Datum", () => {
    expect(daysFromTodayBerlin(30, new Date("2026-12-31T23:30:00Z"))).toBe("2027-01-31");
  });

  it("yearOfIsoDate liest das Jahr des Belegdatums", () => {
    expect(yearOfIsoDate("2027-01-01")).toBe(2027);
    expect(yearOfIsoDate("", new Date("2026-12-31T23:30:00Z"))).toBe(2027);
  });

  it("dayOfYearBerlin und daysInYear", () => {
    expect(dayOfYearBerlin(new Date("2026-12-31T23:30:00Z"))).toBe(1);
    expect(dayOfYearBerlin(new Date("2026-12-31T12:00:00Z"))).toBe(365);
    expect(daysInYear(2028)).toBe(366);
    expect(daysInYear(2100)).toBe(365);
  });
});

describe("Zeitstempel und Tagesabstände (Lauf F, D-15/D-25)", () => {
  it("parseTimestamp liest DB-Zeitstempel ohne Zone als UTC", () => {
    expect(parseTimestamp("2026-10-23 23:30:00")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
    expect(parseTimestamp("2026-10-23T23:30:00")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
    expect(parseTimestamp("2026-10-23 23:30")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
    expect(parseTimestamp("2026-10-23 23:30:00.123456")?.toISOString()).toBe("2026-10-23T23:30:00.123Z");
  });

  it("parseTimestamp beachtet Z und Versatz", () => {
    expect(parseTimestamp("2026-10-23T23:30:00Z")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
    expect(parseTimestamp("2026-10-24T01:30:00+02:00")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
    expect(parseTimestamp("2026-10-24T01:30:00+0200")?.toISOString()).toBe("2026-10-23T23:30:00.000Z");
  });

  it("parseTimestamp: reines Datum = UTC-Mitternacht, ungültig = null", () => {
    expect(parseTimestamp("2026-10-24")?.toISOString()).toBe("2026-10-24T00:00:00.000Z");
    expect(parseTimestamp("2026-02-31")).toBeNull();
    expect(parseTimestamp("2026-02-31 10:00:00")).toBeNull();
    expect(parseTimestamp("gestern")).toBeNull();
    expect(parseTimestamp("")).toBeNull();
    expect(parseTimestamp(null)).toBeNull();
    expect(isIsoDate("2028-02-29")).toBe(true);
    expect(isIsoDate("2026-02-29")).toBe(false);
  });

  it("berlinDateOf: 23:30 UTC ist in Berlin schon der Folgetag", () => {
    expect(berlinDateOf("2026-10-23 23:30:00")).toBe("2026-10-24");
    expect(berlinDateOf("2026-10-23 21:59:59")).toBe("2026-10-23");
    // Winterzeit: erst 23:00 UTC ist Mitternacht
    expect(berlinDateOf("2026-11-30 22:59:00")).toBe("2026-11-30");
    expect(berlinDateOf("2026-11-30 23:00:00")).toBe("2026-12-01");
    expect(berlinDateOf("2026-10-24")).toBe("2026-10-24");
    expect(berlinDateOf("kaputt")).toBeNull();
  });

  it("daysBetweenIso zählt Kalendertage, auch über die Zeitumstellung", () => {
    expect(daysBetweenIso("2026-10-24", "2026-10-24")).toBe(0);
    expect(daysBetweenIso("2026-10-24", "2026-10-31")).toBe(7);
    expect(daysBetweenIso("2026-10-31", "2026-10-24")).toBe(-7);
    // Ende der Sommerzeit am 25.10.2026, Beginn am 29.03.2026
    expect(daysBetweenIso("2026-10-24", "2026-10-26")).toBe(2);
    expect(daysBetweenIso("2026-03-28", "2026-03-30")).toBe(2);
    expect(daysBetweenIso("2026-12-31", "2027-01-01")).toBe(1);
    expect(daysBetweenIso("kaputt", "2026-10-24")).toBeNaN();
  });

  it("daysUntilBerlin rechnet ab dem Berliner Datum", () => {
    // 23:30 UTC am 23.10. ist in Berlin der 24.10.: heute fällig, nicht morgen.
    const now = new Date("2026-10-23T23:30:00Z");
    expect(daysUntilBerlin("2026-10-24", now)).toBe(0);
    expect(daysUntilBerlin("2026-10-23", now)).toBe(-1);
    expect(daysUntilBerlin("2026-10-25", now)).toBe(1);
    // Fälligkeit als Zeitstempel: zählt mit ihrem Berliner Tag
    expect(daysUntilBerlin("2026-10-24T00:00:00Z", now)).toBe(0);
    // Silvester 23:30 UTC: in Berlin Neujahr
    expect(daysUntilBerlin("2027-01-01", new Date("2026-12-31T23:30:00Z"))).toBe(0);
  });
});

describe("Gerät in einer anderen Zeitzone (D-15)", () => {
  const saved = process.env.TZ;
  beforeAll(() => {
    process.env.TZ = "America/New_York";
  });
  afterAll(() => {
    process.env.TZ = saved;
  });

  it("New York 23:00 (Berlin schon Folgetag) ergibt den Berliner Wert", () => {
    // 23.10.2026 23:00 in New York = 24.10.2026 05:00 in Berlin
    const now = new Date("2026-10-24T03:00:00Z");
    expect(new Date(now).getDate()).toBe(23); // Gerät sieht noch den 23.
    expect(todayBerlin(now)).toBe("2026-10-24");
    expect(daysUntilBerlin("2026-10-24", now)).toBe(0);
    expect(daysUntilBerlin("2026-10-31", now)).toBe(7);
  });

  it("am Tag der Zeitumstellung bleibt der Abstand ganzzahlig", () => {
    const now = new Date("2026-10-25T10:00:00Z");
    expect(daysUntilBerlin("2026-10-26", now)).toBe(1);
    expect(daysUntilBerlin("2026-10-24", now)).toBe(-1);
  });
});

describe("msUntilNextBerlinMidnight (D-27)", () => {
  it("Sommerzeit: 23:59 Berlin → eine Minute", () => {
    expect(msUntilNextBerlinMidnight(new Date("2026-07-14T21:59:00Z"))).toBe(60_000);
  });

  it("Winterzeit: 23:30 Berlin am Silvesterabend → 30 Minuten", () => {
    expect(msUntilNextBerlinMidnight(new Date("2026-12-31T22:30:00Z"))).toBe(30 * 60_000);
  });

  it("Tag der Zeitumstellung (25 Stunden) zählt bis Mitternacht Winterzeit", () => {
    // 25.10.2026 00:30 Berlin (Sommerzeit) → 26.10.2026 00:00 Berlin (Winterzeit)
    expect(msUntilNextBerlinMidnight(new Date("2026-10-24T22:30:00Z"))).toBe(24.5 * 3_600_000);
  });

  it("genau Mitternacht → volle 24 Stunden", () => {
    expect(msUntilNextBerlinMidnight(new Date("2026-07-14T22:00:00Z"))).toBe(24 * 3_600_000);
  });
});
