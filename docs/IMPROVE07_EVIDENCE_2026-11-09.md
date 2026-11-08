# IMPROVE07 EVIDENCE — Scholarly Source Adapters (2026-11-08/09)

**Plan:** `plans/improvements/07_SCHOLARLY_SOURCE_ADAPTERS.md` (§0.3 ground corrections govern) + the execution prompt.
**Tree:** brain-server `2bcaa5e7` (S1 base) → `a43d7b4f` (S1) → `ed416ee3` (S2) → `16d26699` (S3) → this commit (S4).
**Commits:** one per step, red-first, the ledger discipline.

---

## 1 · The live probes (2026-11-08, the fixture corpus's provenance)

Every API fact probed LIVE in one session, recorded to `/tmp/i07-probe/` and copied
verbatim into `tests/fixtures/scholar/` (SHA256SUMS is the authority; the README
carries the probe commands). All returned HTTP 200. No key presented (Europe PMC
and the Crossref polite pool need none; PubMed at the no-key 3 req/s tier).

### 1.1 The retracted record — DOI `10.1016/S0140-6736(97)11096-0` (the Wakefield paper)

| lane | what the LIVE response carries |
|---|---|
| **Europe PMC** `resultType=core` | `pubTypeList.pubType` contains exactly **`"Retracted Publication"`**; `doi` returned LOWERCASED; `license` ABSENT (non-OA); `isOpenAccess: "N"`; `commentCorrectionList` records **29 relationships** — 27 `"Comment in"`, **2 `"Retraction in"`** (notice DOIs `10.1016/s0140-6736(04)15715-2` (2004) and `10.1016/s0140-6736(10)60175-4` (2010), extracted from the reference strings by the named `doi_from_reference` scanner) and **1 `"Expression of concern in"`** — recorded, never floored |
| **Crossref** `works/{doi}` (polite pool) | `updated-by` carries BOTH entries on the SAME record: `{DOI: 10.1016/s0140-6736(04)15715-2, type: "correction", label: "Correction", source: "retraction-watch", record-id: "17269", 2004-03-06}` AND `{DOI: 10.1016/s0140-6736(10)60175-4, type: "retraction", label: "Retraction", source: "retraction-watch", record-id: "4036", 2010-02-06}` — **THE CORRECTION TRAP, live-verified: the floor vocabulary is EXACTLY `type == "retraction"`** |
| **PubMed** `esummary` uid 9500320 | `pubtype` contains exactly **`"Retracted Publication"`**; the DOI in `articleids` also lowercase; the `esearch` first leg (`term=<doi>[doi]`) resolves to `idlist: ["9500320"]` |

### 1.2 The control arm (corrected-but-never-retracted)

Crossref DOI `10.1088/1361-6595/aaebdb` (IOP Plasma Sources Sci. Technol.): `updated-by`
carries EXACTLY ONE entry, `type: "correction"`, `source: "publisher"` (the notice is
`10.1088/1361-6595/ab245f`, 2019-07-11). Parses to `retraction: None`. Europe PMC has
no record for this DOI (probe returned `hitCount: 0` — recorded honestly). This record
**NEVER floors** — pinned at S1 (parse), S3 (the floor reader), and by mutation.

### 1.3 The clean records

| record | Europe PMC | Crossref | PubMed |
|---|---|---|---|
| PLOS ONE `10.1371/journal.pone.0266789` (PMID 35476794) | `license: "cc by"` PRESENT, `isOpenAccess: "Y"`, first fullTextUrl is the OA PDF | no `updated-by` | `pubtype: ["Journal Article"]` |
| Lancet GBD YLDs `10.1016/S0140-6736(12)61729-2` (PMID 23245607) | `license` ABSENT, `isOpenAccess: "N"` | license array carries TDM entries ONLY (the parser records a CC license when present, else None — a TDM license is not an OA license) | `pubtype` without the retraction string |

### 1.4 The rate-limit record

- **Crossref live headers** (observed on the probe responses): `x-rate-limit-limit: 5`,
  `x-rate-limit-interval: 1s` — the polite pool via `mailto` is the compliance posture.
- **Europe PMC publishes NO numeric limit** (fair-use etiquette; bulk → OAI-PMH/FTP) —
  so the kernel's OWN token bucket (`BRAIN_SCHOLAR_RATE_PER_SEC`, default 1) is the
  mechanism. Named constants; never the API's problem to discover.
- **NCBI E-utilities** 3 req/s without a key, 10 with (NBK25497, unchanged through 2025).
- **The 429 transcript is honestly synthetic** (`transcript-429.json` marks its own
  provenance): a live 429 was deliberately NOT triggered — provoking one means
  hammering the API, which violates the etiquette the lane exists to honor. The
  headers model the live-observed pair. The backoff decision is pure and tested
  against this transcript (bounded two attempts, retry-after honored, capped 5 s).

### 1.5 The live/CI dual (the ocr-real discipline)

`the_adapters_run_the_live_dual_when_operator_gated` runs ONE of two honest halves:
- **CI (the half that ran in this round's verification):** the replay corpus IS the
  recorded transcript; the route drives the replay transport and asserts the parsed
  shapes. No live call (SA-4).
- **Operator (`BRAIN_SCHOLAR_LIVE_PROBE=1`, outbound internet):** three REAL pulls —
  Europe PMC, PubMed (no key, 3 req/s), Crossref (polite pool, the operator mailto) —
  asserting the live retraction signals and the witness's notice DOI, rate-bounded by
  the local 1 req/s bucket even under the probe. The transcripts this round are the
  §1.1 probes; the fixture corpus is their frozen form.

---

## 2 · The build (one commit per step)

### S1 `a43d7b4f` — the parse layer + the corpus, BEFORE any adapter

- `src/scholar.rs` (new): the three parsers, the `SourceAdapter` trait, the ONE
  normalizer (SA-6), `ScholarError`'s named refusals, `fetched_at` as a parser
  PARAMETER (replay determinism).
- The corpus: 17 files (10 live + 5 synthetic + SHA256SUMS + README) + 11
  expected-metadata contracts.
- The frontmatter vocabulary (SA-8) + ONE guarded ALTER (`knowledge.scholar_meta`,
  schema **1.32.31** — the ceiling, the refuse-newer probe, and the lockstep pins all
  moved together).
- The door passes the keys through (`scholar_meta_from_frontmatter` + in-tx
  `persist_scholar_meta` by document_id).
- **RED LEDGER (measured against the stashed tree):** 6 pins red BY NAME — the three
  door pins `scholar_meta column exists: SqlInputError { … "no such column:
  scholar_meta" }`, the migration pin `the guarded ALTER landed`, the SA-6 pin at the
  missing helper, the SA-8 vocabulary pin at the missing match arm.
- **MUTATION RED-PROOFS:** (1) the floor accepting ANY `updated-by` entry reds
  4-wide ("a corrected-but-never-retracted record NEVER floors"); (2) one flipped
  fixture byte reds the corpus pin ("drifted from SHA256SUMS").
- **The tree's drift-guards caught the round** (each re-pinned in-commit): 8 schema
  stamp pins (r38/r40/r41/r42×2/r43/r44/r47), the agreement-path + clean-cycle r48
  substrate pins, version_axis + per_domain lockstep, the router SQL census
  56 → 57, the error taxonomy (ScholarError: service/display), the test census.

### S2 `ed416ee3` — the Europe PMC adapter + the client + the config + the route

- The allowlist-pinned client: four hosts, `redirect::Policy::none()`, `https_only`,
  `no_proxy`, the named 20 s budget, the product User-Agent; the host check shared by
  BOTH arms (SA-1 behavioral over the whole surface — lookalike subdomains refused).
- The enum-dispatch transport; the REPLAY arm is the only test-side construction
  (doc-hidden installer; SA-4 pinned structurally).
- The kernel's OWN throttle (default 1 req/s) + the PURE backoff decision (the 429
  transcript drives it).
- `ScholarProfile` (the GDL all-or-nothing pattern): Europe PMC one flag; PubMed the
  flag AND its key file (either alone = the named incomplete error); the Crossref
  mailto IS the witness on-switch; the rate clamped 1..=10; fixed error strings;
  `validate_scholar_config()` fatal at boot BEFORE sockets.
- `POST /scholar/fetch` (AuthZ Write): exactly one of doi/pmid through the ONE
  normalizer; the named refusals; the per-fetch audit row (SA-3 — including
  failures); the record crosses the REAL door by CALLING `ingest_markdown` (the
  doors-identical pin reads the call; no private INSERT exists in the route body).
- The vocabularies moved together (`ADD_SOURCES_FOR_JWT` + `INGEST_KINDS` gain
  `scholarly`; `origin_for_source` derives `imported` UNCHANGED); the kind stamps
  BOTH rows (knowledge + vec_knowledge).
- **RED LEDGER:** six compile-compatible pins red BY NAME (the route 404'd as `null`,
  the allowlist const absent, the filter refused the kind), four compile-presence
  reds (E0425/E0432/E0433). The ledger header carries the split honestly.
- openapi.yaml + schema.d.ts regenerated in-commit; `docs/api.md` row;
  `docs/SCHOLAR_ADAPTERS.md` (the protocol-doc pattern); route_guards rows in-commit;
  spire floors re-measured (route 214 → 215, authz 200 → 201).

### S3 `16d26699` — the witness + the trust floor, red-first

- `fetch_crossref_witness` + `merge_witness` (the second-lane cross-check,
  arXiv:2403.13339's honest-data caution): either lane's exact retraction stands; the
  witness's notice DOI + date refine the audit-grade fields; a witness failure
  degrades honestly (`witness: false`); a correction-only witness NEVER manufactures
  a retraction (pinned on the control arm).
- `scholar_meta_floor` — the machine flag reader; `retraction_override: true` is the
  ONLY lift; garbage is (false, false).
- All three legs read `k.scholar_meta`; fusion carries the pair;
  `SearchResult.retracted: bool` SERIALIZED (the health-comply citation flag),
  `retraction_lifted` skip-serialized (the wire carries the fact, not the governance
  state).
- THE TIEBREAK FLOOR: a retracted row orders AS RAW in the equal-score bucket
  regardless of lifecycle — `trust_tier` UNTOUCHED (the derived tier stays
  lifecycle-only; `kcs_state` never rewritten). NEVER across a score gap.
- THE ONLY LIFT: `POST /scholar/override` (Admin-gated; ONE audit row IN-TX with the
  flag write — the retention-override precedent; `lift=false` removes the key so the
  floor re-applies; no-metadata is the named 409).
- **RED LEDGER (measured):** the wire pin red "the search response carried NO
  retracted key", the override pin red `null` (404, unregistered), four
  compile-presence reds. **MUTATION RED-PROOFS:** the floor reader flooring on ANY
  scholarly metadata reds the control-arm pin ((true,false) ≠ (false,false)); the
  tiebreak floor removed reds r8 (2 ≠ 1); both green on revert.
- **The drift-guards caught the round again:** the router SQL census 57 → 59 (the
  override route's two in-tx shapes); the DUP-GUARD caught `yaml_quote` defined 2x —
  REASONED, not allowed: the ump escaper escapes commas for flow-context pairs which
  would corrupt a line-oriented license scalar — renamed `yaml_scalar_quote` with the
  doc saying why; four graph_ppr tests + the bitemporal pin red `no such column:
  k.scholar_meta` (five minimal CREATE TABLEs) until their schemas were extended.
- The in-crate r8–r12 floor pins beside the CG-02 family. r8/r9 use the id-discipline
  shape (the retracted row carries the id the id-ASC final key would put LAST) so a
  broken floor cannot hide behind the final key.
- openapi + schema.d.ts regenerated (the override contract); guards/floors in-commit
  (route 215 → 216, authz 201 → 202).

### S4 (this commit) — the PubMed fallback + the live-gated dual + the e2e witness

- The KEYED two-step (`build_pubmed_*_url_keyed`, `fetch_pubmed_keyed`): the key
  rides the `api_key` param, built by the keyed builders — the only place it exists
  outside the key file; never logged, never echoed, never in an error string.
- The route's PubMed arm reads the key through `secret_file::read_owner_only_token`
  (owner-only permissions, no symlink): a vanished/bad key file is the NAMED
  `scholar_secret_file_invalid` refusal — **the adapter stays OFF rather than
  degraded** (the boot validator refuses enabled-without-configured; this covers the
  file disappearing between boot and now).
- **RED LEDGER (measured against the S3 stash):** the vanished-key pin red at the
  code assert — the S3 tree fetched PubMed WITHOUT reading the key file (200 instead
  of the refusal); the keyed builders were compile-presence reds. Green with S4.
- **THE RETRACTED-PAPER E2E WITNESS** (one pin, the full chain): the Wakefield DOI
  through `POST /scholar/fetch` → the flagged row (`scholar_meta.retracted: "true"` —
  the honest degrade, the EPMC flag carries no date) → the KCS lifecycle stamp → the
  search wire carrying **`retracted: true` AND `trust: "approved"` ON THE SAME HIT**
  (measured live in the debug run, then pinned: the derived tier stays
  lifecycle-faithful on the wire while the floor works at the tiebreak — the
  ordering law itself is the in-crate r8/r9's, exercised over comparator units with
  the id-discipline shape) → the audit rows (the fetch audit + the door's ingest
  audit, `verify_chain` green across them) → the named-act lift removable via
  `/scholar/override` (S3's pin).
- The health-comply SPEC cross-ref updated (`modules/health-comply/SPEC.md` §5.3 in
  the plans repo): the retraction finding class — "a cite of a retracted paper" is
  the audit finding; `/verify` reads the flag.

---

## 3 · The pins (SA-1..SA-8, where each lives)

| # | pin | files |
|---|---|---|
| SA-1 | allowlist structural + behavioral | `src/scholar.rs` unit tests + `sa1_*` (scholar_adapter_pins) |
| SA-2 | retraction end-to-end + the audited only-lift | `r8`–`r12` (search/tests) + `sa2_*` (scholar_floor_pins) + the e2e witness (scholar_s4_pins) |
| SA-3 | OFF-unless-configured + audited per fetch | `sa3_*` (scholar_adapter_pins) + the vanished-key pin (scholar_s4_pins) |
| SA-4 | CI never calls; the replay-only test surface | `sa4_*` (scholar_adapter_pins) + the dual pin (scholar_s4_pins) |
| SA-5 | the correction trap (exact vocabulary; control arm never floors; EoC recorded) | the trap/control/EoC pins (scholar.rs units) + `the_trap_control_arm_*` (scholar_floor_pins) + the mutation proofs |
| SA-6 | the normalizer total and single | the normalizer pins (scholar.rs) + `sa6_*` (scholar_pins) |
| SA-7 | the throttle + the named backoff + the polite pool + the User-Agent | the throttle/backoff/429 pins (scholar.rs units + scholar_adapter_pins) |
| SA-8 | the frontmatter extension backward-compatible; the keys persist | `sa8_*` (scholar_pins) |

## 4 · The verification record (final state)

- `cargo test --lib`: **2769 passed, 0 failed** (after the graph_ppr schema fixes and
  the dup-guard rename).
- The scholar suites: `scholar_pins` 6, `scholar_adapter_pins` 10,
  `scholar_floor_pins` 6, `scholar_s4_pins` 5, `scholar.rs` units 18.
- `main_suite` 354 green (the SQL census re-pinned 59, the DSAR insert-sites fragment
  extended in-commit).
- fmt clean; clippy `-D warnings` green on root all-targets + bench + the crates
  workspace; `cargo deny check advisories licenses bans sources` green (root; the
  workspace policy loop re-run at the round close) — **zero new dependencies** (the
  client uses the locked reqwest; sha2 was already locked).
- Shell lanes: `pnpm check` 0 errors, `pnpm lint` clean, `pnpm test` 404 passed.
- The census re-derived (`docs/test-census.md`) at every commit; the badge re-derived
  by the CI recipe (`bench,migrate` + the platform skip list): 3890 → 3918 (S1) →
  3927 (S2) → 3936 (S3) → this commit's derivation.
- docs-truth: openapi.yaml + docs/api.md + docs/SCHOLAR_ADAPTERS.md agree on both new
  routes; `gen:api` regenerated byte-stable at S2 and S3.

## 5 · Honest notes

- **A stash incident, repaired and recorded:** during the docs-truth baseline check
  the round ran an unconditional `git stash pop` after a `git stash push` that
  silently saved nothing (the compared files were already committed) — the pop
  therefore applied the tree's PRE-EXISTING recovered stash (`R79 'Locks' WIP`, a
  v1.29.3-era artifact that sat in the stash list before this round started), which
  conflicted in four files it owned (`.github/workflows/ci.yml`, `src/webhook.rs`,
  `src/workflow/hostcalls.rs`, `tools/signal-gateway/Cargo.toml`). None of this
  round's surfaces were touched. Repair: the four conflicted files restored to HEAD
  (`git checkout HEAD --`), the R79 stash entry PRESERVED in the stash list
  (a conflicted pop keeps its entry — verified still present), and this round's
  uncommitted S4 files (scholar.rs, memory.rs, the S4 pins, the evidence doc, the
  census re-derivation) verified intact afterwards. The repaired tree re-passed
  docs_truth (20), the census check, fmt, and clippy before the S4 commit. Lesson
  applied: never pair a conditional push with an unconditional pop.
- The `real_sidecar_full_path_when_present` (plan-05) and
  `docling_goldens_round_trip_on_the_recorded_versions` (plan-06) live-tool tests
  each flaked ONCE under full-battery parallelism (sidecar cold start / converter
  contention) and passed in isolation — machine-load transients, both recorded here;
  neither is this round's surface.
- The graph_ppr schema fixes and the dup-guard rename were REQUIRED drift-guard
  catches, landed in S3's commit (the census/sql/dup guards all fired by design).
- The control arm has no Europe PMC or PubMed record (probe `hitCount: 0` / esearch
  `idlist: []`) — a Crossref-only record; the corpus records that honestly and the
  trap pins run over its Crossref shape.
- The 429 transcript is synthetic by design (§1.4); every other fixture is verbatim.
- The route drives the KEYED PubMed path when the key file is configured; the
  unkeyed `fetch_pubmed` remains for the operator-gated live probe (§1.5) at the
  3 req/s tier. This round's verification ran the CI (replay) half of the dual; the
  live transcripts in §1 are the probe session, not a re-run through the route.
