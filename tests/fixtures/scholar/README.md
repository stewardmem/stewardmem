# The scholar fixture corpus (IMPROVE07)

Recorded API transcripts for the scholarly source adapters. **FROZEN AT
COMMIT; `SHA256SUMS` (this directory) is the authority.** CI never performs a
live call (SA-4): every adapter test parses these bytes through the replay
path. The live probes are operator-gated runs recorded into
`docs/IMPROVE07_EVIDENCE_<date>.md` — the plan-05 ocr-real dual.

## Provenance — the live probes (2026-11-08, one session)

Every `cr-*`/`epmc-*`/`pm-*` file below is the VERBATIM response body of a
live API call made 2026-11-08 from this tree (`brain-server` at `2bcaa5e7`),
recorded to `/tmp/i07-probe/` and copied unmodified. The probe commands:

```sh
curl -fsSL "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:%2210.1016/S0140-6736(97)11096-0%22&format=json&resultType=core"
curl -fsSL "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:%2210.1371/journal.pone.0266789%22&format=json&resultType=core"
curl -fsSL "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:%2210.1016/S0140-6736(12)61729-2%22&format=json&resultType=core"
curl -fsSL "https://api.crossref.org/works/10.1016/S0140-6736(97)11096-0?mailto=…"
curl -fsSL "https://api.crossref.org/works/10.1088/1361-6595/aaebdb?mailto=…"
curl -fsSL "https://api.crossref.org/works/10.1371/journal.pone.0266789?mailto=…"
curl -fsSL "https://api.crossref.org/works/10.1016/S0140-6736(12)61729-2?mailto=…"
curl -fsSL "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id=9500320&retmode=json"
curl -fsSL "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id=35476794,23245607&retmode=json"
curl -fsSL "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esearch.fcgi?db=pubmed&term=10.1016%2FS0140-6736(97)11096-0%5Bdoi%5D&retmode=json"
```

All three APIs returned HTTP 200 in the same session; no key was presented
(Europe PMC and Crossref polite pool need none; PubMed at the no-key 3 req/s
tier). The live headers observed on the Crossref response are recorded in the
evidence file (`x-rate-limit-limit: 5` / `x-rate-limit-interval: 1s`).

## The fixtures

| file | what it is | the retraction signal |
|---|---|---|
| `epmc-wakefield-core.json` | Europe PMC `resultType=core`, the Wakefield paper (PMID 9500320) | `pubTypeList` carries exactly `"Retracted Publication"`; `commentCorrectionList` records `"Retraction in"` ×2 (the 2004 and 2010 notices, DOIs extracted where the reference carries one) and `"Expression of concern in"` ×1 — recorded, never floored |
| `cr-wakefield.json` | Crossref `works/{doi}`, the same paper — **THE CORRECTION TRAP FIXTURE** | `updated-by` carries BOTH a 2004 `type:"correction"` AND a 2010 `type:"retraction"` (source `retraction-watch`, `record-id` present). Floor vocabulary is EXACTLY `type == "retraction"` (SA-5): notice DOI `10.1016/s0140-6736(10)60175-4`, date `2010-02-06`. An adapter flooring on ANY `updated-by` entry demotes every corrected paper |
| `pm-wakefield.json` | PubMed esummary, the same paper | `pubtype` carries exactly `"Retracted Publication"` |
| `pm-esearch-wakefield.json` | PubMed esearch — the TWO-STEP's first leg (DOI term → uid), same paper | `idlist` carries `9500320`; the esummary leg is `pm-wakefield.json` |
| `cr-correction-control.json` | Crossref, the control arm: a corrected-but-never-retracted record (IOP Plasma Sources Sci. Technol., DOI `10.1088/1361-6595/aaebdb`) | `updated-by` carries ONE entry, `type:"correction"` (source `publisher`) — the parse yields `retraction: None`. This record NEVER floors |
| `epmc-clean-oa.json` | Europe PMC core, clean OA paper (PLOS ONE, PMID 35476794) | no retraction; `license: "cc by"` PRESENT (the OA shape), `isOpenAccess: "Y"` |
| `epmc-clean-noa.json` | Europe PMC core, clean non-OA paper (Lancet GBD YLDs, PMID 23245607) | no retraction; `license` ABSENT (the non-OA shape — Option, never assumed), `isOpenAccess: "N"` |
| `cr-clean-oa.json` / `cr-clean-noa.json` | Crossref shapes of the two clean records | no `updated-by` |
| `pm-clean.json` | PubMed esummary, both clean PMIDs in one call | `pubtype` without `"Retracted Publication"` |
| `malformed-raw-bytes.json` | SYNTHETIC: invalid JSON bytes | refused by name (`malformed_json`) |
| `malformed-epmc-truncated.json` | SYNTHETIC: truncated EPMC shape (unbalanced braces) | refused by name (`malformed_json`) |
| `wrong-schema-epmc.json` | SYNTHETIC: valid JSON, EPMC envelope, no usable record fields | refused by name (`wrong_schema`) |
| `wrong-schema-crossref.json` | SYNTHETIC: valid JSON, Crossref envelope, wrong `message` shape | refused by name (`wrong_schema`) |
| `wrong-schema-pubmed.json` | SYNTHETIC: valid JSON, PubMed envelope, record without the pubtype/articleids surface | refused by name (`wrong_schema`) |
| `transcript-429.json` | SYNTHETIC rate-limit transcript (Crossref shape) | the backoff pin's input. **A live 429 was deliberately NOT triggered** — provoking one means hammering the API, which violates the rate etiquette the adapter exists to honor. Headers model the live-observed `x-rate-limit-*` pair |

The SYNTHETIC fixtures are marked as such in this README and in the file
bodies where a body exists; their provenance is the plan's design, not a
live capture. Everything else is verbatim.

## The expected-metadata files (`expected/`)

One machine-checked parse contract per live fixture: the fields the parser
MUST produce (`doi` normalized lowercase, `retracted` the exact-vocabulary
derivation, `updates_recorded` the recorded-not-floored count). The parse
tests in `src/scholar.rs` read these and assert equality — a parser change
that drifts the semantics reds here before any adapter is touched.

## Discipline

- A NEW API version is EXPECTED to drift — re-run the probe, re-commit under
  a new name, never overwrite (the docling-goldens rule).
- The Wakefield record is the round's live-verified fixture (the ocr-real
  dual): the trap arm AND the retracted e2e witness parse the same bytes CI
  replays.
- All three APIs returned the Wakefield DOI LOWERCASED in these transcripts
  (`10.1016/s0140-6736(97)11096-0`). The named DOI normalizer (SA-6) remains
  the single lowercase/strip point every identity goes through — it exists
  for the inputs these APIs do NOT normalize: operator-supplied DOIs,
  `https://doi.org/…` resolver prefixes, and mixed-case Crossref/PMC
  reference strings.
