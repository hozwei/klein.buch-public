import { afterEach, describe, expect, it, vi } from "vitest";
import {
  IDEMPOTENCY_HEADER,
  REQUEST_HEADER,
  authLogin,
  invoke,
  newIdempotencyKey,
  onAuthRequired,
  setReauthHandler,
} from "./transport";

type Call = { url: string; init: RequestInit };

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json" },
  });
}

function mockFetch(responses: Array<Response | Error>): Call[] {
  const calls: Call[] = [];
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string, init: RequestInit) => {
      calls.push({ url, init });
      const next = responses.shift();
      if (!next) throw new Error("keine Antwort mehr");
      if (next instanceof Error) throw next;
      return next;
    }),
  );
  return calls;
}

function header(call: Call, name: string): string | undefined {
  return (call.init.headers as Record<string, string> | undefined)?.[name];
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("transport.invoke", () => {
  it("schickt POST mit Pflicht-Header, Schlüssel und JSON-Argumenten", async () => {
    const calls = mockFetch([json(200, [{ id: "1" }])]);
    const out = await invoke<Array<{ id: string }>>("contacts_list", { includeArchived: true });
    expect(out).toEqual([{ id: "1" }]);
    expect(calls[0].url).toBe("/api/cmd/contacts_list");
    expect(calls[0].init.method).toBe("POST");
    expect(header(calls[0], REQUEST_HEADER)).toBe("1");
    expect(header(calls[0], IDEMPOTENCY_HEADER)).toBeTruthy();
    expect(JSON.parse(String(calls[0].init.body))).toEqual({ includeArchived: true });
  });

  it("liefert Fehler als String der Server-Meldung", async () => {
    mockFetch([json(422, { code: "domain", message: "Rechnung ist bereits storniert" })]);
    await expect(invoke("invoices_cancel", {})).rejects.toBe("Rechnung ist bereits storniert");
  });

  it("meldet 401 und 423 an die Auth-Maske", async () => {
    const reasons: string[] = [];
    const off = onAuthRequired((r) => reasons.push(r));
    mockFetch([json(401, { message: "Nicht angemeldet" }), json(423, { message: "gesperrt" })]);
    await expect(invoke("contacts_list")).rejects.toBe("Nicht angemeldet");
    await expect(invoke("contacts_list")).rejects.toBe("gesperrt");
    off();
    expect(reasons).toEqual(["login", "locked"]);
  });

  it("holt bei 202 das Job-Ergebnis ab", async () => {
    const calls = mockFetch([
      json(202, { jobId: "j1" }),
      json(200, { state: "running", id: "j1" }),
      json(200, { state: "done", id: "j1", result: { invoiceNumber: "RE-2026-0001" } }),
    ]);
    const out = await invoke<{ invoiceNumber: string }>("invoices_lock_and_issue", { id: "x" });
    expect(out.invoiceNumber).toBe("RE-2026-0001");
    expect(calls[1].url).toBe("/api/jobs/j1");
  });

  it("wirft die Meldung eines fehlgeschlagenen Jobs", async () => {
    mockFetch([
      json(202, { jobId: "j2" }),
      json(200, { state: "failed", status: 422, error: { message: "Telefon fehlt" } }),
    ]);
    await expect(invoke("invoices_lock_and_issue", {})).rejects.toBe("Telefon fehlt");
  });

  it("wiederholt 503 timeout mit demselben Schlüssel", async () => {
    const timeout = new Response(JSON.stringify({ code: "timeout", message: "Zeitüberschreitung" }), {
      status: 503,
      headers: { "Content-Type": "application/json", "Retry-After": "0.01" },
    });
    const calls = mockFetch([timeout, json(200, { id: "c1" })]);
    const out = await invoke<{ id: string }>("contacts_create", { input: {} });
    expect(out.id).toBe("c1");
    expect(calls).toHaveLength(2);
    expect(header(calls[0], IDEMPOTENCY_HEADER)).toBe(header(calls[1], IDEMPOTENCY_HEADER));
  });

  it("wiederholt 503 restarting nicht", async () => {
    mockFetch([json(503, { code: "restarting", message: "Der Server startet neu" })]);
    await expect(invoke("contacts_create", {})).rejects.toBe("Der Server startet neu");
  });

  it("wiederholt Netzwerkfehler mit demselben Idempotenz-Schlüssel", async () => {
    const calls = mockFetch([new TypeError("Failed to fetch"), json(200, { ok: true })]);
    await invoke("contacts_create", { input: {} });
    expect(calls).toHaveLength(2);
    expect(header(calls[0], IDEMPOTENCY_HEADER)).toBe(header(calls[1], IDEMPOTENCY_HEADER));
  });
});

describe("transport.auth", () => {
  it("authLogin: 401 → ok:false, 200 → ok:true mit Fehlversuchen", async () => {
    const calls = mockFetch([
      json(401, { message: "Anmeldung fehlgeschlagen." }),
      json(200, { state: "ok", failedAttempts: 2, recoveryLeft: 10, usedRecoveryCode: false }),
    ]);
    expect((await authLogin("falsch", { totpCode: "123456" })).ok).toBe(false);
    const r = await authLogin("richtig", { totpCode: "654321" });
    expect(r.ok).toBe(true);
    expect(r.failedAttempts).toBe(2);
    expect(JSON.parse(String(calls[1].init.body))).toEqual({
      passphrase: "richtig",
      totpCode: "654321",
    });
  });

  it("403 reauth_required: Dialog, dann Wiederholung mit demselben Schlüssel", async () => {
    let asked = 0;
    setReauthHandler(async () => {
      asked++;
      return true;
    });
    const calls = mockFetch([
      json(403, { code: "reauth_required", message: "Bitte bestätigen" }),
      json(200, { ok: true }),
    ]);
    await invoke("dsgvo_export", { contactId: "c1" });
    expect(asked).toBe(1);
    expect(calls).toHaveLength(2);
    expect(header(calls[0], IDEMPOTENCY_HEADER)).toBe(header(calls[1], IDEMPOTENCY_HEADER));

    setReauthHandler(async () => false);
    mockFetch([json(403, { code: "reauth_required", message: "Bitte bestätigen" })]);
    await expect(invoke("dsgvo_export", {})).rejects.toBe("Bitte bestätigen");
    setReauthHandler(null);
  });

  it("erzeugt unterschiedliche Schlüssel", () => {
    expect(newIdempotencyKey()).not.toBe(newIdempotencyKey());
  });
});
