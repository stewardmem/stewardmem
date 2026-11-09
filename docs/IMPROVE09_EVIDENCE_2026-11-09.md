# IMPROVE09 — UPSTREAM TRACKING — EVIDENCE OF RECORD

**Date:** 2026-11-09 · **Base:** `667592d8` (1.29.43 + improve08, clean, in sync) · **Branch taken:** (a) NO newer tag → **the posture record + the harness proof; NO bump, NO churn** · **Zero new dependencies** · **No kernel `src/` behavior change** · the pre-existing stash `R79 'Locks' WIP` present at open and untouched throughout.

---

## 1 · The §0 ground, re-verified LIVE (2026-11-09, one session)

Every precondition was re-run from the tree first (the probe block, recorded
to `/tmp/i09-probe/`). The tree facts:

| precondition | result |
|---|---|
| `git status --short --branch` | clean, in sync with origin/main, HEAD `667592d8` |
| `git stash list` | `stash@{0}: R79 'Locks' WIP (RECOVERED …)` — **LEAVE-ALONE baseline held** |
| `Package.swift:77` | `.package(url: "…mlx-swift-lm", from: "3.32.3")` — the tag-based pin |
| `Package.resolved` pins | mlx-swift-lm **3.32.3** · swift-jinja **3.0.0** · swift-transformers **1.4.0** · mlx-swift **0.32.3** (plus eventsource 1.5.1, swift-asn1 1.7.3, swift-collections 1.7.3, swift-crypto 4.5.2, swift-huggingface 0.13.0, swift-numerics 1.1.1, swift-syntax 603.0.2, yyjson 0.13.0) |
| the checkout | `shell-macos-mlx/.build/checkouts/mlx-swift-lm` at `3b339ad6` ("adjustments for running tests on NAX/TF32 hardware (#649)"), `git describe --tags` → **3.32.3 clean** |
| probe files | `EngineFloorPins.swift` + `SpecDecodePins.swift` present; CG-11 receipts present |
| the gaps | `docs/UPSTREAM_TRACKING.md` ABSENT (the gap closed this round); `docs/IMPROVE09_EVIDENCE_*` ABSENT |
| Tauri | `shell/src-tauri/Cargo.toml:35` `tauri = { version = "2.12.4", … }` (also :54 with the test feature); the lock resolves **2.12.5** |

The LIVE upstream probes (all fetched 2026-11-09, transcripts in
`/tmp/i09-probe/*.json`):

| probe | live result | §0.5 |
|---|---|---|
| `mlx-lm-tags.json` (tags API, per_page=5) | **`3.32.3` (`3b339ad6`) is STILL the newest tag**; then 3.31.4, 3.31.3, 2.31.3, 2.30.6 | fact 1 ✓ |
| `mlx-lm-compare.json` (`3b339ad6…main`) | status `ahead`, **ahead_by 87**, behind_by 0, total 87; main HEAD `4ab26799` (2026-11-06T21:05Z), last commit "MLXVLM Qwen35: apply YaRN from rope_parameters (#761)" | fact 1 ✓ |
| `jinja-compare.json` (`a091d6ae…main`) | status **`identical`**, ahead_by 0, behind_by 0 | fact 6 ✓ |
| `st-rel.json` (swift-transformers releases) | **1.4.0** (2026-11-02) newest, then 1.3.5 (2026-10-09) | fact 7 ✓ |
| `ms-rel.json` (mlx-swift releases) | **0.33.0** (2026-10-27) newest; 0.32.3 (2026-09-30) is our pin | fact 8 ✓ |
| `tauri-rel.json` (per_page=40) | stable v2 head **`tauri-v2.12.5`** (2026-10-21); the v3 line is alpha (`tauri-v3.0.0-alpha.6`, 2026-11-06) — NOT an adoption target | fact 9 ✓ |
| `osv-tauri.json` (POST api.osv.dev, tauri @ 2.12.5) | `vulns: []` — **ZERO advisories** at 2.12.5 | fact 9 ✓ |

**The S1 branch decision:** the tag probe still shows `3.32.3` as the newest
tag → law (a) applies — **no churn**: S1 is the posture record + the harness
proof on the CURRENT pin, and the bump-validation checklist lands in
`docs/UPSTREAM_TRACKING.md` as the named act for when the tag lands.

## 2 · The posture decision, recorded

`docs/UPSTREAM_TRACKING.md` exists now (S1's first row + the
bump-validation checklist; S2 seeds the remaining rows). The decision:
**stay on tags; adopt the next tag as a named, measured bump** — with the
87-commit validation debt itemized by family (GDN / KV-memory /
KV-rotation / streaming / ChatSession / Qwen3.5 / MTP) and the standing
numbers carried forward from CG-11 (8B 28.5 tok/s / ~4.6 GiB · 2B 43.6 /
~1.8 · heretic 24–26 / ~6.2 · 9B-4bit 26–28.7 / ~5.9–6.2 · the 4B tiers
41–43.7 / ~2.5–2.8). The class change is recorded in the same place: main
LANDED tower-native MTP speculative decoding (#545/#581), so the C-5
posture's class changed and the ≥40% acceptance question on the Qwen3.5
GDN tiers is UNMEASURED until the bump — `SpecDecodePins` is the harness,
never adopted unmeasured.

## 3 · The harness proof on the CURRENT pin (S1's green)

The swift-suite recipe, run exactly as the tree carries it
(`shell-macos-mlx/`): `swift build --build-tests` → `scripts/setup.sh` →
`swift test` (hermetic), then `BRAIN_FM_MEASURE=1 swift test` (live arms).
Full transcript: `/tmp/i09-probe/swift-suite-transcript2.txt`.

```
=== BUILD EXIT 0 ===
setup: default.metallib -> .build/out/Products/Debug/MLXFMProbeTests.xctest/
       Contents/Resources/mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib
=== HERMETIC RUN ===
✔ Test run with 38 tests in 13 suites passed after 1.674 seconds.
=== HERMETIC EXIT 0 ===
=== LIVE RUN (BRAIN_FM_MEASURE=1) ===
✔ Test run with 38 tests in 13 suites passed after 96.364 seconds.
=== LIVE EXIT 0 ===
```

The live 9B measure arm (the harness's own coherence gate green):

```
[cold] first token 42215.0 ms · total 42453.0 ms · decode  42.01 tok/s · 10 chunks
[cold] >>> The capital of France is Paris.
── coherence: the gate that caught the DWQ ──
[cold] The capital of France is Paris.
✔ Test "Measure Qwen3.5-9B: cold/warm TTFT, decode rate, coherence" passed after 96.362 seconds.
```

Key pins green in the run: `The swift-jinja engine floor` (the UT-1
inheritance — swift-jinja resolves ≥ 3.0.0), `Spec decode pins (P-3)` (the
40% gate + the seven-tier OFF posture + the drafter-not-loaded law),
`KV scheme pins (P-1)`, `Prefix cache pins (P-5)`, `PFlashAbsencePins`,
`Think filter`, `End-to-end grounded chat` (injection + abstention arms).

**The environment incident, recorded honestly:** the FIRST suite run
(`swift-suite-transcript.txt`) failed at build — `CompileMetalFile
arg_reduce.metal` could not spawn `/var/run/com.apple.security.cryptexd/
mnt/com.apple.MobileAsset.MetalToolchain-v27.2.47.0.dW2pUQ/…/metal` (No
such file or directory). The Metal toolchain cryptex had been re-mounted
under a new suffix (`…V5Zhv6`; `xcrun -f metal` resolves there,
`Apple metal version 32023.922`); the on-disk XCBuild scratch
(`.build/out`) carried the dead path in its description. Fix: removed
`.build/out` (derived artifacts only — no source, no pins touched) and
re-ran; everything green as above. The toolchain event is environmental,
not the tree's.

## 4 · The swift-jinja re-check at execution (S4)

Live-probed 2026-11-09: `a091d6ae…main` → status **`identical`**, ahead_by
0; 3.0.0 (2026-10-27) still the newest release. **The decline stands until
a release moves** — `EngineFloorPins` stays at 3.0.0 and is CURRENT; the
table's jinja row carries this fresh date. Had a release moved, the plan's
newer-release review (the `template-debug` fixtures before any pin move)
would have run first; it did not trigger.

## 5 · The pins ledger (UT-3, red-first)

`tests/upstream_tracking_pins.rs` — six tests, std-only (zero new deps; the
date check is a byte-level YYYY-MM-DD parse, not a crate). The genuine
red-first order held: **the pins were written and run BEFORE the table's
seed rows existed** (only S1's posture row landed first, per the
one-commit-per-step order; the red-by-absence ran before ANY of the doc
existed):

| # | run | transcript | result |
|---|---|---|---|
| R1 | absence — the doc did not exist | `pins-red-absence.txt` (cargo exit 101) | **5 failed** (`the_one_home_table_exists`, `…closed_vocabulary`, `…review_date`, `…our_evidence`, `…checklist_present`), each panicking `the one home does not exist: …/docs/UPSTREAM_TRACKING.md — the tracking table is the round's deliverable, never delete it`; the grammar test (`a_declined_row_must_carry_its_reason`) passed — it reads no doc |
| G1 | the table seeded (S1's row + S2's seven rows) | `pins-green-seeded.txt` (exit 0) | **6 passed; 0 failed** |
| R2 | mutation — a field-less row appended (empty status + empty review-date) | `pins-red-mutation-fieldless.txt` (exit 101) | **2 failed, naming the row**: ``row `a row added later without the fields` carries status `` — empty status`` and ``…carries review-date `` — not a YYYY-MM-DD date…(UT-3)`` — the future-path re-red demonstrated |
| R3 | mutation — a bare `declined` row appended (no reason) | `pins-red-mutation-declined.txt` (exit 101) | **1 failed**: ``row `a declined row without its reason` carries status `declined` — declined WITHOUT a reason — the closed vocabulary is `declined+reason`…`` — the grammar's reason clause fires |
| G2 | restored (the mutation rows removed) | `pins-green-restored.txt` (cargo exit 0) | **6 passed; 0 failed** — the committed state is the G2 state |

The pins' laws: UT-3 (every row carries a closed-vocabulary status AND a
review-date), UT-2's receipt face (every row cites our evidence) and
CI-orderable face (the bump-validation checklist must be present — deleting
the gate is loud). Note: the pins landed after S1's posture row by the
round's commit order; R1 is the real absence run (pre-doc), R2/R3 are the
mutation proofs of the same code paths a future bad row would hit.

## 6 · The engagement drafts (in-repo; the operator files)

Recorded VERBATIM in `docs/UPSTREAM_TRACKING.md` (§D-1, §D-2), pins green
over them (6/6). **NO GitHub filing by the agent** — the table's review-date
fields carry the follow-up; filing is the operator's outward-facing act.

- **§D-1 — the C-6 report** (target: ml-explore/mlx-swift-lm): the
  `cachedPromptTokenCount == 0` turn-2 finding on the Qwen3.5 GDN tiers vs
  the 8B's `cached=227` control; the template-debug bisect's cause
  (the primed-think render is not prefix-stable once the final assistant
  turn has content); #155 verified present-and-not-covering (our pin
  3.32.3 carries it); the #663/#644 hook named. **Positioned AFTER the
  bump re-probe** (§0.5 fact 4): the draft files as a confirmation with
  fresh data OR a closed-by-the-fix report, by measurement — the header
  carries the file-only-after gate.
- **§D-2 — the spec-decode engagement** (target: ml-explore/mlx-swift-lm):
  the MTP question — does the tower-native MTP path (#545/#581) clear the
  ≥40% acceptance gate on the Qwen3.5 GDN tiers WITHOUT the throughput
  collapse — with our CG-11 draft-model data (acceptance 25–47% yet
  28.5 → 6.3–9.6 tok/s, ~1.5–4 GiB drafter cost), the preregistered
  harness offered as the measurement, and the literature set (the plan's
  five + HybridSpec 2611.04055 / SpecOPD 2611.02846 / ReplaySSM 2610.13778
  / H-Spec 2609.24197 + the MTP class AngelSpec 2607.25852 / Windowed-MTP
  2607.21535 / DLoop 2610.07659 / HyCo 2610.21436 / Long-Context Drafting
  2610.20865 + Nemotron 3.5 2610.38095 behind #712).

**S4 (the swift-jinja re-check) is folded into this commit** per the
execution prompt's allowance ("One commit with S2/S3's doc state or its own
— either is fine; the row must carry the fresh date"): the live probe
(§4 above, `jinja-compare.json` — identical, ahead_by 0, 2026-11-09) is the
step's act; the table's jinja row has carried the fresh date since S2.

## 7 · Honest notes

- The round touched: `docs/UPSTREAM_TRACKING.md` (new),
  `docs/IMPROVE09_EVIDENCE_2026-11-09.md` (new),
  `tests/upstream_tracking_pins.rs` (new, rust battery only). NO kernel
  `src/` change; NO client-tree pin change (the no-churn branch); zero new
  dependencies (the pins file is std-only — its date parser is 20 lines of
  byte checks, not a new crate).
- The `.build/out` removal (§3's incident) is derived-artifact cleaning in
  the SwiftPM scratch; the next `swift build` re-created it. Nothing
  tracked changed.
- The hermetic live-measure arm skips cleanly without `BRAIN_FM_MEASURE=1`
  (`Measure …` skipped, 38/38 still); the live run's [warm]/[long] arms
  print `decode 0.00 tok/s · 0 chunks` — the harness's own accounting of
  the streamed-batch path, gated green by the suite itself; the recorded
  class from CG-11 (38/38 hermetic AND live) is what S1 proves again.

## 8 · Verify-after

Landed with the final commit — the battery, fmt/clippy, the census, the
badge verify-count, docs-truth, the stash check.
