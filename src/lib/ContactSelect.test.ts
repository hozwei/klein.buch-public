// U4 (Warteliste (15)): Such- und Sortierlogik der Kontaktauswahl. Die Bedienung
// (Suchfeld + natives Auswahlfeld) prüft das UI-Harness im Browser.
import { describe, expect, it } from "vitest";
import { filterContacts, foldDe, sortContacts } from "./ContactSelect.svelte";

const contacts = [
  { id: "1", name: "Zahnrad GmbH", city: "Berlin" },
  { id: "2", name: "Ärztehaus Beispielplatz PartG mbB", city: "Köln" },
  { id: "3", name: "Öko-Gärtnerei Muster", city: "München" },
  { id: "4", name: "Bäckerei Müller", city: "Ulm" },
  { id: "5", name: "Jörg Übermuth-Österreicher", city: "Wien" },
  { id: "6", name: "Grafikbüro Pixel Beispiel", city: null },
  { id: "7", name: "Straßenbau Süd", city: "Essen" },
];

describe("foldDe", () => {
  it("faltet Groß/klein, Umlaute (auch ae/oe/ue) und ß", () => {
    expect(foldDe("Müller")).toBe("muller");
    expect(foldDe("Mueller")).toBe("muller");
    expect(foldDe("ÜBERMUTH")).toBe("ubermuth");
    expect(foldDe("Straße")).toBe("strasse");
    expect(foldDe("Café")).toBe("cafe");
  });
});

describe("sortContacts", () => {
  it("sortiert nach deutscher Kollation (Umlaute beim Grundbuchstaben)", () => {
    expect(sortContacts(contacts).map((c) => c.id)).toEqual(["2", "4", "6", "5", "3", "7", "1"]);
  });
  it("verändert die Eingabe nicht", () => {
    const before = contacts.map((c) => c.id);
    sortContacts(contacts);
    expect(contacts.map((c) => c.id)).toEqual(before);
  });
});

describe("filterContacts", () => {
  it("leerer Suchtext liefert alle", () => {
    expect(filterContacts(contacts, "  ")).toHaveLength(contacts.length);
  });
  it("findet mit und ohne Umlaut", () => {
    expect(filterContacts(contacts, "muell").map((c) => c.id)).toEqual(["4"]);
    expect(filterContacts(contacts, "übermuth").map((c) => c.id)).toEqual(["5"]);
    expect(filterContacts(contacts, "uebermuth").map((c) => c.id)).toEqual(["5"]);
    expect(filterContacts(contacts, "strasse").map((c) => c.id)).toEqual(["7"]);
  });
  it("verlangt alle Wörter, in Name oder Zusatz (Ort)", () => {
    expect(filterContacts(contacts, "beispiel köln").map((c) => c.id)).toEqual(["2"]);
    expect(filterContacts(contacts, "beispiel").map((c) => c.id)).toEqual(["2", "6"]);
    expect(filterContacts(contacts, "beispiel wien")).toEqual([]);
  });
  it("nutzt einen eigenen Zusatz (z. B. E-Mail beim Versand)", () => {
    const withMail = [{ id: "a", name: "Muster GmbH", email: "buero@example.org" }];
    expect(filterContacts(withMail, "example", (c) => c.email ?? "")).toHaveLength(1);
    expect(filterContacts(withMail, "example")).toHaveLength(0);
  });
});
