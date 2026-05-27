// R5-014: zentraler Modal-Open-Counter. Jedes Modal pusht beim Mount/Open
// und poppt beim Unmount/Close; das Root-Layout liest `modalStack.count` und
// setzt `inert` auf Sidebar + Main, sobald irgendein Modal offen ist. Damit
// kann Tab nicht aus dem Modal in die App-Shell wandern (echter Focus-Trap;
// Memory: `role="dialog"` + `aria-modal="true"` allein reichen nicht).
//
// Counter (statt Boolean), weil Modals sich theoretisch stapeln können
// (Confirm über AboutDialog o. ä.). pop ist clamped, damit doppelte
// $effect-Cleanups nicht ins Negative laufen.
//
// R6-Hotfix (Re-Apply 2026-05-27): push/pop laufen in `untrack`, sonst
// trackt der aufrufende `$effect` `modalStack.count` als Dependency (das
// `+= 1` ist syntaktisch ein READ+WRITE — Svelte 5 protokolliert den Read).
// Folge wäre ein effect_update_depth_exceeded, der den BackupGate-Login
// blockiert (Button + Enter ohne Wirkung). Memory `project_block_r6_review`.

import { untrack } from "svelte";

export const modalStack = $state<{ count: number }>({ count: 0 });

export function pushModal(): void {
  untrack(() => {
    modalStack.count += 1;
  });
}

export function popModal(): void {
  untrack(() => {
    modalStack.count = Math.max(0, modalStack.count - 1);
  });
}
