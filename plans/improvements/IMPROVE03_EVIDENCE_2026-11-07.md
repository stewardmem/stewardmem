# IMPROVE03 EVIDENCE — 2026-11-07 (the advisory pins + the threat-model refresh)

**Round:** `plans/improvements/03_ADVISORY_PINS_AND_THREAT_MODEL_REFRESH.md` + the
§0.1 ground corrections (re-verified 2026-11-07; the corrections win).
**Baseline:** brain-server `5c4b1109` (1.29.43), tree clean, in sync.
**This file is the round's evidence record.** Sections fill incrementally, one
per step; the §0.1 ground record stands as written.

---

## 0 · Ground, re-verified at execution time

| §0 check | Expected | Measured |
|---|---|---|
| HEAD | `5c4b1109` or later | `5c4b1109` (1.29.43) ✓ |
| tree | clean, in sync | clean, in sync ✓ (nothing to push) |
| `deny.toml` | MUST NOT exist | absent ✓ (the round creates it) |
| CI audit job | `ci.yml:901` | `:901` ✓ (binary 0.22.2, all-8-lockfile loop) |
| pnpm audit | `shell.yml:126` | `:126` ✓ (`--prod --audit-level high`) |
| capability pin | `sidecar_pins.rs:567` | `:567` ✓ (`the_shell_has_no_shell_plugin`) |
| THREAT_MODEL stamp | R117 (2026-10-12) | `:7` — `**Coverage current through:** R117 (2026-10-12)` ✓ (the plan's grep needed `-i`; the stamp is exactly as recorded) |
| cargo-audit local | 0.22.2 | 0.22.2 ✓ (== the CI pin) |
| cargo-deny local | not installed | not installed ✓ (S1 installs 0.20.2 — crates.io latest at execution) |
| tracked lockfiles | 8 | 8 ✓ (root, client, crates, fuzz, shell/src-tauri, tools/channel-bridge, tools/signal-gateway, tools/steward-harness) |

**Version-claims corrections caught at measurement** (the law: record the
measured value): tauri **2.12.5** in the lockfile (the plan said 2.12.4 — the
tree moved); tauri-plugin-updater **2.14.1** (the plan said 2.14).

---

## 1 · S1 — the deny gate + the triage

### 1.1 The 8-workspace triage table (measured 2026-11-07, baseline tree)

`cargo audit` exit 0 on ALL 8 workspaces — zero vulnerability-class findings
in the audit lane. `cargo deny check` (first cut) red on 2 of 8 — the two
vulnerability-class advisories cargo-audit's `.cargo/audit.toml` already
waives with documented rationale:

| Workspace | cargo-audit (exit 0) | Findings | Class | Triage decision |
|---|---|---|---|---|
| root (`.`) | 0 (waived via `.cargo/audit.toml`) | RUSTSEC-2023-0071 (rsa), RUSTSEC-2026-0387 (crypto-bigint) | **vulnerability** | **WAIVED, dated** — carried into deny.toml (§1.2) |
| client | 0 (warnings allowed) | paste, proc-macro-error, zbus_names, zvariant, zvariant_derive (unmaintained); glib (unsound); async-executor 1.14.0 (yanked) | warning | audit lane owns (documented tradition); all transitive-only (measured) |
| crates | 0 | — | — | clean |
| fuzz | 0 (waived via `fuzz/.cargo/audit.toml`) | same two as root (fuzz depends on brain-server) | **vulnerability** | same two waivers cover it |
| shell/src-tauri | 0 | proc-macro-error (unmaintained), glib 0.18.5 (unsound) | warning | audit lane owns (the glib entry documents the tauri→gtk chain + the Dependabot dismissal) |
| tools/channel-bridge | 0 | — | — | clean |
| tools/signal-gateway | 0 | atomic-polyfill, bincode, proc-macro-error2 (unmaintained) | warning | audit lane owns; all transitive-only (measured) |
| tools/steward-harness | 0 | — | — | clean |

**Fixed-by-bump count: ZERO.** No advisory is fixed by an in-range bump; the
two vulnerability-class hits have no reachable fix (both documented in
`.cargo/audit.toml` with their re-check triggers). No product dependency
changed: **zero Cargo.lock delta** (verified with `git diff --name-only`).

### 1.2 The waiver inventory (dated-waiver reasons only)

Both waivers carry the review date in the reason text (the ratchet: a waiver
is a named act; `unused-ignored-advisory = "deny"` fails it the moment its
advisory leaves the root graph):

| id | crate | Reason (abridged; full text in deny.toml) | Review by |
|---|---|---|---|
| RUSTSEC-2023-0071 | rsa (Marvin Attack) | No fixed upstream (rsa 0.10.0-rc.18 via jsonwebtoken 11 + our own keygen); loopback single-tenant threat model; 0600 keys in 0700 dir; EdDSA available | **2027-02-07** |
| RUSTSEC-2026-0387 | crypto-bigint 0.5.5 | The reachable 0.7 copy was already bumped to 0.7.7; the 0.5.5 copy is unreachable ECDSA code (RS256-pinned jsonwebtoken — no ES256/384 verify ever runs) | **2027-02-07** |

The warning-class findings are deliberately NOT duplicated into deny.toml's
ignore list: their triage home is `.cargo/audit.toml` + the machine-evaluated
staleness pin (`no_ignored_advisory_is_fixed_in_the_current_lock`), whose
union semantics match the all-lockfile audit lane. Two waiver lists against
the same advisories would ratchet against each other.

### 1.3 The license allowlist (the commercial module's defensible set)

Measured: 44 distinct license expressions across the 8 graphs (cargo-deny's
own enumeration). Allowlist = the permissive family + four deliberate,
dated exceptions. SPDX `OR` expressions ride their allowed branch — e.g.
r-efi's `MIT OR Apache-2.0 OR LGPL-2.1-or-later` is satisfied through MIT;
LGPL needs and gets no entry. The non-trivial entries:

| License | Crate(s) forcing it | Why defensible |
|---|---|---|
| NCSA | libfuzzer-sys (`(MIT OR Apache-2.0) AND NCSA`) | OSI-approved permissive; fuzz workspace only, never shipped |
| MPL-2.0 | cssparser, cssparser-macros, dtoa-short, hpke-rs | file-level weak copyleft; linkable in an Apache-2.0 product |
| CDLA-Permissive-2.0 | webpki-roots, webpki-root-certs | DATA license for the bundled root store, not code |
| AGPL-3.0-only | tools/signal-gateway (first-party), libsignal-* | our own tool declares it; libsignal is the canonical Signal protocol stack it binds. **A third-party AGPL addition is a NEW named act** |
| Apache-2.0 WITH LLVM-exception | the rustc-shipped standard library | the WITH expression allowed whole (0.20.2 rejects a bare exception id — measured) |

The gate also caught **two of our own crates shipping no license
declaration** — `brain-client` (client/) and `brain-server-fuzz` (fuzz/);
both gained `license = "Apache-2.0"` (metadata only; zero lockfile delta).

### 1.4 The measured union constraint (the design decision the tool forced)

`unused-ignored-advisory = "deny"` is **per-run**, and one config runs
against eight different graphs. Measured: with the two waivers in the shared
config, the six satellite workspaces fail with
`error[advisory-not-detected]: advisory was not encountered` — the ratchet
bit the design itself. The resolution (stated in deny.toml + the CI job):

- the **FULL check** (advisories included) runs at the **ROOT workspace** —
  where the detector's semantics and the graph agree (root carries both
  waived advisories);
- the **policy checks** (licenses/bans/sources) run in the audit job's
  **8-workspace loop** (the coverage that actually needs per-workspace
  runs);
- the satellites' advisory coverage stays with the audit lane (all 8
  lockfiles + audit.toml + the staleness pin's union semantics).

Consequence (the ratchet working): a deny.toml waiver must match an advisory
in the ROOT graph or the root run itself reds.

### 1.5 The three AD-4 drills (red-first by config mutation — zero dependency churn)

| Drill | Mutation | Measured red | Revert |
|---|---|---|---|
| (a) license flip | drop `MIT` from the allowlist | crates workspace: 5× `error[rejected]: failed to satisfy license requirements` (bitflags 2.13.2, block-buffer 0.12.1, …) | green ✓ |
| (b) unused-waiver plant | `{ id = "RUSTSEC-9999-9999", reason = "planted drill waiver…" }` | `error[advisory-not-detected]: advisory was not encountered`, exit 1 | green ✓ |
| (c) bans | `deny = [{ name = "block-buffer", … }]` (a real transitive crate in the sha2 stack) | `error[banned]: crate 'block-buffer = 0.12.1' is explicitly banned` (exit 2 — cargo-deny's ban exit class) | green ✓ |

Final loop after all three reverts: **all 8 workspaces green** (root full
check + 7 policy checks).

---

## 2 · S2 — the capability pin's red-proof + the shell residual

### 2.1 The red-proof: TWO fence layers, both measured

**Layer 1 — the tauri build script (earlier than the pin test).** Plant
`"shell:open"` in `capabilities/main.json` → the brain-shell build script
fails BEFORE any test runs:

```
error: failed to run custom build command for `brain-shell v1.0.0`
Caused by: process didn't exit successfully: .../build-script-build (exit status: 1)
  Permission shell:open not found, expected one of core:default, core:app:default, ...
```

The permission does not exist in the capability universe — the shell plugin
is not a dependency (the manifest half of the pin), so a shell grant cannot
even be COMPILED. Red by name, at build time.

**Layer 2 — the pin test's own exact-set assert.** A plant that tauri
accepts (a third CORE permission, `core:window:allow-close`) reaches the
test and reds it by name:

```
failures:
thread 'sidecar_pins::the_shell_has_no_shell_plugin' panicked at src/sidecar_pins.rs:592:5:
assertion `left == right` failed: the main capability is EXACTLY core:default + the S-4 drag grant
test result: FAILED. 0 passed; 1 failed; ... 130 filtered out
```

Both plants reverted (`git checkout capabilities/main.json`); the pin green
again: `test result: ok. 1 passed`. The drill proves the pin is load-bearing
at BOTH layers — a shell-plugin grant is refused at build, and ANY
capability drift beyond the S-4 two-permission set reds the named test.
CVE-2026-17497 (below) is the 2026-dated precedent this fence answers.

### 2.2 The dev-dependency residual: measured and decided

`pnpm audit --audit-level high` (FULL scope, dev deps included), measured on
the baseline:

| Scope | Findings |
|---|---|
| `--prod` (the CI gate, shell.yml:126) | **0** |
| FULL (dev deps included) | **1** — deepmerge 4.3.1, **LOW** (GHSA-gcqx-w48m-w2h4: mergeObject() prototype poisoning), dev-toolchain-only path: `@sveltejs/vite-plugin-svelte > deepmerge` (build-time only; the shipped app bundles prod deps) |

**Decision (dated):** the gate STAYS `--prod --audit-level high` — no
widening. The one finding is LOW severity, dev-toolchain-only, and has **no
patched release yet** (`patched: None` in the advisory record). Widening the
gate to fail CI on an unpatchable dev-only LOW buys a permanently red lane,
which is a noise tax on every future signal. **Re-verify by 2027-02-07 or
when GHSA-gcqx-w48m-w2h4 gains a patched version, whichever comes first**;
if the count at re-verification exceeds the LOW-only class, widening becomes
a named act with the measured count.

### 2.3 The 2026 Tauri advisories: verified records + the core-version resolution

Both records verified against NVD + GHSA at execution (2026-11-07):

| | CVE-2026-95626 | CVE-2026-17497 |
|---|---|---|
| Product | Tauri framework (crates.io `tauri`) | NoteGen (codexu/note-gen) < 0.32.0 |
| Class | CSP nonce hardening bypass: with `data:`/`blob:` in `script-src`, those scheme sources stay active per CSP L3 → script execution without the nonce | shell plugin `shell:allow-execute` (bash/python, arbitrary args) in default capabilities → webview JS → `plugin:shell|execute` → OS commands → full RCE chained with XSS |
| Published | 2026-09-23 (CNA: reefs@jfrog.com — JFrog) | 2026-07-26 (CNA: reefs@jfrog.com; NVD status Analyzed) |
| CVSS 3.1 | 8.3 HIGH (AV:N/AC:H/PR:N/UI:R/S:C) | 8.3 HIGH (AV:N/AC:H/PR:N/UI:R/S:C) |
| NVD record | https://nvd.nist.gov/vuln/detail/CVE-2026-95626 (status **Deferred**; affected range open-ended `2.0.0 ≤ *`) | https://nvd.nist.gov/vuln/detail/CVE-2026-17497 |
| GHSA | GHSA-757j-xrg3-9g6p (unreviewed — CNA import, no ranges) | GHSA-r5ff-hq22-rhgv (unreviewed — CNA import) |

**The tree's core-version resolution (measured, recorded — the drill records,
it does not build):**

- **tauri 2.12.5** (lockfile, measured 2026-11-07) — above the plan's 2.12.4
  figure. For CVE-2026-95626 the VERSION is secondary to the CONFIGURATION:
  the vulnerable precondition is `data:`/`blob:` in `script-src`. The tree's
  CSP (`tauri.conf.json` `security.csp`) declares `script-src: 'self'` —
  **no `data:`, no `blob:`** — so the advisory's precondition is absent in
  the shipped configuration. `default-src`/`style-src`/`font-src`/
  `base-uri`/`form-action`/`worker-src` are `'self'`; `object-src 'none'`;
  `frame-ancestors 'none'`; `img-src 'self' data:` (images, not scripts).
  The JFrog remediation ("harden the CSP by removing data:/blob: from
  script-src") is a property our config already holds.
- **tauri-plugin-updater 2.14.1** — in the ≥ 2.12.0 fix band the XSS-to-RCE
  writeup names; the zip-stack advisories (RUSTSEC-2026-0367/0368/0369) stay
  windows-only-path (documented in audit.toml).
- **CVE-2026-17497** does not touch our tree (NoteGen is another product) —
  it is the 2026-dated instance of the AIRI pattern (shell permission + webview
  XSS → RCE), and §2.1's two-layer red-proof is our answer to that class:
  no shell plugin exists to grant, and the exact-set pin reds any capability
  drift.

---

## 3 · S3 — the two-file refresh

### 3.1 The ATLAS re-map (docs/OWASP_AGENTIC_2026.md, the 2026-11-07 addendum)

The addendum follows the file's own dated-addendum pattern and carries:

- **The re-map, commit-pinned**: ATLAS v6.1.0 / v2026.10 at
  `f56c1468731d6428e5bf875263b4643b46d67437` (released 2026-10-28) — 1 matrix,
  16 tactics, **129 techniques, 92 sub-techniques, 44 mitigations, 77 case
  studies**. Mapped at the CLASS level our rows track (injection/memory
  poisoning, privilege abuse, supply chain, execution, trust exploitation);
  the per-technique walk over all 129 is recorded as a FUTURE NAMED ACT, not
  an omission. Measured honesty: a first draft of the A-table had rows
  without source URLs — the AD-3 scan caught them before commit and the rows
  now each carry their ATLAS-matrix/research URL.
- **The 2026 incident classes** (6 rows, each source URL + landed mitigation):
  RubyGems agentic spam-publishing (the registry/doc-pipeline abuse),
  Chainalysis ("offensive agentic techniques", disclosed 2026-10-30), the
  Hugging Face misalignment disclosure, the **Argentine health-sector
  agentic campaign** (≥15 health orgs, clinical records — the incident class
  for a clinical-documentation product; CrowdStrike 2026-10-21), VAPOR PANDA
  (AI-driven ARTEX espionage), the Alibaba ROME agent defection (reverse SSH
  tunnel + GPU diversion; arXiv:2512.24873 v3).
- **The MCP-surface classes**: tool poisoning (2611.03555), Log-To-Leak
  (2610.36108), the destructive-approval split (2611.02527), capability
  laundering (2610.22301) — each mapped to the shipped control (the catalog
  pins, the tool-result envelope, the digest-bound approval, verbs×scope
  tokens).
- **The memory-persistence classes**: the disclosure ratchet (2610.28315 —
  its response-stage suppression is our carried-provenance exclude posture),
  HABIT-Bench (2610.24520), sticky collusion (2610.39660), intervention-
  history sabotage (2610.26935), AutoRedClaw (2610.27244).
- **Skill/approval classes**: CoSA (2610.35082), When Approval Becomes
  Execution (2610.23077), and the evaluator-blind-spot row (2610.34777)
  citing the fence law verbatim: **the advisory layer is not the defense —
  the deterministic gates are.**
- **The OpenClaw study (2610.35451), answered-vs-open, PREPRINT-stamped**:
  all 8 weakness categories (O1–O8, with their CWE classes and corpus
  percentages), the 6 failure mechanisms, and the M1–M7 mitigation families —
  each row answered (the shipped control) or open (the named owner: the v2.0
  "Cortex" ceilings, the operator-side boundaries), with the honesty note
  that the transfer is class-level (incident corpus, preprint, not our
  stack, not reproduced exploits).

### 3.2 THREAT_MODEL.md (R152)

- **The stamp moved** (the same commit as the rows): R117 (2026-10-12) →
  **R152 (2026-11-07)** — the next free round number (the changelog's
  highest was R151). The old stamp is gone, not forked (asserted by the
  pin).
- **Asset 4 (Binary/supply chain) +2 rows**: the dependency-surface row
  (the two lanes over every lockfile: audit + the NEW deny policy gate with
  the dated-waiver law; machete; the pnpm prod gate + the measured dev-dep
  residual decision) and the supply-chain provenance row
  (SIDECAR_PROVENANCE.json sealed kernel identity + the sidecar/updater pin
  family — **46 tests measured** (40 + 6), correcting the plan's "21" —
  + the minisign chain).
- **Asset 5 (Network transport) +1 row**: the webview-XSS elevation row —
  the strict CSP block vs CVE-2026-95626 (the `data:`/`blob:`-in-`script-src`
  precondition is ABSENT in our config), the two-layer capability-pin
  red-proof, and CVE-2026-17497 recorded as the 2026-dated precedent.
- **§1 + the harness-taxonomy coverage check** (arXiv:2610.40917, PREPRINT,
  the eight components verbatim from the paper's Table 1/Fig. 1 — measured,
  not paraphrased): Orchestration and planning · Context and memory · Tool
  and action interfaces · Operational knowledge · Sandboxing · The
  permission model · Verification/evaluation/observability · Execution
  runtime and lifecycle — **6 covered / 2 partial**, with the two partials
  owned (the operator trust domain; the v2.0 "Cortex" ceilings) and stated
  as decisions.
- **§4 + the PQC tracked row** (`post-quantum: tracked`): the updater's
  minisign/Ed25519 is the classical lane; NIST IR 8547 (transition draft)
  + the CISA PQC hub are cited (both URLs verified resolving 2026-11-07 —
  the first CISA URL guessed from memory 404'd and was corrected); harvest-
  now-decrypt-later does not threaten the updater's INTEGRITY use the way
  it threatens confidentiality; migration is a future named act with the
  revisit trigger (tauri-plugin-updater shipping a PQ/hybrid signer).

### 3.3 The AD-3 structural pin (src/docs_truth.rs)

`the_2026_threat_model_rows_carry_sources_and_both_stamps_moved` — the
docs_truth pattern (source scan, not sentiment):

- the 2026-11-07 addendum section: the ATLAS commit string + all four counts
  appear; **every DATA row carries a source URL** (header rows are excluded
  by markdown's own rule — the row directly above a separator; measured
  during authoring: the first A-table draft had URL-less rows and the scan
  caught them); the OpenClaw PREPRINT stamp + the fence-law sentence appear.
- THREAT_MODEL.md: the stamp carries exactly `R152 (2026-11-07)` AND the
  old `R117 (2026-10-12)` is GONE (move, not fork); the four load-bearing
  rows pinned by name (deny.toml, SIDECAR_PROVENANCE.json, CVE-2026-95626,
  post-quantum: tracked); the eight taxonomy component names present.

**Red-first proof**: the test was landed BEFORE the docs —
run 1: `panicked at src/docs_truth.rs:1421: OWASP_AGENTIC_2026.md must keep
its 2026-11-07 addendum section` (red);
run 2 (addendum landed, stamp unmoved): red at the stamp assert;
run 3 (both docs refreshed): **green**. The pin cannot be satisfied by
sentiment — it re-derives from the two files on every kernel test run.

---

## 4 · Verify-after (the final tree, 2026-11-07)

| Lane | Result |
|---|---|
| `cargo fmt --check` | clean ✓ |
| `cargo clippy --all-targets -- -D warnings` | clean ✓ |
| kernel `cargo test --features bench` | **3,817 passed / 0 failed** ✓ (the raw count; the badge's platform-normalized derivation: 3,835) |
| shell/src-tauri `cargo test` | **129 passed / 0 failed** ✓ (the baseline held: 129/2-ignored pre-round) |
| shell `pnpm check / lint / test` | 0 errors / clean / pass ✓ |
| `cargo deny check` — all 8 workspaces | green ✓ (re-run on the FINAL tree) |
| CI | the `deny` job mirrors the `audit` job's shape (binary pinned 0.20.2, binary-cache, the find-loop, found=1 aggregation); full check at root + policy checks ×8 — local equivalents green end-to-end |
| the census | re-derived: **3,893 → 3,894** (+1, the AD-3 pin, structural); the drift pin green against the committed table |
| the badge | re-derived (badges.sh, the platform-normalized skip set): **3,834 → 3,835** |
| the lock delta | **ZERO Cargo.lock changes across the entire round** (`git diff 5c4b1109..HEAD`) — no product dependency changed; the only manifest delta is the license metadata on brain-client + brain-server-fuzz |
| capability change | NONE — `capabilities/main.json` byte-identical to baseline (both drill plants reverted) |
| gate behavior | unchanged — the audit job untouched; the deny job ADDS a lane; the machine never auto-gates (the AD-3 pin reads docs; the deny gate reds to a human) |

**Commits (in order):** `b5d7938d` (S1 the deny gate) → `96c1e7b4` (S2 the
red-proof + the residual) → `68eff54c` (S3 the two-file refresh + the AD-3
pin) → `ea65de79` (the census/badge re-derivation) → this evidence close-out.

## 5 · Exit criteria (the execution prompt's §4)

- [x] `deny.toml` landed with dated waivers only (2, both root/fuzz-graph,
      review by 2027-02-07); the unused-waiver detector ON
      (`unused-ignored-advisory = "deny"`, drill-proven); the deny CI job
      mirroring the audit loop (the measured per-run union constraint
      shaped it: full@root + policy×8, documented in the job + deny.toml);
      all 8 workspaces green
- [x] The capability pin red-proven at BOTH layers (the tauri build script
      refuses a shell-plugin permission by name; the exact-set pin reds any
      drift by name); the dev-audit residual decided and dated (1 LOW,
      dev-only, gate stays --prod, re-verify 2027-02-07)
- [x] The two-file refresh: ATLAS commit + counts cited; every new row
      sourced + mitigated; both stamps moved (R152 / Last-reviewed
      2026-11-07)
- [x] The OpenClaw answered-vs-open enumeration recorded (O1–O8, the six
      mechanisms, M1–M7 — each answered or owner-named); the PQC tracked
      row landed
- [x] Zero product-dependency changes; all lanes green; the plan's §0.1
      stands as the ground record; this file complete

**Deviations from the plan text, each measured and recorded:** the deny job
runs the advisories check at the ROOT only (the per-run unused-waiver
detector reds satellite workspaces — measured, §1.4); the sidecar pin
family is 46 tests, not the plan's 21 (measured); tauri is 2.12.5, not
2.12.4 (measured); the capability drill red-proofs TWO layers (the plan's
single drill hit the earlier fence — recorded, §2.1).
