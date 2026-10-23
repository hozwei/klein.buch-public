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
// noch laufenden ersten Ausführung.

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
  const deadline = Date.now() + RETRY_503_MS;
  let res = await fetchWithRetry(url, init);
  for (;;) {
    const wait = await retryableAfter(res);
    if (wait === null || Date.now() >= deadline) break;
    await sleep(wait * 1000);
    res = await fetchWithRetry(url, init);
  }
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

// ---- Anmeldung (Plan W5.4; W6 ergänzt TOTP, Setup-Token, Abmelden) ---------

export type AuthState = "setup_required" | "login_required" | "ok";

export async function authStatus(): Promise<AuthState> {
  const res = await fetchWithRetry("/api/auth/status", {
    method: "GET",
    credentials: "same-origin",
  });
  if (!res.ok) throw await errorMessage(res);
  return (await res.json()).state as AuthState;
}

async function postAuth(path: string, body: unknown): Promise<Response> {
  return fetchWithRetry(path, {
    method: "POST",
    credentials: "same-origin",
    headers: { "Content-Type": "application/json", [REQUEST_HEADER]: "1" },
    body: JSON.stringify(body),
  });
}

/** Ersteinrichtung mit Passphrase; legt die Sitzung an. */
export async function authSetup(passphrase: string): Promise<void> {
  const res = await postAuth("/api/auth/setup", { passphrase });
  if (!res.ok) throw await errorMessage(res);
}

/** Anmelden/Entsperren; `false` bei falscher Passphrase. */
export async function authLogin(passphrase: string): Promise<boolean> {
  const res = await postAuth("/api/auth/login", { passphrase });
  if (res.status === 401) return false;
  if (!res.ok) throw await errorMessage(res);
  return true;
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
