#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
"""Validate repository provenance, then run the unmodified draft validator.

This is repository tooling, not a framework compiler or runtime. No files are
modified and no network access is needed after dependencies are installed.
"""
from __future__ import annotations

import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
from urllib.parse import unquote, urlparse
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = "my-framework-spec-v0.1.0"
SOURCE_HASHES = {
    f"originals/{PACKAGE}.zip": "4cef70e600dde218154235b8394aec44360254f5ae0c91074b3805287cc86958",
    f"originals/{PACKAGE}.md": "6a43dd5f27c2e4eb99ccbba6890ec6215fa6280a7a758ec38dd6dc44d9025d01",
}
LICENSE_HASH = "0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def validate_preservation(root: Path) -> dict:
    """Verify known source artifacts and every file from the supplied archive."""
    spec = root / "spec"
    manifest = {}
    for line in (spec / "SHA256SUMS").read_text(encoding="utf-8").splitlines():
        digest, filename = line.split("  ", 1)
        require(filename not in manifest, f"Duplicate checksum entry: {filename}")
        manifest[filename] = digest
    require(manifest == SOURCE_HASHES, "Source checksum manifest differs from known inputs")
    for filename, expected in SOURCE_HASHES.items():
        path = spec / filename
        require(not path.is_symlink(), f"Source artifact must not be a symlink: {filename}")
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        require(actual == expected, f"Original artifact changed: {filename}")

    extracted = spec / PACKAGE
    expected_paths = set()
    with ZipFile(spec / f"originals/{PACKAGE}.zip") as archive:
        for member in archive.infolist():
            path = PurePosixPath(member.filename)
            require(not path.is_absolute() and ".." not in path.parts, "Unsafe archive path")
            require(path.parts and path.parts[0] == PACKAGE, "Unexpected archive root")
            if member.is_dir():
                continue
            relative = Path(*path.parts[1:])
            require(relative not in expected_paths, f"Duplicate archive file: {relative}")
            expected_paths.add(relative)
            target = extracted / relative
            require(not target.is_symlink(), f"Imported file must not be a symlink: {relative}")
            require(target.is_file(), f"Missing imported file: {relative}")
            require(target.read_bytes() == archive.read(member), f"Imported file changed: {relative}")
    actual_paths = {path.relative_to(extracted) for path in extracted.rglob("*") if path.is_file()}
    require(actual_paths == expected_paths, "Unexpected files in immutable imported package")
    return {"source_artifacts_verified": len(SOURCE_HASHES), "imported_files_verified": len(expected_paths)}


def validate_scaffold(root: Path) -> dict:
    """Check the license bytes and file targets of new Markdown links."""
    require(hashlib.sha256((root / "LICENSE").read_bytes()).hexdigest() == LICENSE_HASH,
            "LICENSE differs from the official AGPL v3 text")
    require("AGPL-3.0-only" in (root / "README.md").read_text(encoding="utf-8"),
            "README must explicitly select AGPL-3.0-only")
    links = 0
    for path in [*root.glob("*.md"), *(root / "docs").rglob("*.md")]:
        for link in re.findall(r"\[[^\]]+\]\(([^)]+)\)", path.read_text(encoding="utf-8")):
            parsed = urlparse(link)
            if parsed.scheme or parsed.netloc or not parsed.path:
                continue
            target = (path.parent / unquote(parsed.path)).resolve()
            require(root.resolve() in target.parents, f"Link outside repository: {path.name}: {link}")
            require(target.is_file(), f"Broken documentation link: {path.name}: {link}")
            links += 1
    return {"license": "AGPL-3.0-only", "repository_document_links": links}


def run_original_validator(root: Path) -> dict:
    package = root / "spec" / PACKAGE
    result = subprocess.run(
        [sys.executable, "tools/validate_spec.py"], cwd=package,
        text=True, capture_output=True, check=False,
    )
    require(result.returncode == 0,
            "Original validator failed (unchanged):\n" + result.stdout + result.stderr)
    report = json.loads(result.stdout)
    require(isinstance(report, dict) and report.get("status") == "pass",
            "Original validator did not report a pass")
    return report


def main() -> int:
    try:
        provenance = validate_preservation(ROOT)
        scaffold = validate_scaffold(ROOT)
        original = run_original_validator(ROOT)
        print(json.dumps({"status": "pass", **provenance, **scaffold,
                          "original_validator": original}, indent=2, ensure_ascii=False))
        return 0
    except (OSError, ValueError, KeyError) as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
