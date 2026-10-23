// HTTP-Transport zum Server (Plan W5.11, ADR 0039) — ersetzt die Tauri-Bridge.
//
// `invoke(name, args)` schickt `POST /api/cmd/{name}` mit JSON-Argumenten,
// Pflicht-Header `X-KleinBuch-Request` und einem Idempotenz-Schlüssel je
// Aufruf. Fehler kommen wie früher als String (die Meldung des Servers).
// 401 → Anmeldung nötig, 423 → Datenbank gesperrt (Hinweis an die Auth-Maske
// über `onAuthRequired`). 202 → Job: das Ergebnis wird über
// `GET /api/jobs/{id}` abgeholt. Netzwerkfehler und 503 `timeout`/`busy`
// werden mit demselben Schlüssel wiederholt; der Server führt eine Aktion
// dadurch höchstens einmal aus und liefert bei `timeout` das Ergebnis der
// noch laufenden ersten Ausführung. 403 `reauth_required` (Plan W6) öffnet den
// Re-Auth-Dialog (`setReauthHandler`) und wiederholt danach mit demselben
// Schlüssel.

export const REQUEST_HEADER = "X-KleinBuch-Request";
export const IDEMPOTENCY_HEADER = "Idempotency-Key";

/** Warum die Auth-Maske erscheinen soll. */
export type AuthReason = "login" | "locked";

type AuthListener = (reason: AuthReason) => void;
const listeners = new Set<AuthListener>();

/** Meldet sich für „Anmeldung nötig“ an; liefert die Abmeldefunktion. */
export function onAuthRequired(listener: AuthListener): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

function notifyAuth(reason: AuthReason) {
  for (const l of listeners) l(reason);
}

/** Zeigt den Re-Auth-Dialog; `true`, wenn die Re-Auth gelungen ist. */
type ReauthHandler = () => Promise<boolean>;
let reauthHandler: ReauthHandler | null = null;

/** Registriert den Re-Auth-Dialog (einmal, im Layout). */
export function setReauthHandler(handler: ReauthHandler | null) {
  reauthHandler = handler;
}

/** 403 mit `reauth_required`? */
async function needsReauth(res: Response): Promise<boolean> {
  if (res.status !== 403) return false;
  try {
    return (await res.clone().json())?.code === "reauth_required";
  } catch {
    return false;
  }
}

/** Zufälliger Schlüssel (auch ohne `crypto.randomUUID`). */
export function newIdempotencyKey(): string {
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

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Wiederholungen bei Netzwerkfehlern (gleicher Schlüssel). */
const NETWORK_RETRIES = 3;
/** Abstand der Job-Abfragen. */
export const JOB_POLL_MS = 500;
/** So lange wird ein 503 `timeout`/`busy` mit demselben Schlüssel wiederholt. */
export const RETRY_503_MS = 10 * 60_000;

async function errorMessage(res: Response): Promise<string> {
  try {
    const body = await res.json();
    if (body && typeof body.message === "string") return body.message;
  } catch {
    // kein JSON
  }
  return `Fehler ${res.status}`;
}

function handleAuthStatus(res: Response) {
  if (res.status === 401) notifyAuth("login");
  else if (res.status === 423) notifyAuth("locked");
}

async function fetchWithRetry(input: string, init: RequestInit): Promise<Response> {
  let lastError: unknown;
  for (let attempt = 0; attempt <= NETWORK_RETRIES; attempt++) {
    try {
      return await fetch(input, init);
    } catch (e) {
      lastError = e;
      if (attempt < NETWORK_RETRIES) await sleep(300 * 2 ** attempt);
    }
  }
  throw `Server nicht erreichbar (${String(lastError)}).`;
}

/** Holt das Ergebnis eines Jobs ab (bis `done` oder `failed`). */
export async function pollJob<T>(jobId: string): Promise<T> {
  for (;;) {
    const res = await fetchWithRetry(`/api/jobs/${encodeURIComponent(jobId)}`, {
      method: "GET",
      credentials: "same-origin",
    });
    if (!res.ok) {
      handleAuthStatus(res);
      throw await errorMessage(res);
    }
    const job = await res.json();
    if (job.state === "done") return job.result as T;
    if (job.state === "failed") {
      if (job.status === 401) notifyAuth("login");
      if (job.status === 423) notifyAuth("locked");
      throw typeof job.error?.message === "string" ? job.error.message : "Aktion fehlgeschlagen.";
    }
    await sleep(JOB_POLL_MS);
  }
}

/** 503 mit `timeout` oder `busy`: dieselbe Anfrage gleich noch einmal. */
async function retryableAfter(res: Response): Promise<number | null> {
  if (res.status !== 503) return null;
  try {
    const body = await res.clone().json();
    if (body?.code !== "timeout" && body?.code !== "busy") return null;
  } catch {
    return null;
  }
  const seconds = Number(res.headers.get("Retry-After"));
  return Number.isFinite(seconds) && seconds > 0 ? Math.min(seconds, 10) : 2;
}

/**
 * Schickt eine Anfrage; wiederholt sie mit demselben Schlüssel bei 503
 * `timeout`/`busy` und nach einer gelungenen Re-Auth (403 `reauth_required`).
 */
async function sendWithRetries(
  url: string,
  init: RequestInit,
  opts: { retryTimeout?: boolean; networkRetries?: boolean } = {},
): Promise<Response> {
  const retryTimeout = opts.retryTimeout ?? true;
  const send = opts.networkRetries === false ? sendOnce : fetchWithRetry;
  const deadline = Date.now() + RETRY_503_MS;
  let reauthTried = false;
  let res = await send(url, init);
  for (;;) {
    const wait = retryTimeout ? await retryableAfter(res) : null;
    if (wait !== null && Date.now() < deadline) {
      await sleep(wait * 1000);
      res = await send(url, init);
      continue;
    }
    if (!reauthTried && reauthHandler && (await needsReauth(res))) {
      reauthTried = true;
      if (await reauthHandler()) {
        res = await send(url, init);
        continue;
      }
    }
    return res;
  }
}

/** Genau ein Versuch (Auth-POSTs: kein blindes Wiederholen, ein TOTP-Code
 *  gilt nur einmal). */
async function sendOnce(input: string, init: RequestInit): Promise<Response> {
  try {
    return await fetch(input, init);
  } catch (e) {
    throw `Server nicht erreichbar (${String(e)}).`;
  }
}

/** Ruft einen Command-Kern auf (Ersatz für Tauris `invoke`). */
export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const init: RequestInit = {
    method: "POST",
    credentials: "same-origin",
    headers: {
      "Content-Type": "application/json",
      [REQUEST_HEADER]: "1",
      [IDEMPOTENCY_HEADER]: newIdempotencyKey(),
    },
    body: JSON.stringify(args ?? {}),
  };
  const url = `/api/cmd/${encodeURIComponent(cmd)}`;
  const res = await sendWithRetries(url, init);
  if (res.status === 202) {
    const { jobId } = await res.json();
    return pollJob<T>(jobId);
  }
  if (!res.ok) {
    handleAuthStatus(res);
    throw await errorMessage(res);
  }
  const text = await res.text();
  return (text === "" ? undefined : JSON.parse(text)) as T;
}

// ---- Anmeldung (Plan W6: Setup-Token, TOTP + Passphrase, Sitzungen) --------

export type AuthState = "setup_required" | "totp_setup_required" | "login_required" | "ok";

export interface AuthStatus {
  state: AuthState;
  /** Server läuft mit KLEIN_BUCH_INSECURE_HTTP=1 (nur Tests). */
  insecureHttp: boolean;
}

export async function authStatus(): Promise<AuthStatus> {
  const res = await fetchWithRetry("/api/auth/status", {
    method: "GET",
    credentials: "same-origin",
  });
  if (!res.ok) throw await errorMessage(res);
  const body = await res.json();
  return { state: body.state as AuthState, insecureHttp: body.insecureHttp === true };
}

function authInit(body: unknown): RequestInit {
  return {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", [REQUEST_HEADER]: "1" },
    body: JSON.stringify(body),
  };
}

async function postAuth(path: string, body: unknown): Promise<Response> {
  return sendOnce(path, authInit(body));
}

/** Wie `postAuth`, mit Re-Auth-Dialog bei 403 `reauth_required`; keine
 *  Wiederholung nach 503 `timeout` (der Server führt die Aktion zu Ende). */
async function postAuthReauth(path: string, body: unknown): Promise<Response> {
  return sendWithRetries(path, authInit(body), { retryTimeout: false, networkRetries: false });
}

async function jsonOrThrow<T>(res: Response): Promise<T> {
  if (!res.ok) {
    handleAuthStatus(res);
    throw await errorMessage(res);
  }
  return (await res.json()) as T;
}

/** Neues TOTP-Geheimnis mit QR-Code (Einrichtung bzw. Erneuerung). */
export interface TotpOffer {
  secret: string;
  otpauthUri: string;
  qrSvg: string;
}

/** Einrichtung Schritt 1: Setup-Token prüfen, TOTP-Geheimnis anfordern. */
export async function authSetupOffer(setupToken: string): Promise<TotpOffer> {
  return jsonOrThrow(await postAuth("/api/auth/setup", { setupToken, step: "totp" }));
}

/** Einrichtung Schritt 2: atomar mit Passphrase und TOTP-Bestätigung. */
export async function authSetupFinish(
  setupToken: string,
  passphrase: string,
  totpCode: string,
): Promise<{ recoveryCodes: string[] }> {
  return jsonOrThrow(await postAuth("/api/auth/setup", { setupToken, passphrase, totpCode }));
}

/** Zweiter Faktor: TOTP-Code oder Einmal-Code. */
export interface SecondFactor {
  totpCode?: string;
  recoveryCode?: string;
}

export interface LoginResult {
  ok: boolean;
  failedAttempts?: number;
  recoveryLeft?: number;
  usedRecoveryCode?: boolean;
}

/** Anmelden; `ok:false` bei falschen Angaben (immer dieselbe Meldung). */
export async function authLogin(passphrase: string, factor: SecondFactor): Promise<LoginResult> {
  const res = await postAuth("/api/auth/login", { passphrase, ...factor });
  if (res.status === 401) return { ok: false };
  const body = await jsonOrThrow<Omit<LoginResult, "ok">>(res);
  return { ok: true, ...body };
}

/** Re-Auth der Sitzung (5 min); `false` bei falschen Angaben. Eine
 *  abgelaufene Sitzung (401 `unauthenticated`) führt zur Anmelde-Maske. */
export async function authReauth(passphrase: string, factor: SecondFactor): Promise<boolean> {
  const res = await postAuth("/api/auth/reauth", { passphrase, ...factor });
  if (res.status === 401) {
    const body = await res.clone().json().catch(() => null);
    if (body?.code === "unauthenticated") {
      notifyAuth("login");
      throw typeof body.message === "string" ? body.message : "Nicht angemeldet.";
    }
    return false;
  }
  await jsonOrThrow(res);
  return true;
}

export async function authLogout(): Promise<void> {
  await jsonOrThrow(await postAuth("/api/auth/logout", {}));
}

/** „Datenbank sperren“: Pool zu, alle Sitzungen beendet. */
export async function authLock(): Promise<void> {
  await jsonOrThrow(await postAuth("/api/auth/lock", {}));
}

export async function authSecurity(): Promise<{ recoveryLeft: number; insecureHttp: boolean }> {
  const res = await fetchWithRetry("/api/auth/security", {
    method: "GET",
    credentials: "same-origin",
  });
  return jsonOrThrow(res);
}

export async function authTotpRenewOffer(): Promise<TotpOffer> {
  return jsonOrThrow(await postAuthReauth("/api/auth/totp/renew", { step: "totp" }));
}

export async function authTotpRenewFinish(totpCode: string): Promise<string[]> {
  const body = await jsonOrThrow<{ recoveryCodes: string[] }>(
    await postAuthReauth("/api/auth/totp/renew", { totpCode }),
  );
  return body.recoveryCodes;
}

export async function authRecoveryRenew(): Promise<string[]> {
  const body = await jsonOrThrow<{ recoveryCodes: string[] }>(
    await postAuthReauth("/api/auth/recovery/renew", {}),
  );
  return body.recoveryCodes;
}

export async function authChangePassphrase(
  newPassphrase: string,
): Promise<{ backupStarted: boolean }> {
  return jsonOrThrow(await postAuthReauth("/api/auth/passphrase", { newPassphrase }));
}

// ---- Neustart nach Restore ---------------------------------------------------

/**
 * Wartet, bis der Server nach dem Restore neu gestartet ist (`/healthz` erst
 * nicht erreichbar bzw. 503, dann 200), und lädt die Seite neu. Liefert
 * `false` nach `timeoutMs` ohne Neustart.
 */
export async function waitForRestart(timeoutMs = 180_000, intervalMs = 1000): Promise<boolean> {
  const started = Date.now();
  let wentDown = false;
  while (Date.now() - started < timeoutMs) {
    try {
      const res = await fetch("/healthz", { cache: "no-store" });
      if (res.ok && wentDown) {
        location.reload();
        return true;
      }
      if (!res.ok) wentDown = true;
    } catch {
      wentDown = true;
    }
    await sleep(intervalMs);
  }
  return false;
}
