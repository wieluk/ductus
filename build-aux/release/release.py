#!/usr/bin/env python3
"""The git tag is the only place the version is written by hand.

    release.py version v1.2.3   stamps Cargo.toml, Cargo.lock and the metainfo
    release.py notes v1.2.3     prints release notes from conventional commits since the last tag
    release.py metainfo v1.2.3  writes the metainfo's <releases> from the tags
"""

from __future__ import annotations

import datetime
import re
import subprocess
import sys
from pathlib import Path
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[2]
METAINFO = ROOT / "data" / "app.ductus.Ductus.metainfo.xml"
HISTORY = 10
SECTIONS = [("New", "feat"), ("Improved", "perf"), ("Fixed", "fix")]


def normalize(tag: str) -> str:
    version = tag.strip().removeprefix("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version):
        sys.exit(f'"{tag}" is not a version like 1.2.3')
    return version


def replace_once(path: Path, pattern: str, replacement: str, flags: int = re.M) -> None:
    text = path.read_text()
    new, count = re.subn(pattern, replacement, text, count=1, flags=flags)
    if count != 1:
        sys.exit(f"{path.relative_to(ROOT)}: {pattern!r} not found")
    path.write_text(new)


def set_version(version: str) -> None:
    replace_once(ROOT / "Cargo.toml", r'^version = "[^"]*"$', f'version = "{version}"')
    # Cargo.lock too, so --locked builds still accept it.
    members = {
        m.group(1)
        for toml in ROOT.glob("crates/*/Cargo.toml")
        if (m := re.search(r'^name = "([^"]+)"', toml.read_text(), re.M))
    }
    lock = ROOT / "Cargo.lock"
    text = lock.read_text()

    def bump(match: re.Match[str]) -> str:
        name = match.group(1)
        return f'name = "{name}"\nversion = "{version}"' if name in members else match.group(0)

    lock.write_text(re.sub(r'name = "([^"]+)"\nversion = "[^"]*"', bump, text))
    today = datetime.date.today().isoformat()
    replace_once(
        METAINFO,
        r'<release version="[^"]*" date="[^"]*"',
        f'<release version="{version}" date="{today}"',
    )
    print(f"Version {version} in Cargo.toml, Cargo.lock and the metainfo.")


def git(*args: str) -> str:
    # v1.0.0-beta sorts before v1.0.0.
    return subprocess.run(
        ["git", "-c", "versionsort.suffix=-", *args],
        cwd=ROOT, check=True, capture_output=True, text=True,
    ).stdout.strip()


def group(subjects: list[str]) -> list[tuple[str, list[str]]]:
    sections: dict[str, list[str]] = {title: [] for title, _ in SECTIONS}
    other = []
    for subject in subjects:
        m = re.match(r"^([a-z]+)(?:\(([^)]+)\))?!?:\s*(.+)$", subject)
        if not m:
            if not subject.startswith("Merge "):
                other.append(subject[:1].upper() + subject[1:])
            continue
        kind, scope, text = m.groups()
        for title, wanted in SECTIONS:
            if kind == wanted:
                label = scope.upper() if scope in ("ui", "pdf", "ci", "hwr") else (scope or "").capitalize()
                sections[title].append(f"{label}: {text}" if scope else text[:1].upper() + text[1:])
    result = [(title, items) for title, items in sections.items() if items]
    if other:
        result.append(("Changes", other))
    return result


def releases_up_to(tag: str) -> list[dict]:
    tags = [t for t in git("tag", "--list", "v*", "--merged", tag, "--sort=-v:refname").splitlines() if t]
    if not tags or tags[0] != tag:
        sys.exit(f"{tag} is not a v* tag on this history")
    out = []
    for i, name in enumerate(tags[:HISTORY]):
        span = f"{tags[i + 1]}..{name}" if i + 1 < len(tags) else name
        subjects = [s for s in git("log", "--no-merges", "--format=%s", span).splitlines() if s]
        out.append({
            "tag": name,
            "version": name.removeprefix("v"),
            "date": git("log", "-1", "--format=%cs", name),
            "sections": group(subjects),
        })
    return out


def markdown(sections: list[tuple[str, list[str]]]) -> str:
    if not sections:
        return "Maintenance release.\n"
    return "\n".join(
        f"### {title}\n\n" + "\n".join(f"- {item}" for item in items) + "\n"
        for title, items in sections
    )


def releases_xml(releases: list[dict]) -> str:
    entries = []
    for r in releases:
        # Software centres don't offer development releases as stable.
        kind = ' type="development"' if "-" in r["version"] else ""
        body = ["<p>Maintenance release.</p>"] if not r["sections"] else [
            line
            for title, items in r["sections"]
            for line in [f"<p>{escape(title)}</p>", "<ul>", *[f"  <li>{escape(i)}</li>" for i in items], "</ul>"]
        ]
        entries.append("\n".join([
            f'    <release version="{escape(r["version"])}" date="{r["date"]}"{kind}>',
            "      <description>",
            *[f"        {line}" for line in body],
            "      </description>",
            "    </release>",
        ]))
    return "<releases>\n" + "\n".join(entries) + "\n  </releases>"


def main() -> None:
    if len(sys.argv) != 3 or sys.argv[1] not in ("version", "notes", "metainfo"):
        sys.exit(__doc__)
    command, tag = sys.argv[1], sys.argv[2]
    if command == "version":
        set_version(normalize(tag))
    elif command == "notes":
        sys.stdout.write(markdown(releases_up_to(tag)[0]["sections"]))
    else:
        text = METAINFO.read_text()
        new, count = re.subn(r"<releases>.*?</releases>", lambda _: releases_xml(releases_up_to(tag)), text, flags=re.S)
        if count != 1:
            sys.exit("the metainfo has no <releases> block")
        METAINFO.write_text(new)
        print("Releases written into the metainfo.")


if __name__ == "__main__":
    main()
