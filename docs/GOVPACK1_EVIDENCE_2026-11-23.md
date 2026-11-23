# GOVPACK-1 · The Policy-Anchor Registry (K-31) — Round Evidence

**Date:** 2026-11-23 · **Base:** brain-server `348049af` (tag `v1.29.47`) ·
**Plan:** `plans/GOVPACK_EXECUTION_PLAN_K31_K30_2026-11-23.md` gates **A1 + A2 + A4(CLI
subset)** · **Prompt:** `GOVPACK_EXECUTION_PROMPT_K31_2026-11-23.md`

## §0 · The gate

All eleven checks passed on the first run (HEAD `348049af`; manifest present, valid JSON,
12 anchors; both held instrument hashes matched the manifest's measured values;
`AuditKind` at `src/audit/mod.rs:98`; `no_sql_in_handlers_enforced` at
`src/service/mod.rs:323`; the governance-hash migration shape at :3690; the
`verify-governance` CLI precedent at :200; zero hits for `policy_anchor`/`PolicyAnchor`
in kernel src). No hard stop. The gate was re-run at the wrap; `git status` shows no
openapi.yaml or shell/ change — the wire is untouched.

## S0 · The fixture

`tests/fixtures/policy_anchors.json` — byte-identical copy of
`brain-steward-ip/modules/gov/ph-ai-policies/ANCHORS.json` (`diff` clean). Hashes
re-measured live:

```
18263a9a6f426c76479b32a3c3807d919acd0d40f1b64cad8badf9c96144202b  01-deped-order-003-s-2026/DO_s2026_003r-1.pdf
0a4536a74b891cd2dc7f20bffa736a321ad8f9f33678e02a73874735b1bfe01f  02-ched-cmo-21-s-2026/2026-CMO-21_Framework-on-AI-in-Higher-Education.pdf
```

Both match the manifest — no drift, no re-measure owed.

## The live CLI receipts (real binary, real fixture, real corpus)

```
$ brain policy-anchor seed --manifest tests/fixtures/policy_anchors.json \
      --corpus-root ~/Sites/brain-steward-ip/modules/gov/ph-ai-policies --db /tmp/govpack-scratch/scratch.db
  …12 rows printed…
  inserted 12 · skipped 0 · refused 0 (state counts: 2 held / 10 fetch-owed)     exit 0

$ brain policy-anchor seed … (re-run)
  inserted 0 · skipped 12 · refused 0 (state counts: 2 held / 10 fetch-owed)     exit 0

$ brain policy-anchor verify --db … --corpus-root …
  held text: 2/2 verified against the corpus bytes                               exit 0

$ brain policy-anchor verify --db … --corpus-root <tampered copy>
  DRIFT PH-DEPED-DO-003-2026 : …/DO_s2026_003r-1.pdf hashes to 5c0146db…449a4,
  the registry records 18263a9a…202b                                            exit 1

$ brain policy-anchor gate --pack gov.education --db …        (knob unset)
  refused: no pack bindings configured for pack 'gov.education' —
  BRAIN_POLICY_PACK_REQUIRED_ANCHORS is unset or does not name it; no bindings
  is never everything-allowed                                                    exit 2

$ brain policy-anchor gate --pack gov.education --db …
  (knob = [{"pack":"gov.education","anchors":["PH-DEPED-DO-003-2026","PH-CHED-CMO-21-2026"]}])
  pack gate for 'gov.education' — verified: every bound anchor is held, in force, and live
    ✓ PH-DEPED-DO-003-2026  verified (dated 2026-02-20)
    ✓ PH-CHED-CMO-21-2026   verified (dated 2026-07-21)                            exit 0

$ brain policy-anchor gate --pack gov.education --db …        (+ PH-EO-131-2026, fetch-owed)
  pack gate for 'gov.education' — refused
    ✓ PH-DEPED-DO-003-2026  verified
    ✓ PH-CHED-CMO-21-2026   verified
    ✗ PH-EO-131-2026        advisory-only — anchor text not held                 exit 2

$ brain policy-anchor verify --db … --as-of 2026-06-30
  policy-anchor registry as of 2026-06-30 — 8 anchor(s) in force                 exit 0
```

Binding record after the two gate acts: `gov.education` → 3 rows in
`policy_pack_binding`; audit rows: 4 (`policy-anchor` kind — one per seed/reseed/gate
act). The as-of receipt also proves the month-precision law: `2026-06` (PCW MC
2026-03, issued month-precision) compares `<= 2026-06-30` and is counted in force.

**NAMED ACT, the receipt's date label (2026-11-23, found by checking the blog post
against the instruments' own text):** the first receipts printed
`in force from 2026-02-20` / `in force from 2026-07-21`. Both held orders say
they take effect "fifteen (15) days after its publication … and filing" and give
NO calendar date for that (DepEd §XII.2; CHED Art X §31). The registry stores the
signing date (the date the issuer STATED) in `in_force_from`, which is its
documented law, but the printed label claimed a calendar effectivity the text
does not fix. The label is now `dated`, in both print sites, with the law stated
at the shared printer. No pin asserted the old label. The receipts above are
re-captured from the real binary after the change.

## S5 · The pins — every drill RUN, RED observed, REVERTED from the round's own copy

The round's own copies were taken in `/tmp/govpack-drills/` before the first plant
(the R118 lesson — a guard in an edited file is not restored by git checkout; here the
restore source is the round's own byte-copy, and every source file was diff-verified
byte-identical after the drills). Pins green on the committed code; each drill planted
the defect, observed its red, and was reverted to green:

| # | Pin | Plant | Observed red |
|---|---|---|---|
| 1 | `a_superseded_anchor_is_never_in_force_as_of_a_quarter_boundary` | the superseded row's `in_force_until` stamp removed (DB-level plant — the partial unique index makes a stamp-less supersede unlandable, so the drill plants the defect state any future writer could leave) | `assertion left == right failed: left: ["PH-QTR", "PH-QTR"] right: ["PH-QTR"]` — the old version lingers in the as-of answer forever |
| 2 | `an_anchor_whose_text_hash_drifts_refuses_to_seed_by_name` | the hash comparison skipped (`if false && …`) | `a drifted held text must refuse the whole manifest: SeedPlan { … rows: [PlannedAnchor { anchor_id: "PH-TEST-HELD" …` — the prompt's `got Ok(SeedPlan)` shape, the drift sailing in |
| 3 | `a_seed_is_idempotent_and_never_edits_history_in_place` | the replay arm UPDATEs the row in place | `assertion left == right failed: the replay rewrote NOTHING … right: [… "Test instrument PH-IDEM (edited in place)" …]` |
| 4 | `the_pack_gate_refuses_and_names_every_blocker` | `blockers.truncate(1)` before the verdict | `assertion left == right failed: EVERY blocker is named, never just the first: [GateBlocker { anchor_id: "PH-GATE-OWED-2", reason: TextNotHeld }]  left: 1  right: 3` |
| 5 | `the_supersede_is_one_transaction_or_nothing` | the supersede split into two separate transactions (stamp-tx, insert-tx), the poison trigger aborting the insert | `assertion left == right failed: the old row was NOT stamped  left: "superseded"  right: "text_fetch_owed"` — the partial state: stamp committed, new row absent, audit count unchanged |
| 6 | `the_cli_is_wiring_only` | the hash check inlined into `cmd_policy_anchor_seed` (`Sha256::digest`) | `fn cmd_policy_anchor_seed( must stay wiring only: it names 'Sha256' — SQL and hashing live in service::policy_anchor…` |
| 7 | `the_unpacked_deployment_is_unchanged` | the gate's no-bindings arm turned into a silent allow-all (`return Ok(())`) | `assertion left == right failed: an unset knob is the refusal, not an allow-all:  left: Some(0)  right: Some(2)` — through the REAL binary |

Anti-vacuity arm (folded into pin 1): the fixture asserts
`SELECT COUNT(*) FROM policy_anchor WHERE anchor_id = 'PH-QTR'` = **2** before any
as-of verdict — a gate that passed over an empty registry fails loudly, never smiles.

### What the drills taught (recorded, not hidden)

* **An eighth pin, born from fact-checking the blog post against the binary.
  `a_broken_knob_is_refused_and_says_so_on_stderr`.** The post said a knob
  that cannot be parsed is "dropped whole, with a warning". Driving the
  REAL binary to confirm it: stderr was EMPTY and exit 2. The config reader
  WARNs through `tracing`, but the `brain` CLI installs no tracing
  subscriber, so on the command line that warning reached nobody, and a
  mistyped knob printed "unset or does not name it". Refused either way
  (fail-closed held), but the operator was told the wrong reason. The gate
  now parses with config's own pure parser (`parse_required_anchors`, never
  a second grammar) and names the parse error on stderr, one line. The
  three states through the REAL binary: broken → warning + exit 2; unset →
  no warning + exit 2 (unset is not broken); valid → exit 0, silent. Drill
  (the old silent read planted back, reverted from the round's own copy,
  `cmp`-verified): `a broken knob must SAY so on stderr (the CLI has no
  tracing subscriber, so a tracing::warn reaches nobody here): ""`.

* **Pin 3's first form was too weak and the drill caught IT.** The planted in-place
  edit UPDATED `created_at` — and stayed green, because the first seed and the planted
  UPDATE ran inside the same millisecond and this SQLite's `strftime('%f')` returned
  the identical string (measured directly in sqlite3: two `now` calls, same
  millisecond, same value). The pin now compares the FULL row tuple (id, anchor_id,
  created_at, title, in_force_until) and the drill plant edits a CONTENT field; the
  red fired on the second form. A created_at-only history assertion can miss a
  same-millisecond rewrite.
* **Pin 1's drill is a DB-level plant by necessity:** the live-state partial unique
  index refuses a stamp-less supersede outright (the INSERT of a second live row
  violates the index), so the "supersede forgot the stamp" defect cannot land through
  the service — the drill plants the state (stamp removed from a superseded row) that
  any future writer or migration bug could leave, and the as-of answer lingering both
  versions is what reds.
* **Pin 7's first plant broke the match arm's braces** (compile error, not a red) and
  the second was env-gated in a way the subprocess could not see — both caught by
  running the drill rather than asserting it; the final plant is unconditional and the
  red is the subprocess exit code.

## Lanes at the wrap

See the wrap commit and CHANGELOG for the full gate list; the pin lane at commit time:
`cargo test --test policy_anchor_pins` — 9 passed / 0 failed.

## Owed, named (see the prompt §6 and the wrap)

GOVPACK-2 owns the K-30 export, the `/policy-anchors` wire surface, the standards map,
and the tracker page, with G-3 (caller-supplied model identity) as its hard
prerequisite. The ten fetch-owed texts join the registry one hash at a time. JMC
003's joint implementing guidelines (due ~2026-09-07) still need their issuance check.
