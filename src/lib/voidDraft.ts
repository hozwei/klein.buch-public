// Verwerfen eines nummerierten Rechnungs-Entwurfs (Lauf F, W14-032; schlichte
// Schaltfläche seit F3b, Gestaltung im UI/UX-Lauf).
//
// Der Server verlangt eine Begründung von mindestens zehn Zeichen (getrimmt,
// ohne Steuerzeichen) und eine frische Bestätigung mit Daten-Passwort und
// App-Code (Re-Auth, über `transport.ts`). Die Prüfung hier spart nur den
// Umweg über die Fehlermeldung des Servers.

/** Mindestlänge der Begründung (wie `void_draft_core` im Server). */
export const VOID_REASON_MIN_CHARS = 10;

/** Fehlertext für eine zu kurze Begründung, sonst `null`. */
export function voidReasonError(reason: string): string | null {
  const cleaned = reason.trim().replace(/[\u0000-\u001f\u007f-\u009f]/g, "");
  return [...cleaned].length < VOID_REASON_MIN_CHARS
    ? `Bitte einen Grund angeben (mindestens ${VOID_REASON_MIN_CHARS} Zeichen) — er steht am Entwurf und im Protokoll.`
    : null;
}
