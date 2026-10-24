// Browser-Hilfen, die auch ohne Secure Context funktionieren (Plan W9): Über
// HTTP im LAN fehlen `crypto.randomUUID` und `navigator.clipboard`.

/** Zufällige ID (UUID, sonst 32 Hex-Zeichen aus `getRandomValues`). */
export function randomId(): string {
  const c = globalThis.crypto;
  if (c && typeof c.randomUUID === "function") return c.randomUUID();
  const bytes = new Uint8Array(16);
  if (c && typeof c.getRandomValues === "function") {
    c.getRandomValues(bytes);
  } else {
    for (let i = 0; i < bytes.length; i++) bytes[i] = Math.floor(Math.random() * 256);
  }
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** Text in die Zwischenablage; ohne Secure Context über ein verstecktes
 *  Textfeld (`execCommand("copy")`). */
export async function copyText(text: string): Promise<void> {
  if (globalThis.isSecureContext && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const ta = document.createElement("textarea");
  ta.value = text;
  ta.setAttribute("readonly", "");
  ta.style.position = "fixed";
  ta.style.top = "0";
  ta.style.opacity = "0";
  document.body.appendChild(ta);
  ta.select();
  try {
    if (!document.execCommand("copy")) {
      throw new Error("Die Zwischenablage ist in diesem Browser nicht verfügbar.");
    }
  } finally {
    ta.remove();
  }
}
