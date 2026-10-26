// Svelte-Action: hängt ein Element an `document.body` (Lauf 2026.11, U1).
//
// Seiteneigene Dialoge liegen sonst in `<main>`, das das Layout bei offenem
// Modal `inert` setzt (`modalStack`) — der Dialog wäre dann selbst nicht
// bedienbar. Verwendung zusammen mit focusTrap:
//
//   <div class="kb-backdrop" use:portal> <div class="kb-dialog" use:focusTrap={…}> … </div></div>
//
// Beim Zerstören wird das Element wieder entfernt. Styles aus dem
// Svelte-Komponentenbereich bleiben erhalten (die Klassen-Hashes reisen mit).

export function portal(node: HTMLElement, target: HTMLElement | null = null) {
  const host = target ?? document.body;
  host.appendChild(node);
  return {
    destroy() {
      node.remove();
    },
  };
}
