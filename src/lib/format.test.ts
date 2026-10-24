import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { date, dateTime, euro } from "./format";

// Läuft mit TZ=UTC (package.json "test"); der zweite Block stellt das Gerät
// nach New York, damit die Gerätezone nachweislich nicht hilft.
describe("date/dateTime in Europe/Berlin (Lauf F, D-25)", () => {
  it("DB-Zeitstempel 23:30 UTC ist in Berlin der Folgetag", () => {
    expect(date("2026-10-23 23:30:00")).toBe("24.10.2026");
    expect(dateTime("2026-10-23 23:30:00")).toBe("24.10.2026, 01:30:00");
  });

  it("Winterzeit: UTC+1", () => {
    expect(dateTime("2026-12-31 23:30:00")).toBe("01.01.2027, 00:30:00");
    expect(date("2026-12-31 22:59:59")).toBe("31.12.2026");
  });

  it("RFC 3339 mit Z oder Versatz", () => {
    expect(dateTime("2026-10-23T23:30:00Z")).toBe("24.10.2026, 01:30:00");
    expect(dateTime("2026-10-24T01:30:00+02:00")).toBe("24.10.2026, 01:30:00");
  });

  it("reines Datum bleibt der Tag, der es ist", () => {
    expect(date("2026-10-24")).toBe("24.10.2026");
    expect(dateTime("2026-10-24")).toBe("24.10.2026");
  });

  it("leer und ungültig ohne Fehler", () => {
    expect(date(null)).toBe("—");
    expect(date("")).toBe("—");
    expect(date("kein Datum")).toBe("—");
    expect(date("2026-02-31")).toBe("—");
    expect(dateTime(null)).toBe("—");
    expect(dateTime(undefined, "noch nie")).toBe("noch nie");
    expect(dateTime("kaputt")).toBe("kaputt");
  });

  it("euro bleibt unverändert", () => {
    expect(euro(123456)).toBe("1.234,56 €");
  });
});

describe("Gerät in America/New_York (D-25)", () => {
  const saved = process.env.TZ;
  beforeAll(() => {
    process.env.TZ = "America/New_York";
  });
  afterAll(() => {
    process.env.TZ = saved;
  });

  it("reines Datum verrutscht nicht auf den Vortag", () => {
    expect(new Date("2026-10-24").getDate()).toBe(23); // so lag der Fehler
    expect(date("2026-10-24")).toBe("24.10.2026");
  });

  it("Zeitstempel erscheint in Berliner, nicht in New Yorker Zeit", () => {
    expect(dateTime("2026-10-23 23:30:00")).toBe("24.10.2026, 01:30:00");
    expect(date("2026-10-23 23:30:00")).toBe("24.10.2026");
  });
});
