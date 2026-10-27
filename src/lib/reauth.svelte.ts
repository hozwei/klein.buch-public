// Re-Auth-Dialog (Plan W6): kritische Aktionen verlangen eine frische
// Bestätigung mit Daten-Passwort und App-Code (5 min). Der Transport meldet 403
// `reauth_required` über `requestReauth()`; der Dialog (<ReauthDialog/> im
// Root-Layout) löst das Promise mit `true` (bestätigt) bzw. `false`
// (abgebrochen) auf, danach wiederholt der Transport die Aktion.
//
// U4 (V1-09, ADR 0041 Nr. 163): Der Dialog nennt die Aktion („Aktion bestätigen: EÜR als
// CSV herunterladen“). Den Namen leitet der Transport aus dem Command bzw. dem
// Auth-Pfad ab (`reauthActionLabel` in labels.ts).

export const reauthStore = $state<{ visible: boolean; action: string | null }>({
  visible: false,
  action: null,
});

let pending: Array<(ok: boolean) => void> = [];

/** Öffnet den Dialog; `action` ist der verständliche Name der Aktion (oder `null`). */
export function requestReauth(action: string | null = null): Promise<boolean> {
  reauthStore.action = action;
  reauthStore.visible = true;
  return new Promise((resolve) => {
    pending.push(resolve);
  });
}

export function resolveReauth(ok: boolean): void {
  reauthStore.visible = false;
  const waiting = pending;
  pending = [];
  for (const resolve of waiting) resolve(ok);
}
