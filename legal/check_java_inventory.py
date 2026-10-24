#!/usr/bin/env python3
"""Prüft das Java-Inventar des Sidecars (Plan W12).

Liest alle META-INF/maven/*/*/pom.properties aus mustang.jar und kosit-validator.jar und
prüft, dass jede Koordinate group:artifact:version in THIRD-PARTY-JAVA.md steht (als
`group:artifact:version` in Backticks). Zusätzlich müssen alle dort genannten Hinweisdateien
in notices/ existieren und alle maven-Zeilen aus copyleft-sources.txt im Inventar stehen.

Nur Standardbibliothek. Aufruf (ohne Argumente gelten die Pfade im Repo):
    python3 -I legal/check_java_inventory.py [--sidecar DIR]
Exit 0 = vollständig, 1 = Lücken (Liste auf stderr), 2 = Datei fehlt/unlesbar.
"""

import argparse
import os
import re
import sys
import zipfile

JARS = ("lib/mustang.jar", "lib/kosit-validator.jar")
INVENTORY = "THIRD-PARTY-JAVA.md"
NOTICES = "notices"
COPYLEFT = "copyleft-sources.txt"

COORD_IN_BACKTICKS = re.compile(r"`([A-Za-z0-9_.\-]+:[A-Za-z0-9_.\-]+:[A-Za-z0-9_.\-]+)`")
NOTICE_NAME = re.compile(r"\b([A-Za-z0-9_.\-]+-(?:NOTICE|LICENSE)\.txt)\b")


def default_sidecar():
    here = os.path.dirname(os.path.abspath(__file__))
    return os.path.normpath(os.path.join(here, "..", "src-tauri", "sidecar"))


def pom_coordinates(jar_path):
    """Liefert die Koordinaten aller pom.properties im JAR."""
    coords = set()
    with zipfile.ZipFile(jar_path) as z:
        for name in z.namelist():
            parts = name.split("/")
            if len(parts) != 5 or parts[:2] != ["META-INF", "maven"] or parts[4] != "pom.properties":
                continue
            props = {}
            for line in z.read(name).decode("utf-8", "replace").splitlines():
                line = line.strip()
                if not line or line.startswith(("#", "!")) or "=" not in line:
                    continue
                key, value = line.split("=", 1)
                props[key.strip()] = value.strip()
            group, artifact, version = props.get("groupId"), props.get("artifactId"), props.get("version")
            if not (group and artifact and version):
                raise ValueError(f"{jar_path}!/{name}: groupId/artifactId/version unvollständig")
            coords.add(f"{group}:{artifact}:{version}")
    return coords


def main():
    parser = argparse.ArgumentParser(description="Prüft THIRD-PARTY-JAVA.md gegen die pom.properties der Sidecar-JARs.")
    parser.add_argument("--sidecar", default=default_sidecar(),
                        help="Verzeichnis mit lib/, notices/, THIRD-PARTY-JAVA.md (Standard: src-tauri/sidecar)")
    args = parser.parse_args()
    sidecar = args.sidecar

    try:
        with open(os.path.join(sidecar, INVENTORY), encoding="utf-8") as f:
            inventory = f.read()
        found = {}
        for rel in JARS:
            for coord in pom_coordinates(os.path.join(sidecar, rel)):
                found.setdefault(coord, []).append(os.path.basename(rel))
        copyleft_lines = []
        copyleft_path = os.path.join(sidecar, COPYLEFT)
        if os.path.exists(copyleft_path):
            with open(copyleft_path, encoding="utf-8") as f:
                copyleft_lines = [line.strip() for line in f if line.strip()]
    except (OSError, ValueError, zipfile.BadZipFile) as err:
        print(f"check_java_inventory: {err}", file=sys.stderr)
        return 2

    listed = set(COORD_IN_BACKTICKS.findall(inventory))
    problems = []

    missing = sorted(c for c in found if c not in listed)
    for coord in missing:
        problems.append(f"fehlt im Inventar: {coord} ({', '.join(sorted(found[coord]))})")

    for name in sorted(set(NOTICE_NAME.findall(inventory))):
        if not os.path.isfile(os.path.join(sidecar, NOTICES, name)):
            problems.append(f"Hinweisdatei fehlt: {NOTICES}/{name}")

    for line in copyleft_lines:
        kind, _, value = line.partition(" ")
        if kind == "maven" and value not in listed:
            problems.append(f"{COPYLEFT}: {value} steht nicht im Inventar")
        elif kind not in ("maven", "url"):
            problems.append(f"{COPYLEFT}: unbekannte Zeile: {line}")

    if problems:
        print(f"Java-Inventar unvollständig ({len(problems)}):", file=sys.stderr)
        for p in problems:
            print(f"  - {p}", file=sys.stderr)
        return 1
    print(f"Java-Inventar vollständig: {len(found)} Koordinaten aus {len(JARS)} JARs.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
