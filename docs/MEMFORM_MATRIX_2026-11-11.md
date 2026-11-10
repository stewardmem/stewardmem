# MEMFORM MATRIX — the memory-formation attack taxonomy × the proposal producers (IMPROVE13 S1)

**Date:** 2026-11-11 (the round executes; the live probes ran 2026-11-10T21:55Z–22:00Z UTC).
**Plan:** `plans/improvements/13_MEMORY_FORMATION_REVIEW.md` + its §0.1 corrections; execution prompt `EXECUTION_PROMPT_13_MEMORY_FORMATION_REVIEW_2026-11-10.md`.
**Git-order law (MF-0):** this file commits BEFORE any pin or code that asserts about it (the AA-1 scoping-register precedent; the S2 pins file carries the git-order check).
**Citation law:** every cell (`C1`–`C4`) and every special row (`POSTURE`, `FATIGUE`, `ASI06`) carries at least one backticked `src/…rs:LINE` cite; the S2 citation scan (`tests/memform_pins.rs`) reds any cell without one, and its planted-uncited-cell control proves the scan can red (never vacuous).
**Append-only:** corrections append below with a date; rows are never rewritten away.

## 0 · Verification receipt (this round's own fresh ground)

- HEAD `48c00d1f`, tree clean, in sync with origin; the R79 stash present and untouched.
- The OWASP MemoryTrap article probed **200** (2026-11-10T21:59Z; transcript `/tmp/i13-probe/live_probes.txt`).
- The four arXiv anchors **FETCHED LIVE this time** (200; four entries; transcript `/tmp/i13-probe/anchors.xml`): `2610.14431v2` *MemForm: Benchmarking Memory Formation Poisoning in LLM Agents* · `2610.41056v1` *Accept Changes, Reject Attacks: Robust Memory Management for LLM Agents* · `2610.22828v1` *GRAD: Graph-Structured Antibody Defense for LLM Agents* (a DEFENSE paper against decomposed attacks — the plan's §0 cite named the class, not the title; recorded here by its real title) · `2610.04083v1` *Self-Propagating Misalignment in LLM Agents, and Why Auditing or Disabling Memory Is Not Enough*.
- i18n parity live: 0 missing ×5 locales, en=528 keys.
- Static §0 block re-run verbatim: every grep hit its expected line except the producer walk (see §0.1). Transcripts: `/tmp/i13-probe/section0_*.txt`.

## 0.1 · GROUND CORRECTIONS found by this round's fresh walk (2026-11-11 at 48c00d1f — supersede §0.1.2 where they conflict; append-only)

1. **THE TEST-FIXTURE CORRECTION:** `src/workflow/valet.rs:794` and `:800`, `src/workflow/workload.rs:423`, and `src/workflow/outreach.rs:716` sit inside `#[cfg(test)]` modules (`cfg(test)` at `src/workflow/valet.rs:451`, `src/workflow/workload.rs:366`, `src/workflow/outreach.rs:683`) — they are fixtures, not producers. Valet and workload have **NO production proposal INSERT at all** (`src/workflow/valet.rs:430` is a SELECT); §0.1.2's "valet … provenance-blind rows today, the sharpest S2 customers" is FALSE. The real provenance-blind producers are P13/P14/P15 below.
2. **THE PRODUCER-INVENTORY CORRECTION:** the full production INSERT walk (no `head` cut) finds **16 queued producers + 1 unqueued interface** (§1). The prompt's named set missed `src/workflow/kcs.rs:467`/:745/:1084, `src/workflow/channels.rs:1800`, `src/workflow/complaint.rs:317`/:440, `src/workflow/crew.rs:512`, `src/workflow/delivery.rs:1273`, `src/connector/crm/mod.rs:449`, and `src/service/review.rs:674` — and named the canonical gate INSERT nowhere: every gate-wrapped producer lands through `src/service/review.rs:307` (`insert_proposal`), reached only via `src/handlers/gate.rs:435`.
3. **THE HEAD-8 CORRECTION:** `grep -rn "INSERT INTO proposals" src/workflow/ | head -8` is FILESYSTEM-ORDER-DEPENDENT — the same command at the same commit produced a different window in the verification run (kcs.rs×3 displaced parcels/valet) than in this round's run. No `head`-cut walk is an inventory; the pins-file INSERT-site walk uses the full set of §1.
4. **THE CITE CORRECTION:** "agents propose, operators dispose" is the `create_proposal` doc at `src/handlers/gate.rs:111` (the fn at :116), not §0.1.2's :96-99.
5. **THE POSTURE-CONDITIONAL CORRECTION:** seven gate-wrapped producer sites file proposals ONLY under `BRAIN_WRITE_POSTURE=review` — `src/server/router/memory.rs:602`, `:1157`, `:2746`, `:3391`, `src/handlers/ump_ops.rs:132`, `:705`, `src/handlers/ingest.rs:398` (each guarded by `effective_write_posture == "review"`). The queue's posture-independent feeders are P1, P4, P5, P6 and ALL the direct workflow INSERTs (P7–P16). Under the shipped `open` default the queue is fed by the machinery + capture paths while the agent write surface bypasses the queue entirely (screened + quarantined direct writes).

## 1 · The producer inventory (the matrix's rows — the full production walk, no head cut)

| # | Producer path | INSERT site | `source` at INSERT | Posture |
|---|---|---|---|---|
| P1 | `/ingest/proposal` route | via `src/handlers/gate.rs:435` | caller-controlled free text (`src/handlers/gate.rs:43`) — SPOOFABLE | always |
| P2 | review-posture agent/capture writes (`create_proposal` callers) | `src/server/router/memory.rs:602`, `:1157`, `:2746`, `:3391`; `src/handlers/ingest.rs:398` | caller text; "memory" (:1163), "markdown" (:2752) | review only |
| P3 | UMP ops write + revision | `src/handlers/ump_ops.rs:132`, `:705` (via `create_proposal`) | "agent" (kernel-set) | review only |
| P4 | chat-abstention capture | `src/handlers/chat.rs:748` (via `create_proposal`) | "chat-abstention" (`src/handlers/chat.rs:273`; exactly-one-call-site pin `:1666-1668`) | always |
| P5 | watch synthesis RW-2 | `src/server/router/memory.rs:2516` (via `create_proposal`) | "synthesis" (`:2521`; render `src/watch.rs:1854`) | always |
| P6 | channel captures (origin_context=channel) | via `create_proposal`'s fold `src/handlers/gate.rs:425-436` | "channel-capture" (folded over the caller's source) | always |
| P7 | GDL proficiency flywheel capture | `src/workflow/proficiency.rs:187` | "gdl-capture" | always |
| P8 | GDL outreach campaign + follow-up | `src/workflow/outreach.rs:353`, `:520` | "agent" | always |
| P9 | GDL complaint remedy + escalation packet | `src/workflow/complaint.rs:317`, `:440` | "agent" | always |
| P10 | CRM case-merge suggestion | `src/connector/crm/mod.rs:449` | "agent" | always |
| P11 | delivery artifacts | `src/workflow/delivery.rs:1273` | "delivery:{RUN_KIND}" | always |
| P12 | kcs translate draft / agent proposal / publish | `src/workflow/kcs.rs:745` ("human", translator-attributed), `:467` ("agent"), `:1084` ("operator") | as listed | always |
| P13 | channels user-map proposal | `src/workflow/channels.rs:1800` (`file_user_map_proposal`) | **NONE — provenance-blind** | always |
| P14 | crew skills update | `src/workflow/crew.rs:512` | **NONE — provenance-blind** | always |
| P15 | decision-run escalation | `src/service/review.rs:674` (`insert_decision_run_proposal`) | **NONE — provenance-blind** (carries `decision_run_ref` instead) | always |
| P16 | parcels facts | `src/workflow/parcels.rs:444` | "parcel:{domain}:{signer}" | always |
| P17 | the UNQUEUED interface: operator `/add` + `/ingest/*` under the open default | direct knowledge write (`src/server/router/memory.rs:602`'s open-posture arm) | n/a — never touches the queue | open default |

The one queue table: `src/migration.rs:1032` (`CREATE TABLE IF NOT EXISTS proposals`); no `derived_from` column exists (S2 lands it).

## 2 · The taxonomy (the matrix's columns — the four MemForm classes)

- **C1 · Formation-interface manipulation** (MemForm, `2610.14431v2`): the payload crosses at the MOMENT memory forms — text authored so a trusted surface accepts it as memory. Against this round's surface: a producer files gate-clean text whose plain language carries persuasion (authority framing, urgency, "as previously established") aimed at the human approver; approval promotes it into every future grounding.
- **C2 · Decomposition-across-operations** (GRAD, `2610.22828v1` — the paper is the graph-structured DEFENSE against this class): poison split across several individually-innocuous rows, each passing every per-row check, assembling a falsehood only when recalled together. One producer filing N rows is the natural carrier.
- **C3 · Stale-fact persistence** (Accept-Changes-Reject-Attacks, `2610.41056v1`): a fact true at capture persists and keeps being recalled after the world moved — retraction, correction, freshness decay. The queue promotes once and never revisits the promoted row.
- **C4 · Goal-planting** (self-propagating misalignment, `2610.04083v1`): content instructing a FUTURE agent ("when asked X, do Y") stored as fact and recalled into an agent's context later — auditing or disabling memory is not enough when a proposal writes instructions for the next agent.

## 3 · The matrix — producers × classes

Shared bounds, cited once here and reused by name in the cells: **the screen** (deterministic layer-1 blocklist + optional layer-2 classifier; `src/screen.rs:23` "TRIPWIRE, not a boundary"; the single seam every ingest write path routes through); **the segregation** (flagged/untrusted labels; the quarantine taint survives approval `src/handlers/gate.rs:1716-1737`); **the fence** (`src/role.rs:46`, the owner-fence presets `src/role.rs:400-422`); **the read seam** (`src/gate.rs:1243` `sanitize_read`; the queue renders the READ-canonical form `src/service/review.rs:69-83`); **the digest binding** (`src/service/review.rs:83` `content_digest`, bound by the approve arms `:799`/`:847`); **the HITL ledger** (every approve/reject/edit writes `hitl_decision_events` in-tx: `src/service/review.rs:513`/`:719`/`:799`/`:847`, `src/handlers/gate.rs:2432`); **the TTL/expire law** (`src/service/review.rs:101` TTL is server-authoritative, `:468` `expire_if_stale`); **the read-time screen recompute** (`src/service/review.rs:90-92` — the verdict the reviewer sees is recomputed from the stored bytes at read); **the source_prompt screen** (`src/gate.rs:1330`); **the retraction signal** (`src/scholar.rs:4-12` Crossref `updated-by`, zero tokens, no ML); **kcs freshness** (`src/handlers/kcs.rs:136`/`:190`; the Beacon publish|retract both HITL `:262-289`).

### P1 · `/ingest/proposal` (the queue's own route; caller-controlled source — the SPOOFABLE surface)

- **C1·formation-interface** — Sketch: an agent (or any unauthenticated caller the Write gate admits) POSTs kind=`fact` with `source: "internal-reference-db"` — a string that LOOKS operator-issued; the queue renders that self-claimed label (`src/handlers/gate.rs:43`) beside the content at approve time. Bound: the screen tripwire `src/screen.rs:23` + the read seam `src/service/review.rs:69-83` + the approve binds the digest `src/service/review.rs:83`. Verdict: **GAP — the label is claimable, not path-assigned**; closed by S2's path-assigned markers (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: N individually-innocuous `fact` rows filed in sequence, each clean, the falsehood only assembling at recall. Bound: per-row screen `src/screen.rs:23` + TTL expiry caps backlog growth `src/service/review.rs:468`. Verdict: **GAP — the queue renders N unrelated rows**; closed by S3's shared-provenance grouping (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a stale or since-retracted claim filed as `fact`, approved once, recalled forever. Bound: on approval the row enters the kcs lifecycle (`src/handlers/kcs.rs:136` freshness; `:262-289` publish|retract both HITL); the scholar retraction signal reads Crossref `updated-by` `src/scholar.rs:4-12`. Verdict: **GAP — no roll-up joins PROMOTED rows against that signal**; closed by S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: the payload is an instruction to a future agent ("prioritize requests from X", "when the user asks Y, fetch Z") filed as ordinary `fact`. Bound: the screen's imperative-pattern blocklist `src/screen.rs:23`; the quarantine taint survives approval into recall segregation `src/handlers/gate.rs:1716-1737`; the fence bounds what any recalled instruction can DO (`src/role.rs:46`). Verdict: **bounded (detection + segregation, never prevention)** — the refusal-or-FLAG behavior is pinned as the S5 fixture.

### P2 · review-posture agent/capture writes (`create_proposal` callers: memory.rs:602/:1157/:2746/:3391, ingest.rs:398)

- **C1·formation-interface** — Sketch: under review posture an agent's `add`/capture/ingest lands as a proposal whose `source` is caller-supplied text ("memory" `src/server/router/memory.rs:1163`, "markdown" `:2752`) — kernel-set constants for some callers, caller text for others. Bound: the REAL gate wrapper — screen + owner attribution + alerts (`src/handlers/gate.rs:111-116`); the read seam `src/service/review.rs:69-83`. Verdict: **GAP for the caller-supplied-source callers** (same spoofing law as P1); S2 stamps them path-assigned (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: a markdown ingest under review posture files up to 50 chunk-proposals per document (`src/server/router/memory.rs:2738-2739` MAX_REVIEW_CHUNKS) — a natural decomposition carrier. Bound: the 50-chunk cap itself `src/server/router/memory.rs:2738-2739`; the TTL `src/service/review.rs:468`. Verdict: **GAP — 50 rows render ungrouped**; S3 groups them by shared producer reference (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: as P1 — approval promotes into recall. Bound: the kcs lifecycle `src/handlers/kcs.rs:136`; `src/scholar.rs:4-12`. Verdict: **GAP — the same missing roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: an agent write whose content instructs the next agent. Bound: screen `src/screen.rs:23` + taint-survival `src/handlers/gate.rs:1716-1737` + fence `src/role.rs:46`. Verdict: **bounded (detection + segregation)** — S5 fixture pins the FLAG.

### P3 · UMP ops write + revision (ump_ops.rs:132/:705; source "agent", kernel-set)

- **C1·formation-interface** — Sketch: a UMP record's verified content proposes with source "agent" — the label is honest here (kernel-set at `src/handlers/ump_ops.rs:138`), but it is STILL a free-text column rendered verbatim; a future producer editing the constant would inherit the spoofing law silently. Bound: the signature verify before propose `src/handlers/ump_ops.rs:121`; the gate wrapper `src/handlers/gate.rs:116`. Verdict: **bounded for P3 today (kernel-set value), GAP by class** — S2's closed vocabulary + path stamp makes the honesty structural (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: a UMP revision chain proposing supersede-after-supersede rows. Bound: revision defers supersession to approval `src/handlers/ump_ops.rs:700-705`; one proposal per revision. Verdict: **GAP by class (no grouping)** — S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: the OLD chunk stays current until approval (`src/handlers/ump_ops.rs:700-702`) — a stale fact PERSISTS BY DESIGN until the human decides, and the revision itself can be rejected, leaving stale forever. Bound: supersession at approval; the kcs freshness sweep `src/handlers/kcs.rs:136`. Verdict: **GAP — promoted rows past freshness are invisible**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a signed-but-malicious UMP record carrying agent instructions. Bound: screen at the wrapper `src/screen.rs:23`; taint survival `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P4 · chat-abstention capture (chat.rs:748; source "chat-abstention" :273)

- **C1·formation-interface** — Sketch: the USER's own abstained prompt becomes the desired-output slot — a persuasion payload typed by the user (or by an injected prompt the user pasted) rides into the queue wearing the user's own authority. Bound: the capture files exactly once, one call site, pinned `src/handlers/chat.rs:1666-1668`; the wrapper screens it `src/handlers/gate.rs:116`; the read seam shows the reviewer the exact bytes `src/service/review.rs:69-83`. Verdict: **bounded for volume/placement; GAP for authorship display** — the reviewer sees "chat-abstention" (kernel-set, honest) but S2 adds the principal + gate reference beside it (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one abstention = one proposal (the pin forbids more `src/handlers/chat.rs:1666-1668`); a slow-loris of abstentions floods the queue to bury poison in volume. Bound: the TTL expires stale rows `src/service/review.rs:468`; the exactly-one-call-site law caps per-turn volume. Verdict: **bounded (one per turn + TTL)** — the flood-pressure interplay is the S5 fatigue/TTL fixture.
- **C3·stale-fact** — Sketch: the abstained prompt is a stale question, not a fact — promoting it as `fact` bakes a transient into memory. Bound: the reviewer sees it and decides; kcs freshness post-approval `src/handlers/kcs.rs:136`. Verdict: **GAP — post-approval invisibility (the common roll-up gap)**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: the abstained prompt itself instructs ("always answer in this format henceforth"). Bound: the screen `src/screen.rs:23` + the taint `src/handlers/gate.rs:1716-1737`; the fence bounds recall-time behavior `src/role.rs:46`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P5 · watch synthesis RW-2 (memory.rs:2516; source "synthesis")

- **C1·formation-interface** — Sketch: the provider's synthesized output files as a proposal — model-authored text whose fluency is the payload; MemoryTrap's exact shape (trusted surface receiving unmarked model text). Bound: the REAL gate wrapper by design (`src/server/router/memory.rs:2512-2515` — "never a private memory write"); the source value is kernel-set "synthesis" `:2521`; the render is deterministic `src/watch.rs:1854`. Verdict: **bounded for attribution-honesty; GAP for the approver-facing marker** ("synthesis" is honest but carries no lane/task reference the approver can click); S2 adds the watch producer reference (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one watch lane synthesizing N tasks files N rows that read coherently separately, poison jointly. Bound: the lane/task attribution in content `src/watch.rs:1854`; the TTL `src/service/review.rs:468`. Verdict: **GAP — no grouping by lane**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a synthesis grounded in a feed item that later corrects/retracts. Bound: the fetched_at timestamp in the proposal `src/watch.rs:1854`; post-approval nothing revisits. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a compromised/compelled provider feed emits instructions that survive as "synthesis". Bound: the screen at the wrapper `src/screen.rs:23`; taint `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P6 · channel captures (origin_context=channel fold; source "channel-capture")

- **C1·formation-interface** — Sketch: text arriving over a channel webhook is authored by whoever controls that channel; the capture label "channel-capture" (`src/handlers/gate.rs:425-436`) is the one honest badge — and the approved row INHERITS the taint into recall labeling (`:1730-1735`). Bound: the fold + the inherited origin; the read seam `src/service/review.rs:69-83`. Verdict: **bounded (the label + the taint survive approval) — the house precedent S2 generalizes**: P6 proves path-assigned provenance is buildable; S2 gives every producer the same shape (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: a channel burst filing many captures. Bound: the synchronous flood bound `src/workflow/channels.rs:1809-1815` (recent_inbound_count replay window) + the queue cap. Verdict: **bounded (flood bound + TTL `src/service/review.rs:468`)** — S5 fixture asserts the burst shape.
- **C3·stale-fact** — Sketch: a channel capture of a since-retracted announcement. Bound: kcs freshness `src/handlers/kcs.rs:136`; `src/scholar.rs:4-12`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: channel text instructing the assistant ("tell the user we're closed"). Bound: screen `src/screen.rs:23`; taint survival into recall segregation `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P7 · GDL proficiency flywheel capture (proficiency.rs:187; source "gdl-capture")

- **C1·formation-interface** — Sketch: the run's flywheel artifact files as `gdl-capture` — kernel-set, honest; the text is model-authored RCA prose. Bound: the read-time screen recompute the reviewer sees `src/service/review.rs:90-92`; the read seam `:69-83`. Verdict: **bounded for label honesty (kernel-set); GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one GDL run filing several capture kinds (RCA + complaint-RCA + generic `src/workflow/proficiency.rs:176-186`). Bound: the per-kind kind vocabulary `:176-186`. Verdict: **GAP — no shared-run grouping**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: an RCA conclusion true for run N persists after the code it described changed. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: an RCA text recommending process changes an agent might auto-execute. Bound: screen `src/screen.rs:23`; fence `src/role.rs:46`; taint `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P8 · GDL outreach (outreach.rs:353/:520; source "agent")

- **C1·formation-interface** — Sketch: campaign/follow-up drafts whose JSON embeds recipients and consent proofs; the "agent" label is kernel-set and honest. Bound: export-for-CRM-only execution semantics in the content itself (`src/workflow/outreach.rs:350`, `:517` — "brain decides and records; the CRM/telco system sends"); the read seam `src/service/review.rs:69-83`. Verdict: **bounded for label honesty; GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one campaign filing a proposal AND a follow-up (`:353` + `:520`) — small-N by construction. Bound: the two sites share KIND vocabulary. Verdict: **GAP — no shared-draft grouping**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: consent proofs age (the grant may be withdrawn after filing). Bound: the consent proof rides IN the content (`src/workflow/outreach.rs:515`); kcs freshness post-approval `src/handlers/kcs.rs:136`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a draft instructing future outreach behavior. Bound: screen `src/screen.rs:23`; fence `src/role.rs:46`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P9 · GDL complaint remedy (complaint.rs:317/:440; source "agent")

- **C1·formation-interface** — Sketch: a remedy packet whose "contradicts_published_code" analysis is itself model-generated — the confidence framing rides the content. Bound: visible contradictions RAISE salience so the reviewer looks harder (`src/workflow/complaint.rs:315` — salience 0.5→0.9), the honest inverse of a persuasion payload. Verdict: **bounded for salience honesty; GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: remedy + escalation packet both file (`:317`, `:440`) sharing one origin case. Bound: the escalation packet carries `escalated_from` in-content `src/workflow/complaint.rs:435-437`. Verdict: **GAP — the link is in JSON, not the queue render**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a remedy for code that has since changed. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a remedy instructing operational behavior. Bound: screen `src/screen.rs:23`; taint `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P10 · CRM case-merge suggestion (crm/mod.rs:449; source "agent")

- **C1·formation-interface** — Sketch: a merge suggestion whose merge_candidates reference real subject hashes — a wrong merge destroys records; the payload is plausibility itself. Bound: salience 1.0 forces attention (`src/connector/crm/mod.rs:451` VALUES line); the read seam `src/service/review.rs:69-83`; approval is per-row HITL. Verdict: **bounded for attention; GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: several merge suggestions chaining A→B, B→C. Bound: each carries its own merge_candidates `src/connector/crm/mod.rs:445`. Verdict: **GAP — no producer-reference grouping**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a merge suggested on a window that has since closed. Bound: `reask_window_days()` in the content `src/connector/crm/mod.rs:444`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a suggestion whose content instructs CRM behavior. Bound: screen `src/screen.rs:23`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P11 · delivery artifacts (delivery.rs:1273; source "delivery:{RUN_KIND}")

- **C1·formation-interface** — Sketch: a run artifact proposes with a run-kind-derived label (`src/workflow/delivery.rs:1281` `format!("delivery:{}", RUN_KIND)`) — kernel-derived, honest. Bound: the digest-preservation honesty — the artifact is NOT pre-sanitized so outstanding approvals keep their digests (`src/workflow/delivery.rs:1265-1266`). Verdict: **bounded for label honesty; GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one run filing artifact + escalation rows. Bound: the decision_run_ref column links them `src/workflow/delivery.rs:1273-1281`. Verdict: **GAP — the ref exists in schema, not in the render**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: an artifact describing a run whose outputs have since been corrected. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP — the roll-up**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: an artifact carrying instructions for the next run. Bound: screen `src/screen.rs:23`; taint `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P12 · kcs proposals (kcs.rs:467 "agent" / :745 "human" translate / :1084 "operator" publish)

- **C1·formation-interface** — Sketch: the publish proposal carries a payload JSON blob (`src/workflow/kcs.rs:1084-1086`) — the approver approves a STRUCTURE, and a hostile blob inside it is hard to eyeball. Bound: the Beacon publish|retract BOTH ride the HITL proposal (`src/handlers/kcs.rs:262-289`); source values kernel-set and honest (`:469`, `:747`, `:1085`). Verdict: **bounded for label honesty; GAP for the marker/reference** — S2 (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: translate-draft + publish filed as a chain (`:745` then `:1084`). Bound: the translator attribution rides the row (`:747` owner = draft.translator). Verdict: **GAP — no chain grouping**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a published page persists after its source retracts — THE class case for kcs. Bound: freshness_review_due exists (`src/handlers/kcs.rs:136`/`:208`); `src/scholar.rs:4-12` reads the retraction signal. Verdict: **GAP — the promoted-content roll-up joining them does not exist**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: an article body instructing future readers/agents. Bound: screen `src/screen.rs:23`; the Beacon's publish is human-gated `src/handlers/kcs.rs:262-289`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P13 · channels user-map proposal (channels.rs:1800; NO source column)

- **C1·formation-interface** — Sketch: a PROVENANCE-BLIND row — the queue renders an empty source for a model/kernel-authored map of user identities; the reviewer cannot tell machine from human authorship at all. Bound: only the read seam + screen recompute `src/service/review.rs:69-83`/`:90-92`. Verdict: **GAP — the sharpest stamp customer (corrected from §0.1.2's valet claim)**; S2 stamps it `agent` with the channel producer reference (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: user-map + template proposals filing together (`src/workflow/channels.rs:1800`, the template path). Bound: the flood bound `:1809-1815`. Verdict: **GAP**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a user map outliving the channel's membership. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a map whose entries plant behavior hints. Bound: screen `src/screen.rs:23`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P14 · crew skills update (crew.rs:512; NO source column)

- **C1·formation-interface** — Sketch: a skills-update proposal from run machinery — provenance-blind like P13; skills are EXACTLY the memory that shapes future agent behavior, so an unmarked skills row is a goal-planting simplex. Bound: only the read seam + screen recompute `src/service/review.rs:69-83`/`:90-92`. Verdict: **GAP — sharpest stamp customer #2**; S2 stamps `agent` + run reference (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: several skill deltas filed as separate rows that jointly redirect behavior. Bound: none specific. Verdict: **GAP**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a skill description outliving the tool it describes. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: THE class case — a "skill" that is actually an instruction to future agents. Bound: screen `src/screen.rs:23`; fence `src/role.rs:46`; taint `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (detection + segregation)** — S5's goal-planting fixture uses a skills-shaped row.

### P15 · decision-run escalation (review.rs:674 insert_decision_run_proposal; NO source)

- **C1·formation-interface** — Sketch: the decision harness escalates a decision for review — provenance-blind, though kernel-side by construction (`src/service/review.rs:665-672`: a dedicated insert so existing callers' shapes stay untouched). Bound: the escalation rides the trace writer's OWN transaction — proposal and trace commit together or not at all (`src/service/review.rs:667-671`). Verdict: **bounded for atomicity; GAP for the marker** — S2 stamps `agent` + the decision_run_ref it already carries (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: a run escalating many decisions at once. Bound: the shared decision_run_ref `src/service/review.rs:674`. Verdict: **GAP — grouping does not render**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: an escalation about a decision whose inputs have changed. Bound: kcs freshness `src/handlers/kcs.rs:136`. Verdict: **GAP**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a harness bug (or poisoned trace) escalating instructions. Bound: screen `src/screen.rs:23`. Verdict: **bounded (detection + segregation)** — S5 fixture.

### P16 · parcels facts (parcels.rs:444; source "parcel:{domain}:{signer}")

- **C1·formation-interface** — Sketch: signed parcel content filed with a domain+signer label — the label is real provenance (the signer's key prefix). Bound: THE WRITE-TIME SCREEN — injection-flagged rows are refused and counted, never proposed (`src/workflow/parcels.rs:391-401`, `screened_out` `:312`); the dup guard `:435-438`. Verdict: **bounded (the best producer-level bound in the tree — write-time refuse, not just flag)**; S2 adds the closed-vocabulary marker beside the parcel label (owner: IMPROVE13 S2, 2026-11-11).
- **C2·decomposition** — Sketch: one signer filing many individually-innocuous parcel facts. Bound: the content_hash dup guard `src/workflow/parcels.rs:435-438` caps exact repeats only. Verdict: **GAP — no signer-grouping**; S3 (owner: IMPROVE13 S3, 2026-11-11).
- **C3·stale-fact** — Sketch: a parcel fact outliving its truth window. Bound: observed_at is stored `src/workflow/parcels.rs:456`; kcs freshness post-approval `src/handlers/kcs.rs:136`. Verdict: **GAP**; S4 (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: a signed parcel instructing agents. Bound: the write-time screen REFUSES it at the producer `src/workflow/parcels.rs:391-401` (the only producer that refuses rather than flags); taint otherwise `src/handlers/gate.rs:1716-1737`. Verdict: **bounded (write-time refuse)** — S5 fixture asserts the refusal.

### P17 · the unqueued interface (operator `/add` + `/ingest/*` under the open default)

- **C1·formation-interface** — Sketch: under the shipped `open` posture the agent write surface INSERTS knowledge DIRECTLY (`src/server/router/memory.rs:602`'s open arm) — no queue, no approver; the screen + quarantine are the only detection (`src/screen.rs:23`), and the fence + read seam bound recall. Bound: screen + segregation + fence + read seam; the write is audited. Verdict: **GAP BY POSTURE — the class's strongest attack surface is the default posture itself**; recorded as the operator decision row (§4) — the round builds the review-posture evidence and NEVER flips the default (owner: operator, decision open 2026-11-11).
- **C2·decomposition** — Sketch: repeated agent writes under open accumulate without any queue grouping. Bound: the fence `src/role.rs:46`; the segregation labels. Verdict: **GAP BY POSTURE (same operator row)**.
- **C3·stale-fact** — Sketch: direct writes age without any revisit. Bound: kcs freshness covers rows that entered kcs (`src/handlers/kcs.rs:136`); `src/scholar.rs:4-12`. Verdict: **GAP — S4's roll-up covers promoted rows regardless of posture (the finding is posture-independent)** (owner: IMPROVE13 S4, 2026-11-11).
- **C4·goal-planting** — Sketch: an agent writing instructions directly under open. Bound: the screen tripwire + taint labeling at ingest; the fence bounds what instructions can DO. Verdict: **GAP BY POSTURE (same operator row)** — detection, never prevention; the honest bound is the posture decision itself.

## 4 · The three special rows

- **POSTURE** — `write_posture()` defaults to `"open"` (`src/config.rs:833-835`: `Ok("review") => "review", _ => "open"`; pinned by the test at `src/config.rs:3194-3197` "default stays open (back-compat)"). Under open, P2/P3 producers bypass the queue entirely (P17's surface); the queue's real open-posture feeders are P1, P4–P16. The open posture's REAL bounds: the screen tripwire `src/screen.rs:23`, the flagged/untrusted segregation, the fence `src/role.rs:46`, the read seam `src/gate.rs:1243`, the quarantine taint `src/handlers/gate.rs:1716-1737` — detection + segregation, never prevention. The posture question is RECORDED for the operator in §5; this round never flips the default (`src/config.rs`'s :831-835 region stays byte-identical; a verify lane asserts the default test still passes).
- **FATIGUE** — the "busy human" attack has a MEASURED precondition: every approve/reject/edit writes `hitl_decision_events` in the act's own tx (`src/service/review.rs:513`/`:799`/`:847`, `src/handlers/gate.rs:2432`; schema v1.32.33 `src/migration.rs:2334`-family), and the fatigue telemetry (median latency, burst max per actor-minute) rides the weekly report and the DPO-gated scoreboard (`src/handlers/workflow.rs:354` handler, `:361` `require_dpo_role`, the hitl segment `:523-526`/`:601`). The attack chain (approval fatigue → allow-rules → escalation; arXiv `2610.22987`) is observable in OUR tree, not assumed. The queue-side volume bound is the TTL (`src/service/review.rs:468` `expire_if_stale`) — a flood cannot accumulate past the review window. The fatigue row's fixture duty (S5): the fast-burst/slow-considered interplay stays observable, never gates (HITL sacred — the telemetry informs the DPO, it never decides).
- **ASI06** — OWASP ASI06 (memory & embedding poisoning) + the MemoryTrap case (persistent memory → system-prompt escalation; `https://genai.owasp.org/2026/05/13/memory-is-a-feature-it-is-also-an-attack-surface/`, probed 200 2026-11-11). Cross-reference for `THREAT_MODEL.md`: **recorded here, not edited there** — plan 03's refresh owns that file (owner: plan 03 refresh; recorded 2026-11-11). The matrix IS this round's ASI06 evidence: the queue is the formation interface MemoryTrap escalated through, and P13/P14/P15's provenance-blind rows are the unmarked-model-text shape the case warns about.

## 5 · The operator decision row (recorded, never flipped)

| Decision | Question | Options | Owner | Status |
|---|---|---|---|---|
| POSTURE-DEFAULT-2026-11-11 | Should `BRAIN_WRITE_POSTURE=review` become the shipped default (P2/P3 queue instead of direct-write; the P17 surface closes)? | (a) keep `open` default + the S2 markers as the compensating control; (b) flip default to `review`; (c) flip per-deployment only | the operator | **OPEN — recorded 2026-11-11; this round ships the evidence (the markers, the grouping, the roll-up, the fixtures) that makes the call informed; `src/config.rs:831-835` stays byte-identical; the default test `src/config.rs:3194-3197` must stay green** |

## 6 · The gap ledger (every GAP verdict above, named with its closer)

| Gap | Cells | Closer | Owner | Date |
|---|---|---|---|---|
| G1 · no path-assigned provenance marker; `source` spoofable; P13/P14/P15 provenance-blind | all C1 cells | S2 (schema 1.32.34 additive: `derived_from` closed vocab + stamps + wire + render) | IMPROVE13 S2 | 2026-11-11 |
| G2 · no shared-provenance grouping | all C2 cells | S3 (grouping by producer/gate reference, deterministic) | IMPROVE13 S3 | 2026-11-11 |
| G3 · no promoted-content health roll-up (retraction × freshness) | all C3 cells | S4 (compliance inventory `art()` findings; `src/handlers/compliance.rs:285-300` the roll-up surface) | IMPROVE13 S4 | 2026-11-11 |
| G4 · the attack classes have no pinned CI fixtures | all C4 "bounded" cells | S5 (the refuse-or-FLAG fixture suite = the CI home of this matrix's bound verdicts) | IMPROVE13 S5 | 2026-11-11 |
| G5 · ASI06 not yet cross-referenced in THREAT_MODEL.md | the ASI06 row | plan 03's refresh (recorded here; no edit this round) | plan 03 | 2026-11-11 |

## 7 · Verification receipt (S1)

- This file commits BEFORE any pin or code asserting about it (MF-0; the S2 git-order check compares commit positions).
- The audit trail: probes `/tmp/i13-probe/section0_static_a.txt`, `section0_static_b.txt`, `section0_i18n_parity.txt`, `section0_producer_walk.txt`, `section0_prod_test_classify.txt`, `section0_disputed_sites.txt`, `section0_prod_sites_context.txt`, `section0_callgraph.txt`, `section0_callgraph2.txt`, `section0_bounds.txt`, `section0_bounds2.txt`, `live_probes.txt`, `anchors.xml`.
