// Steuerstatus am Datum eines Belegs für die Editoren (ADR 0042 Nr. 3).
//
// Die Editoren (Rechnung, Angebot, Umwandlung, Abo-Vorlage) zeigen den Status am
// maßgeblichen Datum des Belegs, nicht den von heute. `taxStatusAtDate` fragt ihn
// je Datum beim Server ab (`tax_status_at`, zwischengespeichert) und hält den
// letzten Wert, bis der neue da ist — so springen die Steuerspalten nicht.
// Aufruf nur während der Initialisierung einer Komponente (nutzt `$effect`).

import { taxStatusAt } from "#lib/api.ts";
import { isIsoDate } from "#lib/date.ts";
import { TaxStatusCache } from "#lib/taxStatus.ts";

export interface TaxStatusAtDate {
  /** §19 am Datum; `null`, bis der Status für ein Datum geladen ist. */
  readonly isKlein: boolean | null;
  /** Datum, zu dem `isKlein` gehört. */
  readonly date: string | null;
  /** Ladefehler (Text) oder `null`. */
  readonly error: string | null;
  /** Lädt den Status für `day` und setzt ihn (z. B. vor dem Ende des Ladens). */
  load(day: string): Promise<void>;
}

export function taxStatusAtDate(
  getDate: () => string,
  fetch: (day: string) => Promise<boolean> = (d) => taxStatusAt(d).then((r) => r.isKleinunternehmer),
): TaxStatusAtDate {
  const cache = new TaxStatusCache(fetch);
  let isKlein = $state<boolean | null>(null);
  let resolvedFor = $state<string | null>(null);
  let error = $state<string | null>(null);

  function set(day: string, v: boolean) {
    isKlein = v;
    resolvedFor = day;
    error = null;
  }

  $effect(() => {
    const day = getDate();
    if (!isIsoDate(day)) return;
    const known = cache.peek(day);
    if (known !== undefined) {
      set(day, known);
      return;
    }
    let alive = true;
    cache.get(day).then(
      (v) => {
        if (alive) set(day, v);
      },
      (e: unknown) => {
        if (alive) error = String(e);
      },
    );
    return () => {
      alive = false;
    };
  });

  return {
    get isKlein() {
      return isKlein;
    },
    get date() {
      return resolvedFor;
    },
    get error() {
      return error;
    },
    async load(day: string) {
      if (!isIsoDate(day)) return;
      const v = await cache.get(day);
      // Inzwischen anderes Datum: den Wert setzt der Effect für das neue Datum.
      if (getDate() === day) set(day, v);
    },
  };
}
