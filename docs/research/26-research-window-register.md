# 26 · The Research Window Register

**Date:** 2026-11-26 · **Status:** ACTIVE · **Owner:** StewardMem research
**Policy:** `brain-steward-ip/docs/RESEARCH_WINDOW_POLICY.md` (the rule this file operationalizes)
**Inputs:** [18 · The Cited Work](./18-cited-work.md) and [19 · The Standards Register](./19-standards-register.md) answer *what is cited*. This file answers *what we decided about it, and why*.

---

## What this file is

This is a **decision log, not a bibliography.** One row per paper or standard, one decision per row, one reason per decision. It exists so that a later session can answer four questions about any source the product cites without leaving this file: **which layer is it in, what did we decide, why, and when was it last verified.**

Three laws govern every row:

1. **A refused row with a written reason is a success.** An adopted row with no reason is a liability.
2. **Every date, ID, article number, and designation is verified, never recalled.** Preprints are labelled in the title cell.
3. **Refusals are recorded, not deleted.** Re-opening a refused paper needs a named trigger: a new version that changes a result, a contradicting benchmark, or a shipped attack.

A row without a reason is a defect, not a formatting choice.

---

## The schema

Every row carries the nine columns policy §6 lists (the prompt calls it "eight-field"; the enumerated list has nine, and this file implements the list, not the count):

| Field | Meaning |
|---|---|
| `id` | arXiv ID or canonical URL |
| `title` | Short title; `(preprint)` appended when the source is not peer-reviewed |
| `date` | First submission or publication date, `YYYY-MM-DD` |
| `layer` | `adapt` / `verify` / `re-verify` / `ignore` |
| `status` | `adopted` / `triaged` / `refused` / `parked` |
| `mechanism` | The one mechanism the source bears on |
| `claim` | The StewardMem claim or feature it touches |
| `decision` | `build` / `test` / `cite` / `refuse` / `park` |
| `reason` | The one-line justification, written so it does not get re-litigated |

**Layer meanings** (policy §3): `adapt` = ≤ 6 months old and changes a shipped claim or fills a named gap; `verify` = older but a reference implementation, benchmark, or standard, re-checked against the tree; `re-verify` = an older source contradicted by a newer one; `ignore` = older and off-topic, or newer and unreplicated or off-scope.

**Decision meanings:** `build` = a plan item exists; `test` = an eval or probe, no product change; `cite` = referenced in a note or plan, not built; `refuse` = deliberately not integrated; `park` = a named trigger would re-open it.

---

## Cadence

The four commands and the failure rule are recorded in §Cadence (R5). Until then, the policy's §7 is authoritative.

---

## 1 · Active window (2026-06-01 → 2026-11-26)

Backfilled from the 180-day sweep of 2026-11-26 (all eight lanes, `--max-per-lane 25`, no `--mark-seen`) and reconciled against notes 20–25 and the two plans. The sweep command, the counts, and the exclusion list are in §Evidence. Papers already adopted get an `adapt / adopted` row that links to the plan; papers a prior note decided get a row that links to the note.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|

## 2 · Foundation floor

Reference implementations, benchmarks, and standards that are older than the adaptation window. These are **verified against the tree, not re-adopted.** Every `verify` row names a tree anchor (`path:line` or a test name) and a check date; a row with no anchor is `verify / gap` and becomes a plan item.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|

## 3 · Standards / regulation

Continuous, not date-bounded. Every row carries a canonical URL and `verified YYYY-MM-DD`. A moved designation is corrected in [19](./19-standards-register.md) and recorded here as `re-verify / corrected`, naming old → new.

| id | title | date | layer | status | mechanism | claim | decision | reason |
|---|---|---|---|---|---|---|---|---|

## Evidence

The sweep command and date, the raw counts, the reconciliation arithmetic, the exclusion list with reasons, and the foundation gaps. Written in R7.
