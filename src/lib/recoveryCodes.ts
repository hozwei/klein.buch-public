// Einmal-Codes kopieren oder als Textdatei speichern (U6-008): gleiche Lösung bei
// der Einrichtung (`BackupGate.svelte`) und nach „Neue Einmal-Codes erzeugen“ bzw.
// „Zweiten Faktor neu einrichten“ (`/settings/security`). Die Datei entsteht im
// Browser (Blob); nichts geht an den Server.

/** Inhalt von Zwischenablage und Textdatei. */
export function recoveryCodesText(codes: readonly string[]): string {
  return (
    "Klein.Buch – Einmal-Codes (jeder Code gilt einmal und ersetzt nur den App-Code)\n\n" +
    codes.join("\n") +
    "\n"
  );
}

/** Kopieren geht nur mit der Zwischenablage-API (Secure Context). */
export function canCopyRecoveryCodes(): boolean {
  return typeof navigator !== "undefined" && !!navigator.clipboard?.writeText;
}

/** Kopiert die Codes; zurück kommt die Meldung für die Statuszeile. */
export async function copyRecoveryCodes(codes: readonly string[]): Promise<string> {
  try {
    await navigator.clipboard.writeText(recoveryCodesText(codes));
    return "Codes kopiert. Füg sie in deinen Passwort-Manager ein.";
  } catch {
    return "Kopieren ging nicht. Speichere die Codes als Textdatei oder schreib sie ab.";
  }
}

/** Speichert die Codes als `klein-buch-einmal-codes.txt`; zurück kommt die Meldung. */
export function saveRecoveryCodes(codes: readonly string[]): string {
  const url = URL.createObjectURL(
    new Blob([recoveryCodesText(codes)], { type: "text/plain;charset=utf-8" }),
  );
  const a = document.createElement("a");
  a.href = url;
  a.download = "klein-buch-einmal-codes.txt";
  a.rel = "noopener";
  a.style.display = "none";
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 0);
  return "Textdatei gespeichert. Leg sie außerhalb dieses Servers ab.";
}
