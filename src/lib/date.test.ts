import { describe, expect, it } from "vitest";
import {
  addDaysIso,
  dayOfYearBerlin,
  daysFromTodayBerlin,
  daysInYear,
  todayBerlin,
  yearBerlin,
  yearOfIsoDate,
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
