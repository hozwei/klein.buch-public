// Re-Auth-Dialog (Plan W6): kritische Aktionen verlangen eine frische
// Bestätigung mit Passphrase und TOTP (5 min). Der Transport meldet 403
// `reauth_required` über `requestReauth()`; der Dialog (<ReauthDialog/> im
// Root-Layout) löst das Promise mit `true` (bestätigt) bzw. `false`
// (abgebrochen) auf, danach wiederholt der Transport die Aktion.

export const reauthStore = $state<{ visible: boolean }>({ visible: false });

let pending: Array<(ok: boolean) => void> = [];

export function requestReauth(): Promise<boolean> {
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
