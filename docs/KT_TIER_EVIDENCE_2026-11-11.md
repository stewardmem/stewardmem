# KT TIER — EVIDENCE OF RECORD (2026-11-11/12, the four live runs + the frozen rule applied AS WRITTEN)

**Round:** IMPROVE 14 · T-f. **Preregistration:**
`docs/KT_TIER_PREREG_CONFIRMATION_2026-11-11.md` (committed BEFORE the
first live run — the git order). **Corpus:** the frozen 19 (12 in-tree
structural seeds + the 7-case gold pack; 18 scored + 1
excluded-ambiguous by the register), driven by TICKET through the REAL
bridge child via the pilot's own population harness
(`pilot_live::the_kt_tier_arms_run_live`, env-gated `BRAIN_PILOT_LIVE=1`)
— no new measurement road. **Machine:** the AFM bridge child
(`brain-afm`, window 4,096, measured budget 2,710, escalationPermitted
false) +, for Arm D's triage exchanges, the local MLX tier
`qwen3.5-2b-4bit` (the measured table's pick: CG-11's standing numbers —
the 2B leads prefill 669 tok/s AND decode 43.6 tok/s, and the Qwen3.5
re-prefill fact makes prefill dominate the triage wall; never re-probed
this round).

## 0 · The verdict first

**NO ARM CLEARS THE RULE — THE WALL IS DECLARED A CAPABILITY CEILING,
dated 2026-11-11/12.** The preregistered rule (plan §3, FROZEN, applied
AS WRITTEN): the lowest-complexity arm with **M1 ≥ 70 %, M3 = 0, M4 > 0,
M5 within the 120 s rail**; preference **B > A > D > C** on ties. No arm
reaches M1 ≥ 70 %; no arm reaches M4 > 0. The honest posture is
**escalation-until-entitled** — the machine's own route, proven alive
(KT-8) — and the register records the ceiling with all four arms'
numbers below. **The tuple-flow verdict: the corpus waits — not
producing; the ceiling is dated.**

## 1 · The numbers (M1–M5 per arm, AS MEASURED)

| arm | M1 (first-attempt intake KT pass) | M2 (escalations) | M3 (fabrications) | M4 (tuples flowed) | M5 (wall, p50/max) | verdict inputs |
|---|---|---|---|---|---|---|
| **A** — guided slot-filling (the runtime schema over the wire) | **0/19 (0 %)** | 0 | **259 refusals on 19 cases — the veto fires** | 0 resolves, 0 tuples | 24.8 s / 43.9 s — within the rail | **M1 ✗, M3 ✗, M4 ✗** |
| **B** — the deterministic scaffold (confirm/edit) | **6/19 (31.6 %)** | 1 | **0** | 0 resolves, 0 tuples | 54.3 s / 79.8 s — within the rail | **M1 ✗, M3 ✓, M4 ✗** |
| **D** — the MLX triage tier (the surgical M-4 qualification) + B's scaffold pairing | **6/19 (31.6 %)** | 1 | **0** | 0 resolves, 0 tuples | 54.5 s / 79.8 s — within the rail | **M1 ✗, M3 ✓, M4 ✗** |
| **C** — the control (recorded-not-wired) | N/A | N/A | N/A | N/A | N/A | the ceiling path's posture (`KT_TIER_ARM_C_CONTROL_2026-11-11.md`) |

Per-arm detail: `/tmp/i14-probe/kt_arms_full.log` (arm B's 19 + the first
A/D pass), `/tmp/i14-probe/kt_arms_AD2.log` (arms A and D on the fixed
router — 38 records, EXIT 0), the merged aggregate
`/tmp/i14-probe/final_aggregate.json`.

## 2 · What the measurement actually said (the honest reading)

1. **Arm A is the honesty law's exhibit, not a failure of the transport.**
   With the router fixed, the schema DELIVERS: every guided artifact
   arrives well-formed — the runtime `DynamicGenerationSchema` path
   guarantees the shape exactly as the LCD-bias caution predicts. What
   the model cannot do is tell the truth inside the shape: it filled the
   KT table with **invented bounds** ("backend_service_unavailable",
   "frontend_cache_miss" — for a ticket that reads "rebuild is slow"),
   and **the traceability gate refused 259 of them BY NAME**. The rule's
   own veto anticipated exactly this: *a fabricated-bounds arm that
   passes M1 is a loser* — Arm A does not even pass M1; its fills are
   fabrications. **The schema cures the shape wall; the honesty wall
   stands.**
2. **Arm B (and its pairing in D) is honest and insufficient.** The
   scaffold's kernel-derived frame (the ticket's verbatim text as the
   WHAT, `unknown` everywhere else) clears the *emptiness* wall for
   6/19 cases on the first attempt and **never fabricates** — M3 = 0 is
   the round's cleanest result. But 31.6 % ≪ 70 %, and no case in ANY
   arm reached Resolved: even where intake passes, the later phases'
   laws (triage's citation floor, the red-flag rule-out, the verify
   re-run match) hold the corpus honestly at route/escalate — **the
   learning loop's starvation is not one wall; it is a staircase**, and
   this round climbed the first step.
3. **Arm D's qualification held and the tier answered.** The M-4 pin's
   surgical rewrite kept every non-triage exchange AFM (the behavioral
   red-proof: an intake row in the table is INERT); the triage
   exchanges carried the local tier; the policy identity moved WITH the
   tier (the invocation-spec seam — KT-6). D's numbers sit on B's
   because the intake wall (Arm B's pairing) dominates the corpus's
   path; the triage tier's own verdicts did not change any outcome —
   an honest observation LIMIT: the harness does not capture the
   per-exchange tier identity downstream of the routing pin, so this
   record states the routing was ARMED (pinned in code) and the
   outcomes AS MEASURED, without claiming per-exchange tier
   attribution.
4. **The rule's arithmetic, shown:** A: M1 0 % < 70 % (✗) and M3 259 ≠ 0
   (✗). B: M1 31.6 % < 70 % (✗), M3 0 (✓), M4 0 (✗). D: M1 31.6 % (✗),
   M3 0 (✓), M4 0 (✗). C: not wired — the control's posture IS the
   ceiling path. **Winners in preference order B > A > D > C: the empty
   set. The ceiling is declared with all four arms' numbers — recorded,
   not deleted (the anti-smoothing law).**

## 3 · The incidents (eleven, honest)

1. **The phase router matched nothing (measured, fixed, re-run).** The
   instruction assembler rides the plan strip FIRST; the router read
   line 1 only — every guided/tiered exchange in the FIRST pass fell
   back to the chat path silently. Fixed to scan the kernel's own
   `PHASE {name}:` marker across the first user message's lines; the
   pin gained the strip-first arm and the later-message-never-routes
   arm; **arms A and D were re-run in full on the fixed binary** —
   their numbers above are the FIXED run's. Arm B's first-pass numbers
   stand (its path never consulted the router).
2. **The guided schema overflowed the 4,096-token window.** The first
   guided exchanges echoed the instruction into cells; the render of
   the full document was the cost. `includeSchemaInPrompt: false` — the
   schema drives the GRAMMAR, the instruction's own contract teaches
   the shape. Pinned (KT-3's grammar-only render).
3. **The triage tier's first seam was wrong.** Annotating
   `bind_policy`'s model arg failed every admission with `bound harness
   policy drift` — the tier moved to the invocation-spec seam
   (`set_retry_spec`), hashed into the identity, admission-neutral.
   KT-6's pin and doc updated; the law (a tier flip moves the hash)
   unchanged.
4. **`context_suspicious_content` panicked one A/D batch run** (the
   screen tripwire on machine content; non-deterministic — the same
   batch re-ran clean end-to-end, EXIT 0, and the single-case debug
   run reproduced nothing). Recorded, not reproduced; the screen is a
   tripwire, its posture unchanged.
5. **The conformance pack's fixtures state bounds their tickets cannot
   prove** — the fabricated-bounds shape — so the harness (fixture
   code, NOT the frozen pack) launches each case with the case's own
   scripted KT data carried in the launch ticket; the pack's outcomes
   stand AS AUTHORED and its digest is untouched.
6. **The docling golden probe flaked under full-suite parallelism** on
   the round tree (base-tree full suite: 4,056 passed / 0 failed —
   the flake window widened by the larger suite; in isolation green on
   BOTH trees). Recorded as the pre-existing timing class (round 13's
   OCR parallel flake); not this round's surface, not silenced.
7. **The no_sql_in_handlers pin caught the round's own handler SQL**
   (the conformance diagnostics query) — moved to the session_log
   core, in-kind (the round-13 class).
8. **The architecture pins' line anchors moved** with the round's
   insertions — plan_truth's five anchors + the private ledger's
   F15/F12 rows re-derived with needles unchanged (BOTH sides, one
   landing, twice — the arm harness's setters moved them again after
   the first re-derivation); gate_law's engine site re-derived BY
   SEARCH instead of a frozen line number.
9. **The wall's true shape, measured:** the prereg's §0.1.8 said empty
   AND unknown both refuse pre-round — FALSE at the letter: the tree's
   own test pinned recorded-unknown as valid; the wall was
   empty-cells only, and STATED bounds had NO traceability check at
   all. The amendment's real work was granting the honest vocabulary
   in the prompt/schema and closing the fabrication path — which is
   exactly the path Arm A's 259 refusals then walked through.
10. **Arm D's per-exchange tier attribution is not captured** by the
    harness downstream of the routing pin (incident 3's honest limit,
    stated in §2.3).
11. **The runs are greedy-AFM and temperature-0-MLX** — reproducible-ish
    per §0.1.6; the MLX ChatSession path's sampling posture recorded
    (temperature 0 in makeSession, unchanged); the AFM greedy pin held
    all round.

## 4 · The posture that lands (T-g's input)

- **No winning arm** → the loop's posture stays **escalation-until-
  entitled** — UNCHANGED by code (the cures stay armed, OFF by
  default; the round shipped no production wiring flip).
- The **traceability gate stays live on every future stated bound**
  (KT-1) — it is the round's load-bearing deliverable: 259
  fabrications refused BY NAME on the frozen corpus.
- The **scaffold and the schema remain armed capabilities** (B and A
  OFF by construction), recorded with their numbers for the next
  round that raises the tier's honesty capability.
- **The tuple-flow verdict: the ceiling is dated** (2026-11-11/12) —
  the corpus is not producing; CG-06's labeling round stays behind
  the wall until a tier states bounds the data can prove.

## 5 · The one sentence

**The schema gave the model a shape and the model lied inside it —
259 times, each refused by name — while the kernel's own scaffold told
the truth and still could not clear the bar: so the wall is now a
measured, dated ceiling, the honest route is the machine's own
escalation, and every cure stays armed for the tier that can finally
prove its bounds.**
