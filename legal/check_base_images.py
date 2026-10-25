#!/usr/bin/env python3
"""Basis-Images im Dockerfile = Angaben in versions.json (Plan W12).

Die Lizenz- und Quellenseiten (`/legal/quellen.html`, SOURCES.md) und das
Quellarchiv nennen die Basis-Images und die JDK-Version aus
`src-tauri/sidecar/versions.json`; gebaut wird aus den FROM-Zeilen des
Dockerfiles. Ändert ein Update (z. B. ein Dependabot-Vorschlag) nur eines
davon, schlägt diese Prüfung in CI fehl.

Außerdem (W14-021, Lauf F7a): Jedes Container-Image, das die Workflows als
Dienst oder Job-Container (`image:`) bzw. die CI-Skripte als feste Variable
(`…_IMAGE="…"`) nutzen, ist per Digest gepinnt (`name:tag@sha256:…`).

Aufruf aus der Wurzel des öffentlichen Repos (bzw. `klein-buch/`):
    python3 -I legal/check_base_images.py
Exit 0 = gleich, 1 = Abweichung, 2 = Datei fehlt oder unlesbar.
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCKERFILE = ROOT / "Dockerfile"
VERSIONS = ROOT / "src-tauri" / "sidecar" / "versions.json"
# Öffentliches Repo: .github/ in der Wurzel; Projektordner: neben klein-buch/.
GITHUB_DIRS = [ROOT / ".github", ROOT.parent / ".github"]

# Stufe im Dockerfile → Schlüssel in versions.json
STAGES = {
    "web": ("images", "node"),
    "server": ("images", "rust"),
    "legal-tools": ("images", "rust"),
    "jre": ("jdk", "image"),
    "runtime-base": ("images", "runtime"),
}
FROM = re.compile(r"^FROM\s+(\S+)\s+AS\s+(\S+)\s*$", re.IGNORECASE)
WORKFLOW_IMAGE = re.compile(r"""^\s*(?:-\s+)?image:\s*['"]?([^\s'"#]+)""")
SCRIPT_IMAGE = re.compile(r"""^\s*[A-Z0-9_]*IMAGE=['"]?([^\s'"$]+)['"]?\s*$""")


def ci_image_problems():
    """Images aus Workflows (`image:`) und CI-Skripten (`…_IMAGE=…`) ohne Digest."""
    gh = next((d for d in GITHUB_DIRS if (d / "workflows").is_dir()), None)
    if gh is None:
        return ["kein .github/workflows gefunden"], 0
    problems, count = [], 0
    sources = [(p, WORKFLOW_IMAGE) for p in sorted((gh / "workflows").glob("*.y*ml"))]
    sources += [(p, SCRIPT_IMAGE) for p in sorted((gh / "scripts").glob("*.sh"))]
    for path, pattern in sources:
        for no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            m = pattern.match(line)
            if not m:
                continue
            count += 1
            if "@sha256:" not in m.group(1):
                problems.append(f"{path.parent.name}/{path.name}:{no}: {m.group(1)} ohne Digest")
    return problems, count


def main() -> int:
    try:
        lines = DOCKERFILE.read_text(encoding="utf-8").splitlines()
        versions = json.loads(VERSIONS.read_text(encoding="utf-8"))
    except (OSError, ValueError) as e:
        print(f"nicht lesbar: {e}", file=sys.stderr)
        return 2
    found = {}
    for line in lines:
        m = FROM.match(line.strip())
        if m:
            found[m.group(2)] = m.group(1)
    problems = []
    for stage, (group, key) in STAGES.items():
        image = found.get(stage)
        expected = (versions.get(group) or {}).get(key)
        if image is None:
            problems.append(f"Stufe {stage}: keine FROM-Zeile mit Image")
        elif "@sha256:" not in image:
            problems.append(f"Stufe {stage}: {image} ohne Digest")
        elif image != expected:
            problems.append(f"Stufe {stage}: Dockerfile {image} ≠ versions.json {group}.{key} {expected}")
    # Die JDK-Version im Tag muss zu jdk.version passen (21.0.12_8 ↔ 21.0.12+8).
    jdk_image = found.get("jre", "")
    jdk_version = (versions.get("jdk") or {}).get("version", "")
    if jdk_version and jdk_version.replace("+", "_") not in jdk_image:
        problems.append(f"Stufe jre: {jdk_image} passt nicht zu jdk.version {jdk_version}")
    try:
        ci_problems, ci_count = ci_image_problems()
    except OSError as e:
        print(f"nicht lesbar: {e}", file=sys.stderr)
        return 2
    problems += ci_problems
    for p in problems:
        print(p, file=sys.stderr)
    if problems:
        return 1
    print(f"Basis-Images ok ({len(STAGES)} Stufen), CI-Images mit Digest ({ci_count}).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
