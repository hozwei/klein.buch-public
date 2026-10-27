// U4 (Warteliste (6)): Entwürfe werden unter `/invoices/<id>/edit` bearbeitet.
// Alte Links und Lesezeichen `/invoices/new?edit=<id>` (Lauf F5) leiten dorthin weiter.

import { redirect } from "@sveltejs/kit";

export function load({ url }: { url: URL }): void {
  const id = url.searchParams.get("edit");
  if (id) throw redirect(307, `/invoices/${encodeURIComponent(id)}/edit`);
}
