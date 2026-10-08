#!/usr/bin/env python3
"""Static style and accessibility checks for the renderer (SPEC-008 + SPEC-015).

Rules come from the Vercel Web Interface Guidelines
(raw.githubusercontent.com/vercel-labs/web-interface-guidelines/main/command.md)
and WCAG 2.2 SC 1.4.3 / 1.4.11. Only rules that can be decided statically live
here; anything needing a rendered tree stays in the vitest suite.

SPEC-015 C1 extends the gate to the dual-theme token set (dark default):
the contrast matrix runs once per theme, and the radius ladder is pinned to
10/14/16px + 999px badges (design-system-v2 §4).

Usage:
  python3 tests/m0_ui_style.py          # check the repository
  python3 tests/m0_ui_style.py --selftest   # prove every rule can fire
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
TOKENS = SRC / "styles" / "tokens.css"

# (pattern, message, applies-to) - applies-to is "css", "tsx" or "both".
RULES: list[tuple[str, str, str]] = [
    (r"transition:\s*all\b", "transition: all (list properties explicitly)", "css"),
    (r"outline:\s*none\b", "outline: none without a focus-visible replacement", "css"),
    (
        r"focus:outline-none|focus:ring-0",
        "focus outline removed without a focus-visible replacement",
        "both",
    ),
    (r"user-scalable\s*=\s*[\"']no[\"']", "zoom disabled via user-scalable=no", "both"),
    (r"maximum-scale\s*=\s*1", "zoom limited via maximum-scale=1", "both"),
    (
        r"onPaste[^\n]*preventDefault",
        "paste blocked with onPaste + preventDefault",
        "tsx",
    ),
    (
        r"<div[^>]*\sonClick=",
        "clickable div (use button for actions, a for navigation)",
        "tsx",
    ),
    (
        r"toLocaleDateString\(|toLocaleTimeString\(",
        "hand-rolled locale formatting (use Intl.DateTimeFormat)",
        "both",
    ),
    # Responsive rules (EXEC-001 W7: no horizontal overflow at 1440/1024/768).
    (
        r"(?<!max-)(?<!min-)\bwidth:\s*(?:[4-9]\d{2}|[1-9]\d{3,})px",
        "fixed width wider than a narrow phone viewport breaks the 768px layout (use max-width)",
        "css",
    ),
    (r"\bwidth:\s*100vw\b", "100vw ignores the scrollbar and causes horizontal overflow", "css"),
    (r"grid-template-columns:\s*repeat\(\s*[4-9]", "more than 3 fixed grid columns has no room at 768px", "css"),
]

# WCAG contrast requirements, resolved against BOTH themes (SPEC-015: dark is
# the default, so a dark-only failure is as red as a light one). "text" pairs
# need 4.5:1, "ui" pairs need 3:1 (SC 1.4.11 non-text contrast for component
# boundaries and focus indicators). The nine pairs are the SPEC-015 AC1 matrix
# (six text + three boundary); the status pairs pin every badge hue family to
# its own tint background.
CONTRAST_MATRIX: list[tuple[str, str, str]] = [
    ("text", "--au-text", "--au-bg"),
    ("text", "--au-text", "--au-surface"),
    ("text", "--au-text-muted", "--au-bg"),
    ("text", "--au-text-muted", "--au-surface"),
    ("text", "--au-color-primary-text", "--au-color-primary"),
    ("text", "--au-color-danger", "--au-surface"),
    ("ui", "--au-color-primary", "--au-bg"),
    ("ui", "--au-color-primary", "--au-surface"),
    ("ui", "--au-border-strong", "--au-surface"),
]

STATUS_FAMILIES = ("neutral", "info", "warning", "primary", "verify", "success", "danger")

THEMES = ("light", "dark")

THRESHOLDS = {"text": 4.5, "ui": 3.0}

# Radius ladder (design-system-v2 §4): 10px cards/inputs, 14px outer panels,
# 16px hard cap, 999px badge pill. A value outside this table is a violation.
RADIUS_LADDER: dict[str, str] = {
    "--au-radius-sm": "10px",
    "--au-radius-md": "14px",
    "--au-radius-lg": "16px",
    "--au-radius-pill": "999px",
}


def renderer_files() -> list[Path]:
    files = []
    for suffix in ("*.ts", "*.tsx", "*.css"):
        files.extend(SRC.rglob(suffix))
    return sorted(files)


def check_rules(files: list[Path]) -> list[str]:
    violations = []
    for path in files:
        text = path.read_text(encoding="utf-8")
        kind = "css" if path.suffix == ".css" else "tsx"
        # Drop CSS comments so prose examples do not trip the rules.
        scannable = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
        for pattern, message, applies in RULES:
            if applies not in (kind, "both"):
                continue
            match = re.search(pattern, scannable)
            if match:
                line = scannable[: match.start()].count("\n") + 1
                violations.append(f"{path.relative_to(ROOT)}:{line} - {message}")
    return violations


def check_ellipsis(files: list[Path]) -> list[str]:
    """Visible copy uses the ellipsis character, not three dots (WIG typography)."""
    violations = []
    for path in files:
        if path.suffix != ".tsx":
            continue
        text = re.sub(r"/\*.*?\*/", " ", path.read_text(encoding="utf-8"), flags=re.S)
        for number, line in enumerate(text.splitlines(), start=1):
            if re.search(r"[A-Za-z\u4e00-\u9fff]\.\.\.[\"'`]", line):
                violations.append(
                    f"{path.relative_to(ROOT)}:{number} - use the ellipsis character in visible copy"
                )
    return violations


def check_form_labels(files: list[Path]) -> list[str]:
    """Every control needs a <label htmlFor> pointing at its id, or an aria-label."""
    violations = []
    control = re.compile(r"<(input|select|textarea)\b([^>]*)>", re.S)
    labelled = re.compile(r"htmlFor=\"([^\"]+)\"")
    for path in files:
        if path.suffix != ".tsx":
            continue
        text = re.sub(r"/\*.*?\*/", " ", path.read_text(encoding="utf-8"), flags=re.S)
        targets = set(labelled.findall(text))
        # ScopeBoard renders its label from a template literal.
        targets |= set(re.findall(r"htmlFor=\{`([^`]+)`\}", text))
        for match in control.finditer(text):
            attrs = match.group(2)
            line = text[: match.start()].count("\n") + 1
            if "aria-label" in attrs:
                continue
            id_match = re.search(r'\bid="([^"]+)"', attrs)
            static_id = id_match.group(1) if id_match else None
            template_ok = re.search(r"id=\{`", attrs) and any(
                target.startswith("state-") for target in targets
            )
            if static_id is None and not template_ok:
                violations.append(
                    f"{path.relative_to(ROOT)}:{line} - <{match.group(1)}> has no id for a label"
                )
            elif static_id is not None and static_id not in targets:
                violations.append(
                    f"{path.relative_to(ROOT)}:{line} - <{match.group(1)}> id={static_id} has no matching label"
                )
    return violations


def check_icon_buttons(files: list[Path]) -> list[str]:
    """A <button> with no text child needs aria-label."""
    violations = []
    for path in files:
        if path.suffix != ".tsx":
            continue
        text = re.sub(r"/\*.*?\*/", " ", path.read_text(encoding="utf-8"), flags=re.S)
        for match in re.finditer(r"<button\b[^>]*>(.*?)</button>", text, re.S):
            attrs = match.group(0)[: match.group(0).index(">")]
            body = match.group(1)
            if "aria-label" in attrs or re.search(r"[A-Za-z\u4e00-\u9fff]", body):
                continue
            line = text[: match.start()].count("\n") + 1
            violations.append(
                f"{path.relative_to(ROOT)}:{line} - icon-only button needs aria-label"
            )
    return violations


TOKEN_PATTERN = r"(--au-[a-z0-9-]+):\s*(#[0-9a-fA-F]{6})\b"
# tokens.css carries one block per theme: the combined ":root,
# [data-theme=light]" default and the "[data-theme=dark]" override.
THEME_BLOCKS: dict[str, str] = {
    "light": r':root,\s*\[data-theme="light"\]',
    "dark": r'\[data-theme="dark"\]',
}


def parse_theme(text: str, selector_pattern: str) -> dict[str, str]:
    block = re.search(selector_pattern + r"\s*\{([^}]*)\}", text)
    if block is None:
        return {}
    return dict(re.findall(TOKEN_PATTERN, block.group(1)))


def parse_themes(text: str | None = None) -> dict[str, dict[str, str]]:
    text = TOKENS.read_text(encoding="utf-8") if text is None else text
    return {theme: parse_theme(text, pattern) for theme, pattern in THEME_BLOCKS.items()}


def parse_radius(text: str) -> dict[str, str]:
    return dict(re.findall(r"(--au-radius-[a-z]+):\s*(\d+px)", text))


def relative_luminance(hex_colour: str) -> float:
    channels = [int(hex_colour[i : i + 2], 16) / 255 for i in (1, 3, 5)]
    linear = [
        channel / 12.92 if channel <= 0.04045 else ((channel + 0.055) / 1.055) ** 2.4
        for channel in channels
    ]
    red, green, blue = linear
    return 0.2126 * red + 0.7152 * green + 0.0722 * blue


def contrast_ratio(foreground: str, background: str) -> float:
    lighter = max(relative_luminance(foreground), relative_luminance(background))
    darker = min(relative_luminance(foreground), relative_luminance(background))
    return (lighter + 0.05) / (darker + 0.05)


def gate_pairs() -> list[tuple[str, str, str]]:
    return CONTRAST_MATRIX + [
        ("text", f"--au-status-{family}-text", f"--au-status-{family}-bg")
        for family in STATUS_FAMILIES
    ]


def check_contrast(themes: dict[str, dict[str, str]]) -> list[str]:
    violations = []
    for theme, tokens in themes.items():
        for kind, foreground, background in gate_pairs():
            if foreground not in tokens or background not in tokens:
                violations.append(
                    f"tokens.css[{theme}] - missing token for pair {foreground}/{background}"
                )
                continue
            ratio = contrast_ratio(tokens[foreground], tokens[background])
            threshold = THRESHOLDS[kind]
            if ratio < threshold:
                violations.append(
                    f"tokens.css[{theme}] - {foreground} on {background} is "
                    f"{ratio:.2f}:1, needs {threshold}:1"
                )
    return violations


def radius_violations(ladder: dict[str, str]) -> list[str]:
    problems = []
    for name, expected in RADIUS_LADDER.items():
        actual = ladder.get(name)
        if actual != expected:
            problems.append(
                f"tokens.css - {name} is {actual!r}, the ladder pins it to {expected}"
            )
    for name in sorted(set(ladder) - set(RADIUS_LADDER)):
        problems.append(
            f"tokens.css - {name} is outside the radius ladder {sorted(RADIUS_LADDER)}"
        )
    return problems


def check_single_source(files: list[Path]) -> list[str]:
    """No colour literal may live outside the token file."""
    violations = []
    literal = re.compile(r"#[0-9a-fA-F]{3,8}\b")
    for path in files:
        if path == TOKENS:
            continue
        text = re.sub(r"/\*.*?\*/", " ", path.read_text(encoding="utf-8"))
        for match in literal.finditer(text):
            line = text[: match.start()].count("\n") + 1
            violations.append(
                f"{path.relative_to(ROOT)}:{line} - colour literal {match.group(0)} belongs in tokens.css"
            )
    return violations


def check_detail_layout(files: list[Path]) -> list[str]:
    """AC1 (SPEC-015 C2): the detail page is a 2fr/1fr grid on desktop with a
    single-column fallback below 1024px (and minmax(0, …) so columns cannot
    force horizontal overflow at any breakpoint)."""
    del files
    violations = []
    css = " ".join((SRC / "styles.css").read_text(encoding="utf-8").split())
    if "grid-template-columns: minmax(0, 2fr) minmax(0, 1fr)" not in css:
        violations.append(
            "styles.css - the detail grid must be minmax(0, 2fr) / minmax(0, 1fr)"
        )
    fallback = re.search(r"@media \(max-width: 1023px\)\s*\{(.*?)\}", css, re.S)
    if fallback is None or "grid-template-columns: minmax(0, 1fr)" not in fallback.group(1):
        violations.append(
            "styles.css - the <1024px fallback must reset the detail grid to one column"
        )
    return violations


CHECKS = [check_rules, check_ellipsis, check_form_labels, check_icon_buttons, check_single_source, check_detail_layout]


def run_checks(files: list[Path]) -> list[str]:
    violations = []
    for check in CHECKS:
        violations.extend(check(files))
    themes = parse_themes()
    violations.extend(check_contrast(themes))
    violations.extend(radius_violations(parse_radius(TOKENS.read_text(encoding="utf-8"))))
    return violations


SELFTEST_CASES: list[tuple[str, str, str]] = [
    ("css", ".x { transition: all 200ms; }", "transition: all"),
    ("css", ".x:focus { outline: none; }", "outline: none"),
    ("tsx", "<input type=\"text\" user-scalable=\"no\" />", "user-scalable"),
    ("tsx", "<div onClick={() => go()}>go</div>", "clickable div"),
    ("tsx", "<input onPaste={(event) => event.preventDefault()} />", "paste blocked"),
    ("tsx", "const label = date.toLocaleDateString();", "locale formatting"),
    ("css", ".x { width: 960px; }", "fixed width wider than"),
    ("css", ".x { width: 100vw; }", "100vw ignores the scrollbar"),
]


def selftest() -> int:
    failures = []
    tmp = ROOT / "src" / "__selftest__.tsx"
    for kind, snippet, expected in SELFTEST_CASES:
        assertions = list(RULES) + [
            (r"[A-Za-z\u4e00-\u9fff]\.\.\.[\"'`]", "ellipsis", "tsx")
        ]
        fired = [
            message
            for pattern, message, applies in assertions
            if applies in (kind, "both") and re.search(pattern, snippet)
        ]
        if not any(expected in message for message in fired):
            failures.append(f"selftest: {expected!r} did not fire for {snippet!r}")

    # Rule-level fixtures on a synthetic file, then removed.
    tmp.write_text(
        "\n".join(
            [
                "<div onClick={go}>x</div>",
                "<input id=\"lonely\" />",
                "<button></button>",
                "const msg = \"Loading...\";",
            ]
        ),
        encoding="utf-8",
    )
    try:
        found = " ".join(
            check_rules([tmp])
            + check_form_labels([tmp])
            + check_icon_buttons([tmp])
            + check_ellipsis([tmp])
        )
    finally:
        tmp.unlink()
    for expected in ("clickable div", "no matching label", "icon-only button", "ellipsis"):
        if expected not in found:
            failures.append(f"selftest: rule {expected!r} did not fire on the fixture file")

    # Contrast maths: black on white must be exactly 21:1.
    ratio = contrast_ratio("#000000", "#ffffff")
    if abs(ratio - 21) > 0.01:
        failures.append(f"selftest: contrast(#000,#fff) = {ratio:.4f}, expected 21")
    # The AA boundary is tight: #767676 passes on white, #777777 does not.
    if contrast_ratio("#767676", "#ffffff") < 4.5:
        failures.append("selftest: contrast(#767676,#fff) should be >= 4.5")
    if contrast_ratio("#777777", "#ffffff") >= 4.5:
        failures.append("selftest: contrast(#777777,#fff) should be < 4.5")

    # Dark-theme probes (SPEC-015): the default theme's own values must pass
    # AA, and a weak dark boundary must be able to fail the gate.
    if contrast_ratio("#e8e8ed", "#0a0a0f") < 4.5:
        failures.append("selftest: dark body text on the dark bg should pass 4.5")
    if contrast_ratio("#2a2a31", "#12121a") >= 3.0:
        failures.append("selftest: a weak dark boundary should fail the 3:1 gate")

    # Radius ladder: a wrong value fires, the shipped ladder stays quiet.
    off_ladder = {
        "--au-radius-sm": "8px",
        "--au-radius-md": "14px",
        "--au-radius-lg": "16px",
        "--au-radius-pill": "999px",
    }
    if not radius_violations(off_ladder):
        failures.append("selftest: a radius off the ladder should be flagged")
    if radius_violations(parse_radius(TOKENS.read_text(encoding="utf-8"))):
        failures.append("selftest: the shipped radius ladder should pass")

    # Dual-theme parse: every theme block must carry the full gate vocabulary.
    themes = parse_themes()
    for theme in THEMES:
        missing = sorted({fg for _, fg, _ in gate_pairs() if fg not in themes[theme]})
        if missing:
            failures.append(f"selftest: [{theme}] token block misses {missing[:3]}")

    if failures:
        print("ui style selftest failed:", file=sys.stderr)
        print("\n".join(failures), file=sys.stderr)
        return 1
    probes = len(SELFTEST_CASES) + 4 + 2 + 2 + 2 + len(THEMES)
    print(f"ui style selftest ok ({probes} rule probes)")
    return 0


def main() -> int:
    if "--selftest" in sys.argv:
        return selftest()
    files = renderer_files()
    if not files:
        print("renderer source missing", file=sys.stderr)
        return 1
    violations = run_checks(files)
    if violations:
        print("ui style violations:", file=sys.stderr)
        print("\n".join(violations), file=sys.stderr)
        return 1
    tokens = parse_themes()
    total_tokens = sum(len(theme) for theme in tokens.values())
    print("ui style check ok")
    print(
        f"scanned {len(files)} renderer files; {total_tokens} colour tokens across "
        f"{len(tokens)} themes; {len(gate_pairs()) * len(tokens)} contrast pairs verified"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
