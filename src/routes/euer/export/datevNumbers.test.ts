// ADR 0042 Nr. 15: Pflichtnummern des DATEV-Stapels, Meldungen am Feld.
import { describe, expect, it } from "vitest";
import {
  datevFileName,
  datevNumberErrors,
  numberHint,
  packageWithoutDatev,
  parseStoredNumber,
  rangeError,
  serverErrorField,
} from "./datevNumbers.ts";

describe("datevNumberErrors", () => {
  it("verlangt beim Einzel-Export beide Nummern", () => {
    const e = datevNumberErrors(null, null, "datev");
    expect(e.consultant).toMatch(/^Beraternummer fehlt/);
    expect(e.client).toMatch(/^Mandantennummer fehlt/);
    expect(datevNumberErrors(29098, null, "datev").client).toMatch(/^Mandantennummer fehlt/);
    expect(datevNumberErrors(29098, null, "datev").consultant).toBe("");
    expect(datevNumberErrors(29098, 55003, "datev")).toEqual({ consultant: "", client: "" });
  });

  it("nimmt beim Paket beide leer an, eine allein nicht", () => {
    expect(datevNumberErrors(null, null, "package")).toEqual({ consultant: "", client: "" });
    const e = datevNumberErrors(null, 55003, "package");
    expect(e.consultant).toMatch(/^Beraternummer fehlt: .*ohne DATEV-Stapel/);
    expect(e.client).toBe("");
  });

  it("prüft die Bereiche aus der Formatbeschreibung", () => {
    expect(datevNumberErrors(1000, 1, "datev").consultant).toMatch(/1001 und 9999999/);
    expect(datevNumberErrors(1001, 1, "datev")).toEqual({ consultant: "", client: "" });
    expect(datevNumberErrors(9_999_999, 99_999, "datev")).toEqual({ consultant: "", client: "" });
    expect(datevNumberErrors(10_000_000, 1, "datev").consultant).not.toBe("");
    expect(datevNumberErrors(1001, 100_000, "datev").client).toMatch(/1 und 99999/);
    // Ein Wert außerhalb wird auch beim Paket benannt, die andere Nummer fehlt dann.
    const e = datevNumberErrors(12, null, "package");
    expect(e.consultant).toMatch(/^Beraternummer prüfen/);
    expect(e.client).toMatch(/^Mandantennummer fehlt/);
  });
});

describe("Hilfsfunktionen", () => {
  it("rangeError lässt leere Felder in Ruhe", () => {
    expect(rangeError("consultant", null)).toBe("");
    expect(rangeError("client", 0)).not.toBe("");
  });

  it("numberHint nennt den Bereich", () => {
    expect(numberHint("consultant")).toBe("Pflicht für den DATEV-Stapel; 1001 bis 9999999.");
    expect(numberHint("client")).toBe("Pflicht für den DATEV-Stapel; 1 bis 99999.");
  });

  it("packageWithoutDatev nur ohne beide Nummern", () => {
    expect(packageWithoutDatev(null, null)).toBe(true);
    expect(packageWithoutDatev(1001, null)).toBe(false);
  });

  it("serverErrorField ordnet die Server-Meldungen zu", () => {
    expect(serverErrorField("Beraternummer fehlt: Der DATEV-Buchungsstapel …")).toBe("consultant");
    expect(serverErrorField("Beraternummer muss zwischen 1001 und 9999999 liegen.")).toBe("consultant");
    expect(serverErrorField("Mandantennummer fehlt: …")).toBe("client");
    expect(serverErrorField("Server nicht erreichbar.")).toBeNull();
  });

  it("datevFileName wie datev_csv::file_name", () => {
    expect(datevFileName(2026, "SKR03")).toBe("EXTF_Buchungsstapel_2026_SKR03.csv");
    expect(datevFileName(2026, "skr04")).toBe("EXTF_Buchungsstapel_2026_SKR04.csv");
    expect(datevFileName(2027, "XYZ")).toBe("EXTF_Buchungsstapel_2027_SKR03.csv");
  });

  it("parseStoredNumber nimmt nur ganze Zahlen über 0", () => {
    expect(parseStoredNumber(null)).toBeNull();
    expect(parseStoredNumber("")).toBeNull();
    expect(parseStoredNumber("0")).toBeNull();
    expect(parseStoredNumber("1,5")).toBeNull();
    expect(parseStoredNumber("29098")).toBe(29098);
  });
});
