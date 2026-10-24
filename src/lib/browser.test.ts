import { afterEach, describe, expect, it, vi } from "vitest";
import { randomId } from "./browser";

describe("randomId", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("nimmt randomUUID, wenn vorhanden", () => {
    expect(randomId()).toMatch(/^[0-9a-f-]{36}$/);
  });

  it("kommt ohne Secure Context (ohne randomUUID) aus", () => {
    const getRandomValues = (a: Uint8Array) => {
      a.fill(171);
      return a;
    };
    vi.stubGlobal("crypto", { getRandomValues });
    expect(randomId()).toBe("ab".repeat(16));
  });
});
