#!/usr/bin/env python3
from pathlib import Path
import json, re, sys
root = Path(__file__).resolve().parents[1]
violations = []
cargo = (root / "src-tauri/Cargo.toml").read_text()
if "updater" in cargo.lower():
    violations.append("Cargo.toml mentions updater")
conf = json.loads((root / "src-tauri/tauri.conf.json").read_text())
if conf.get("bundle", {}).get("active") is not True:
    violations.append("bundle.active is not true")
if "updater" in json.dumps(conf).lower():
    violations.append("tauri.conf mentions updater")
pkg = (root / "package.json").read_text()
if "plugin-updater" in pkg:
    violations.append("package.json has plugin-updater")
app = (root / "src/App.tsx").read_text()
if "tabIndex={-" in app or 'tabIndex="-' in app:
    violations.append("negative tabIndex in App.tsx")
if "properties/" in app:
    violations.append("renderer imports properties/")
for needle in ["button", "input"]:
    if needle not in app:
        violations.append(f"App.tsx missing {needle}")
if violations:
    print("pack gate failed:", file=sys.stderr)
    print("\n".join(violations), file=sys.stderr)
    sys.exit(1)
print("pack gate ok")
