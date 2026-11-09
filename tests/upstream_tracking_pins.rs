//! The upstream-tracking pins (IMPROVE09 S2): the one-home table's shape,
//! red-first.
//!
//! RED LEDGER (2026-11-09, against `667592d8` before this round's docs):
//!   1. absence — `docs/UPSTREAM_TRACKING.md` did not exist (the gap the
//!      round closes, tree-verified twice); every test in this file red
//!      with the file-absent failure: "the one home does not exist".
//!   2. (post-landing mutation proof, S2) a row appended WITHOUT a status
//!      and WITHOUT a review-date re-reds `every_row_carries_a_status…`
//!      and `every_row_carries_a_review_date…`, naming the row — the pin
//!      working on the future path. A `declined` row without a reason
//!      reds `…closed_vocabulary` (the `declined+reason` grammar).
//!
//! Green: the table landed (S1's posture row + S2's seed rows) and the
//! pins went green against it in the same round.
//!
//! The laws pinned here (plan 09 §3 UT-3 + the execution prompt):
//!   UT-3  every row of `docs/UPSTREAM_TRACKING.md`'s table carries a
//!         `status` in the CLOSED vocabulary
//!         (tracking | proposed | landed | declined+reason) AND a
//!         `review-date` (YYYY-MM-DD) — no row without either. A future
//!         row missing a field reds these pins by name; that is the pin
//!         working, not a false alarm.
//!   UT-2  (receipt face) every row carries a non-empty `our evidence`
//!         cell — a claim about upstream without our receipt is not a
//!         row. The full UT-2 law (a bump lands ONLY with the battery +
//!         the tier witness table + the dependency diff in the evidence
//!         file) is convention + receipt, not a unit test.
//!   The checklist face — the bump-validation checklist section must be
//!   present in the doc: it IS the CI-orderable checklist any future bump
//!   must re-run; deleting it deletes the measurement gate.

use std::path::{Path, PathBuf};

/// The one home's path, at the repo root (plan 09 §1.4: "at the
/// brain-server repo root — tree-verified absent, the gap this round
/// closes").
fn tracking_doc() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/UPSTREAM_TRACKING.md")
}

/// A row of the tracking table, cells already trimmed.
struct Row {
    item: String,
    evidence: String,
    status: String,
    review_date: String,
    raw: String,
}

/// Read the doc and parse its table (shape (a): the read is in the test
/// helper every test names).
fn parse_table() -> Result<(Vec<String>, Vec<Row>), String> {
    let path = tracking_doc();
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("the one home does not exist: {} — the tracking table is the round's deliverable, never delete it", path.display()))?;
    parse_table_from(&text)
}

/// Parse the markdown table out of the doc text: the header row carries
/// the `status` and `review-date` column names; separator rows are
/// skipped; every remaining pipe row is a tracked row.
fn parse_table_from(text: &str) -> Result<(Vec<String>, Vec<Row>), String> {
    let mut header: Option<Vec<String>> = None;
    let mut rows: Vec<Row> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells.iter().all(|c| c.is_empty() || c.chars().all(|ch| ch == '-' || ch == ':')) {
            continue; // the |---|---| separator
        }
        match &header {
            None => {
                let lower: Vec<String> = cells.iter().map(|c| c.to_lowercase()).collect();
                if lower.iter().any(|c| c == "status") && lower.iter().any(|c| c == "review-date") {
                    header = Some(cells);
                }
                // A pipe row before the header (e.g. a small inline table
                // elsewhere in the prose) is not the tracking table.
            }
            Some(cols) => {
                let idx = |name: &str| -> Option<usize> {
                    cols.iter()
                        .position(|c| c.to_lowercase() == name)
                };
                let cell = |i: Option<usize>| -> String {
                    i.and_then(|i| cells.get(i).cloned()).unwrap_or_default()
                };
                rows.push(Row {
                    item: cell(idx("item")),
                    evidence: cell(idx("our evidence")),
                    status: cell(idx("status")),
                    review_date: cell(idx("review-date")),
                    raw: trimmed.to_string(),
                });
            }
        }
    }
    let cols = header.ok_or_else(|| {
        "no tracking-table header found (a row carrying `status` and `review-date` column names) — the one home lost its table".to_string()
    })?;
    Ok((cols, rows))
}

/// The closed status vocabulary: the cell's HEAD token must be one of
/// `tracking | proposed | landed | declined`; a `declined` row MUST
/// carry its reason (`declined+reason` — `declined — <reason>`); the
/// other heads may carry an annotation after them.
fn status_head_is_closed(status: &str) -> Result<(), String> {
    let s = status.trim();
    if s.is_empty() {
        return Err("empty status".to_string());
    }
    let head = s
        .split(|c: char| c.is_whitespace() || c == '—' || c == '–' || c == '-' || c == ':')
        .find(|t| !t.is_empty())
        .unwrap_or("");
    match head {
        "tracking" | "proposed" | "landed" => Ok(()),
        "declined" => {
            let reason = s[head.len()..].trim().trim_start_matches(['—', '–', '-', ':']).trim();
            if reason.is_empty() {
                Err("declined WITHOUT a reason — the closed vocabulary is `declined+reason`: record why the item is refused".to_string())
            } else {
                Ok(())
            }
        }
        other => Err(format!(
            "`{other}` is outside the closed vocabulary (tracking | proposed | landed | declined+reason)"
        )),
    }
}

/// YYYY-MM-DD, std-only: 4 digits, dash, 2 digits (1-12), dash, 2 digits
/// (1-31). Zero new dependencies — the law holds here too.
fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(|c| c.is_ascii_digit());
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let month: u32 = s[5..7].parse().unwrap_or(0);
    let day: u32 = s[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

// ── the pins ─────────────────────────────────────────────────────────────

#[test]
fn the_one_home_table_exists() {
    // RED (the ledger's shape 1): the file was absent before this round —
    // this test's message IS the gap, by name.
    let (_, rows) = parse_table().expect("the one home must exist with its table");
    assert!(
        !rows.is_empty(),
        "the tracking table parsed but carries zero rows — a table with no rows tracks nothing"
    );
}

#[test]
fn every_row_carries_a_status_in_the_closed_vocabulary() {
    let (_, rows) = parse_table().expect("the one home must exist");
    assert!(!rows.is_empty(), "no rows to pin");
    for row in &rows {
        match status_head_is_closed(&row.status) {
            Ok(()) => {}
            Err(why) => panic!(
                "row `{}` carries status `{}` — {why}\n  at: {}",
                if row.item.is_empty() { "(unnamed)" } else { &row.item },
                row.status,
                row.raw
            ),
        }
    }
}

#[test]
fn every_row_carries_a_review_date() {
    let (_, rows) = parse_table().expect("the one home must exist");
    assert!(!rows.is_empty(), "no rows to pin");
    for row in &rows {
        assert!(
            is_iso_date(row.review_date.trim()),
            "row `{}` carries review-date `{}` — not a YYYY-MM-DD date; every row must say when it was last re-checked (UT-3)\n  at: {}",
            if row.item.is_empty() { "(unnamed)" } else { &row.item },
            row.review_date,
            row.raw
        );
    }
}

#[test]
fn every_row_carries_our_evidence() {
    // UT-2's receipt face: a row that names upstream but cites nothing of
    // ours is a rumor, not a tracked fact.
    let (_, rows) = parse_table().expect("the one home must exist");
    assert!(!rows.is_empty(), "no rows to pin");
    for row in &rows {
        assert!(
            !row.evidence.trim().is_empty(),
            "row `{}` carries no `our evidence` cell — every row cites our receipt (the CG evidence files, the pins, the round evidence)\n  at: {}",
            if row.item.is_empty() { "(unnamed)" } else { &row.item },
            row.raw
        );
    }
}

#[test]
fn the_bump_validation_checklist_is_present() {
    // UT-2's CI-orderable face: the bump-validation checklist section is
    // what any future mlx-swift-lm bump MUST re-run (the CG-11 battery,
    // the C-6 re-probe, the MTP measurement against the 40% gate, the
    // streaming witness, the dependency diff). Deleting the section
    // deletes the measurement gate — this pin makes that loud.
    let path = tracking_doc();
    let text = std::fs::read_to_string(&path)
        .map_err(|_| format!("the one home does not exist: {}", path.display()))
        .expect("the one home must exist");
    assert!(
        text.to_lowercase().contains("bump-validation"),
        "docs/UPSTREAM_TRACKING.md lost its bump-validation checklist — the checklist IS the gate any bump re-runs (UT-2); restore it"
    );
}

#[test]
fn a_declined_row_must_carry_its_reason() {
    // The `declined+reason` grammar, directly: a declined cell without a
    // reason is refused. Proven on the real table only if a declined row
    // exists today; the grammar function is exercised on synthetic cells
    // so the law is pinned regardless of the table's live posture.
    assert!(status_head_is_closed("declined — moved to the v3 boundary act").is_ok());
    assert!(status_head_is_closed("declined").is_err(), "bare `declined` must red");
    assert!(status_head_is_closed("declined ").is_err(), "whitespace-only reason must red");
    assert!(status_head_is_closed("tracking").is_ok());
    assert!(status_head_is_closed("proposed — draft filed by the operator").is_ok());
    assert!(status_head_is_closed("landed").is_ok());
    assert!(status_head_is_closed("promoted").is_err(), "outside the vocabulary");
    assert!(status_head_is_closed("").is_err(), "empty status must red");
}
