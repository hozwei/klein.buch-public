// Stärke-Schätzung für eine neue Passphrase (Lauf 2026.11 U4, Warteliste (16);
// ADR 0041 Nr. 13, ASVS V2.1.8). Gemeinsamer Helfer für die Einrichtung
// (`BackupGate.svelte`) und den Passphrase-Wechsel (Einstellungen → Sicherheit).
//
// Nur ein Hinweis beim Tippen: Ob eine Passphrase angenommen wird, entscheidet
// der Server (`server::auth::check_new_passphrase`: mindestens 16 Zeichen,
// höchstens 1.024, Sperrliste bekannter Sätze, einfache Muster, Firmenname/E-Mail).
// Die Heuristik zählt Länge, Wörter und verschiedene Zeichen; sie blockiert nichts.

/** Mindestlänge wie der Server (`server::auth::MIN_PASSPHRASE`). */
export const PASSPHRASE_MIN_CHARS = 16;
/** Höchstlänge wie der Server (`server::passphrase_policy::MAX_CHARS`). */
export const PASSPHRASE_MAX_CHARS = 1024;

/** 0 = zu kurz/zu lang, 1 = schwach bzw. ausreichend, 2 = gut, 3 = stark. */
export type PassphraseStrengthLevel = 0 | 1 | 2 | 3;

export interface PassphraseStrength {
  level: PassphraseStrengthLevel;
  /** Sichtbarer Text hinter „Stärke (Schätzung): “. */
  text: string;
}

/** Wörter mit mindestens 3 Zeichen, getrennt durch Leerzeichen oder Satzzeichen. */
export function countPassphraseWords(p: string): number {
  return p
    .trim()
    .split(/[\s\-_.,;:+/]+/)
    .filter((w) => [...w].length >= 3).length;
}

/**
 * Schätzt die Stärke einer neuen Passphrase; `null` bei leerem Feld (dann keine
 * Anzeige). Zeichen zählen als Unicode-Zeichen (Umlaute und Zeichen außerhalb der BMP je 1), wie
 * `chars().count()` im Server.
 */
export function estimatePassphraseStrength(p: string): PassphraseStrength | null {
  if (!p) return null;
  const len = [...p].length;
  if (len < PASSPHRASE_MIN_CHARS) {
    return { level: 0, text: `Zu kurz: ${len} von mindestens ${PASSPHRASE_MIN_CHARS} Zeichen.` };
  }
  if (len > PASSPHRASE_MAX_CHARS) {
    return { level: 0, text: `Zu lang: höchstens ${PASSPHRASE_MAX_CHARS.toLocaleString("de-DE")} Zeichen.` };
  }
  const unique = new Set([...p.toLocaleLowerCase("de")]).size;
  if (unique < 6) {
    return { level: 1, text: "Schwach: viele Wiederholungen. Nimm 3–4 zufällige Wörter." };
  }
  const words = countPassphraseWords(p);
  if (len >= 28 || words >= 4) return { level: 3, text: "Stark." };
  if (len >= 20 || words >= 3) return { level: 2, text: "Gut." };
  return { level: 1, text: "Ausreichend. Länger ist besser, z. B. mit einem weiteren Wort." };
}
