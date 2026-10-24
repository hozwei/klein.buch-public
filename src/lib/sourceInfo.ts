// Quellangaben der laufenden Version (AGPL §13, Plan W12) für Anmeldung,
// Einrichtung und „Über"-Dialog. Die Werte liefert der Server ohne Sitzung
// (`GET /api/public/info`); ohne bekannten Commit gibt es keinen Link —
// nie auf einen Zweig wie `main` verlinken.

import type { PublicInfo } from "./transport";

/** Anzeige ohne bekannten Commit (wie `build_info::DEV_SOURCE_TEXT` im Server). */
export const DEV_SOURCE_TEXT = "Entwicklungs-Build, Quellstand unbekannt";

/** Kurzform eines Commit-SHA (7 Zeichen) bzw. `null`. */
export function shortCommit(commit: string | null | undefined): string | null {
  return commit ? commit.slice(0, 7) : null;
}

/** Link nur, wenn er auf http(s) zeigt (der Server prüft das ebenfalls). */
export function safeSourceLink(info: PublicInfo | null): string | null {
  const link = info?.sourceLink ?? null;
  return link && /^https?:\/\//i.test(link) ? link : null;
}

/** Beschriftung des Quell-Links: Tag und Commit-Kurzform bzw. der Dev-Text. */
export function sourceLabel(info: PublicInfo | null): string {
  const short = shortCommit(info?.commit);
  if (!info || !short || !safeSourceLink(info)) return DEV_SOURCE_TEXT;
  return info.tag ? `${info.tag} (Commit ${short})` : `Commit ${short}`;
}
