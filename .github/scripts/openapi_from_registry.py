#!/usr/bin/env python3
"""OpenAPI-Beschreibung der Command-API für den ZAP-API-Scan (Plan W14).

Liest `src-tauri/src/commands/registry.rs` und beschreibt jeden Eintrag der
Registry (`ENTRIES`, Paare `("name", h::name)`) als `POST /api/cmd/{name}` mit
einem freien JSON-Körper. Die Registry ist seit Lauf F6 genau die Allowlist des
Dispatchers (keine gesperrten Einträge mehr, ADR 0039 Nr. 138); jeder andere
Name ergibt 404. Den Abgleich der Anzahl mit `REGISTRY.len()` prüft
`core_cut_test::registry_source_matches_openapi_pattern`. Nur Standardbibliothek.

Aufruf aus der Wurzel des öffentlichen Repos:
    python3 .github/scripts/openapi_from_registry.py http://localhost:8080 > openapi.json
"""

import json
import re
import sys
from pathlib import Path


def find_registry() -> Path:
    here = Path(__file__).resolve().parent
    for root in (here.parent.parent, here.parent.parent / "klein-buch"):
        p = root / "src-tauri" / "src" / "commands" / "registry.rs"
        if p.exists():
            return p
    sys.exit("registry.rs nicht gefunden")


def main() -> None:
    server = sys.argv[1] if len(sys.argv) > 1 else "http://localhost:8080"
    src = find_registry().read_text(encoding="utf-8")
    names = re.findall(r'\(\s*"([a-z0-9_]+)",\s*h::[a-z0-9_]+,?\s*\)', src)
    cmds = sorted(set(names))
    if len(cmds) < 100:
        sys.exit(f"nur {len(cmds)} Commands gefunden — Registry-Format geändert?")
    paths = {
        f"/api/cmd/{name}": {
            "post": {
                "operationId": name,
                "requestBody": {
                    "required": True,
                    "content": {"application/json": {"schema": {
                        "type": "object", "additionalProperties": True,
                        "example": {}}}},
                },
                "responses": {"200": {"description": "OK"}, "202": {"description": "Job"}},
            }
        }
        for name in cmds
    }
    doc = {
        "openapi": "3.0.3",
        "info": {"title": "Klein.Buch Command-API (Scan)", "version": "scan"},
        "servers": [{"url": server}],
        "paths": paths,
    }
    json.dump(doc, sys.stdout, indent=1)
    print(f"{len(cmds)} Commands beschrieben", file=sys.stderr)


if __name__ == "__main__":
    main()
