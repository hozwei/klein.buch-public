// Formatierungs-Helper für deutsche Zahlen-, Datums- und Geld-Darstellung.

const NUMBER_DE = new Intl.NumberFormat("de-DE", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

const DATE_DE = new Intl.DateTimeFormat("de-DE", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
});

export function euro(cents: number): string {
  return `${NUMBER_DE.format(cents / 100)} €`;
}

export function date(iso: string | null | undefined): string {
  if (!iso) return "—";
  return DATE_DE.format(new Date(iso));
}
