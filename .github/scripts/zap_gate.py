#!/usr/bin/env python3
"""Gate für die ZAP-Berichte (Plan W14).

Fehlschlag, wenn ein Alarm mit Risiko Mittel oder Hoch (riskcode ≥ 2) auftritt,
der nicht mit Begründung in der Ausnahmeliste steht. Niedrige und informative
Alarme werden nur aufgelistet. Nur Standardbibliothek.

Aufruf: zap_gate.py <ausnahmen.tsv> <bericht.json> [<bericht.json> …]
Ausnahmen: je Zeile `<alertRef oder pluginId>\t<Begründung>`; `#` beginnt einen
Kommentar. Eine alertRef (z. B. `10055-6`) nimmt nur diesen Unterfall aus, eine
reine Plugin-ID alle Alarme der Regel.
"""

import json
import sys
from pathlib import Path


def load_exceptions(path: Path) -> dict[str, str]:
    out = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        key, _, reason = line.partition("\t")
        if not reason.strip():
            sys.exit(f"Ausnahme {key} ohne Begründung")
        out[key.strip()] = reason.strip()
    return out


def main() -> int:
    if len(sys.argv) < 3:
        sys.exit(__doc__)
    exceptions = load_exceptions(Path(sys.argv[1]))
    blocking, listed = [], []
    for report in sys.argv[2:]:
        data = json.loads(Path(report).read_text(encoding="utf-8"))
        for site in data.get("site", []):
            for alert in site.get("alerts", []):
                risk = int(alert.get("riskcode", "0"))
                plugin = str(alert.get("pluginid", ""))
                ref = str(alert.get("alertRef", "")) or plugin
                name = alert.get("name") or alert.get("alert", "")
                count = len(alert.get("instances", []))
                urls = sorted({i.get("uri", "") for i in alert.get("instances", [])})[:3]
                entry = f"[{Path(report).stem}] {ref} {name} (Risiko {risk}, {count}×) {urls}"
                reason = exceptions.get(ref) or exceptions.get(plugin)
                if risk >= 2 and reason is None:
                    blocking.append(entry)
                else:
                    note = f" — Ausnahme: {reason}" if reason and risk >= 2 else ""
                    listed.append(entry + note)
    for e in listed:
        print("info  ", e)
    for e in blocking:
        print("GATE  ", e)
    if blocking:
        print(f"{len(blocking)} Alarm(e) Mittel/Hoch ohne Ausnahme", file=sys.stderr)
        return 1
    print("ZAP-Gate ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
