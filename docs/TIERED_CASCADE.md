# THE TIERED CASCADE (IMPROVE 17)

**One sentence:** the memory is the intelligence — a deterministic recall read
answers most queries verbatim at zero generative tokens, a guided AFM
classifier routes the rest to a Qwen writer whose every claim the KERNEL
span-verifies, and the frontier exists only behind an explicit per-task grant
that is audited before it ever touches the network.

**Status:** S0 EXECUTED 2026-11-13 — the gate PASSED as found; S1–S5 wired.
The gate's evidence lives in the plans repo
(`plans/improvements/IMPROVE17_EVIDENCE_2026-11-13.md` +
`plans/10_EVIDENCE_RAW/improve17-cascade/`); the benchmark harness is
`src/bin/cascade_bench.rs` (feature `cascade-bench`, the `bench` binary's
shape, no production code inside it).

## The measurement gate (the adoption IS the table)

The preregistered law: no cascade code exists until the benchmark decides the
boundaries. The recorded verdict (base phase, 106 judged queries × 25 gold
docs, the committed `tests/fixtures/eval_queries.md` set, sha256
`cc0bdbb7…` in the run meta):

| arm | strict % | graded % | lat p50/p95 (ms) | tok in | wall (s) |
|---|---|---|---|---|---|
| A single Qwen full-context | 68.9 | 75.9 | 1862/5500 | 62,738 | 237.7 |
| B single AFM | 49.1 | 50.9 | 2850/3765 | 64,646 | 252.0 |
| C the functional cascade | 69.8 | 77.4 | 1254/1504 | 30,597 | 139.7 |
| D the oracle cascade | **81.1** | **85.8** | **4**/4533 | **7,304** | **102.3** |

The oracle cascade beats single Qwen on BOTH preregistered axes: +12.2 strict
points AND −57% wall / −88% tokens-in at equal-or-better accuracy. The tier
boundaries the data named: **AFM routes, it does not write** (49.1% base with
20.8% content-policy refusals; 0.9% on the accreted memory — the window
constraint measured); **the deterministic read IS the answer** (80/80 strict
at ~3 ms when the top hit is the answer); **the writer composes and the
kernel judges** (the synthesize leg's paraphrase class is why the claims gate
exists); **the frontier is the ~1% tail** (T1's measured classification rate).

## The tiers

| Tier | What serves | Model | Tokens |
|---|---|---|---|
| T1 the coordinator | `agentloop::cascade::classify_route` — a fresh-session guided turn through the brain-afm bridge | afm3-core (never a `model` field — absent IS afm3-core) | measured, audited |
| T2 the reader | `run_recall` (the same core `/recall` serves) + `evidence_from_results` + `fenced_evidence_block` | **none — deterministic by construction** (the module's async surface is pinned; the reader adds none) | **zero** |
| T3 the writer | `writer_turn` through the bridge's MLX tier (the CG-10 session-kind door) + `judge_answer` | `BRAIN_CASCADE_WRITER_MODEL`, default the S0-measured `qwen3.5-9b-4bit` | measured, audited |
| T4 the frontier | `escalate_frontier` through `HttpProvider` (the operator's existing `BRAIN_GDL_PROVIDER_*` seam — SSRF-screened at construction, redirect-none) | the provider profile's model | per-task, audited twice |

## The laws (what this cascade is and is not)

1. **TC-1 — no auto-promotion:** every cascade output lands as a returned
   value (a chat answer or a proposal); memory entry is HITL-only. The
   coordinator and the escalation carry no INSERT; the source pins hold it.
2. **TC-2 — the route is a typed value:** `RouteDecision` is a four-case
   enum; the AFM tier's guided generation constrains the SHAPE, the kernel
   validates the VALUE (`from_wire`). Out-of-roster → the preregistered
   fallback (`synthesize` — go and read), and the echo is RECORDED, never
   swallowed.
3. **TC-3 — the reader is deterministic and model-absent:** same query →
   same evidence → same fenced bytes (pinned); the reader adds no async
   surface at all (pinned structurally).
4. **TC-4 — the kernel gate:** the writer's prose is sentence-split and
   span-matched against the evidence with the S0 preregistered metric device
   (the ONE declared normalizer + the two-sided 6-gram window). The claims'
   hit indices are DERIVED, never model-reported; zero verified sentences
   abstain.
5. **TC-5 — the frontier is per-task, audited-before-network:** an
   `EscalationGrant` constructed WITH a reason; no Default, no standing
   permission, no session-scoped cache (the arXiv:2610.22987
   approval-fatigue law). The config's existence is not a permission. The
   request row precedes the provider construction — a network failure leaves
   its evidence row standing. The output is a PROPOSAL.
6. **TC-6 — one session = one tier:** the coordinator's turn carries no
   model (absent = afm3-core); the writer's carries the configured writer
   tier; the frontier's carries the provider profile's. A mid-session swap
   is never expressible (the bridge's `model_locked` law backs it).
7. **TC-7 — every tier transition is audited:** `AuditKind::Cascade` (one
   variant + one `as_str` arm, the `DocCorrection` precedent) — the model
   asked and served, the raw guided value, the roster verdict, confidence,
   tokens, duration, the query sha256 as target. No text rides a row.
8. **TC-8 — benchmark before code:** S0's committed evidence decides the
   boundaries; the harness commit carried no production code; the stop would
   have been a success of the method.

The evidence fence is the whole cascade's spine: T2's block rides the
kernel's own `wrap_fenced` (recalled memory is DATA, never instructions), T3
sees nothing else, and the packer's caps are the chat spine's grounding-budget
law (≤6 hits, 1,000 chars per hit, 4,000 per block) — the caps the S0 noise
phase proved necessary (whole-document chunks).

## Configuration walkthrough (the operator posture)

```sh
# T1/T3 need the bridge child (prebuilt or via `swift build` in shell-macos):
export BRAIN_AFM_BRIDGE=/path/to/brain-afm        # or install beside the kernel

# T3's writer tier — the S0-measured default needs no configuration:
# export BRAIN_CASCADE_WRITER_MODEL=qwen3.5-9b-4bit   # (the default)

# T4 exists ONLY behind the operator's existing GDL seam — OFF = the named
# refusal, and every escalation still needs the per-task grant:
# export BRAIN_GDL_PROVIDER_BASE_URL=...   # the existing BRAIN_GDL_PROVIDER_*
# export BRAIN_GDL_PROVIDER_MODEL=...      # variables — no new ones added
# export BRAIN_GDL_PROVIDER_SECRET_FILE=...
# export BRAIN_GDL_PROVIDER_SECRET_ROOT=...
```

## The pins

The pins live in `tests/cascade_source_pins.rs` (the standalone structural
class: file-presence, the model-assignment roster, the async surface, the
audit-kind shape, no-memory-write, the screened door, no-allowlist) and
in-module (`src/agentloop/cascade.rs` `#[cfg(test)]` — the behavioral class:
the roster validation, the oracle law, the summary shape, the caps, the
determinism, the kernel gate, the trigger law, the refusals). All born
red-first against their pre-step trees with the transcripts recorded in the
plans repo (`red-ledger-s1.txt` + the per-step commit records). The live
e2e witness (`witness::the_cascade_answers_one_query_through_all_tiers`)
runs in the macOS lane when the bridge child exists and skips by name when
it does not — the transcript recorded in the plans repo.

## The code map

- `src/agentloop/cascade.rs` — the whole cascade: T1 (route + summary +
  audit), T2 (evidence chunks + the fenced block + the caps), T3 (the writer
  turn + the kernel gate + the audit), T4 (the grant + the refusals + the
  audited-before-network orchestration + the proposal), the pins, the
  witness.
- `src/audit/mod.rs` — `AuditKind::Cascade` (one variant + one arm).
- `src/config.rs` — `cascade_writer_model()` (the S0-measured default).
- `src/bin/cascade_bench.rs` — the S0 harness (feature-gated; the benchmark
  that decided all of the above).

## The science rows

- S0's frontier: the oracle cascade 2.3× faster and 8.6× fewer input tokens
  than single-Qwen at +12.2 strict points — the workload's number, measured
  on the committed judged set (the literature's numbers — FrugalGPT "up to
  98%", RouteLLM ">2×" — are cited with their precision notes in the
  evidence file, never substituted for the measurement).
- The accreted-memory perturbation: B collapses to 0.9% (the window
  constraint measured), C −6.6 / D −3.7 (no collapse — the miscalibration
  trigger cannot fire), D-under-noise still beats A-at-comfort.
- The junk-hit abstain miss (the arXiv:2608.17711 lesson alive in the data):
  the retrieval+router pair abstained where the single models did — recorded
  as the wiring target the abstain-signal law answers.
- The full-context arm is operationally rejected: four infrastructure deaths
  under 17.7K-token prefills on the 16 GB machine (four distinct failure
  signatures, recorded) — while every capped cascade arm survived.
- The e2e witness (2026-11-13): one query, T2 6 ms/5 hits/0 tokens, T1
  guided route `answer_from_memory` roster-ok at confidence 0.95 in 1,885 ms,
  evidence-as-answer at zero generative tokens, the kernel gate citing hit
  0, the audit row in the chain, the harness record exported.
