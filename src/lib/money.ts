// Deutsche Betrags- und Mengeneingabe (Lauf 2026.11, Block U1; Plan §4.2,
// ADR 0041). Reine Funktionen ohne I/O.
//
// Geld bleibt Integer-Cent (Schema-Hardline): Eingaben werden als Text
// zerlegt und über Ziffern-Strings bzw. `bigint` in Cent umgerechnet — nie
// über `Number(text) * 100`. Mengen sind auch im Server `f64` (bis zu 4
// Nachkommastellen, `domain::money::QUANTITY_DECIMALS`); sie werden aus dem
// normalisierten Dezimal-String gelesen, nicht durch Division.
//
// Verbindliche Leseregel (Plan §4.2):
// - Leerzeichen am Rand und ein `€` am Rand werden ignoriert.
// - Kommen `,` und `.` vor, ist das letzte davon das Dezimalzeichen, das
//   andere der Tausendertrenner (Gruppen à 3 Ziffern, sonst Fehler).
// - Nur `,`: Dezimalzeichen (mehr als ein Komma: Fehler).
// - Nur `.`: genau ein Punkt mit 1–2 Ziffern danach ist ein Dezimalpunkt
//   (`12.5`, `12.50`), sonst Tausenderpunkt (`1.234`, `1.234.567`). Das gilt
//   auch für Mengen mit mehr Nachkommastellen: `1.234` ist 1234, nicht
//   1,234 — die Anzeige nach dem Verlassen des Feldes („1.234“) macht die
//   Auslegung sichtbar, und wer 1,234 meint, tippt das Komma.
// - Leerzeichen innerhalb der Zahl gelten als Tausendertrenner (DIN 5008
//   kennt die Gliederung mit Leerzeichen); sie müssen wie Punkte Dreiergruppen
//   bilden und dürfen nicht mit einem zweiten Tausendertrenner gemischt werden.
//   `12 5` ist deshalb ein Fehler, nicht 125.
// - Ein Tausender-Gruppenanfang `0` (`0.123`) ist ein Fehler, damit `0.125`
//   nicht still als 125 gelesen wird.
// - Mehr Nachkommastellen als erlaubt: Fehler, kein stilles Runden.
// - Vorzeichen `-`, `−` (U+2212) oder `+` nur am Anfang; `-`/`−` nur mit
//   `allowNegative`.
// - Leere Eingabe ist kein Fehler, sondern `value: null`; ob das Feld Pflicht
//   ist, entscheidet der Aufrufer.

/** Größter einzeln erfasster Betrag in Cent: 1 Mrd. € (Server: `domain::money::MAX_AMOUNT_CENTS`). */
export const MAX_AMOUNT_CENTS = 100_000_000_000;
/** Größte Menge je Position (Server: `domain::money::MAX_QUANTITY`). */
export const MAX_QUANTITY = 1_000_000;
/** Nachkommastellen einer Menge (Server: `domain::money::QUANTITY_DECIMALS`). */
export const QUANTITY_DECIMALS = 4;

export type ParseResult<T> =
  | { ok: true; value: T | null; normalized: string }
  | { ok: false; error: string };

export interface DecimalOptions {
  /** Erlaubte Nachkommastellen (0 = ganze Zahl). */
  maxFractionDigits: number;
  /** `-`/`−` zulassen (Standard: nein). */
  allowNegative?: boolean;
  /** Beispiel für die allgemeine Fehlermeldung, z. B. „1.234,56“. */
  example?: string;
  /** Wort für Fehlermeldungen: „Betrag“, „Wert“, „Menge“ … */
  noun?: string;
}

// Leerraum: normale, geschützte (U+00A0), schmale geschützte (U+202F) und
// weitere Unicode-Leerzeichen.
const SPACE = /[\s   ]/;
const SPACES_EDGE = /^[\s   ]+|[\s   ]+$/g;

function trimSpaces(s: string): string {
  return s.replace(SPACES_EDGE, "");
}

function stripEuroEdges(s: string): string {
  let t = trimSpaces(s);
  if (t.endsWith("€")) t = trimSpaces(t.slice(0, -1));
  if (t.startsWith("€")) t = trimSpaces(t.slice(1));
  return t;
}

/** Prüft Tausendergruppen: erste Gruppe 1–3 Ziffern ohne führende 0, Rest genau 3. */
function joinGroups(part: string, sep: string): string | null {
  const groups = part.split(sep);
  if (groups.length === 1) return /^\d*$/.test(part) ? part : null;
  const [first, ...rest] = groups;
  if (!/^[1-9]\d{0,2}$/.test(first)) return null;
  for (const g of rest) if (!/^\d{3}$/.test(g)) return null;
  return groups.join("");
}

function fractionError(maxFractionDigits: number): string {
  if (maxFractionDigits === 0) return "Bitte eine ganze Zahl ohne Nachkommastellen eingeben.";
  if (maxFractionDigits === 1) return "Höchstens 1 Nachkommastelle möglich.";
  return `Höchstens ${maxFractionDigits} Nachkommastellen möglich.`;
}

/**
 * Liest eine deutsche (oder englische) Dezimalzahl nach der Leseregel und
 * liefert sie skaliert als ganze Zahl: `value = Wert × 10^maxFractionDigits`
 * (`bigint`, damit nichts überläuft). `normalized` ist die Zahl im
 * DIN-Format (Tausenderpunkt, Dezimalkomma, Nachkommastellen ohne
 * überflüssige Nullen bis auf die Geld-Darstellung des Aufrufers).
 */
export function parseDecimal(text: string, opts: DecimalOptions): ParseResult<bigint> {
  const maxFrac = opts.maxFractionDigits;
  const noun = opts.noun ?? "Wert";
  const example = opts.example ?? (maxFrac > 0 ? "1.234,56" : "1234");
  const generic = `Bitte ${noun === "Betrag" ? "einen Betrag" : noun === "Menge" ? "eine Menge" : "eine Zahl"} wie ${example} eingeben.`;

  let s = stripEuroEdges(text ?? "");
  if (s === "") return { ok: true, value: null, normalized: "" };

  let negative = false;
  const sign = s[0];
  if (sign === "-" || sign === "−" || sign === "+") {
    negative = sign !== "+";
    s = stripEuroEdges(s.slice(1));
    if (s === "") return { ok: false, error: generic };
  }
  if (negative && !opts.allowNegative) {
    return {
      ok: false,
      error: noun === "Betrag" ? "Nur Beträge ab 0 möglich, ohne Minuszeichen." : "Nur Werte ab 0 möglich, ohne Minuszeichen.",
    };
  }

  // Innenliegende Leerzeichen → Tausendertrenner „ “ (eins pro Stelle).
  let body = "";
  for (const ch of s) body += SPACE.test(ch) ? " " : ch;
  if (!/^[0-9., ]+$/.test(body)) return { ok: false, error: generic };

  const hasComma = body.includes(",");
  const hasDot = body.includes(".");
  const hasSpace = body.includes(" ");
  let decimalSep: string | null = null;
  let groupSep: string | null = null;

  if (hasComma && hasDot) {
    decimalSep = body.lastIndexOf(",") > body.lastIndexOf(".") ? "," : ".";
    groupSep = decimalSep === "," ? "." : ",";
    if (body.indexOf(decimalSep) !== body.lastIndexOf(decimalSep)) return { ok: false, error: generic };
    if (hasSpace) return { ok: false, error: generic };
  } else if (hasComma) {
    if (body.indexOf(",") !== body.lastIndexOf(",")) return { ok: false, error: generic };
    decimalSep = ",";
  } else if (hasDot) {
    const first = body.indexOf(".");
    const onlyOne = first === body.lastIndexOf(".");
    const after = body.length - first - 1;
    if (onlyOne && after >= 1 && after <= 2) decimalSep = ".";
    else groupSep = ".";
  }
  if (hasSpace) {
    if (groupSep !== null) return { ok: false, error: generic };
    groupSep = " ";
  }

  let intRaw = body;
  let fracRaw = "";
  if (decimalSep !== null) {
    const at = body.indexOf(decimalSep);
    intRaw = body.slice(0, at);
    fracRaw = body.slice(at + 1);
  }
  if (!/^\d*$/.test(fracRaw)) return { ok: false, error: generic };
  const intDigits = groupSep !== null ? joinGroups(intRaw, groupSep) : /^\d*$/.test(intRaw) ? intRaw : null;
  if (intDigits === null) return { ok: false, error: generic };
  if (intDigits === "" && fracRaw === "") return { ok: false, error: generic };
  if (fracRaw.length > maxFrac) return { ok: false, error: fractionError(maxFrac) };

  let scaled = BigInt((intDigits || "0") + fracRaw.padEnd(maxFrac, "0"));
  if (negative && scaled !== 0n) scaled = -scaled;
  return { ok: true, value: scaled, normalized: formatScaled(scaled, maxFrac, false) };
}

/** Gruppiert eine Ziffernfolge mit Tausenderpunkten. */
function groupThousands(digits: string): string {
  return digits.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
}

/**
 * Skalierte ganze Zahl → DIN-Format. `fixed`: immer alle Nachkommastellen
 * (Geld), sonst ohne überflüssige Nullen (Mengen).
 */
function formatScaled(scaled: bigint, frac: number, fixed: boolean): string {
  const neg = scaled < 0n;
  const abs = (neg ? -scaled : scaled).toString().padStart(frac + 1, "0");
  const intPart = abs.slice(0, abs.length - frac);
  let fracPart = frac > 0 ? abs.slice(abs.length - frac) : "";
  if (!fixed) fracPart = fracPart.replace(/0+$/, "");
  const out = groupThousands(intPart) + (fracPart ? `,${fracPart}` : "");
  return neg ? `-${out}` : out;
}

export interface MoneyOptions {
  allowNegative?: boolean;
  /** Obergrenze des Betrags in Cent (Standard: 1 Mrd. € wie der Server). */
  maxAbsCents?: number;
}

/**
 * Text → Cent (`number`, garantiert `Number.isSafeInteger`). `value: null`
 * bei leerer Eingabe.
 */
export function parseMoneyToCents(text: string, opts: MoneyOptions = {}): ParseResult<number> {
  const r = parseDecimal(text, {
    maxFractionDigits: 2,
    allowNegative: opts.allowNegative,
    noun: "Betrag",
    example: "1.234,56",
  });
  if (!r.ok || r.value === null) return r as ParseResult<number>;
  const max = BigInt(Math.min(opts.maxAbsCents ?? MAX_AMOUNT_CENTS, Number.MAX_SAFE_INTEGER));
  const abs = r.value < 0n ? -r.value : r.value;
  if (abs > max) {
    return { ok: false, error: `Betrag zu groß (höchstens ${formatEuro(Number(max))}).` };
  }
  const cents = Number(r.value);
  return { ok: true, value: cents, normalized: formatCents(cents) };
}

export interface QuantityOptions {
  /** Nachkommastellen (Standard 4 wie der Server). 0 = ganze Zahl. */
  maxFractionDigits?: number;
  allowNegative?: boolean;
  /** Obergrenze des Betrags (inklusive); Standard: keine außer safe integer. */
  max?: number;
  /** Wort für Fehlermeldungen (Standard „Menge“). */
  noun?: string;
}

/**
 * Text → Zahl (Menge, Kilometer, Nutzungsdauer, Prozent, Tage …). Gelesen
 * wird aus dem normalisierten Dezimal-String (`Number("1.5")`), also exakt
 * die nächste `f64` zur Eingabe — keine Division.
 */
export function parseQuantity(text: string, opts: QuantityOptions = {}): ParseResult<number> {
  const frac = opts.maxFractionDigits ?? QUANTITY_DECIMALS;
  const noun = opts.noun ?? "Menge";
  const r = parseDecimal(text, {
    maxFractionDigits: frac,
    allowNegative: opts.allowNegative,
    noun,
    example: frac === 0 ? "12" : frac === 1 ? "1,5" : frac === 2 ? "1,25" : "1,5 oder 0,125",
  });
  if (!r.ok || r.value === null) return r as ParseResult<number>;
  const neg = r.value < 0n;
  const digits = (neg ? -r.value : r.value).toString().padStart(frac + 1, "0");
  const plain = frac > 0 ? `${digits.slice(0, -frac)}.${digits.slice(-frac)}` : digits;
  const value = Number((neg ? "-" : "") + plain);
  if (!Number.isFinite(value) || Math.abs(value) > Number.MAX_SAFE_INTEGER) {
    return { ok: false, error: "Zahl zu groß." };
  }
  if (frac === 0 && !Number.isSafeInteger(value)) return { ok: false, error: "Zahl zu groß." };
  if (opts.max !== undefined && Math.abs(value) > opts.max) {
    return { ok: false, error: `Höchstens ${formatQuantity(opts.max, frac)} möglich.` };
  }
  return { ok: true, value: value === 0 ? 0 : value, normalized: formatQuantity(value, frac) };
}

/**
 * Hinweis zur Auslegung einer Menge (U7, R1-08): Ein einzelner Punkt mit drei Ziffern
 * („1.250“) gilt als Tausenderpunkt (Plan-Regel), die DIN-Anzeige nach dem Verlassen
 * („1.250“) sieht aber genauso aus wie die Eingabe. Wer englisch „1.250“ für 1,25
 * tippt (iOS mit englischer Region zeigt nur „.“), sähe die Auslegung sonst nur am
 * Zeilen-Netto. Leer, wenn die Eingabe eindeutig ist oder das Feld weniger als drei
 * Nachkommastellen erlaubt (dann ist „1.250“ nur als 1250 lesbar).
 */
export function quantityReadingNote(text: string, maxFractionDigits = QUANTITY_DECIMALS): string {
  if (maxFractionDigits < 3) return "";
  const m = /^\s*(\d{1,3})\.(\d{3})\s*$/.exec(text);
  if (!m) return "";
  const r = parseQuantity(text, { maxFractionDigits });
  if (!r.ok || r.value === null) return "";
  const dec = m[2].replace(/0+$/, "");
  const alt = dec ? `${m[1]},${dec}` : m[1];
  return `Gelesen als ${formatQuantity(r.value, maxFractionDigits, false)} (Punkt = Tausender). Meinst du ${alt}? Dann mit Komma eingeben.`;
}

/** Echtes Minuszeichen (U+2212) für negative Beträge (U4, V3-22). */
const MINUS = "\u2212";

/**
 * Cent → `1.234,56` (DIN 5008, ohne €). Ganzzahl-Rechnung, keine Float-
 * Formatierung. Eine nicht ganze Eingabe (nur Anzeige-Rechnungen wie
 * Menge × Preis) wird kaufmännisch gerundet; nicht endlich: „—“.
 * Negative Beträge beginnen mit dem echten Minus „−“ (U+2212, so breit wie eine
 * Ziffer — die Fluchten tabellarischer Ziffern bleiben); der Betragsparser nimmt
 * „−“ und „-“ an, Eingabefelder (MoneyInput) zeigen deshalb auch „−“.
 */
export function formatCents(cents: number): string {
  if (!Number.isFinite(cents)) return "—";
  let c = cents;
  if (!Number.isInteger(c)) c = Math.sign(c) * Math.round(Math.abs(c));
  if (Number.isSafeInteger(c)) {
    const neg = c < 0;
    const abs = Math.abs(c);
    const rest = abs % 100;
    const euros = (abs - rest) / 100;
    const out = `${groupThousands(String(euros))},${String(rest).padStart(2, "0")}`;
    return neg ? `${MINUS}${out}` : out;
  }
  return formatScaled(BigInt(c), 2, true).replace(/^-/, MINUS);
}

/** Cent → `1.234,56 €` mit geschütztem Leerzeichen (U+00A0) vor `€`. */
export function formatEuro(cents: number): string {
  return `${formatCents(cents)} €`;
}

const quantityFormats = new Map<string, Intl.NumberFormat>();

/**
 * Menge → `1,5`, `1.000`, `0,125` (DIN 5008, ohne überflüssige Nullen,
 * höchstens `maxFractionDigits` Nachkommastellen). `grouping: false` für
 * Kennzahlen ohne Tausenderpunkt (Port, Jahr, Nummernkreis-Zähler).
 */
export function formatQuantity(
  value: number,
  maxFractionDigits = QUANTITY_DECIMALS,
  grouping = true,
): string {
  if (!Number.isFinite(value)) return "—";
  const key = `${maxFractionDigits}/${grouping}`;
  let f = quantityFormats.get(key);
  if (!f) {
    f = new Intl.NumberFormat("de-DE", {
      minimumFractionDigits: 0,
      maximumFractionDigits: maxFractionDigits,
      useGrouping: grouping,
    });
    quantityFormats.set(key, f);
  }
  // Intl kennzeichnet -0 als „-0“.
  return f.format(value === 0 ? 0 : value).replace("−", "-");
}
