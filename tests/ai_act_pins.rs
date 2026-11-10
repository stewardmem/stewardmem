//! The AI-Act pins (IMPROVE12): the Act's transparency duty is already law
//! (general application 2026-08-02 passed) and the Digital Omnibus moved the
//! rest — so the corrected calendar is code, the registers that record the
//! scoping decision cite their own evidence, and the one disclosure gap the
//! obligation register names is a CI-asserted deliverable the way the CRA
//! row has been since September.
//!
//! RED LEDGER (2026-11-10, before S4's shell keys):
//!   1. the transparency arm — `chat.disclosure.ai_interaction` does not
//!      exist in any catalog and no page consumes it (tree-verified: the
//!      chat vocabulary is per-answer provenance, en.json:418/:429-433, not
//!      interaction disclosure). Both transparency tests red with the named
//!      deliverable while the kernel's service-level notice is already
//!      live — the duty was real, its plain-language surface was not.
//!   2. AA-1 is red-first BY ABSENCE for this file: no pins binary asserted
//!      the registers' citation discipline before this one existed. Its
//!      non-vacuity rides the planted-row arm below (a checker that
//!      cannot red is the one defect a structural scan must never have).
//!
//! The laws pinned here (plan 12 §3 + the execution prompt's gating laws):
//!   AA-1  every obligation-register row cites an artifact (file:line) or
//!         names `gap:` + `owner:` + `date:` — no uncited compliance claims;
//!         the scoping register's commit precedes the register (the git
//!         order, checkable).
//!   AA-2  the AI-Act deadlines are `deadline()` consts carrying sources,
//!         and the transparency deliverable is asserted NOW (the date has
//!         passed — the CRA mechanism with no watch phase left for this
//!         arm); the deferral rows (Annex III / Annex I) are WATCH rows by
//!         construction: calendar records, never duty claims.
//!   The lockstep law: a date re-mapped in code without the register (or
//!         vice versa) fails here — the stamped-application-date discipline
//!         generalized to the whole corrected calendar.

use std::path::Path;
use std::process::Command;

/// Read a repo file relative to the manifest dir (the reg_watch `doc()`
/// helper's integration-test twin — the pins read the tree, not copies).
fn doc(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} must exist and be readable: {e}"))
}

/// Parse `const NAME: (i64, u32, u32) = deadline(y, m, d);` out of the
/// reg_watch source — the calendar-as-code rows are asserted from their
/// REAL definition, so a const renamed or re-shaped reds here by absence.
fn deadline_const(src: &str, name: &str) -> (i64, u32, u32) {
    let needle = format!("const {name}: (i64, u32, u32) = deadline(");
    let at = src.find(&needle).unwrap_or_else(|| {
        panic!(
            "src/reg_watch.rs must define `{name}` as a deadline() const — \
             the calendar-as-code row is missing or was re-shaped without \
             this pin"
        )
    });
    let tail = &src[at + needle.len()..];
    let end = tail
        .find(')')
        .unwrap_or_else(|| panic!("`{name}`'s deadline( tuple is unterminated"));
    let nums: Vec<i64> = tail[..end]
        .split(',')
        .map(|p| {
            p.trim()
                .parse()
                .unwrap_or_else(|e| panic!("`{name}` carries a non-numeric deadline field: {e}"))
        })
        .collect();
    assert_eq!(
        nums.len(),
        3,
        "`{name}`'s deadline( tuple must be (year, month, day)"
    );
    (nums[0], nums[1] as u32, nums[2] as u32)
}

/// The doc-comment block directly above a const (the contiguous `///` lines)
/// — the source-cite half of the calendar-as-code law is asserted from the
/// const's OWN block, so a cite added elsewhere in the file cannot satisfy
/// it by accident.
fn const_doc_block(src: &str, name: &str) -> String {
    let needle = format!("const {name}: (i64, u32, u32) = deadline(");
    let at = src
        .find(&needle)
        .unwrap_or_else(|| panic!("src/reg_watch.rs must define `{name}`"));
    let before = &src[..at];
    let lines: Vec<&str> = before.lines().collect();
    let mut block = Vec::new();
    for line in lines.iter().rev() {
        if line.trim_start().starts_with("///") {
            block.push(*line);
        } else if !line.trim().is_empty() || !block.is_empty() {
            break;
        }
    }
    block.reverse();
    block.join("\n")
}

/// A backticked span names a tree artifact: a repo-relative path under the
/// scanned trees, a root doc, or the threat model. CODE spans (.rs/.json/
/// .svelte/.ts/.sh) must carry a line number (file:line — the plan's law);
/// whole-file artifacts (.md registers, the .xcprivacy manifest) name the
/// file, and a section or line anchor is welcome but not required.
fn is_tree_cite(span: &str) -> bool {
    let prefixes = [
        "src/",
        "docs/",
        "shell/",
        "tests/",
        "scripts/",
        "THREAT_MODEL.md",
        "COMPLIANCE.md",
        "README.md",
    ];
    if !prefixes.iter().any(|p| span.starts_with(p)) || !span.contains('.') {
        return false;
    }
    let is_code = [".rs", ".json", ".svelte", ".ts", ".sh"]
        .iter()
        .any(|ext| span.contains(ext));
    if !is_code {
        return true;
    }
    // some ":digit" must follow a colon
    let mut chars = span.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' && chars.peek().is_some_and(|d| d.is_ascii_digit()) {
            return true;
        }
    }
    false
}

/// The AA-1 scan: every obligation-table row and evidence-base bullet in
/// the register carries a backticked tree cite — or, for a table row, the
/// `gap:` + `owner:` + `date:` triple. Split out so the control below can
/// run the SAME scan over planted text (a checker that cannot fail is the
/// one defect a structural scan must never have).
fn uncited_register_rows(body: &str) -> Vec<String> {
    let mut offending = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let table_row = trimmed.starts_with('|')
            && !trimmed.starts_with("|---")
            && !trimmed.starts_with("| Article");
        let evidence_row = trimmed.starts_with("- **");
        if !table_row && !evidence_row {
            continue;
        }
        let cited = line
            .split('`')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .any(|(_, span)| is_tree_cite(span));
        let tripled =
            line.contains("`gap:`") && line.contains("`owner:`") && line.contains("`date:`");
        let ok = if table_row { cited || tripled } else { cited };
        if !ok {
            let label: String = trimmed.chars().take(72).collect();
            offending.push(label);
        }
    }
    offending
}

/// AA-2, the calendar half: the four AI-Act deadline consts exist, parse,
/// carry the CORRECTED Omnibus calendar, and stay ordered — and each new
/// const's own doc block carries its source (the Service Desk timeline for
/// the deferral rows). The transparency + transitional rows were already
/// pinned (`AI_ACT_APPLICATION`, `AI_ACT_ART50_MARKING`) — this pin holds
/// the whole four-date calendar in one frame so a stale-date revert anywhere
/// reds beside its siblings.
#[test]
fn ai_act_deadline_consts_carry_the_corrected_omnibus_calendar() {
    let src = doc("src/reg_watch.rs");
    let application = deadline_const(&src, "AI_ACT_APPLICATION");
    let marking = deadline_const(&src, "AI_ACT_ART50_MARKING");
    let annex_iii = deadline_const(&src, "AI_ACT_HIGH_RISK_ANNEX_III");
    let annex_i = deadline_const(&src, "AI_ACT_ANNEX_I_EMBEDDED");
    assert_eq!(
        application,
        (2026, 8, 2),
        "general application (Art 113) is 2026-08-02 — re-mapping it requires \
         a source URL in the same change"
    );
    assert_eq!(
        marking,
        (2026, 12, 2),
        "the Art 50(2) transitional end is 2026-12-02 (reg (EU) 2026/1744 \
         Art 111(4)) — re-mapping it requires a source URL in the same change"
    );
    assert_eq!(
        annex_iii,
        (2027, 12, 2),
        "the Annex III deferral is 2027-12-02 (the Digital Omnibus timetable, \
         Service Desk timeline probed 200 2026-11-10) — a register pinned to \
         the stale phase-in is the fiction this lane refuses; re-mapping it \
         requires a source URL in the same change"
    );
    assert_eq!(
        annex_i,
        (2028, 8, 2),
        "Annex I (embedded high-risk) is 2028-08-02 — re-mapping it requires \
         a source URL in the same change"
    );
    assert!(
        application < marking && marking < annex_iii && annex_iii < annex_i,
        "the four-date calendar must stay ordered {application:?} < {marking:?} \
         < {annex_iii:?} < {annex_i:?} — an inversion is a transcription error"
    );
    let doc_iii = const_doc_block(&src, "AI_ACT_HIGH_RISK_ANNEX_III");
    assert!(
        doc_iii.contains("ai-act-service-desk.ec.europa.eu") && doc_iii.contains("DEFERRED"),
        "the Annex III const's own doc block must carry its source (the \
         Service Desk timeline) and name the deferral — a calendar row \
         without its source is exactly the uncited claim AA-1 kills in the \
         registers"
    );
    let doc_i = const_doc_block(&src, "AI_ACT_ANNEX_I_EMBEDDED");
    assert!(
        doc_i.contains("Service Desk") && doc_i.contains("2028"),
        "the Annex I const's own doc block must name its source (the Service \
         Desk timeline) and the date"
    );
    // The stale "tracked in docs, not in code" provenance line carries its
    // dated correction — the sentence that WAS true when written must not
    // silently become a lie when the consts landed.
    let marking_block = const_doc_block(&src, "AI_ACT_ART50_MARKING");
    assert!(
        marking_block.contains("CORRECTED 2026-11-10"),
        "the Art 50 marking doc block must keep the dated correction noting \
         the deferral horizons moved INTO code — provenance edits are \
         appended, never erased"
    );
}

/// AA-2, the transparency arm — the deliverable asserted NOW, because the
/// date has passed: general application (2026-08-02) put the Art 50 duties
/// in force three months before this pin landed, so there is no watch phase
/// left. The CRA mechanism, live-duty form: the plain-language
/// AI-interaction key exists in EVERY catalog and the chat page consumes it.
/// RED at S3 (transcript recorded) — greens when S4 lands the keys and the
/// consumption.
#[test]
fn ai_act_transparency_deliverable_is_declared() {
    const KEY: &str = "chat.disclosure.ai_interaction";
    let en = doc("shell/src/lib/i18n/en.json");
    assert!(
        en.contains(&format!("\"{KEY}\"")),
        "the Art 50(1) plain-language deliverable is due NOW — the duty is in \
         force (general application 2026-08-02 passed), the obligation \
         register names this gap with an owner and a date, and the shell's \
         per-answer tier vocabulary is provenance, not interaction \
         disclosure; land `{KEY}` in en.json beside that vocabulary"
    );
    for locale in ["de", "fr", "es", "nl"] {
        let raw = doc(&format!("shell/src/lib/i18n/{locale}.json"));
        assert!(
            raw.contains(&format!("\"{KEY}\"")),
            "locale {locale} is missing `{KEY}` — paraglide falls back to the \
             BASE LOCALE for missing keys, so a absent non-en key compiles \
             green and silently serves English (the AA-3 fallback trap); the \
             disclosure exists in all five catalogs or it does not exist"
        );
    }
    let page = doc("shell/src/routes/+page.svelte");
    assert!(
        page.contains("m['chat.disclosure.ai_interaction']"),
        "the chat page must CONSUME the disclosure key — a key no page \
         renders is a register fiction, not a disclosure"
    );
}

/// The transparency closure's append-only receipt: the obligation register
/// carries a DATED closure appendix naming the landed key and recording the
/// privacy-manifest review verdict. The gap row stays; the closure appends
/// (the append-only law — corrections are dated rows, never rewrites).
/// RED at S3 beside the deliverable arm — greens with S4's append.
#[test]
fn ai_act_transparency_closure_is_recorded_in_the_register() {
    let register = doc("docs/AI_ACT_OBLIGATION_REGISTER_2026-11-10.md");
    assert!(
        register.contains("### 2026-11-10 — S4 closure"),
        "the transparency closure must land as a dated append to the \
         obligation register (the append-only law) — the gap row stays, the \
         closure row cites what landed"
    );
    assert!(
        register.contains("chat.disclosure.ai_interaction"),
        "the closure row must cite the landed key by name — a closure that \
         names nothing closes nothing"
    );
    assert!(
        register.contains("PrivacyInfo.xcprivacy"),
        "the privacy-manifest review verdict must be recorded in the \
         register (the S4 acceptance: reviewed, and edited only if a gap \
         was named)"
    );
}

/// AA-1, the citation scan: every obligation-table row and evidence-base
/// bullet in the register cites an artifact (file:line; whole-file
/// artifacts name the file) or names `gap:` + `owner:` + `date:` — no
/// uncited compliance claims. Non-vacuity: the planted-row arm below proves
#[test]
fn ai_act_obligation_register_rows_all_cite() {
    let register = doc("docs/AI_ACT_OBLIGATION_REGISTER_2026-11-10.md");
    let offending = uncited_register_rows(&register);
    assert!(
        offending.is_empty(),
        "AA-1: every obligation row must cite an artifact (file:line) or \
         name gap+owner+date — uncited rows: {offending:?}"
    );
}

/// The control that keeps the AA-1 scan honest: it MUST red a planted
/// uncited table row and a planted uncited evidence bullet, MUST pass a
/// fully cited row, and MUST pass a legal gap row (gap+owner+date is the
/// honest form — the scan refuses uncited claims, not gaps).
#[test]
fn control_ai_act_citation_scan_reds_on_a_planted_uncited_row() {
    let planted_table_row = "| Art 99 | a fabricated obligation with no evidence behind it | nothing | clean | nowhere |\n";
    let offending = uncited_register_rows(planted_table_row);
    assert_eq!(
        offending.len(),
        1,
        "the scan must red the planted uncited table row — an empty result \
         means the checker cannot fail (the one defect a structural scan \
         must never have)"
    );
    let planted_bullet = "- **Orphan**: claims evidence without a cite\n";
    assert_eq!(
        uncited_register_rows(planted_bullet).len(),
        1,
        "an evidence bullet without a tree cite must red — the evidence base \
         gets the same law as the obligations table"
    );
    let cited_row = "| Art 4 | literacy | `docs/AI_LITERACY.md` + `src/handlers/well_known.rs:91` | satisfied | EUR-Lex Art 4 |\n";
    assert!(
        uncited_register_rows(cited_row).is_empty(),
        "a fully cited row must pass — the control proves the checker \
         discriminates rather than reds everything"
    );
    let gap_row = "| Art 51 | a real, named gap | — | `gap:` named here | `owner:` operator `date:` 2026-01-01 |\n";
    assert!(
        uncited_register_rows(gap_row).is_empty(),
        "a gap+owner+date row must pass — gap rows are the honest form; \
         uncited claims are the defect"
    );
    let code_span_without_line =
        "| Art 52 | bad cite | `src/handlers/compliance.rs` (no line) | satisfied | — |\n";
    assert_eq!(
        uncited_register_rows(code_span_without_line).len(),
        1,
        "a CODE cite without its line number is not a cite — file:line is \
         the plan's law for code artifacts"
    );
}

/// AA-1, the git-order half: the scoping register's ADD commit precedes the
/// obligation register's — the operator's decision is recorded before any
/// obligation row exists to cite it (the AA-1 git order, checkable in the
/// checkout; the no_engagement_name precedent for git-in-test).
#[test]
fn ai_act_scoping_register_commit_precedes_the_obligation_register() {
    let first_add = |rel: &str| -> i64 {
        let out = Command::new("git")
            .args(["log", "--diff-filter=A", "--format=%ct", "--", rel])
            .output()
            .expect("git log must run — the AA-1 git order is checkable only in a checkout");
        assert!(
            out.status.success(),
            "git log failed for {rel} — the pin cannot verify the order, so \
             it must not report green"
        );
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .last()
            .filter(|l| !l.trim().is_empty())
            .unwrap_or_else(|| {
                panic!(
                    "{rel} has no add-commit — the register must be tracked \
                     for the git order to be checkable"
                )
            })
            .trim()
            .parse()
            .unwrap_or_else(|e| panic!("{rel}'s add-commit epoch is not a number: {e}"))
    };
    let scoping = first_add("docs/AI_ACT_SCOPING_REGISTER_2026-11-10.md");
    let obligations = first_add("docs/AI_ACT_OBLIGATION_REGISTER_2026-11-10.md");
    assert!(
        scoping <= obligations,
        "the scoping register must be committed FIRST (the AA-1 git order): \
         scoping added @{scoping}, obligations added @{obligations} — the \
         operator's decision precedes the rows that cite it"
    );
}

/// The lockstep law: the corrected calendar appears in the REGISTER the
/// code names, and the deferral horizons appear in the compliance map — a
/// date re-mapped in code without the record (or vice versa) fails here
/// (the CRA stamped-application-date discipline, generalized to the whole
/// four-date calendar).
#[test]
fn ai_act_corrected_dates_appear_in_the_records_the_code_names() {
    let scoping = doc("docs/AI_ACT_SCOPING_REGISTER_2026-11-10.md");
    for date in [
        "2025-08-02",
        "2026-08-02",
        "2026-12-02",
        "2027-12-02",
        "2028-08-02",
    ] {
        assert!(
            scoping.contains(date),
            "the scoping register lost {date} — the calendar table is the \
             record the deadline consts name; the code and the record move \
             together or not at all"
        );
    }
    assert!(
        scoping.contains("ai-act-service-desk.ec.europa.eu"),
        "the scoping register must carry the calendar's official source (the \
         Service Desk timeline) — per-row sources are the whole method"
    );
    let compliance = doc("docs/compliance.md");
    assert!(
        compliance.contains("2027-12-02") && compliance.contains("2028-08-02"),
        "docs/compliance.md must carry the Omnibus deployer horizons \
         (2027-12-02 / 2028-08-02) — the deadline consts and the compliance \
         map are one calendar; either side moving alone is the drift the \
         lockstep kills"
    );
}
