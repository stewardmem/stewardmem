#!/usr/bin/env python3
"""Check the research-window register (docs/research/26-research-window-register.md).

Pins implemented (the mechanical ones from plans/research-window §2):
  1  no_register_row_without_a_reason
  2  no_adapt_row_older_than_six_months_without_an_exception
  3  every_foundation_row_names_a_tree_anchor
  4  every_standards_row_has_a_verified_date_and_url
  7  every_active_window_paper_is_adopted_or_explicitly_refused  (with --sweep)
  8  anti-vacuity: a pass over an empty register is a failure

Pins 5 (seen-state) and 6 (link-not-copy) are drills, not table checks; their
receipts live in the register's Evidence section.

Run from anywhere: python3 scripts/check-research-register.py
Add --sweep <report.md> to reconcile a sweep report against the register.
"""
import argparse
import pathlib
import re
import sys

REGISTER = pathlib.Path(__file__).resolve().parent.parent / "docs" / "research" / "26-research-window-register.md"
ID = re.compile(r"\b\d{4}\.\d{4,5}\b")
ISO_DATE = re.compile(r"\b(\d{4}-\d{2}-\d{2})\b")
ANCHOR = re.compile(r"\.rs(:\d+)?\b|tests?/|\btest\b|::[a-z_]+|\b[a-z_]+\.rs\b|\bgap:")
SECTION = re.compile(r"^## (\d) · ")


def rows_of(text, section):
    """Yield (line_no, cells) for data rows in the numbered section."""
    current = None
    for n, line in enumerate(text.splitlines(), 1):
        m = SECTION.match(line)
        if m:
            current = m.group(1)
            continue
        if line.startswith("## "):
            current = None
            continue
        if current != str(section) or not line.startswith("|"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if not cells or set("".join(cells)) <= set("-: "):
            continue
        if cells[0].lower() == "id":
            continue
        if len(cells) != 9:
            yield n, None, cells
            continue
        yield n, cells, None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sweep", help="path to a sweep report .md to reconcile")
    args = ap.parse_args()

    text = REGISTER.read_text(encoding="utf-8")
    failures = []
    counts = {}

    for sec, name in ((1, "active"), (2, "foundation"), (3, "standards")):
        n_rows = 0
        for line_no, cells, malformed in rows_of(text, sec):
            if malformed is not None:
                failures.append(f"{name} row at line {line_no}: {len(malformed)} cells, expected 9")
                continue
            n_rows += 1
            ident, title, date, layer, status, mechanism, claim, decision, reason = cells

            # pin 1 — reason present and not a placeholder
            if reason.strip().lower() in {"", "tbd", "-", "n/a", "none"}:
                failures.append(f"{name} row {ident}: empty/placeholder reason")

            # pin 2 — adapt rows inside the window, or an explicit exception
            if layer == "adapt":
                m = ISO_DATE.search(date)
                if not m:
                    failures.append(f"{name} row {ident}: adapt row has no ISO date")
                elif m.group(1) < "2026-06-01" and "exception:" not in reason.lower():
                    failures.append(f"{name} row {ident}: adapt row dated {m.group(1)} with no exception:")

            # pin 3 — verify rows name a resolvable-looking tree anchor
            if layer == "verify" and not ANCHOR.search(mechanism):
                failures.append(f"{name} row {ident}: verify row mechanism has no tree anchor ({mechanism!r})")

            # pin 4 — standards rows carry a verified date and a canonical URL
            if sec == 3:
                if not re.search(r"verified \d{4}-\d{2}-\d{2}", reason):
                    failures.append(f"{name} row {ident}: no 'verified YYYY-MM-DD'")
                if "https://" not in reason and "https://" not in mechanism:
                    failures.append(f"{name} row {ident}: no https:// canonical URL")
        counts[name] = n_rows

    # pin 8 — anti-vacuity
    if sum(counts.values()) == 0:
        failures.append("anti-vacuity: the register has zero rows")

    # pin 7 — reconciliation
    if args.sweep:
        swept = set(ID.findall(pathlib.Path(args.sweep).read_text(encoding="utf-8")))
        registered = set()
        for sec in (1, 2, 3):
            for _line, cells, _bad in rows_of(text, sec):
                if cells:
                    registered |= set(ID.findall(cells[0]))
        exclusions = set()
        for line in text.splitlines():
            if line.startswith("- ") and " — " in line:
                exclusions |= set(ID.findall(line.split(" — ", 1)[0]))
        if not swept:
            failures.append("anti-vacuity: the sweep report has zero ids")
        missing = sorted(swept - registered - exclusions)
        extra = sorted(exclusions - (swept - registered))
        if missing:
            failures.append(f"reconciliation: {len(missing)} swept ids neither registered nor excluded: {missing}")
        if extra:
            failures.append(f"reconciliation: {len(extra)} exclusion ids that are not swept-minus-register: {extra}")
        counts["swept"] = len(swept)
        counts["excluded"] = len(exclusions)

    print("register counts: " + ", ".join(f"{k}={v}" for k, v in counts.items()))
    if failures:
        print(f"\nFAIL ({len(failures)}):")
        for f in failures:
            print("  " + f)
        return 1
    print("all pins hold")
    return 0


if __name__ == "__main__":
    sys.exit(main())
