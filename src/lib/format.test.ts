import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { date, dateTime, dateTimeShort, euro, iban, percent } from "./format";

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

  it("euro: DIN 5008 mit geschütztem Leerzeichen (U+00A0) vor €", () => {
    expect(euro(123456)).toBe("1.234,56 €");
    expect(euro(0)).toBe("0,00 €");
    expect(euro(-5)).toBe("-0,05 €");
    expect(euro(100_000_000_000)).toBe("1.000.000.000,00 €");
  });
});

describe("percent / iban (Lauf 2026.11 U1)", () => {
  it("percent mit U+00A0 vor %", () => {
    expect(percent(19)).toBe("19 %");
    expect(percent(7.5)).toBe("7,5 %");
    expect(percent(0)).toBe("0 %");
    expect(percent(-0)).toBe("0 %");
    expect(percent(Number.NaN)).toBe("—");
  });

  it("iban in Vierergruppen", () => {
    expect(iban("DE89370400440532013000")).toBe("DE89 3704 0044 0532 0130 00");
    expect(iban(" de89 3704 0044 0532 013000 ")).toBe("DE89 3704 0044 0532 0130 00");
    expect(iban("DE89")).toBe("DE89");
    expect(iban("")).toBe("—");
    expect(iban(null)).toBe("—");
  });
});

describe("dateTimeShort (Lauf 2026.11 U1)", () => {
  it("ohne Sekunden, Berliner Zeit (Sommer- und Winterzeit)", () => {
    expect(dateTimeShort("2026-10-23 23:30:59")).toBe("24.10.2026, 01:30");
    expect(dateTimeShort("2026-12-31 23:30:00")).toBe("01.01.2027, 00:30");
    expect(dateTimeShort("2026-10-23T23:30:00Z")).toBe("24.10.2026, 01:30");
  });

  it("reines Datum ohne Uhrzeit, leer und ungültig wie dateTime", () => {
    expect(dateTimeShort("2026-10-24")).toBe("24.10.2026");
    expect(dateTimeShort(null)).toBe("—");
    expect(dateTimeShort("", "noch nie")).toBe("noch nie");
    expect(dateTimeShort("kaputt")).toBe("kaputt");
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
    expect(dateTimeShort("2026-10-23 23:30:00")).toBe("24.10.2026, 01:30");
  });
});
