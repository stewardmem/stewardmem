# Roadmap

StewardMem ships in small, verifiable, named releases. This page summarizes the
journey to the current version and where it is going. The full per-version record
is [CHANGELOG.md](./CHANGELOG.md); the narrative history lives in
[roadmap-and-release-history.md](./roadmap-and-release-history.md). (The former
root-level `ROADMAP.md` plan file was never git-tracked and was moved to the
private plans archive on 2026-10-04 — this page is the in-repo roadmap.)

---

## Current status

**v1.29.x — the current server line (1.29.49 — the dependency refresh:
the SQLite stack moves to the newest pool-allowed band as one coherent
set — rusqlite 0.42.0 + r2d2_sqlite 0.37.2 + libsqlite3-sys 0.40.0,
engine unchanged at 3.54.1, the read-both-ways drill proving the old
and new binaries read each other's stores byte-identically; the audit
silences — askama 0.16 drops the unmaintained basic-toml from the graph
and the ttf-parser warning is waived with a measured trigger; the
code-index subcommands' documented --index flag actually parses;
before it, 1.29.48 — the policy-anchor
registry: the kernel gains a governed registry of the AI policy
instruments a deployment is governed by, every instrument carrying the
SHA-256 of its original text or an honest statement that the text is not
held; `brain policy-anchor seed|verify|gate` lands the registry, answers
"which instruments were in force on this date", and refuses to let a pack
switch on over a policy text it cannot verify, naming every blocker;
instrument history is append-only and every registry act is audited in
its own transaction; the vocabulary is `verified / not verified /
superseded`, never "compliant"; schema 1.32.36; all 22 open CodeQL
alerts reviewed and closed with the guard named; wire contract
byte-identical — the stamps move in lockstep only; before it, 1.29.47 —
the just-works surface: the thirteen UI2 steps under the operator's
plain-language law — no AI language in any user-facing string in any
locale, 238 dash-bearing values reworked and a pin keeping them that way
— the macOS window's title hidden with the accessible one kept, the
composer badge stating posture instead of the retrieval embedder's id,
the import disclosure agreement-free, the review badge ringed, the chips
secondary, the sidebar's fills translucent on glass, a calm searching
line, the session history grouped by day and naming itself from its
first prompt (the release's one kernel change), the palette's Chats
group, and Settings reduced to three categories; beside it, the install
lane's Swift death root-caused as a rotated OS cryptex with a staleness
guard, the Swift sidecars arm64-only as a named act, and the README
rewritten as the front door; before that, 1.29.46 — the quiet surface:
the shell's chat canvas renders the answer and ONE quiet provenance row —
three stacked rows per turn became a single 11px muted token row whose
sentences all stay (the verdict dot + sentence, the raw tier word, the
cite-truth quoted/paraphrase counts, the sources chip), the greeting
learned the system serif, the Art-50 disclosure demoted to one muted
line, the on-device posture became a lock badge on the composer, and the
status bar died with every fact re-homed — the wire state on the rail
dot, the model on the badge, rows/integrity/context in the tooltip;
shell-only: zero kernel files, zero wire changes, zero new dependencies;
before it, the 1.29.45 recut — the served-chat
repair JOINED by the cite-truth verdict and the writer-posture round:
a citation that names an in-range hit is now TOLD whether the hit's
text actually contains the claim — `supported` means verified-
quoted, `partial` is the honest label for cited-but-paraphrased, the
matcher is pure text arithmetic with no model in the gate, one shared
normalizer and one shared span law serve both the verdict and the
cascade judge, the counts ride the support event additively at every
arm, the decision vocabulary and the stored verdict are untouched, and
the live receipt caught a real misattributed citation the old law
passed as supported; beside it the writer-posture round lands as the
measured default — the locally-cached writer tier with its hidden
reasoning disabled so answer tokens own the budget, CLOSING the known
debt the first cut named as its next round's work — the grounding
budget doubled with a deeper recall pool, rerank-aware packing
relevance, follow-up grounding only for elliptical follow-ups, and both
empty-answer shapes refused honestly; the served-chat repair itself:
the
chat writer's MLX resource bundle ships scripted in the Tools
installer, both app lanes, and the dev installer with a packaging pin
refusing a bundle-less writer; the evidence-as-answer arm refuses a single
document for a multi-part question or a low-confidence route and falls to
the writer; the route classifier's confidence calibrated by instruction
after the bench measured it flat-wrong; the writer budget at its own
ceiling; the chat turn route carries its own five-minute HTTP window; the
citation surface names its sources compactly with the unsupported claim
never collapsed; the grounding pool drops the media-attachment stub shape
at read time — and the release standing state repaired: the test-count
badge carries the measured healthy derivation again after the first
cut's truncated paste kept CI's drift gate red; beneath it, 1.29.44's
standing-state repair:
every CI red since 2026-11-09 closed at its mechanism, the
RUSTSEC-2026-0459/0502 close through the vendored `[patch]` override, both
remaining CodeQL alerts fixed at the transport sink, an OCR capability probe
that names a machine unable to recognize, and the dependency tree re-derived
to the latest in-range versions with all audits clean; over 1.29.43's loop
lanes — certainty, trust, ranked and re-entering cases, splits, eval hygiene,
hand-down, chat follow-ups, the tuned native tiers, the armed rerank tier —
1.29.35's live loop (the on-device pilot's first real measurements, the
afm-stdio transport, the CRANK scheduler's first flip), 1.29.34's live chat
answers, 1.29.33's durable sessions, 1.29.32's grounded chat spine, governed
row hashes and self-installing app, with the two audit passes of 1.29.3
"Hardening" beneath it).** StewardMem ships one operator
GUI over one HTTP API: the **SvelteKit + Tauri shell** (`shell/` — a
typed-wire SvelteKit SPA with a Tauri desktop core, its
client generated from the kernel's `openapi.yaml`). `/app` serves the shell
build by default (`BRAIN_CLIENT_DIST`, default `shell/build`; the installer
pins the repo-absolute path). The Dioxus control surface that shared the seat
retired 2026-11-13 — the removal the old freeze text waited on, its parity
gates long passed.
Mobile is a compile-smoke target only — no store submission has shipped. The v1.28 line
built the governed loop, then turned it into an enterprise platform:
1.28.15–1.28.35 ran the governed loop and closed the ISO 10002/10003
complaint lifecycle with consent-first outreach; 1.28.36–1.28.45 added the
Keystone order-of-care closure, the conformance line (WCAG 2.2 AA gates, WFM
seam, tier profiles), and the channel edges (Signal/WhatsApp/Slack/Teams)
with the valet assistant; 1.28.46–1.28.52 was the Foundation Line (the
service-layer convergence, handler SQL enforced to zero); 1.28.53–1.28.55
the Spire Line (the thin-binary law); and **1.28.58–1.28.62 the Enterprise
Line** — concurrent truth + visible contention gauges, the durability policy
+ lock-wait telemetry, opt-in CPU parallelism, the warm standby, the seven
CodeQL closures, and 1.28.62 "Attestation": provenance marks on
engine-generated artifacts, the principal kill-switch, approval-fatigue
telemetry, and the cryptographic inventory. **1.28.63–1.28.80 hardened it**:
seam vocabulary, egress and process boundaries, the operator/agent token
split, screen and read-seam hygiene, key lifecycle, origin taint labels,
dormant-exec hardening, finished erasure, unconditional quarantine,
third-pass close-out, and the Lockdown transport/approval/visibility
controls; 1.28.81–1.28.92 kept closing (fix releases, the seventh-pass
closures, Notary's off-host anchor + physical shred, Ledger's loop record
layers + exec OS boundary). **1.29.0–1.29.2 opened the governed delivery
line**: the GDL boundary with governed decisions and model identity (the
digest-pinned model registry), delivery persistence, and the engine executor
the loop can actually call — with the bindings, governed-release (replay-gate
promoted), and operate/delivery-read-model rounds continuing unreleased on
top. The server core (retrieval, graph, governance) is stable and heavily tested
(4,342 tests across the workspace, machine-derived by `scripts/test-census.py`;
the README badge's 4,255 counts the passed-after-skips run the build performs —
[test-census.md](./test-census.md) explains the difference).

---

## The path so far

| Line | Theme | What it delivered |
|---|---|---|
| **v0.9.x** | Foundations | Hybrid retrieval + RRF, Obsidian vault ingest, CommonMark chunker, sources/revisions, structured `QueryDoc`, evidence + provenance, connectors, capacity envelopes, migration rehearsal |
| **v1.0 "Domains"** | Multi-domain | Per-domain knowledge graphs, centroid auto-routing, cross-domain RRF, domain lifecycle |
| **v1.1–v1.2 "Harden + AuthN"** | Security | Audit hash chain, constant-time auth, JWT/JWS + AuthZ layer, OIDC/JWKS, revocation |
| **v1.3 "Bedrock"** | Memory safety | Panic elimination, `unsafe` audit, cargo-fuzz, proptests, configurable worker threads |
| **v1.4 "Calibrate"** | Retrieval quality | Bi-temporal edges, submodular evidence packing, typed-edge graphs, regression harness |
| **v1.5–v1.10** | Cognitive stack | Calibrated abstention, span verification, atomic supersession, faithful explanations, reviewable proposals, opt-in anticipation, ordered procedures |
| **v1.11–v1.12** | Graph retrieval | HippoRAG-2-style Personalized PageRank leg, noise-aware weights + hub dampening + complexity-gated rescue, AuthZ wiring completion |
| **v1.13–v1.14** | Route + Gate | Retrieval routing, write-back gating with human approval, decay, access scopes, PII controls, GDPR export/purge |
| **v1.15 "Observe"** | Compliance | Read-event audit, recall traces, DSAR workflow, deletion certificates, COMPLIANCE.md |
| **v1.16 "Client"** | The GUI | Dioxus control surface — connection machine, review, recall trace, DSAR, audit, security, styled dashboard, mobile-responsive + secure token storage |
| **v1.17 "Govern"** | Governance + UMP | Per-kind retention, Art 30, UMP 1.0 conformance through L3, eval ship-gate + SBOM |
| **v1.18 "Compliant"** | Accessibility | WCAG 2.2 AA + i18n + secret-safe console history + Art 50 origin marker |
| **v1.20 "Polish"** | Client + harden | System-following theme, offline queue, and the GhostJacking-hardening audit tail (SHA-256 digests, read-seam masking, cross-domain DSAR, reviewer calibration, read-path cost + FTS-vocabulary PRF weights) |
| **v1.21–v1.24** | Profiles → Connectors | Preset knob bundles + `brain setup`, legal hold + region + compliance pack, role postures + client-auditor domains, connector registry + translate template |
| **v1.25–v1.27** | PH-Compliant → Cascade | Breach workflow + transfer register + TIA/DPA, fail-closed erasure + fence forgeability, backup v3, console `--json`, and the graph edge-supersession + history fix (1.27.22) |
| **v1.28.15–1.28.35** | The governed loop | FirstLight runs the loop; Anvil mediates engine tool-effects; Settle contract-tests settlement; Goodwill closes ISO 10002/10003; Outreach adds consent-first care |
| **v1.28.36–1.28.45** | Conformance + channels | Keystone closes the Order-of-Care gaps; Access/Lexicon/Advocate/Handshake/Terrain complete the Conformance Line; Valet ships the assistant; Switchboard/Caravel/Herald open the governed channel edges (Signal, WhatsApp, Slack, Teams) |
| **v1.28.46–1.28.55** | Foundation + Spire | The service-layer convergence (handler SQL enforced to zero) and the thin-binary law, machine-enforced |
| **v1.28.58–1.28.62** | The Enterprise Line | Throughput (concurrent truth + the calendar as code), Headroom (durability + lock telemetry), Loom (opt-in parallelism), Standby (warm DR + CodeQL closure), Attestation (provenance marks, the principal kill-switch, approval-fatigue telemetry, the crypto inventory) |
| **v1.28.63–1.28.80** | Hardening to Lockdown | Seam vocabulary, egress/process boundaries, operator/agent token split, screen + read-seam hygiene, key lifecycle, origin labels, exec mediation hardening (dormant, then wired + OS-bounded in .92), finished erasure, unconditional quarantine, third-pass close-out, Lockdown (transport, quorum, visibility) |
| **v1.28.81–1.28.85** | AgBOM + fix releases | AgBOM (live agent bill of materials, .81), Vigil (.82), Recall (.83), Quarterly (.84), SixthPass (.85) |
| **v1.28.86–1.28.90** | Seventh-pass closures + service | Attrbane (.86, read-seam attribute tier), Ownerstamp (.87, owner stamps + crew seam), Clocktruth (.88), Bounded (.89), Refresh (.90, dependency service bump) |
| **v1.28.91–1.28.92** | Evidence + the loop record | Notary (.91: off-host `brain anchor` / `--verify`, physical `brain shred`), Ledger (.92: the governed diagnostic loop through 1.32.7, disagreement corpus + account record layers, LAYA System-1 Phase 0, exec OS boundary) |
| **v1.29.0–1.29.2** | The governed delivery line opens | 1.29.0: the GDL boundary, governed decisions, and model identity (digest-pinned model registry); 1.29.1 "Delivery persistence": the loop's storage plane; 1.29.2 "Engines": the executor the delivery loop can actually call |
| **Planned research lane** | Red-team harness | End-to-end red-team harness (PipePoison-class write→retrieve→utilize chains), multimodal carrier screening (EXIF/OCR/QR per MMPIBench vectors), MCP OAuth Protected Resource Metadata on HTTP mode (2026-07-28 spec) |
| **v2.0 "Cortex"** | Multi-team tenancy + authorization-state integrity | Tenancy plus EAL-class permission records bound to source events |

---

## Where it's going

| Milestone | Theme |
|---|---|
| **v2.0 "Cortex"** | Multi-team tenancy — the first externally-pilotable release (consumes the v1.2 AuthN/AuthZ foundation) |
| **v2.x** | Distributed revocation, limits/regions, federation |
| **v3.x** | Sovereign + survive (resilience), federated deployments — per-tenant application-level DB keys are not in the accepted design; the at-rest boundary remains operator-managed volume encryption (see the residency panel in [deployment.md](./deployment.md)) |
| **v4.0** | Sovereign standard |

The v1.19–v1.29 intermediate milestones (profiles, regulated modes, roles,
connectors, BPO operations, the hardening/correctness line, the four
v1.28 lines, hardening through 1.28.80, the AgBOM/fix releases, the
seventh-pass closures, the Notary/Ledger evidence + loop record, and the
1.29 governed delivery line) are
complete. Next: **v2.0 "Cortex"** —
multi-team tenancy, the first externally-pilotable release. The plan is
evidence-gated: work is only shipped
when it is verifiable and earned by a need, not speculation.

---

## Guiding principles

- **Evidence-gated, not roadmap-gated.** Features ship only when they are
  verifiable and justified. Several plan items are explicitly deferred rather than
  shipped for their own sake.
- **Deterministic by default.** No LLM in the retrieval hot path; no surprise
  token cost; no hidden personalization or push.
- **One binary, edge-first.** A single Rust binary with embedded SQLite, bounded
  memory, and no cloud dependency.
- **Honest ceilings.** Every release documents what it does **not** do, so claims
  never outrun implementation.

---

## Next steps

- [Overview](./overview.md) — what StewardMem is and who it is for.
- [API](./api.md) — the endpoint surface available today.
- The full per-version record: [CHANGELOG.md](./CHANGELOG.md) and
  [roadmap-and-release-history.md](./roadmap-and-release-history.md).
