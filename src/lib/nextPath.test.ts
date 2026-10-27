// U6-005: `?next=` auf „Meine Firmendaten“ führt nur zu Pfaden innerhalb der App.
import { describe, expect, it } from "vitest";
import { safeNextPath } from "./nextPath.ts";

describe("safeNextPath", () => {
  it("nimmt relative Pfade der App samt Suche und Anker", () => {
    expect(safeNextPath("/invoices/new")).toBe("/invoices/new");
    expect(safeNextPath("/quotes/abc?x=1#y")).toBe("/quotes/abc?x=1#y");
    expect(safeNextPath("/")).toBe("/");
  });

  it("lehnt leere, absolute und protokollrelative Ziele ab", () => {
    expect(safeNextPath(null)).toBeNull();
    expect(safeNextPath("")).toBeNull();
    expect(safeNextPath("invoices/new")).toBeNull();
    expect(safeNextPath("https://example.org/")).toBeNull();
    expect(safeNextPath("//example.org/")).toBeNull();
    expect(safeNextPath("/\\example.org/")).toBeNull();
    expect(safeNextPath("javascript:alert(1)")).toBeNull();
    expect(safeNextPath("/\texample")).toBeNull();
    expect(safeNextPath("/" + "a".repeat(600))).toBeNull();
  });

  it("lehnt Server-Pfade ab und normalisiert Punkt-Segmente", () => {
    expect(safeNextPath("/api/cmd/x")).toBeNull();
    expect(safeNextPath("/files/x")).toBeNull();
    expect(safeNextPath("/legal")).toBeNull();
    expect(safeNextPath("/invoices/../api/cmd/x")).toBeNull();
    expect(safeNextPath("/apix")).toBe("/apix");
  });
});

// U7 R2-01: Punkt-Segmente, Kodierungen und leere Segmente dürfen nach der
// Normalisierung keinen protokollrelativen Link und keinen Server-Pfad ergeben.
describe("safeNextPath nach Normalisierung", () => {
  const sameOrigin = (p: string) => {
    const u = new URL(p, "https://buch.example/settings/seller");
    return u.origin === "https://buch.example" && !p.startsWith("//");
  };

  it.each([
    "/.//evil.example",
    "/.//evil.example/login",
    "/x/..//evil.example/",
    "/%2e//evil.example",
    "/%2E%2E//evil.example",
    "/.//api/cmd/x",
    "/x/../..//evil.example",
    "/%2f%2fevil.example",
    "/%2F/evil.example",
    "/%5cevil.example",
    "/%5C%5Cevil.example",
    "/\\evil.example",
    "/x\\..\\..\\evil.example",
    "/\t/evil.example",
    "/\n/evil.example",
    "/%09/evil.example",
    "/%0a/evil.example",
    "/invoices//x",
    "/%61pi/cmd/x",
    "/API/cmd/x",
    "/x/%2e%2e/api/cmd/x",
    "/%",
  ])("lehnt %j ab", (raw) => {
    expect(safeNextPath(raw)).toBeNull();
  });

  it.each(["/invoices/new", "/quotes/abc?x=1#y", "/x/../invoices", "/./contacts", "/help/bel%C3%A4ge"])(
    "lässt %j zu und bleibt auf demselben Origin",
    (raw) => {
      const r = safeNextPath(raw);
      expect(r).not.toBeNull();
      expect(sameOrigin(r!)).toBe(true);
    },
  );

  it("normalisiert Punkt-Segmente innerhalb der App", () => {
    expect(safeNextPath("/x/../invoices")).toBe("/invoices");
    expect(safeNextPath("/./contacts")).toBe("/contacts");
  });
});
