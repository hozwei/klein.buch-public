import { afterEach, describe, expect, it, vi } from "vitest";
import {
  IDEMPOTENCY_HEADER,
  REQUEST_HEADER,
  authLogin,
  authRevokeOtherSessions,
  authSessions,
  invoke,
  newIdempotencyKey,
  onAuthRequired,
  setReauthHandler,
  uploadFile,
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

  // W14-037 (Lauf F4a): lange Anmeldung → 202, Abholung ohne Passphrase.
  it("authLogin: 202 → Ergebnis über /api/auth/login/poll", async () => {
    vi.useFakeTimers();
    try {
      const calls = mockFetch([
        json(202, { state: "pending" }),
        json(202, { state: "pending" }),
        json(200, { state: "ok", failedAttempts: 0, recoveryLeft: 10, usedRecoveryCode: false }),
        json(202, { state: "pending" }),
        json(401, { code: "auth_failed", message: "Anmeldung fehlgeschlagen." }),
      ]);
      const ok = authLogin("richtig", { totpCode: "654321" });
      await vi.advanceTimersByTimeAsync(2500);
      await expect(ok).resolves.toMatchObject({ ok: true, recoveryLeft: 10 });
      expect(calls.map((c) => c.url)).toEqual([
        "/api/auth/login",
        "/api/auth/login/poll",
        "/api/auth/login/poll",
      ]);
      expect(JSON.parse(String(calls[1].init.body))).toEqual({});
      expect(header(calls[1], REQUEST_HEADER)).toBe("1");
      const failed = authLogin("falsch", { totpCode: "123456" });
      await vi.advanceTimersByTimeAsync(1500);
      await expect(failed).resolves.toEqual({ ok: false });
    } finally {
      vi.useRealTimers();
    }
  });

  // W14-005 (Lauf F4a): Sitzungsübersicht und „andere abmelden“ (Re-Auth).
  it("authSessions und authRevokeOtherSessions", async () => {
    const session = {
      id: "a1b2c3d4",
      loginAt: "2026-10-25 08:00:00",
      lastSeenAt: "2026-10-25 08:05:00",
      current: true,
      reauthActive: false,
    };
    let asked = 0;
    setReauthHandler(async () => {
      asked++;
      return true;
    });
    const calls = mockFetch([
      json(200, { count: 1, sessions: [session] }),
      json(403, { code: "reauth_required", message: "Bitte bestätigen" }),
      json(200, { state: "ok", revoked: 2 }),
    ]);
    expect(await authSessions()).toEqual([session]);
    expect(calls[0].url).toBe("/api/auth/sessions");
    expect(await authRevokeOtherSessions()).toBe(2);
    expect(asked).toBe(1);
    expect(calls[2].url).toBe("/api/auth/sessions/others/revoke");
    setReauthHandler(null);
  });
});

describe("transport.uploadFile", () => {
  it("schickt multipart mit Pflicht-Header, ohne Idempotenz-Schlüssel", async () => {
    const ticket = { uploadId: "u1", fileName: "beleg.pdf", mimeType: "application/pdf", sizeBytes: 8 };
    const calls = mockFetch([json(200, ticket)]);
    const file = new File(["%PDF-1.7"], "beleg.pdf", { type: "application/pdf" });
    await expect(uploadFile("beleg", file)).resolves.toEqual(ticket);
    expect(calls[0].url).toBe("/api/upload/beleg");
    expect(calls[0].init.method).toBe("POST");
    expect(header(calls[0], REQUEST_HEADER)).toBe("1");
    expect(header(calls[0], IDEMPOTENCY_HEADER)).toBeUndefined();
    const body = calls[0].init.body as FormData;
    expect((body.get("file") as File).name).toBe("beleg.pdf");
  });

  it("liefert 413/415 als Meldung und meldet 401", async () => {
    mockFetch([json(415, { code: "unsupported_media_type", message: "Dateityp nicht erlaubt." })]);
    const file = new File(["<html>"], "x.pdf");
    await expect(uploadFile("beleg", file)).rejects.toBe("Dateityp nicht erlaubt.");
    const seen: string[] = [];
    const off = onAuthRequired((r) => seen.push(r));
    mockFetch([json(401, { code: "unauthenticated", message: "Bitte anmelden" })]);
    await expect(uploadFile("anhang", file)).rejects.toBe("Bitte anmelden");
    expect(seen).toEqual(["login"]);
    off();
  });
});
