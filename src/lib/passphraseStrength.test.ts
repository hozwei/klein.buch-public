// U4 (Warteliste (16)): gemeinsame Stärke-Schätzung für neue Passphrasen
// (Einrichtung in BackupGate, Wechsel unter Einstellungen → Sicherheit).
import { describe, expect, it } from "vitest";
import {
  PASSPHRASE_MAX_CHARS,
  PASSPHRASE_MIN_CHARS,
  countPassphraseWords,
  estimatePassphraseStrength,
} from "./passphraseStrength.ts";

describe("estimatePassphraseStrength", () => {
  it("zeigt bei leerem Feld nichts", () => {
    expect(estimatePassphraseStrength("")).toBeNull();
  });

  it("nennt unter der Mindestlänge des Servers die Zeichenzahl", () => {
    expect(PASSPHRASE_MIN_CHARS).toBe(16);
    expect(estimatePassphraseStrength("kurz")).toEqual({ level: 0, text: "Zu kurz: 4 von mindestens 16 Zeichen." });
    expect(estimatePassphraseStrength("a".repeat(15))?.level).toBe(0);
  });

  it("zählt Unicode-Zeichen wie der Server (Umlaute, Zeichen außerhalb der BMP je 1)", () => {
    // 15 Zeichen, davon Umlaute und „𝔸“ (U+1D538, 2 UTF-16-Einheiten): weiter zu kurz.
    const p = "Ärger über 𝔸 öl";
    expect([...p].length).toBe(15);
    expect(estimatePassphraseStrength(p)).toEqual({ level: 0, text: "Zu kurz: 15 von mindestens 16 Zeichen." });
  });

  it("meldet Passphrasen über der Höchstlänge", () => {
    expect(PASSPHRASE_MAX_CHARS).toBe(1024);
    const r = estimatePassphraseStrength("abcdefghij".repeat(103));
    expect(r).toEqual({ level: 0, text: "Zu lang: höchstens 1.024 Zeichen." });
    expect(estimatePassphraseStrength("abcdefghijklmnop".repeat(64))?.level).toBe(3);
  });

  it("stuft viele Wiederholungen als schwach ein", () => {
    expect(estimatePassphraseStrength("aaaaabbbbbcccccdddd")).toEqual({
      level: 1,
      text: "Schwach: viele Wiederholungen. Nimm 3–4 zufällige Wörter.",
    });
    // Groß/klein zählt nicht als verschieden.
    expect(estimatePassphraseStrength("AaAaAbBbBbCcCcDdDdEe")?.level).toBe(1);
  });

  it("bewertet Länge und Wortzahl", () => {
    expect(estimatePassphraseStrength("Tannenbaum7Kerze")).toEqual({
      level: 1,
      text: "Ausreichend. Länger ist besser, z. B. mit einem weiteren Wort.",
    });
    expect(estimatePassphraseStrength("Ampel Gurke Hafen")).toEqual({ level: 2, text: "Gut." });
    expect(estimatePassphraseStrength("Tannenbaum7Kerze-Blau")?.level).toBe(2);
    expect(estimatePassphraseStrength("Ampel Gurke Hafen Zebra")).toEqual({ level: 3, text: "Stark." });
    expect(estimatePassphraseStrength("Tannenbaum7KerzeLichtSchnee!")?.level).toBe(3);
  });
});

describe("countPassphraseWords", () => {
  it("zählt Wörter ab 3 Zeichen, getrennt durch Leer- und Satzzeichen", () => {
    expect(countPassphraseWords("Ampel-Gurke_Hafen.Zebra")).toBe(4);
    expect(countPassphraseWords("ab cd Efg")).toBe(1);
    expect(countPassphraseWords("  Öl Äpfel Übermut  ")).toBe(2);
    expect(countPassphraseWords("")).toBe(0);
  });
});
