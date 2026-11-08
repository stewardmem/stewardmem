# SCHOLAR ADAPTERS — Europe PMC / Crossref / PubMed (IMPROVE07)

The scholarly source lane: read-only fetchers over the three publisher APIs,
the published retraction signal read **deterministically at zero tokens**.

## The laws (inherited, restated because this lane carries them)

- **Deterministic zero-token.** The publishers publish the retraction signal
  in their metadata — Crossref's `updated-by` array (the Retraction-Watch
  integration), Europe PMC's `pubTypeList`, PubMed's `pubtype` — so the
  kernel READS it. No ML retraction detection exists (arXiv:2502.15429 /
  Pub-Guard-LLM is the foil: a model that guesses retraction status is a
  determinism violation by construction). No LLM anywhere in the lane.
- **An adapter is a FETCHER, never a promoter.** Fetched records render to
  YAML frontmatter + abstract body and cross the EXISTING markdown door's
  internals — screen, strip, dedupe, trust-label bounds, HITL posture — by
  literally calling `ingest_markdown`. No private insert exists to bypass
  with. The HITL gate is untouched; the ONLY tier lift is the audited HITL
  `retraction_override` act (plan S3).
- **No data leaves except the allowlisted hosts.** The client construction
  pins `www.ebi.ac.uk | europepmc.org | eutils.ncbi.nlm.nih.gov |
  api.crossref.org` structurally and refuses any other host BY NAME;
  `redirect::Policy::none()` — a redirect is an allowlist bypass, never
  followed (the provider_http law).
- **CI never performs a live call (SA-4).** Every test runs the adapters
  over the REPLAY transport against the frozen fixture corpus
  (`tests/fixtures/scholar/`, SHA256SUMS + probe provenance in its README).
  The live pulls are operator-gated runs recorded into the evidence file
  (the plan-05 ocr-real dual).
- **Named refusals.** Every failure is a closed, named code — no API bytes
  flow into error strings.

## 1 · The correction trap (the load-bearing fact)

A retracted article's Crossref record carries `updated-by[]` — and on the
same record the array holds BOTH a 2004 `type:"correction"` entry AND the
2010 `type:"retraction"` entry (verified live 2026-11-08 on DOI
`10.1016/S0140-6736(97)11096-0`, the Wakefield paper). An adapter that
floors on ANY `updated-by` entry demotes every corrected paper in the
corpus. The floor's vocabulary is EXACTLY:

| API | the exact signal |
|---|---|
| Crossref | `updated-by[].type == "retraction"` |
| Europe PMC | `pubTypeList.pubType` contains exactly `"Retracted Publication"` |
| PubMed | `pubtype` contains exactly `"Retracted Publication"` |

Everything else — `correction`, `expression-of-concern` (present on the
same Wakefield record as `"Expression of concern in"` in Europe PMC's
`commentCorrectionList`), `"Retraction in"` comment relationships — is
RECORDED as provenance (`SourceRecord.updates`) and NEVER floors (SA-5;
the honest-data caution of arXiv:2403.13339: upstream signals can be wrong,
so the floor is a recorded signal plus a HITL override, never a silent
demotion).

## 2 · Three adapters, three postures

| adapter | role | auth | the retraction signal | the audit-grade fields |
|---|---|---|---|---|
| **Europe PMC** | PRIMARY — no key | none ("public API", privacy-notice terms) | `pubTypeList` exact string | `commentCorrectionList` notice relationships |
| **Crossref** | the retraction WITNESS | none; `?mailto=` joins the polite pool | `updated-by[].type` exact | notice DOI + retraction date + Retraction-Watch `record-id` |
| **PubMed** | the key-configured FALLBACK | free key file for the 10 req/s tier (3 req/s without) | `pubtype` exact string | PMID + articleids |

Europe PMC quirks recorded by probe: DOIs return LOWERCASED (the named DOI
normalizer is the single lowercase/strip point — SA-6, arXiv:2603.03126's
cross-source norm); `license` is ABSENT on non-OA records (Option, never
assumed); `resultType=core` is the parse shape.

## 3 · The route: `POST /scholar/fetch`

Token-gated operator route (the AuthZ `Write` gate — it writes through the
door). Payload: `{"doi": "…", "adapter": "auto"|"europepmc"|"pubmed"}` (or
`{"pmid": "…"}`; exactly one identifier). The handler:

1. resolves the profile (below) — **unconfigured → `scholar_unconfigured`
   503** (OFF unless configured, SA-3);
2. fetches via the selected adapter over the allowlist-pinned transport;
   the Crossref witness cross-checks when configured (plan S3);
3. renders YAML frontmatter (the scholarly vocabulary:
   `doi | pmid | source_api | fetched_at | license | retracted`) + a body
   whose first line is the deterministic status line
   (`Retracted: <date> — notice DOI <doi>` when retracted) so reviewer and
   chunks both see it;
4. calls `ingest_markdown` — the REAL door handler, not a copy (the
   no-bypass pin reads the call): title law, 1 MiB cap, screen, chunk,
   HITL posture, dedupe, the ingest audit row — unchanged;
5. writes ONE fetch audit row per on-state fetch (the allowlisted host,
   the identifier, the outcome) and persists the machine flag into the
   row's `scholar_meta` JSON column (ONE guarded ALTER, schema 1.32.31);
   a deduped re-serve still refreshes the flag.

The search kind is `scholarly` (`ADD_SOURCES_FOR_JWT` + `INGEST_KINDS`
gain it in the same commit; `origin_for_source("scholarly")` derives
`imported` unchanged — a fetched record is not a trusted local write).

## 4 · The config surface (the GDL all-or-nothing profile pattern)

| variable | meaning |
|---|---|
| `BRAIN_SCHOLAR_EPMC_ENABLED` | Europe PMC adapter ON (no key needed) |
| `BRAIN_SCHOLAR_PUBMED_ENABLED` | PubMed adapter ON — REQUIRES the key file (see below) |
| `BRAIN_SCHOLAR_PUBMED_API_KEY_FILE` | the NCBI API key, read through the secret-file law (chmod 600 / 0400, no symlinks, never the key inline, never in an env value) |
| `BRAIN_SCHOLAR_CROSSREF_MAILTO` | the polite-pool address — its presence IS the witness's on-switch |
| `BRAIN_SCHOLAR_RATE_PER_SEC` | the LOCAL throttle, default 1 |

Unconfigured = OFF. A partial profile = a named config error at boot
(`validate_scholar_config`, the GDL pattern: `BRAIN_SCHOLAR_PUBMED_ENABLED`
without its key file is a fatal config, not a degraded adapter). Errors are
fixed strings — never a configured value.

## 5 · Rate limits honored by construction (SA-7)

- **The kernel's own token bucket is the compliance mechanism.** Europe PMC
  publishes NO numeric limit (fair-use etiquette); Crossref's live headers
  say `x-rate-limit-limit: 5` per `x-rate-limit-interval: 1s`; NCBI is
  3 req/s without a key, 10 with. The kernel defines ONE conservative local
  throttle — `BRAIN_SCHOLAR_RATE_PER_SEC`, default **1** — per adapter,
  named constants, never the API's problem to discover.
- **Named backoff on 429/5xx**: a bounded retry honoring `retry-after`
  (capped), then the named refusal `scholar_rate_limited` (the decision is
  a pure function, tested against the recorded 429 transcript — itself
  honestly synthetic: a live 429 was NOT provoked, because hammering the
  API to trigger it violates the etiquette this lane exists to honor).
- **The `mailto` polite pool on Crossref** and a User-Agent identifying the
  product on every request.
- **Etiquette, stated**: bulk consumption belongs to the publishers' bulk
  services (Europe PMC OAI-PMH/FTP, Crossref snapshots), never to hammering
  the REST endpoints. This lane is for operator-initiated single-record
  fetches and (plan 08) a paced watch — not corpus harvests.
