// G2-DOC.3.1 — /help-Index leitet auf die Willkommens-Seite um.
//
// Die Welcome-Seite (`willkommen.md`, Kategorie `erste-schritte`, order 10)
// existiert bereits aus G2-DOC.2.1. Eine zweite Cover-Übersicht wäre Redundanz,
// daher direkter Redirect. Spätere Sub-Sub-Blöcke können das überschreiben,
// wenn G2-DOC.3.x eine Landingpage mit Such-Highlight braucht.

import { redirect } from "@sveltejs/kit";

export function load(): never {
  throw redirect(307, "/help/willkommen");
}
