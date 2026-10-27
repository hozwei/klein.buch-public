#!/usr/bin/env python3
"""Baut die Seiten unter /legal/ für das Docker-Image (Plan W12, ADR 0039).

Nur Standardbibliothek (python3 ≥ 3.11), läuft in einer Stufe des Image-Builds.
Zwei Teilbefehle, weil die Eingaben in verschiedenen Stufen entstehen:

  licenses  third-party-licenses.html, LICENSE.txt, legal.css
            Eingaben: Ausgabe von `cargo about generate`, JSON von
            `cargo metadata --locked --format-version 1 --filter-platform
            x86_64-unknown-linux-gnu` (die Pfade darin müssen im selben
            Dateisystem lesbar sein — Cargo-Registry der Rust-Stufe),
            sidecar/THIRD-PARTY-JAVA.md, sidecar/notices/, js-licenses.json
            aus dem Vite-Build, LICENSE, legal.css.

  sources   quellen.html, SOURCES.md, debian-pakete.tsv
            Eingaben: sidecar/versions.json, Paketliste der Laufzeit-Stufe
            (`dpkg-query -W -f='${Package}\\t${Version}\\t${source:Package}\\t${source:Version}\\n'`),
            Quell-Angaben des Builds (URL, Commit, Tag).

Beide Befehle brechen mit Exit 1 ab, wenn ein Pflichtteil fehlt oder das
Ergebnis den Platzhalter der Entwicklungs-Fassung enthält.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
import shutil
import sys
import urllib.parse
from pathlib import Path

PLACEHOLDER = "Dev-Build-Platzhalter"
REQUIRED_STRINGS = ("SIL OPEN FONT LICENSE", "GUST Font License", "Zetetic", "BY-SA 3.0")
DEFAULT_REQUIRED_JS = ("svelte", "@sveltejs/kit", "marked", "minisearch")
RUST_BEGIN = "<!-- kb:rust:begin -->"
RUST_END = "<!-- kb:rust:end -->"

# Mitgelieferter C-Code, dessen Lizenz fest auf die Seite gehört — auch wenn das
# liefernde Crate eine Build-Abhängigkeit ist (openssl-src) und cargo-about es
# deshalb nicht aufführt.
FIXED_NOTICES = (
    ("libsqlite3-sys", "sqlcipher/LICENSE", "SQLCipher (Zetetic LLC), über libsqlite3-sys gebündelt"),
    ("openssl-src", "openssl/LICENSE.txt", "OpenSSL 3, über openssl-src gebündelt"),
)
# Weitere Lizenztexte von gebündeltem C-Code; nur, wenn das Crate im
# ausgelieferten Abhängigkeitsgraphen steckt.
CONDITIONAL_NOTICES = (
    ("ring", "LICENSE-BoringSSL", "BoringSSL-Anteile von ring"),
    ("ring", "LICENSE-other-bits", "weitere Anteile von ring"),
    ("aws-lc-sys", "LICENSE", "AWS-LC, über aws-lc-sys gebündelt"),
)
FONTS_CRATE = "typst-assets"
CSL_CRATE = "hayagriva"
JRE_LEGAL_DEFAULT = "/opt/klein-buch/sidecar/jre/legal/"


class BuildError(Exception):
    pass


def esc(text: str) -> str:
    return html.escape(text, quote=True)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n")
    except OSError as e:
        raise BuildError(f"{path}: nicht lesbar ({e.strerror})") from e


def load_json(path: Path):
    try:
        return json.loads(read_text(path))
    except json.JSONDecodeError as e:
        raise BuildError(f"{path}: kein gültiges JSON ({e})") from e


def slug(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-") or "x"


def pre(text: str, cls: str = "license-text") -> str:
    return f'<pre class="{cls}">{esc(text.rstrip())}</pre>'


# ---------------------------------------------------------------------------
# cargo metadata
# ---------------------------------------------------------------------------


class Metadata:
    """Abhängigkeitsgraph aus `cargo metadata --filter-platform …`."""

    def __init__(self, data: dict):
        try:
            self.packages = {p["id"]: p for p in data["packages"]}
            resolve = data["resolve"]
            self.nodes = {n["id"]: n for n in resolve["nodes"]}
        except (KeyError, TypeError) as e:
            raise BuildError(f"cargo metadata: Feld {e} fehlt (mit --format-version 1 und ohne --no-deps erzeugen)") from e
        root = resolve.get("root")
        if not root:
            members = data.get("workspace_members") or []
            if len(members) != 1:
                raise BuildError("cargo metadata: keine eindeutige Wurzel (resolve.root)")
            root = members[0]
        if root not in self.packages or root not in self.nodes:
            raise BuildError("cargo metadata: Wurzel-Paket fehlt im Graphen")
        self.root = root
        self.normal = self._walk(lambda kind: kind is None)
        self.shipped_or_build = self._walk(lambda kind: kind != "dev")

    def _walk(self, accept) -> set[str]:
        seen: set[str] = set()
        stack = [self.root]
        while stack:
            node = self.nodes.get(stack.pop())
            if node is None:
                continue
            for dep in node.get("deps", []):
                kinds = [k.get("kind") for k in dep.get("dep_kinds", [])]
                if not any(accept(k) for k in kinds):
                    continue
                pkg = dep.get("pkg")
                if pkg and pkg not in seen and pkg != self.root:
                    seen.add(pkg)
                    stack.append(pkg)
        return seen

    def root_package(self) -> dict:
        return self.packages[self.root]

    def pkg(self, pid: str) -> dict:
        return self.packages[pid]

    def by_name(self, name: str, ids: set[str]) -> list[dict]:
        found = [self.packages[i] for i in ids if self.packages[i]["name"] == name]
        return sorted(found, key=lambda p: p["version"])

    @staticmethod
    def pkg_dir(pkg: dict) -> Path:
        return Path(pkg["manifest_path"]).parent


def notice_files(directory: Path) -> list[Path]:
    try:
        entries = sorted(directory.iterdir(), key=lambda p: p.name)
    except OSError as e:
        raise BuildError(f"{directory}: nicht lesbar ({e.strerror})") from e
    return [p for p in entries if p.is_file() and p.name.lower().startswith("notice")]


# ---------------------------------------------------------------------------
# Teile der Lizenzseite
# ---------------------------------------------------------------------------


def rust_part(path: Path) -> str:
    raw = read_text(path)
    start, end = raw.find(RUST_BEGIN), raw.find(RUST_END)
    if start >= 0 and end > start:
        frag = raw[start + len(RUST_BEGIN):end]
    else:
        m = re.search(r"<body[^>]*>(.*)</body>", raw, re.S | re.I)
        frag = m.group(1) if m else raw
        frag = re.sub(r"<h1[^>]*>.*?</h1>", "", frag, count=1, flags=re.S | re.I)
    if "<pre" not in frag:
        raise BuildError(f"{path}: Rust-Teil ohne Lizenztexte (cargo about generate mit about.hbs?)")
    return frag.strip()


def shipped_notices_part(meta: Metadata) -> tuple[str, str]:
    """Abschnitt „Mitgelieferte Hinweise“ und den Schriften-NOTICE-Text."""
    out: list[str] = []
    fonts_notice = ""
    packages = sorted((meta.pkg(i) for i in meta.normal), key=lambda p: (p["name"], p["version"]))
    for pkg in packages:
        files = notice_files(meta.pkg_dir(pkg))
        if not files:
            continue
        name, version = pkg["name"], pkg["version"]
        out.append(f'<section class="notice" id="notice-{slug(name)}-{slug(version)}">')
        out.append(f"<h3>{esc(name)} {esc(version)}</h3>")
        for f in files:
            text = read_text(f)
            if name == FONTS_CRATE:
                fonts_notice += text
                out.append(
                    f'<p class="hint"><code>{esc(f.name)}</code>: Wortlaut im Abschnitt '
                    f'<a href="#schriften">Eingebettete Schriften</a>.</p>'
                )
            else:
                out.append(f"<h4><code>{esc(f.name)}</code></h4>")
                out.append(pre(text))
        out.append("</section>")

    out.append('<h3 id="c-code">Mitgelieferter C-Code</h3>')
    out.append(
        "<p>Diese Bibliotheken werden beim Bauen aus dem Quelltext übersetzt und stecken im "
        "Server-Binary. Ihre Lizenztexte stehen hier wörtlich.</p>"
    )
    for crate, rel, title in FIXED_NOTICES:
        pkgs = meta.by_name(crate, meta.shipped_or_build)
        if not pkgs:
            raise BuildError(f"cargo metadata: Paket {crate} fehlt (Pflicht-Hinweis {rel})")
        for pkg in pkgs:
            f = meta.pkg_dir(pkg) / rel
            if not f.is_file():
                raise BuildError(f"{f}: Pflicht-Hinweis fehlt")
            out.append(f"<h4>{esc(title)} — <code>{esc(pkg['name'])} {esc(pkg['version'])}/{esc(rel)}</code></h4>")
            out.append(pre(read_text(f)))
    for crate, rel, title in CONDITIONAL_NOTICES:
        for pkg in meta.by_name(crate, meta.normal):
            f = meta.pkg_dir(pkg) / rel
            if f.is_file():
                out.append(f"<h4>{esc(title)} — <code>{esc(pkg['name'])} {esc(pkg['version'])}/{esc(rel)}</code></h4>")
                out.append(pre(read_text(f)))
    return "\n".join(out), fonts_notice


def fonts_part(meta: Metadata, fonts_notice: str) -> str:
    pkgs = meta.by_name(FONTS_CRATE, meta.normal)
    if not pkgs or not fonts_notice.strip():
        raise BuildError(f"cargo metadata: {FONTS_CRATE} oder dessen NOTICE fehlt (eingebettete Schriften)")
    versions = ", ".join(p["version"] for p in pkgs)
    return "\n".join(
        [
            "<p>Typst bettet über das Crate "
            f"<code>{esc(FONTS_CRATE)}</code> ({esc(versions)}) Schriften in das Server-Binary ein, "
            "die beim Erzeugen von PDF-Belegen verwendet werden können: Libertinus Serif "
            "(SIL Open Font License 1.1), New Computer Modern (GUST Font License), "
            "DejaVu (Bitstream-Vera-Lizenz) und die PDF-Standardschriften von Foxit (PDFium, "
            "BSD-3-Clause; seit Typst 0.15, für eingebettete PDF-Grafiken). Dazu kommen "
            "ICC-Farbprofile (CC0-1.0). "
            "Der folgende Text ist die NOTICE-Datei des Crates im Wortlaut.</p>",
            pre(fonts_notice),
            "<p class=\"hint\">Die Schriften des Betriebssystems im Image (z. B. "
            "<code>fonts-dejavu-core</code>) stammen aus Debian; ihre Lizenzen stehen unter "
            "<code>/usr/share/doc/&lt;paket&gt;/copyright</code>, die Paketliste unter "
            '<a href="quellen.html">Quellen</a>.</p>',
        ]
    )


def csl_part(meta: Metadata) -> str:
    pkgs = meta.by_name(CSL_CRATE, meta.normal)
    if not pkgs:
        return ""
    versions = ", ".join(p["version"] for p in pkgs)
    return (
        "<p>Die von Typst genutzte Literaturverzeichnis-Bibliothek "
        f"<code>{esc(CSL_CRATE)}</code> ({esc(versions)}) enthält in ihrer Standard-Ausstattung "
        "Zitierstile und Sprachdateien des Projekts "
        '<a href="https://citationstyles.org/">Citation Style Language</a> '
        "(Verzeichnis <code>archive/</code>). Sie stehen unter der Lizenz Creative Commons "
        "Namensnennung – Weitergabe unter gleichen Bedingungen 3.0 (CC BY-SA 3.0, "
        '<a href="https://creativecommons.org/licenses/by-sa/3.0/">Lizenztext</a>); '
        "die Autorinnen und Autoren der einzelnen Stile nennen die Stildateien selbst "
        '(Quellen: <a href="https://github.com/citation-style-language/styles">styles</a>, '
        '<a href="https://github.com/citation-style-language/locales">locales</a>). '
        "Klein.Buch verwendet sie unverändert und erzeugt keine Literaturverzeichnisse. "
        "Den Hinweis des Crates zeigt der Abschnitt "
        f'<a href="#notice-{slug(pkgs[0]["name"])}-{slug(pkgs[0]["version"])}">Mitgelieferte Hinweise</a>.</p>'
    )


def java_part(inventory: Path, notices_dir: Path, versions: dict | None, jre_legal: str) -> str:
    inv = read_text(inventory)
    if not inv.strip():
        raise BuildError(f"{inventory}: Java-Inventar ist leer")
    if not notices_dir.is_dir():
        raise BuildError(f"{notices_dir}: Ordner mit den Java-Hinweisen fehlt")
    files = sorted(p for p in notices_dir.rglob("*") if p.is_file() and not p.name.startswith("."))
    if not files:
        raise BuildError(f"{notices_dir}: keine Hinweisdateien")
    out = []
    if versions:
        jdk = versions.get("jdk") or {}
        rows = []
        for key, label in (("kosit_validator", "KoSIT-Validator"), ("mustang", "Mustang")):
            c = versions.get(key) or {}
            rows.append((label, c.get("version", ""), c.get("license", ""), c.get("source_url", "")))
        x = versions.get("xrechnung_config") or {}
        rows.append(("XRechnung-Konfiguration", x.get("version", ""), x.get("license", ""), x.get("source_url", "")))
        rows.append(
            (f"Java-Laufzeit ({jdk.get('vendor', 'OpenJDK')}, jlink)", jdk.get("version", ""),
             jdk.get("license", ""), jdk.get("source_url", ""))
        )
        out.append("<table><thead><tr><th>Komponente</th><th>Version</th><th>Lizenz</th><th>Quelle</th></tr></thead><tbody>")
        for label, ver, lic, url in rows:
            out.append(f"<tr><td>{esc(label)}</td><td>{esc(ver)}</td><td>{esc(lic)}</td><td>{link(url)}</td></tr>")
        out.append("</tbody></table>")
    out.append(
        "<p>Die Lizenz- und Hinweistexte der Module der Java-Laufzeit (OpenJDK, GPL-2.0 mit "
        f"Classpath Exception) liegen im Image unter <code>{esc(jre_legal)}</code>.</p>"
    )
    out.append(f"<h3>Inventar der Java-Komponenten (<code>{esc(inventory.name)}</code>)</h3>")
    out.append(pre(inv, "markdown"))
    out.append("<h3>Hinweis- und Lizenzdateien</h3>")
    for f in files:
        rel = f.relative_to(notices_dir).as_posix()
        out.append(f'<section class="notice" id="java-{slug(rel)}"><h4><code>{esc(rel)}</code></h4>')
        out.append(pre(read_text(f)))
        out.append("</section>")
    return "\n".join(out)


def js_part(path: Path, required: tuple[str, ...]) -> str:
    data = load_json(path)
    if not isinstance(data, list) or not data:
        raise BuildError(f"{path}: Liste der JS-Pakete ist leer")
    names = set()
    out = []
    for entry in sorted(data, key=lambda e: (str(e.get("name")), str(e.get("version")))):
        missing = [k for k in ("name", "version", "license", "licenseText") if not str(entry.get(k) or "").strip()]
        if missing:
            raise BuildError(f"{path}: Eintrag {entry.get('name')!r} ohne {', '.join(missing)}")
        name, version = entry["name"], entry["version"]
        names.add(name)
        out.append(f'<section class="package" id="js-{slug(name)}">')
        out.append(f'<h3>{esc(name)} {esc(version)} <span class="spdx">(SPDX: {esc(entry["license"])})</span></h3>')
        out.append(pre(entry["licenseText"]))
        if str(entry.get("noticeText") or "").strip():
            out.append("<h4>NOTICE</h4>")
            out.append(pre(entry["noticeText"]))
        out.append("</section>")
    absent = [n for n in required if n not in names]
    if absent:
        raise BuildError(f"{path}: Pflicht-Pakete fehlen: {', '.join(absent)}")
    return "\n".join(out)


def extra_part(files: list[Path]) -> str:
    out = []
    for f in files:
        out.append(f'<section class="notice" id="extra-{slug(f.name)}"><h3><code>{esc(f.name)}</code></h3>')
        out.append(pre(read_text(f)))
        out.append("</section>")
    return "\n".join(out)


def link(url: str) -> str:
    if not url:
        return ""
    if not re.match(r"^https?://", url):
        return esc(url)
    return f'<a href="{esc(url)}">{esc(url)}</a>'


def page(title: str, intro: str, sections: list[tuple[str, str, str]]) -> str:
    toc = "".join(f'<li><a href="#{sid}">{esc(head)}</a></li>' for sid, head, _ in sections)
    body = "\n".join(f'<h2 id="{sid}">{esc(head)}</h2>\n{content}' for sid, head, content in sections)
    return (
        "<!doctype html>\n<html lang=\"de\">\n<head>\n<meta charset=\"utf-8\">\n"
        '<meta name="viewport" content="width=device-width, initial-scale=1">\n'
        f"<title>{esc(title)}</title>\n"
        '<link rel="stylesheet" href="legal.css">\n</head>\n<body>\n<main class="legal">\n'
        f"<h1>{esc(title)}</h1>\n{intro}\n"
        f'<nav class="toc" aria-label="Inhalt"><ol>{toc}</ol></nav>\n{body}\n'
        "</main>\n</body>\n</html>\n"
    )


def check_output(text: str, required: tuple[str, ...], where: str) -> None:
    if PLACEHOLDER in text:
        raise BuildError(f"{where}: enthält „{PLACEHOLDER}“")
    absent = [s for s in required if s not in text]
    if absent:
        raise BuildError(f"{where}: Pflicht-Angaben fehlen: {', '.join(absent)}")


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")


def cmd_licenses(a: argparse.Namespace) -> None:
    meta = Metadata(load_json(a.cargo_metadata))
    root = meta.root_package()
    versions = load_json(a.versions) if a.versions else None
    required_js = tuple(s for s in a.require_js.split(",") if s) if a.require_js is not None else DEFAULT_REQUIRED_JS

    notices, fonts_notice = shipped_notices_part(meta)
    sections = [
        ("rust", "Rust-Bibliotheken", rust_part(a.rust_html)),
        ("hinweise", "Mitgelieferte Hinweise", notices),
        ("schriften", "Eingebettete Schriften", fonts_part(meta, fonts_notice)),
    ]
    csl = csl_part(meta)
    if csl:
        sections.append(("zitierstile", "Eingebettete Zitierstile (CSL)", csl))
    sections += [
        ("java", "Java-Komponenten (E-Rechnung)", java_part(a.java_inventory, a.java_notices, versions, a.jre_legal)),
        ("javascript", "JavaScript-Bibliotheken der Oberfläche", js_part(a.js_licenses, required_js)),
    ]
    if a.extra_notice:
        sections.append(("weitere", "Weitere Hinweise", extra_part(a.extra_notice)))

    intro = (
        '<p class="meta">'
        f"Klein.Buch {esc(root['version'])} steht unter der GNU Affero General Public License, "
        'Version 3 oder später (<a href="LICENSE.txt">Lizenztext</a>). Diese Seite nennt die '
        "Komponenten Dritter, die im Image enthalten sind, mit ihren Lizenzen und Hinweisen. "
        'Herkunft und Quelltexte nennt die Seite <a href="quellen.html">Quellen</a>; die '
        "Lizenzen der Debian-Pakete des Images stehen unter "
        "<code>/usr/share/doc/&lt;paket&gt;/copyright</code>.</p>"
    )
    text = page("Klein.Buch — Lizenzen und Hinweise Dritter", intro, sections)
    check_output(text, REQUIRED_STRINGS, "third-party-licenses.html")

    license_text = read_text(a.license)
    if "GNU AFFERO GENERAL PUBLIC LICENSE" not in license_text:
        raise BuildError(f"{a.license}: keine AGPL-Lizenzdatei")
    a.out.mkdir(parents=True, exist_ok=True)
    write(a.out / "third-party-licenses.html", text)
    write(a.out / "LICENSE.txt", license_text)
    shutil.copyfile(a.css, a.out / "legal.css")


# ---------------------------------------------------------------------------
# Quellen
# ---------------------------------------------------------------------------


def read_debian_packages(path: Path) -> list[tuple[str, str, str, str]]:
    rows = []
    for line in read_text(path).splitlines():
        if not line.strip() or line.startswith("#"):
            continue
        cols = line.split("\t")
        if cols[0] in ("Package", "Paket"):
            continue
        if len(cols) < 2 or not cols[0] or not cols[1]:
            raise BuildError(f"{path}: Zeile ohne Paket und Version: {line!r}")
        pkg, ver = cols[0].strip(), cols[1].strip()
        src = (cols[2].strip() if len(cols) > 2 else "") or pkg
        srcver = (cols[3].strip() if len(cols) > 3 else "") or ver
        rows.append((pkg, ver, src, srcver))
    if not rows:
        raise BuildError(f"{path}: keine Debian-Pakete")
    return sorted(rows)


def build_ref(source_url: str, commit: str, tag: str, version: str) -> dict:
    url = source_url.rstrip("/")
    if not re.match(r"^https?://[^\s\"'<>]+$", url):
        raise BuildError(f"Quell-URL ungültig: {source_url!r}")
    ref = {"url": url, "commit": None, "tag": None, "tree": None, "release": None, "archive": None}
    if re.fullmatch(r"[0-9a-f]{40}", commit or ""):
        ref["commit"] = commit
        ref["tree"] = f"{url}/tree/{commit}"
    if tag and re.fullmatch(r"v[0-9][0-9A-Za-z.+_-]*", tag):
        ref["tag"] = tag
        ref["release"] = f"{url}/releases/tag/{tag}"
        ref["archive"] = f"klein-buch-{version or tag[1:]}-quellen-dritter.tar.zst"
    return ref


def snapshot_url(src: str, ver: str) -> str:
    q = urllib.parse.quote
    return f"https://snapshot.debian.org/package/{q(src, safe='')}/{q(ver, safe='')}/"


def cmd_sources(a: argparse.Namespace) -> None:
    versions = load_json(a.versions)
    for key in ("kosit_validator", "mustang", "xrechnung_config", "jdk"):
        if not isinstance(versions.get(key), dict):
            raise BuildError(f"{a.versions}: Eintrag {key} fehlt")
    debs = read_debian_packages(a.debian_packages)
    ref = build_ref(a.source_url, a.commit, a.tag, a.version)
    title = f"Klein.Buch {a.version}".strip()
    images = versions.get("images") or {}
    jdk = versions["jdk"]
    components = [
        ("KoSIT-Validator", versions["kosit_validator"]),
        ("Mustang (Mustang-CLI)", versions["mustang"]),
        ("XRechnung-Konfiguration", versions["xrechnung_config"]),
    ]

    # --- quellen.html
    kb = []
    kb.append(f"<p>Quelltext: {link(ref['url'])}</p>")
    if ref["commit"]:
        kb.append(f"<p>Commit: <code>{esc(ref['commit'])}</code> — {link(ref['tree'])}</p>")
    else:
        kb.append("<p><strong>Entwicklungs-Build, Quellstand unbekannt.</strong></p>")
    if ref["tag"]:
        kb.append(f"<p>Tag: <code>{esc(ref['tag'])}</code> — {link(ref['release'])}</p>")
        kb.append(
            "<p>Die Quelltexte der Komponenten Dritter (OpenJDK des exakten Builds mit "
            "jlink-Befehl, Debian-Quellpakete, Quell-JARs der MPL-/EPL-Teile, CEN-Schematron) "
            f"liegen als <code>{esc(ref['archive'])}</code> bei diesem Release.</p>"
        )
    kb.append(
        "<p>Wer das Image mit eigenen Änderungen betreibt, nennt seinen Quellstand über "
        "<code>KLEIN_BUCH_SOURCE_URL</code> (AGPL-3.0 §13).</p>"
    )

    java = ["<table><thead><tr><th>Komponente</th><th>Version</th><th>Lizenz</th><th>Quelle</th><th>SHA-256</th></tr></thead><tbody>"]
    for label, c in components:
        java.append(
            f"<tr><td>{esc(label)}</td><td>{esc(c.get('version', ''))}</td><td>{esc(c.get('license', ''))}</td>"
            f"<td>{link(c.get('source_url', ''))}</td><td><code>{esc(c.get('sha256', ''))}</code></td></tr>"
        )
    java.append(
        f"<tr><td>Java-Laufzeit: {esc(jdk.get('vendor', ''))} (per jlink verkleinert)</td>"
        f"<td>{esc(jdk.get('version', ''))}</td><td>{esc(jdk.get('license', ''))}</td>"
        f"<td>{link(jdk.get('source_url', ''))}</td><td><code>{esc(jdk.get('image', ''))}</code></td></tr>"
    )
    java.append("</tbody></table>")
    if versions.get("jlink_modules"):
        java.append(f"<p>jlink-Module: <code>{esc(versions['jlink_modules'])}</code></p>")

    img = ["<table><thead><tr><th>Stufe</th><th>Image (mit Digest)</th></tr></thead><tbody>"]
    for k in sorted(images):
        img.append(f"<tr><td>{esc(k)}</td><td><code>{esc(images[k])}</code></td></tr>")
    if jdk.get("image"):
        img.append(f"<tr><td>jdk</td><td><code>{esc(jdk['image'])}</code></td></tr>")
    img.append("</tbody></table>")

    deb = [
        f"<p>{len(debs)} Pakete der Laufzeit-Stufe (<a href=\"debian-pakete.tsv\">debian-pakete.tsv</a>). "
        "Lizenzen: <code>/usr/share/doc/&lt;paket&gt;/copyright</code> im Image; Quellpakete "
        "über snapshot.debian.org.</p>",
        "<table><thead><tr><th>Paket</th><th>Version</th><th>Quellpaket</th></tr></thead><tbody>",
    ]
    for pkg, ver, src, srcver in debs:
        deb.append(
            f"<tr><td>{esc(pkg)}</td><td>{esc(ver)}</td>"
            f'<td><a href="{esc(snapshot_url(src, srcver))}">{esc(src)} {esc(srcver)}</a></td></tr>'
        )
    deb.append("</tbody></table>")

    intro = (
        '<p class="meta">Herkunft des Images: Quellstand von Klein.Buch, Komponenten Dritter mit '
        'Version, Quelle und Prüfsumme. Lizenzen: <a href="third-party-licenses.html">Lizenzen '
        'und Hinweise Dritter</a>, <a href="LICENSE.txt">Lizenz von Klein.Buch</a>.</p>'
    )
    sections = [
        ("klein-buch", title, "\n".join(kb)),
        ("java", "Java-Komponenten", "\n".join(java)),
        ("images", "Basis-Images", "\n".join(img)),
        ("debian", "Debian-Pakete", "\n".join(deb)),
    ]
    html_text = page("Klein.Buch — Quellen", intro, sections)

    # --- SOURCES.md
    md = [f"# {title} — Quellen", ""]
    md.append(f"- Quelltext: {ref['url']}")
    md.append(f"- Commit: {ref['commit']} ({ref['tree']})" if ref["commit"] else "- Entwicklungs-Build, Quellstand unbekannt")
    if ref["tag"]:
        md.append(f"- Tag: {ref['tag']} ({ref['release']})")
        md.append(f"- Quellen Dritter: Release-Asset `{ref['archive']}`")
    md += ["", "## Java-Komponenten", "", "| Komponente | Version | Lizenz | Quelle | SHA-256 |", "|---|---|---|---|---|"]
    for label, c in components:
        md.append(f"| {label} | {c.get('version', '')} | {c.get('license', '')} | {c.get('source_url', '')} | `{c.get('sha256', '')}` |")
    md.append(
        f"| Java-Laufzeit: {jdk.get('vendor', '')} (jlink) | {jdk.get('version', '')} | {jdk.get('license', '')} "
        f"| {jdk.get('source_url', '')} | `{jdk.get('image', '')}` |"
    )
    if versions.get("jlink_modules"):
        md += ["", f"jlink-Module: `{versions['jlink_modules']}`"]
    md += ["", "## Basis-Images", ""]
    md += [f"- {k}: `{images[k]}`" for k in sorted(images)]
    md += ["", "## Debian-Pakete", "", "| Paket | Version | Quellpaket |", "|---|---|---|"]
    md += [f"| {p} | {v} | {s} {sv} |" for p, v, s, sv in debs]
    md_text = "\n".join(md) + "\n"

    for name, text in (("quellen.html", html_text), ("SOURCES.md", md_text)):
        check_output(text, (), name)
    a.out.mkdir(parents=True, exist_ok=True)
    write(a.out / "quellen.html", html_text)
    write(a.out / "SOURCES.md", md_text)
    write(a.out / "debian-pakete.tsv", "".join("\t".join(r) + "\n" for r in debs))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="Baut die Seiten unter /legal/ (Klein.Buch, Plan W12).")
    sub = ap.add_subparsers(dest="cmd", required=True)

    lic = sub.add_parser("licenses", help="third-party-licenses.html, LICENSE.txt, legal.css")
    lic.add_argument("--out", type=Path, required=True)
    lic.add_argument("--rust-html", type=Path, required=True, help="Ausgabe von cargo about generate")
    lic.add_argument("--cargo-metadata", type=Path, required=True, help="JSON von cargo metadata --filter-platform …")
    lic.add_argument("--java-inventory", type=Path, required=True, help="sidecar/THIRD-PARTY-JAVA.md")
    lic.add_argument("--java-notices", type=Path, required=True, help="sidecar/notices/")
    lic.add_argument("--js-licenses", type=Path, required=True, help="js-licenses.json aus dem Vite-Build")
    lic.add_argument("--license", type=Path, required=True, help="LICENSE von Klein.Buch")
    lic.add_argument("--css", type=Path, required=True, help="legal/legal.css")
    lic.add_argument("--versions", type=Path, help="sidecar/versions.json (Tabelle im Java-Teil)")
    lic.add_argument("--jre-legal", default=JRE_LEGAL_DEFAULT, help="Pfad von <jre>/legal im Image")
    lic.add_argument("--extra-notice", type=Path, action="append", default=[], help="weitere Hinweisdatei (mehrfach)")
    lic.add_argument("--require-js", default=None, help="Komma-Liste der Pflicht-Pakete im JS-Teil")
    lic.set_defaults(func=cmd_licenses)

    src = sub.add_parser("sources", help="quellen.html, SOURCES.md, debian-pakete.tsv")
    src.add_argument("--out", type=Path, required=True)
    src.add_argument("--versions", type=Path, required=True, help="sidecar/versions.json")
    src.add_argument("--debian-packages", type=Path, required=True, help="Ausgabe von dpkg-query -W (TSV)")
    src.add_argument("--source-url", required=True)
    src.add_argument("--commit", default="")
    src.add_argument("--tag", default="")
    src.add_argument("--version", default="", help="CalVer von Klein.Buch")
    src.set_defaults(func=cmd_sources)

    a = ap.parse_args(argv)
    try:
        a.func(a)
    except BuildError as e:
        print(f"build_legal: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
