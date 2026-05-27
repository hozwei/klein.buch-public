// G4.1-Fix — Display-Helper für die App-Version.
//
// Cargo/Tauri verlangen semver (MAJOR.MINOR.PATCH). Klein.Buch nutzt
// CalVer im Schema `YYYY.M.PATCH` (Beispiele: 2026.5.0 = Mai-Release,
// 2026.5.1 = Bugfix im selben Monat, 2026.6.0 = Juni-Release).
//
// Im UI zeigen wir den Patch nur, wenn er > 0 ist — saubere
// Monats-Releases tragen kein nacktes „.0", Bugfixes bleiben aber
// klar erkennbar. Präfix ist großes „V" (Manuel-Vorgabe 2026-05-26).
//
// Fällt der Input nicht parsbar aus, geben wir „V" + Rohwert zurück
// (keine Crashes — der Footer darf bei einem unbekannten Format
// nicht stumm bleiben).
export function formatVersionDisplay(version: string): string {
  if (!version) return "";
  const [major, minor, patch] = version.split(".");
  let core = major ?? "";
  if (minor !== undefined && minor !== "") core += `.${minor}`;
  if (patch !== undefined && patch !== "" && patch !== "0") {
    core += `.${patch}`;
  }
  return `V${core}`;
}
