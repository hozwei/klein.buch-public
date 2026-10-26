import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { focusTrap, nextTrapTarget, TABBABLE_SELECTOR } from "./focusTrap";

// Kein jsdom im Projekt: ein schmales Schein-DOM reicht für die Logik der
// Falle (Fokus, Tasten, Stapel, Scroll-Sperre, Rücksprung).

const TABBABLE_TAGS = new Set(["button", "a", "input", "select", "textarea"]);

class FakeEl {
  tag: string;
  id = "";
  parent: FakeEl | null = null;
  kids: FakeEl[] = [];
  attrs = new Map<string, string>();
  tabIndex = 0;
  isConnected = true;
  constructor(tag: string, id = "") {
    this.tag = tag;
    this.id = id;
    if (!TABBABLE_TAGS.has(tag)) this.tabIndex = -1;
  }
  add(...k: FakeEl[]): this {
    for (const c of k) {
      c.parent = this;
      this.kids.push(c);
    }
    return this;
  }
  all(): FakeEl[] {
    return this.kids.flatMap((k) => [k, ...k.all()]);
  }
  contains(o: unknown): boolean {
    for (let n = o as FakeEl | null; n; n = n.parent) if (n === this) return true;
    return false;
  }
  focus(): void {
    doc.activeElement = this;
  }
  hasAttribute(n: string): boolean {
    return this.attrs.has(n);
  }
  setAttribute(n: string, v: string): void {
    this.attrs.set(n, v);
  }
  // für die Ausgabe von vitest bei Fehlschlägen
  getAttributeNames(): string[] {
    return [...this.attrs.keys()];
  }
  closest(sel: string): FakeEl | null {
    for (let n: FakeEl | null = this; n; n = n.parent) {
      if (sel.includes("[inert]") && n.attrs.has("inert")) return n;
      if (sel.includes("[hidden]") && n.attrs.has("hidden")) return n;
    }
    return null;
  }
  querySelectorAll(sel: string): FakeEl[] {
    expect(sel).toBe(TABBABLE_SELECTOR);
    return this.all().filter((e) => TABBABLE_TAGS.has(e.tag) && !e.attrs.has("disabled"));
  }
  querySelector(sel: string): FakeEl | null {
    if (sel === "[data-initial-focus]") return this.all().find((e) => e.attrs.has("data-initial-focus")) ?? null;
    if (sel.startsWith("#")) return this.all().find((e) => e.id === sel.slice(1)) ?? null;
    return null;
  }
}

type Listener = (e: unknown) => void;
const doc = {
  activeElement: null as FakeEl | null,
  body: new FakeEl("body"),
  documentElement: { style: { overflow: "" } },
  listeners: new Map<string, Set<Listener>>(),
  addEventListener(t: string, fn: Listener) {
    if (!this.listeners.has(t)) this.listeners.set(t, new Set());
    this.listeners.get(t)!.add(fn);
  },
  removeEventListener(t: string, fn: Listener) {
    this.listeners.get(t)?.delete(fn);
  },
  key(key: string, shiftKey = false) {
    const ev = { key, shiftKey, preventDefault: vi.fn(), stopPropagation: vi.fn() };
    for (const fn of this.listeners.get("keydown") ?? []) fn(ev);
    return ev;
  },
};

function dialog() {
  const d = new FakeEl("div", "dlg");
  const cancel = new FakeEl("button", "cancel");
  const input = new FakeEl("input", "field");
  const ok = new FakeEl("button", "ok");
  d.add(cancel, input, ok);
  doc.body.add(d);
  return { d, cancel, input, ok };
}

const flush = () => new Promise<void>((r) => queueMicrotask(r));
type Trap = ReturnType<typeof focusTrap>;
const mount = (n: FakeEl, o?: Parameters<typeof focusTrap>[1]): Trap =>
  focusTrap(n as unknown as HTMLElement, o);

describe("nextTrapTarget", () => {
  const items = ["a", "b", "c"];
  it("Tab auf dem letzten Element springt zum ersten", () => {
    expect(nextTrapTarget(items, "c", false, true)).toBe("a");
  });
  it("Umschalt+Tab auf dem ersten springt zum letzten", () => {
    expect(nextTrapTarget(items, "a", true, true)).toBe("c");
  });
  it("in der Mitte bleibt der Browser zuständig", () => {
    expect(nextTrapTarget(items, "b", false, true)).toBeNull();
    expect(nextTrapTarget(items, "b", true, true)).toBeNull();
  });
  it("Fokus außerhalb holt an den Anfang bzw. das Ende zurück", () => {
    expect(nextTrapTarget(items, "x", false, false)).toBe("a");
    expect(nextTrapTarget(items, "x", true, false)).toBe("c");
    expect(nextTrapTarget(items, null, false, true)).toBe("a");
  });
  it("ohne fokussierbare Elemente kein Ziel", () => {
    expect(nextTrapTarget([], null, false, false)).toBeNull();
  });
  it("ein einziges Element bleibt fokussiert", () => {
    expect(nextTrapTarget(["a"], "a", false, true)).toBe("a");
    expect(nextTrapTarget(["a"], "a", true, true)).toBe("a");
  });
});

describe("focusTrap", () => {
  let trigger: FakeEl;
  beforeEach(() => {
    vi.useFakeTimers();
    doc.body = new FakeEl("body");
    doc.listeners.clear();
    doc.documentElement.style.overflow = "";
    trigger = new FakeEl("button", "trigger");
    doc.body.add(trigger);
    trigger.focus();
    vi.stubGlobal("document", doc);
    vi.stubGlobal("HTMLElement", FakeEl);
    vi.stubGlobal("Node", FakeEl);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("fokussiert beim Öffnen das erste Element und sperrt das Scrollen", async () => {
    const { d, cancel } = dialog();
    const t = mount(d);
    await flush();
    expect(doc.activeElement).toBe(cancel);
    expect(doc.documentElement.style.overflow).toBe("hidden");
    t.destroy();
    expect(doc.documentElement.style.overflow).toBe("");
  });

  it("data-initial-focus hat Vorrang vor dem Selektor", async () => {
    const { d, ok, input } = dialog();
    ok.setAttribute("data-initial-focus", "");
    const t = mount(d, { initialFocus: "#field" });
    await flush();
    expect(doc.activeElement).toBe(ok);
    t.destroy();
    trigger.focus();
    ok.attrs.delete("data-initial-focus");
    const t2 = mount(d, { initialFocus: "#field" });
    await flush();
    expect(doc.activeElement).toBe(input);
    t2.destroy();
  });

  it("Tab und Umschalt+Tab bleiben im Dialog", async () => {
    const { d, cancel, ok } = dialog();
    const t = mount(d);
    await flush();
    ok.focus();
    const ev = doc.key("Tab");
    expect(ev.preventDefault).toHaveBeenCalled();
    expect(doc.activeElement).toBe(cancel);
    const back = doc.key("Tab", true);
    expect(back.preventDefault).toHaveBeenCalled();
    expect(doc.activeElement).toBe(ok);
    t.destroy();
  });

  it("deaktivierte Elemente zählen nicht", async () => {
    const { d, cancel, input, ok } = dialog();
    ok.setAttribute("disabled", "");
    const t = mount(d);
    await flush();
    input.focus();
    doc.key("Tab");
    expect(doc.activeElement).toBe(cancel);
    t.destroy();
  });

  it("Escape ruft onEscape nur in der obersten Falle", async () => {
    const a = dialog();
    const b = dialog();
    const escA = vi.fn();
    const escB = vi.fn();
    const ta = mount(a.d, { onEscape: escA });
    await flush();
    const tb = mount(b.d, { onEscape: escB });
    await flush();
    doc.key("Escape");
    expect(escB).toHaveBeenCalledTimes(1);
    expect(escA).not.toHaveBeenCalled();
    tb.destroy();
    doc.key("Escape");
    expect(escA).toHaveBeenCalledTimes(1);
    ta.destroy();
  });

  it("ohne onEscape bleibt Escape wirkungslos", async () => {
    const { d } = dialog();
    const t = mount(d);
    await flush();
    const ev = doc.key("Escape");
    expect(ev.preventDefault).not.toHaveBeenCalled();
    t.destroy();
  });

  it("gibt den Fokus beim Schließen an den Auslöser zurück", async () => {
    const { d } = dialog();
    const t = mount(d);
    await flush();
    t.destroy();
    doc.activeElement = doc.body; // Dialog ist aus dem DOM verschwunden
    vi.runAllTimers();
    expect(doc.activeElement).toBe(trigger);
  });

  it("stiehlt den Fokus nicht, wenn inzwischen ein anderes Element ihn hat", async () => {
    const { d } = dialog();
    const other = new FakeEl("input", "other");
    doc.body.add(other);
    const t = mount(d);
    await flush();
    t.destroy();
    other.focus();
    vi.runAllTimers();
    expect(doc.activeElement).toBe(other);
  });

  it("returnFocus: false lässt den Fokus stehen", async () => {
    const { d } = dialog();
    const t = mount(d, { returnFocus: false, lockScroll: false });
    await flush();
    expect(doc.documentElement.style.overflow).toBe("");
    t.destroy();
    doc.activeElement = doc.body;
    vi.runAllTimers();
    expect(doc.activeElement).toBe(doc.body);
  });

  it("ohne fokussierbaren Inhalt bekommt der Dialog selbst den Fokus", async () => {
    const d = new FakeEl("div", "empty");
    doc.body.add(d);
    const t = mount(d);
    await flush();
    expect(doc.activeElement).toBe(d);
    expect(d.attrs.get("tabindex")).toBe("-1");
    t.destroy();
  });
});
