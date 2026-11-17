# IMPROVE 22A — The cite-truth verdict: the evidence file (2026-11-17)

**The plan:** `plans/improvements/IMPLEMENTATION_PLAN_22_TRUTH_JOURNEY_2026-11-17.md`
§2/A1 (the private plans repo). **The execution order:**
`EXECUTION_PROMPT_22A_CITE_TRUTH_VERDICT_2026-11-17.md`. **The law in one
sentence:** a citation that names an in-range hit is now TOLD whether the
hit's text actually contains the claim — `supported` is earned by the quote,
`partial` is the honest label for cited-but-paraphrased, and the kernel's
verdict stays pure text arithmetic with no model in the gate.

## 0 · The opening act — the tree was dirty with another round

The §0 gate passed on every anchor, but the tree carried ~400 uncommitted
lines of the **writer-posture round** (the v1.29.45 notes' named "next
round's measured work"): budget 2048, recall limit 12, the elliptical
follow-up heuristic, per-turn writer sessions, empty-final guards, the
qwen3.5-4b-4bit default with thinking disabled, rerank-aware packing
relevance. With the operator's named yes (option a), the 22A session
committed it first as its own commit after verifying it — and it had NEVER
run its own suite: its tier-selection pin **self-deadlocked**
(`set_chat_model`'s non-reentrant mutex acquired three times in one scope),
its `s3_writer_config` pin still asserted the retired 9B default, the census
had drifted, and two deterministic-abstain fixtures broke under the
follow-up heuristic ("please find out …" is self-contained, so the query
never blew the 2,000-char bound — the fixtures now open with "and", the
elliptical carry). All four repaired, named in the commit (`e2c15c2e`);
verified lib 2849 + integration 4218 (two recorded RAM-contention flakes,
both green solo).

## 1 · The steps (one commit each)

| Step | Commit | The act |
|---|---|---|
| S0 | `19c8b10d` | The red battery, written FIRST — receipts below |
| S1 | `026b27d7` | The matcher + the verdict in `chat_support.rs`; the four call sites (compile-forced into S1 — a signature change and its callers are one atomic act; S3 kept the wire emissions) |
| S2 | `62ede837` | Cascade delegates; the no-op theorem pinned; three drills |
| S3 | `25136662` | All four SSE emissions gain `quoted`/`paraphrase`; the dup-guard catch (below); census re-derived |
| S4 | `d4c5e442` | The spine passes the counts through (additive-nullable; present-but-unlawful = malformed); no openapi regen (the SSE envelope is loose — `type: string`, the R118 precedent; the spine fixtures are the contract) |
| S5 | this commit | Register row · census · this evidence file |
| S6 | follows | The live wire receipt (below, §4) |

## 2 · The S0 red receipts (captured on the pre-implementation tree)

- **(e) `cascade_and_support_share_one_matcher`** (structural, ran red):
  `cascade.rs must delegate the matcher to service::chat_support (the ONE
  normalizer and the ONE span law) — found no delegation`
- **(a)–(d)** (against the new signature, compile red — the R138 precedent):
  `E0308 mismatched types` (evidence `&[&str]` where `hit_count: usize`
  stands) × 4, `E0609 no field 'quoted' on type 'Support'` × 4,
  `E0609 no field 'paraphrase'` × 4.

## 3 · The design decision the analysis forced (recorded for the next reader)

The prompt's S2 letter says `judge_answer`'s span logic "calls
`chat_support::claim_is_quoted`". **Measured against the tree, that literal
delegation is impossible byte-identically**: `claim_is_quoted` must include
the verbatim-containment arm for the receipt's ARM 2 (a 5-word claim inside
7-word evidence shares no 6-gram), and with that arm the per-chunk predicate
WIDENS the judge — a short near-verbatim writer sentence ("Benefits include
antioxidant support." ⊆ the green-tea hit) would gain a cite it never had,
changing `judged.claims` on the live lane. F2's own prescription ("revert,
re-extract verbatim") was applied up front: the judge keeps the VERBATIM
span predicate (`chat_support::spans_verified`), and `claim_is_quoted` (the
union: containment ∨ shared-span) is the verdict's matcher — for
judge-derived claims the span arm holds **by construction**, so
`quoted == cited` on cascade arms is a theorem
(`judge_derived_claims_are_quoted_exactly_when_cited`), not a fixture hope.
The S0e pin holds the sharing (the delegation names + no local matcher);
the deviation from the prompt's letter is named here and in the register
row.

### The drills (run, red, reverted from the round's own copies)

1. Cascade's in-file suite green UNCHANGED at delegation — 25/25 (F2).
2. A planted second matcher (`fn normalize` re-added to cascade.rs) reds
   (e): `cascade.rs hosts a SECOND matcher — the span law lives in
   service::chat_support and cascade delegates (one matcher, two callers)`.
3. The span arm removed from `claim_is_quoted` (containment only) reds the
   no-op theorem: `left: 0, right: 1` over the span-covered-not-contained
   sentence — the pin bites on exactly the arm that makes delegation a
   no-op.

### The dup-guard catch (a finding about the guard itself)

`dup_guard` flagged `spans_verified defined 2x: bin/cascade_bench.rs:435,
service/chat_support.rs:151`. The pair existed ALL ALONG (cascade.rs +
cascade_bench.rs) — but the production copy was `pub(crate)`, and the
guard's visibility strip matches `pub ` **with a space**, so every
`pub(crate)` definition has been invisible to the guard since it was born.
The moved copy went `pub fn` and became the first visible production site.
Fixed per the guard's own prescription (a reasoned ALLOWED_DUPES entry
extending IMPROVE17's deliberate bench self-containment); the parser
blindness is recorded, not widened this round.

## 4 · The S5 derived-artifact findings (both pre-existing reds, fixed here)

**THE BADGE WAS A TRUNCATED LIB LINE, AND CI HAS BEEN RED SINCE THE WRAP.**
The README's `tests-2860` badge came from the v1.29.45 wrap's own derivation
run — whose log the wrap notes record carried a mid-run flake red. The
number 2860 is (numerically) the LIB TARGET's own result line, not the
gate-style sum across every binary; it was pasted as the badge, and CI's
`--verify-count` has refused it ever since — the failing run's own error
line (run 43732333228, at the pre-round HEAD):

```
ERR: README test-count badge drifts from the build — badge says 2860, the run derives 4233.
```

The badge now reads **4241** — CI's derivation at the pre-round HEAD (4233)
plus exactly the 8 new Rust tests this tree adds over it (the writer-posture
round's 2: the packing cross-encoder pin and the tier-selection defaults
pin; 22A's 6: the five S0 battery pins + the no-op theorem pin).

**THE LOCAL DERIVATION IS FLAKY — and the badge lane caught this round's
own miscount.** (1) `scripts/badges.sh`'s count command runs `cargo
test` WITHOUT `--no-fail-fast`, so a single mid-suite flake (the docling
goldens' recorded RAM-contention class — 4 failures this session under
full-suite load, green solo every time) truncates the sum at the failure
point: the same class as the b2e8b980 repair, measured three times at 3310
(everything before docling_pins). The script's pipeline also masks
cargo's non-zero exit (the awk tail owns the status), so a truncated count
prints as though it were healthy — the badges.sh `--no-fail-fast` +
exit-status law is named as its own round. (2) The first badge paste
(4241) was THIS ROUND's arithmetic error, caught by the dryrun's
badge-verify-count lane: a healthy derivation gives **4243** — CI's 4233
at the pre-round HEAD plus exactly TEN new Rust tests (the writer-posture
round's 4: the packing cross-encoder pin, the tier-selection defaults pin,
and two in-file build_grounding_query pins the first count missed; 22A's
6: the five S0 battery pins + the no-op theorem pin). **Local and CI
derivations AGREE — the b2e8b980 cross-platform parity claim HOLDS**; the
earlier "11-test platform skew" reading was the miscount (the diagnostic
run's `--skip docling` analysis double-counted the lib's landlock skips as
docling skips). The badge now carries 4243, the measured healthy
derivation.

**THE COMMENTS GATE caught the round's own prose** — `chat_support.rs:150
"…(the F2 law)"` red under `comments_never_reference_versions_plans_audit_ids`;
the label dropped, the invariant sentence kept ("delegation must be a
no-op").

## 5 · The S6 live wire receipt (the G-1 two-arm pattern; the operator's
8765 untouched — scratch port 8799, scratch DB, the tree's own debug kernel)

Setup: `target/debug/brain-server` (1.29.45 + this round) under
`BRAIN_DB_PATH/BRAIN_DATA_ROOT/AUTH_TOKEN_FILE` in `~/round-scripts/22a-s6/`,
`BRAIN_AFM_BRIDGE` = the built Swift bridge, `BIND_PORT=8799`. Two memories
ingested (chunk 1: the green-tea/matcha note; chunk 2: an Eiffel Tower
sentence as the decoy). The verbatim frames live in
`~/round-scripts/22a-s6/{arm1,arm2}-raw.sse`.

### ARM 2 — the verbatim-quoted turn (cascade armed, the evidence-as-answer
route): `supported` with `quoted ≥ 1`

Question: *"What does the note say happens to green tea leaves soon after
picking?"* → the route served the memory itself (`tier: "deterministic"`),
the judge derived both sentences' claims, and the support event on the wire:

```
data: {"cited":2,"decision":"supported","paraphrase":0,"quoted":2,"uncited":0,"unknown_refs":0}
```

### ARM 1 — the paraphrased-citation turn (cascade OFF, the AFM tier's own
words): `partial` with `quoted: 0` — AND THE ROUND'S LAW CAUGHT A REAL
MISATTRIBUTION LIVE

Question: *"In your own words, why do producers steam green tea leaves
after they are picked?"* → the AFM tier answered in its own words
(`"To preserve their green color and their catechins including EGCG."`,
`tier: "on_device"`), and its ONE claim — whose text is the green-tea
sentence VERBATIM — cited **[1]: the Eiffel Tower chunk** (the grounding
sent [0]=green-tea, [1]=Eiffel). The support event on the wire:

```
data: {"cited":1,"decision":"partial","paraphrase":1,"quoted":0,"uncited":0,"unknown_refs":0}
```

The verdict's reading: the cite is IN RANGE (`cited: 1`) but the cited
hit's text does not carry the claim (`quoted: 0`) — so the label is
`partial`, disclosed as `paraphrase`. **Under the pre-round law this turn
was `supported`** — an in-range cite was the whole test, and a model
citing the WRONG hit passed. The receipt is stronger than the planned
shape: not a constructed paraphrase but a live misattribution the
cite-truth verdict caught on the first question asked of the real
FoundationModels tier. The F1 falsifier did not fire: the normalizer
needed no fixes, and quote-rate is measurable on this machinery (E1's
instrument, owed).

Cleanup: the scratch kernel and its bridge child killed, port 8799 freed,
the scratch data rooted in `~/round-scripts/22a-s6/` (not /tmp — the
machine's cleaner), and the operator's 8765 kernel answered healthy
before and after (untouched, read-only probes only).

## 6 · The round's one-sentence law, for the CHANGELOG (held for the
1.29.46 wrap — plan 22's Phase F composes the entry)

> A citation that names an in-range hit is now TOLD whether the hit's text
> actually contains the claim — `supported` is earned by the quote,
> `partial` is the honest label for cited-but-paraphrased, and the kernel's
> verdict stays pure text arithmetic with no model in the gate.

## 7 · S7 — the full gate run

fmt ✓ · clippy ×10 lanes (default, bench, bench,migrate, otel, six feature
lanes) ✓ · mas CHECK ✓ (the recorded gate; --all-targets-under-mas is a
named pre-existing red, out of surface) · deny ✓ (chrono RUSTSEC-2026-0543
triaged as a dated waiver — a brand-new unmaintained advisory against a
direct dep) · audit ✓ · lipstyk ✓ (base c06cddaf, after the posture round's
packing match was simplified per the tool's own suggestion) · targeted
suites ✓ (chat_support+cascade 34, pen_test 8/8, chat handler unit 27,
packing 13) · shell ✓ (vitest 432/2 skipped, svelte-check 0/0, eslint
clean) · the 37-lane ci-dryrun running as the closing witness
(~/round-scripts/logs/dryrun-summary.txt — fmt, clippy-default,
test-default all PASS at the time of this writing; the balance recorded
in the wrap commit).
