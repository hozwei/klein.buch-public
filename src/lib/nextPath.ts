// Rücksprungziel aus `?next=` (U6-005, U7 R2-01): nur relative Pfade innerhalb
// der App, keine offene Weiterleitung. Erlaubt ist ein Pfad, der mit genau
// einem „/“ beginnt (kein „//host“, kein „/\host“), ohne Backslash und
// Steuerzeichen (auch nicht prozentkodiert) und nicht auf die Server-Pfade
// `/api`, `/files` und `/legal`. Geprüft wird der Rohtext UND der normalisierte
// Pfad: `new URL()` löst Punkt-Segmente auf („/.//evil.example“ → „//evil.example“),
// deshalb darf auch das Ergebnis keine leeren Segmente enthalten und muss beim
// erneuten Auflösen auf demselben Origin bleiben. Zurück kommt der normalisierte
// Pfad samt Suche und Anker, sonst `null`.

const SERVER_PREFIXES = ["/api", "/files", "/legal"];
const BASE = "http://kb.invalid";
// Backslash (Browser lesen „/\host“ als „//host“) und Steuerzeichen (Tab,
// Zeilenumbruch werden beim URL-Parsen entfernt).
const FORBIDDEN = /[\\\u0000-\u001f\u007f]/;

function decodePath(path: string): string | null {
  try {
    return decodeURIComponent(path);
  } catch {
    return null;
  }
}

function isSafePath(path: string): boolean {
  if (!path.startsWith("/") || path.startsWith("//")) return false;
  // Leere Segmente („/a//b“) haben in der App keine Bedeutung; abgelehnt, damit
  // keine spätere Normalisierung daraus ein „//host“ machen kann.
  if (path.includes("//")) return false;
  if (FORBIDDEN.test(path)) return false;
  const lower = path.toLowerCase();
  return !SERVER_PREFIXES.some((p) => lower === p || lower.startsWith(p + "/"));
}

export function safeNextPath(raw: string | null | undefined): string | null {
  if (!raw || raw.length > 512) return null;
  if (!raw.startsWith("/") || raw.startsWith("//")) return null;
  if (FORBIDDEN.test(raw)) return null;
  let url: URL;
  try {
    url = new URL(raw, BASE);
  } catch {
    return null;
  }
  if (url.origin !== BASE) return null;
  const path = url.pathname;
  // Normalisierter Pfad: roh und dekodiert („%2f“, „%5c“, „%2e“, „%09“).
  if (!isSafePath(path)) return null;
  const decoded = decodePath(path);
  if (decoded === null || !isSafePath(decoded)) return null;
  const result = path + url.search + url.hash;
  // Gegenprobe: Das Ergebnis muss als relativer Link auf demselben Origin bleiben.
  let check: URL;
  try {
    check = new URL(result, BASE + "/settings/seller");
  } catch {
    return null;
  }
  if (check.origin !== BASE || check.pathname !== path) return null;
  return result;
}
