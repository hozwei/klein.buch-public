import { describe, expect, it } from "vitest";
import { DEV_SOURCE_TEXT, safeSourceLink, shortCommit, sourceLabel } from "./sourceInfo";
import type { PublicInfo } from "./transport";

const SHA = "0123456789abcdef0123456789abcdef01234567";

function info(over: Partial<PublicInfo> = {}): PublicInfo {
  return {
    version: "2026.10.0",
    commit: SHA,
    tag: null,
    sourceUrl: "https://example.org/klein-buch",
    sourceLink: `https://example.org/klein-buch/tree/${SHA}`,
    licenseSpdx: "AGPL-3.0-or-later",
    ...over,
  };
}

describe("sourceInfo", () => {
  it("zeigt Commit-Kurzform und Tag", () => {
    expect(shortCommit(SHA)).toBe("0123456");
    expect(sourceLabel(info())).toBe("Commit 0123456");
    expect(sourceLabel(info({ tag: "v2026.10.0" }))).toBe("v2026.10.0 (Commit 0123456)");
  });

  it("ohne Commit oder Link: Entwicklungs-Build, kein Link", () => {
    const dev = info({ commit: null, sourceLink: null });
    expect(safeSourceLink(dev)).toBeNull();
    expect(sourceLabel(dev)).toBe(DEV_SOURCE_TEXT);
    expect(sourceLabel(null)).toBe(DEV_SOURCE_TEXT);
  });

  it("verlinkt nur http(s)", () => {
    const bad = info({ sourceLink: "javascript:alert(1)" });
    expect(safeSourceLink(bad)).toBeNull();
    expect(sourceLabel(bad)).toBe(DEV_SOURCE_TEXT);
  });
});
