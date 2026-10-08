#!/usr/bin/env python3
"""EXEC-001 W7 exit: the product must build, test and run without the
prototype directory.

`properties/` is a design reference only (development-plan §1.2/§10). This check
keeps that true by refusing any build, test or source reference to it, so the
product cannot quietly grow a dependency on the prototype. The removal drill
itself was run by hand on 2026-09-16 (npm test, vite build, lint, typecheck, all
after moving `properties/` aside); this keeps the property enforceable.
"""
from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# Files that describe how the product is built, tested or linted.
CONFIGS = [
    "package.json",
    "package-lock.json",
    "vite.config.ts",
    "tsconfig.json",
    "tsconfig.app.json",
    "tsconfig.node.json",
    "eslint.config.js",
    "index.html",
]

SOURCE_DIRS = ["src", "src-tauri/src", "src-tauri/capabilities", "src-tauri/permissions"]

# A reference is anything that would make the product read the prototype:
# an import, a build input, or a script that walks into it.
PATTERNS = [
    r"from\s+[\"'][^\"']*properties",
    r"require\(\s*[\"'][^\"']*properties",
    r"[\"']properties/",
    r"\.\./properties",
]


def config_violations() -> list[str]:
    violations = []
    for name in CONFIGS:
        path = ROOT / name
        if not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        if name == "package-lock.json":
            # The lockfile only matters if a real dependency points at it.
            for pattern in PATTERNS:
                if re.search(pattern, text):
                    violations.append(f"{name}: references the prototype directory")
            continue
        for pattern in PATTERNS:
            if re.search(pattern, text):
                violations.append(f"{name}: {pattern} points at the prototype")
    return violations


def source_violations() -> list[str]:
    violations = []
    for directory in SOURCE_DIRS:
        base = ROOT / directory
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if not path.is_file() or path.suffix not in {".ts", ".tsx", ".rs", ".css", ".json", ".toml"}:
                continue
            text = path.read_text(encoding="utf-8", errors="ignore")
            for pattern in PATTERNS:
                if re.search(pattern, text):
                    violations.append(
                        f"{path.relative_to(ROOT)}: references the prototype directory"
                    )
    return violations


def dist_inputs() -> list[str]:
    """The bundle input must not include the prototype either."""
    config = ROOT / "src-tauri" / "tauri.conf.json"
    violations = []
    if config.is_file():
        data = json.loads(config.read_text(encoding="utf-8"))
        frontend = data.get("build", {}).get("frontendDist", "")
        if "properties" in str(frontend):
            violations.append(f"tauri.conf.json: frontendDist points at {frontend}")
    return violations


def main() -> int:
    violations = config_violations() + source_violations() + dist_inputs()
    if violations:
        print("repo isolation violations:", file=sys.stderr)
        print("\n".join(violations), file=sys.stderr)
        return 1
    scanned = sum(
        1
        for directory in SOURCE_DIRS
        for path in (ROOT / directory).rglob("*")
        if path.is_file()
    ) if all((ROOT / d).is_dir() for d in SOURCE_DIRS) else 0
    print("repo isolation check ok")
    print(f"no build, test or source reference to properties/; {scanned} source files scanned")
    return 0


if __name__ == "__main__":
    sys.exit(main())
