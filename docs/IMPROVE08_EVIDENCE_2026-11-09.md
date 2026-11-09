# IMPROVE08 — RESEARCH WATCH MODULE — EVIDENCE OF RECORD

**Date:** 2026-11-09 · **Base:** `597fe74f` (1.29.43 + improve07, schema 1.32.31) · **Close:** five commits, S1..S5, one commit per step · **Schema ceiling:** 1.32.31 → **1.32.32** (every lockstep stamp pin moved with it) · **Zero new dependencies** (the named hash is the already-locked xxh3; the parsers are hand-rolled std-side; dates ride the already-locked chrono; the client is the already-locked reqwest).

---

## 1 · The §0 ground, re-verified LIVE (2026-11-09, one session)

Every feed fact was re-probed live from this tree before any code was
written (the §0 preconditions' probe block, recorded to `/tmp/i08-probe/`):

| probe | result |
|---|---|
| `curl https://export.arxiv.org/api/query?search_query=cat:cs.CR+AND+all:prompt+injection&start=0&max_results=3` | HTTP 200, Atom 1.0, 3 `<entry>` (opensearch totalResults/startIndex/itemsPerPage present; ids with version, published vs updated, summary, authors, three links, categories, `arxiv:doi`/`arxiv:comment`) |
| `curl https://export.arxiv.org/rss/cs.CL` | HTTP 200, **ZERO `<item>`** + `<skipDays>Saturday/Sunday</skipDays>` — the UTC clock was 20:17 Sunday 2026-11-08 (the feed still carried the Sunday build); **the empty state is honest, never an error** |
| `curl https://lobste.rs/rss.rss` | HTTP 200, RSS 2.0, `<ttl>120`, `<guid>` identity, RFC-822 `-0600` dates, entity-encoded `<description>` |
| `curl https://www.cisa.gov/cybersecurity-advisories/all.xml` | HTTP 200, RSS 2.0, **30 items, 5,685,118 bytes live** — over provider_http's 4 MiB precedent → the chunked read under `WATCH_MAX_FEED_BYTES = 16 MiB` |
| `curl https://www.nist.gov/news-events/news/rss.xml` | HTTP 200, a dozen declared namespaces — the local-name, prefix-tolerant parser's case |
| `<!DOCTYPE` / `<!ENTITY` scan over all five | **0 occurrences everywhere** — and the parser refuses both BY NAME regardless (the XXE posture, never expanded) |

The weekday arXiv-RSS-with-items shape could not be recorded (UTC Sunday;
the feed rebuilds 05:00 UTC) — the weekday fixture is the MARKED-SYNTHETIC
file `tests/fixtures/watch/arxiv-rss-weekday-items.SYNTHETIC.xml` (the 07
honestly-synthetic precedent), parsed directly in tests and **never served
by the replay URL map** (the map serves the live-recorded Sunday state).

## 2 · The fixture corpus (`tests/fixtures/watch/`)

Six files, `SHA256SUMS` is the authority, README carries the provenance
(probe commands, dates, the live/synthetic split, the trim):

- `arxiv-atom.xml` — LIVE, whole (the arXiv API lane)
- `arxiv-rss-sunday-empty.xml` — LIVE, whole (the empty-state fixture)
- `lobsters.rss` — LIVE, whole (ttl/guid/RFC-822)
- `nist.rss` — LIVE, whole (prefix tolerance)
- `cisa-trim-prefix3.xml` — LIVE, **byte-prefix through item 3** (18,835 of
  5.7 MB; the trim is MARKED, the prefix is the honest unterminated bytes;
  5.7 MB does not belong in git and CI never downloads it — the replay map
  serves the trim)
- `arxiv-rss-weekday-items.SYNTHETIC.xml` — **SYNTHETIC, marked**

The corpus byte-integrity pin (`rw6_the_fixture_corpus_round_trips_byte_identically`)
re-hashes every file through the locked sha2 and fails on drift. Drift
re-records under a NEW name; nothing is overwritten (the 07 corpus law).

## 3 · The five-step build, with the red ledgers

### S1 — the deterministic core + the ONE guarded migration (`c77c6874`)

- `src/watch.rs`: the closed vocabularies (`arxiv_api|rss|atom|adapter`;
  `destination` is the CLOSED value `proposals` — nothing else parses),
  the etiquette constants by construction
  (`WATCH_ARXIV_MIN_INTERVAL_SECS=3`, `WATCH_ARXIV_CADENCE_SECS=86_400` —
  sub-daily arXiv lanes refuse at create, `WATCH_MAX_FEED_BYTES=16 MiB`),
  lane validation split pure/screen (`watch_lane_host_required` for
  non-https/userinfo/hostless; the SSRF half through the ONE
  `webhook::resolve_and_validate_sink` — private/metadata literal IPs
  refuse with no DNS, CI-safe), and **the pure tick core**
  `(lane, items, seen, fetched_at) → (proposals, seen_additions, accounting)`
  — model-absent (RW-4), identity-less items land as `accounting.failed`
  (never filed, never silently dropped), adapter lanes carry the plan-07
  passthrough with the `Retracted:` status line rendered.
- The migration: THREE guarded CREATEs (`watch_configs` / `watch_seen` /
  `watch_ticks`), the stamp's both arms moved, the refuse-newer probe
  moved to 1.32.33 (never pinned AT the ceiling), `PARITY_TABLES` carries
  all three tables.
- **RED LEDGER:** `watch_pins` compile-presence (E0432/E0433 — the module
  absent); main_suite E0425 (`SCHEMA_VERSION_V1_32_32` absent); ONE failed
  pin in EACH of the six moved lockstep suites (version_axis r51,
  per_domain probe pair, main_suite ×16, scholar sa8, agreement, rbac r47,
  clean_cycle r48) against the still-1.32.31 stamp — green in the same
  commit.

### S2 — the parsers + the corpus + the transport (`39861a69`)

- The hand-rolled std-side scanner: local-name matching (prefix-tolerant),
  ONE-level entity decode (the five built-ins; decoded HTML stays DATA),
  CDATA passthrough, dates through chrono (RFC-822 re-rendered to RFC-3339
  **with the publisher's offset preserved** — normalization is uniform
  FORMAT, never a silent UTC conversion), DOCTYPE/ENTITY refused BY NAME
  before any parsing, malformed (the opened-never-closed balance law — a
  truncated document is never an honest empty state), wrong-schema (an
  HTML/JSON body), the arXiv error-entry-as-Atom refusal (an errors# id is
  NEVER parsed as zero items).
- The transport: a per-tick VALUE (no process global — replay and live
  cannot leak into each other), live pinned to THE lane's host by
  construction (redirect::none + https_only + no_proxy + named budgets +
  the structural builder pin), the pre-I/O pin checks (foreign host →
  `watch_fetch_host_mismatch`, non-https refused — both BEFORE the client
  is consulted), the chunked bounded read (`Response::chunk()` accumulating
  through `push_bounded` → the cap → `watch_feed_too_large`, the refusing
  chunk never appended; exactly-at-the-cap allowed — pinned over an
  honestly-synthetic oversized body), the replay arm serving the corpus
  byte-identically with the unmapped URL the named `watch_replay_gap`.
- **RED LEDGER:** compile-presence (parse_feed/ParsedFeed/WatchTransport/
  push_bounded/build_arxiv_query_url absent); then the pins caught a REAL
  parser bug — `read_tag_name` stripped `'/'` unconditionally, so open
  tags read as not-a-tags and elements matched from close-tag spans (fixed
  with the four-tuple signature + the is_close discipline in both
  extractors); and the pins caught their own two wrong expectations (the
  byte cap counts exactly-at as allowed — 16 chunks then refuse; the
  RFC-822 offset preservation above).
- The dup-guard caught the round: `replay_fixture_for` defined 2× with
  scholar.rs — **RENAMED** `watch_replay_fixture_file` per the yaml_quote
  precedent (the contracts stated: scholar maps recorded API envelopes by
  identifier slot; the watch maps whole feed transcripts by lane URL).

### S3 — the filing + the tick surfaces + the CHECKED accounting (`54025ba1`)

- **THE ONE SEAM MOVES DOWN ONE LEVEL:** `create_proposal`'s
  validation/screen/scoring/insert extracted as
  `create_proposal_core(&Connection, &dyn Embedder, req, owner) -> (ProposalResponse, ScreenResult)`
  — the async wrapper keeps the caller-shaped laws (agent-class channel
  forcing, the pool, otel + alerts) and **the tick files through the EXACT
  SAME CORE inside its own `BEGIN IMMEDIATE`** — no private insert exists
  to bypass with. The screen-routing pin re-pointed at the core (its law
  stated stronger: the screen rides the CORE either way).
- `run_tick`: the freshness gate BEFORE any fetch (the last FETCH's tick
  row drives it — skips don't extend the window; the RSS ttl is LEARNED
  from the parse and rides the lane row), fetch+parse OUTSIDE the tx, then
  ONE `WorkflowTx::begin` carrying the seen inserts + the gate filings
  (source `watch`, kind `fact`, the S1 provenance header; a screen REJECT
  fails THAT item and never the lane) + the accounting row + **the CHECKED
  audit write** — a tick whose accounting INSERT fails FAILS named
  (`watch_audit_write_failed`) and the WHOLE transaction rolls back. A
  feed-level refusal still writes its ok=0 audited row.
- `AuditKind::Watch` (additive — the Import precedent; the audit module's
  byte-hash pin re-pinned with the named touch).
- `POST /watch/tick` (Admin; serde-defaulted payload validated AFTER the
  gate — the `{}` probe 403s, never 422s) + `brain watch add|tick|list`
  (the CLI family; the model loads sync behind a bounded 60 s wait — the
  tick refuses named rather than file unscored).
- **MUTATION PROOF (RW-3):** the checked write replaced by the bare
  best-effort `audit::record` form → the tick "succeeded" filing **25
  proposals with `audit_id: 0`** — no evidence row, the exact violation
  RW-3 exists for; reverted, the pin green again. The committed pin is
  behavioral without mutation: a corrupt chain epoch forces the checked
  write's refusal → the tick fails NAMED and the rollback leaves NOTHING
  (no seen, no tick row, no proposals, no audit rows).
- **RED LEDGER (honest note):** the S3 implementation landed ahead of its
  pins (the refactor made the seams concrete first), so S3's red evidence
  is the mutation proof above, not compile-presence; every other step's
  compile-presence reds are recorded.
- The drift-guards caught the round FOUR ways: `docs/cli-reference.md`
  rows (every SUBCOMMANDS entry ships its doc row), the r48 route-table
  inventory (222/210 → 224/212 → **226/214** at S4 — a named act, not
  drift), the audit byte-hash (re-pinned with the named touch), and the
  screen-site pin (re-pointed at the core seam).

### S4 — the synthesis provider posture (`7a748865`)

- `ResearchProviderProfile` (the GDL/ScholarProfile all-or-nothing shape —
  none = unconfigured/OFF, partial = the named fatal at boot via
  `validate_research_config()` beside `validate_scholar_config()` in the
  SAME fail-closed boot region; the local-endpoint posture honored: http
  allowed ONLY as the plan's preferred loopback-agent shape) +
  `POST /research/synthesize` (Admin; the act audited BEFORE the network
  leg through the CHECKED form, the task riding AS A HASH).
- `synthesize_task`: the ONE egress screen, the provider_http client
  construction law (redirect::none — a redirect is a screen bypass —
  no_proxy, https_only per scheme, the named timeouts, `resolve_to_addrs`
  pinning the VALIDATED set so a rebinding loses), the response bounded at
  the provider_http ceiling, the report extractor (the GPT-Researcher/
  STORM field or the raw body — UNTRUSTED either way). **THE OUTPUT LANDS
  AS A PROPOSAL through the REAL gate wrapper** (source `synthesis`, the
  header naming provider host + model + task) — never memory.
- Contract rows in-commit: route_guards 226/214, spire floors 218/204,
  openapi.yaml + docs/api.md + **schema.d.ts regenerated** (gen:api NOT a
  no-op — +187 lines).
- **RED LEDGER:** compile-presence (the profile/validator/route absent).
- The dup-guard caught the round again: `host_of` defined 2× with
  brain-connector-crm.rs — **RENAMED** `watch_provider_host_of`.

### S5 — the e2e witness + the live dual (this commit)

- `tests/watch_e2e.rs` — the full loop over the replay corpus through the
  REAL router (the scholar-pins harness shape): lane create (SSRF-screened)
  → `POST /watch/tick` (replay; the honest report on the wire) → proposals
  carrying the provenance header (`[Watch] lane: … / Feed: lobste.rs (rss) /
  Item: https://lobste.rs/s/… / Near-Key: …`) → the reviewer READS the
  queue (the digest-binding law: approve binds to the bytes rendered) →
  `POST /proposals/{id}/approve?digest=…` → **knowledge row: source `watch`
  verbatim, node_kind `fact`, origin `imported`** → **recall surfaces it
  with `ingest_kind: "watch"`, `origin: "imported"`, `memory_kind: "fact"`,
  trust `raw` on the wire** → the refusal twin: reject → **zero knowledge
  rows for the rejected bytes** → the audit chain verifies across the tick
  + the gate. PLUS the wire probe: an unauthenticated `{}` to /watch/tick
  lands 401 at the gate, NEVER the extractor's 422.
  (The witness caught three of its own probe bugs: the approve route
  REQUIRES the content_digest — the digest-binding law exercised for real;
  knowledge's kind column is `node_kind`; and the first absence probe was
  vacuous — SQLite correlated the unknown `proposals.content_hash` to the
  outer `knowledge.content_hash`, making `x = (SELECT x)` always true; the
  twin now probes the BYTES.)
- **THE LIVE-GATED DUAL** (operator-gated, `BRAIN_WATCH_LIVE_PROBE=1`,
  one real lane, ticked twice — the transcript of record):

  ```
  $ BRAIN_DB_PATH=/tmp/i08-e2e-live.db ./target/debug/brain watch add \
      "arxiv-health-compliance" --kind arxiv_api \
      --url "https://export.arxiv.org/api/query?search_query=cat:cs.CR+AND+all:prompt+injection&start=0&max_results=5" \
      --cadence 86400
  watch lane created: id=1 name=arxiv-health-compliance

  $ BRAIN_WATCH_LIVE_PROBE=1 brain watch tick arxiv-health-compliance
  {"audit_id":6,"deduped":0,"failed":0,"filed":5,"items_seen":5,
   "lane":"arxiv-health-compliance","proposal_ids":[1,2,3,4,5],
   "refusal":null,"skipped_fresh":false,"skipped_fresh_count":0}

  $ BRAIN_WATCH_LIVE_PROBE=1 brain watch tick arxiv-health-compliance
  {"audit_id":7,"deduped":0,"failed":0,"filed":0,"items_seen":0,
   "lane":"arxiv-health-compliance","proposal_ids":[],
   "refusal":null,"skipped_fresh":true,"skipped_fresh_count":1}

  $ brain watch list
  {"lanes":[{"cadence_secs":86400,"destination":"proposals","enabled":true,
   "failed":0,"filed":0,"host":"export.arxiv.org","id":1,
   "items_seen":0,"last_tick":"2026-11-09T01:55:32Z","name":
   "arxiv-health-compliance","skipped_fresh":1,"source_kind":"arxiv_api"}]}
  ```

  The live lane's SECOND tick is the once-per-day law ON THE LIVE PATH:
  `skipped_fresh: true`, items_seen 0, **never a second fetch** (RW-8's
  behavioral face, live). The five filed proposals carry the full
  provenance header and sit in the human queue.

## 4 · The pins map (RW-1..RW-8)

| # | where |
|---|---|
| RW-1 | structural: `rw1_the_watch_module_writes_no_knowledge` (no `INSERT INTO knowledge` in the module or the handler region) + the closed-destination pins (`rw1_destination_is_the_closed_value_proposals`, `rw2_no_lane_destination_names_synthesis`) + behavioral: the e2e witness (approve → knowledge; reject → nothing) |
| RW-2 | structural: `rw2_the_provider_is_invoked_only_on_its_route` (the invocation-site scan: definition + route, exactly two src files; the CLI never); behavioral: the all-or-nothing profile pins (in-crate), the stub-server network pin, the output-is-a-proposal pin (in-crate: pending, source `synthesis`, knowledge empty) |
| RW-3 | behavioral: `rw3_a_failed_accounting_insert_fails_the_tick_named_and_rolls_back` (the corrupt-epoch form — nothing half-recorded) + the real-tick pin verifying the checked row's kind + detail HASH on the chain + THE MUTATION PROOF (the bare form filed 25 with audit_id 0) |
| RW-4 | `rw4_the_replayed_tick_files_nothing_new` (core) + `rw4_the_replayed_tick_files_nothing_new_in_the_db` (DB form: tick twice + a fresh DB → byte-identical proposal sets) + the per-lane seen-state pin + the near-key determinism pin |
| RW-5 | `rw5_lane_host_is_required_and_https_only` + `rw5_private_and_metadata_hosts_refused_at_create` + `rw5_the_byte_cap_refuses_named` + `rw5_the_live_transport_is_host_pinned_pre_io` + `rw5_the_client_builder_is_pinned_structurally` |
| RW-6 | `rw6_the_fixture_corpus_round_trips_byte_identically` + `rw6_the_replay_transport_serves_the_corpus_and_refuses_the_gap` |
| RW-7 | `rw7_the_five_live_shapes_parse` (incl. the empty-channel honesty) + `rw7_dtd_and_entity_are_refused_by_name_never_expanded` + `rw7_malformed_and_wrong_schema_are_named` + `rw7_arxiv_error_entry_is_a_named_refusal` |
| RW-8 | the constants pin + the cadence-law create refusal + the freshness-floor pins + `rw8_the_retick_inside_the_floor_is_the_honest_skip` + the live dual's second tick (§3 S5) |

## 5 · Honest notes

- **The docling golden flake (×2, aborted runs):** the full battery hit
  `docling_goldens_round_trip_on_the_recorded_versions` twice — the
  plan-05/06 live-tool class recorded in improve07; **green in isolation**
  both times, and the final S4/S5 batteries ran the completed set green.
- **A transient target-dir corruption:** one battery run failed at the
  doctest binary with missing `.rlib` extern paths (E0463) after an
  interrupted run; a clean dev-profile rebuild resolved it — no code in
  it. The final battery (102 binaries) includes the doctest binary green.
- **The weekday RSS fixture is marked-synthetic** (§1); the replay URL map
  never serves synthetic bytes.
- **The CISA fixture is the marked byte-prefix trim** — the parser reads
  its three items honestly; the trim is documented in the corpus README.
- **Lane-create DNS:** lane create (and the e2e/tick pins that call it)
  resolve the lane's public host through the SSRF screen — a DNS lookup,
  not an HTTP fetch; CI's replay-only law (RW-6) is untouched, but a CI
  sandbox WITHOUT a resolver would red the lane-create pins. The e2e uses
  the public replay lane (`lobste.rs`); the live dual is the
  operator-gated arm.
- **S3's red-first ordering:** the implementation landed ahead of its pins
  (the refactor made the seams concrete first); S3's red evidence is the
  mutation proof, not compile-presence — recorded here and in the pins
  file's ledger.
- **The `research_synthesize` audit row rides `AuditKind::Ingest`** (the
  /scholar/fetch precedent: a route-level fetch/filing act); the TICK's
  accounting is the dedicated `AuditKind::Watch`.

## 6 · The 7-day operator soak — HANDED OFF

**The soak is the OPERATOR's act after this round.** The recipe (the
health-compliance query set the plan names):

```sh
BRAIN_DB_PATH=<operator-db> ./target/debug/brain watch add \
  "arxiv-health-compliance" --kind arxiv_api \
  --url "https://export.arxiv.org/api/query?search_query=cat:cs.CR+AND+all:%22prompt+injection%22&start=0&max_results=100" \
  --cadence 86400
# external cron (never a daemon), once per day:
#   BRAIN_WATCH_LIVE_PROBE=1 ./target/debug/brain watch tick arxiv-health-compliance
# review the queue daily (GET /proposals?status=pending), approve/reject by id;
# append the queue transcript to THIS file under §7 (the soak's record).
```

The lane honors the documented etiquette by construction: one fetch per
day per query (the manual's law), the 3 s inter-call constant pinned, and
a re-tick inside the window is the honest `skipped_fresh` line — the
publisher is never double-called by this kernel.

## 7 · The soak transcript

(operator appends: date · tick report · queue contents reviewed ·
approve/reject decisions — the loop's 7-day health record.)
