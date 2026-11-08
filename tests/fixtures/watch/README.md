# The watch fixture corpus (IMPROVE08)

Recorded feed transcripts for the research watch's parsers and the replay
transport. **FROZEN AT COMMIT; `SHA256SUMS` (this directory) is the
authority.** CI never performs a live fetch (RW-6, the SA-4 discipline
inherited from plan 07): every parser test parses these bytes through the
replay path. The live probes are operator-gated runs recorded into
`docs/IMPROVE08_EVIDENCE_<date>.md` — the plan-05/07 dual.

## Provenance — the live probes (2026-11-09, one session)

Every file below is the VERBATIM response body of a live HTTP GET made
2026-11-09 (UTC 04:16) from this tree (`brain-server` at `597fe74f`),
recorded to `/tmp/i08-probe/` and copied unmodified — except the two files
explicitly marked SYNTHETIC/TRIMMED here. The probe commands:

```sh
curl -fsSL "https://export.arxiv.org/api/query?search_query=cat:cs.CR+AND+all:prompt+injection&start=0&max_results=3" -o arxiv-atom.xml
curl -fsSL "https://export.arxiv.org/rss/cs.CL" -o arxiv-rss-sunday-empty.xml
curl -fsSL "https://lobste.rs/rss.rss" -o lobsters.rss
curl -fsSL "https://www.cisa.gov/cybersecurity-advisories/all.xml" -o cisa-all.xml   # 5,685,118 bytes live
curl -fsSL "https://www.nist.gov/news-events/news/rss.xml" -o nist.rss
```

All five returned HTTP 200 in the same session. A `<!DOCTYPE`/`<!ENTITY`
scan over every probe returned ZERO occurrences (the parser refuses both by
name regardless — the XXE posture, never expanded).

## The files

| file | shape | status |
|---|---|---|
| `arxiv-atom.xml` | Atom 1.0, the arXiv API lane (3 `<entry>`: id-with-version, published vs updated, summary, authors, three links, categories, `arxiv:doi`/`arxiv:comment`) | LIVE, whole |
| `arxiv-rss-sunday-empty.xml` | RSS 2.0, ZERO `<item>` + `<skipDays>` Sat/Sun | LIVE, whole — the empty-state fixture (0 items is honest, never an error) |
| `lobsters.rss` | RSS 2.0 with `<ttl>120`, `<guid>` identity, RFC-822 `-0600` dates, entity-encoded `<description>` | LIVE, whole |
| `nist.rss` | RSS 2.0 declaring a dozen namespaces (dc/atom/content/foaf/og/rdfs/schema/sioc/sioct/skos/xsd) | LIVE, whole — the prefix-tolerance fixture |
| `cisa-trim-prefix3.xml` | RSS 2.0, the first 18,835 bytes of the 5.7 MB `all.xml` — the RAW BYTE PREFIX through the third `</item>`, UNTERMINATED (the document close is truncated away) | LIVE, byte-prefix TRIM (5.7 MB does not belong in git; the trim is marked, the prefix is honest) |
| `arxiv-rss-weekday-items.SYNTHETIC.xml` | the weekday RSS-with-items shape (CDATA/escaped descriptions, guid identity) | **SYNTHETIC** — probed 2026-11-08/09 in UTC-Sunday the live feed carried zero items; the weekday shape follows the recorded feed grammar and is NOT a transcript |

## The replay map (the transport's URL → fixture table)

The replay transport maps the lane's feed URL to the fixture bytes:

- `https://export.arxiv.org/api/query…` → `arxiv-atom.xml`
- `https://export.arxiv.org/rss/cs.CL` → `arxiv-rss-sunday-empty.xml` (the live-recorded state; the weekday shape is the marked-synthetic file, parsed directly in tests, never served by the URL map)
- `https://lobste.rs/rss.rss` → `lobsters.rss`
- `https://www.cisa.gov/cybersecurity-advisories/all.xml` → `cisa-trim-prefix3.xml` (CI never downloads 5.7 MB — the replay serves the marked prefix)
- `https://www.nist.gov/news-events/news/rss.xml` → `nist.rss`

A URL the corpus does not know is the named `watch_replay_gap` refusal,
never a silent live fallback (RW-6).

## Identity + hash recipe (the dedupe law's audit trail)

`item_key` is the named per-lane hash (xxh3-64, hex) of the feed's own
identity: the arXiv id WITH version (`2410.05451v3`) for API lanes; `<guid>`
else the trimmed `<link>` for RSS; the `<id>` for generic Atom. The
near-key is xxh3-64 of the title lowercased, punctuation-stripped,
whitespace-collapsed. Both are model-absent (RW-4). The RAW identity rides
the proposal header the human reviews.

## Drift discipline

Re-record under a NEW name on drift; never overwrite (the plan-07 corpus
law). A fixture whose SHA256 no longer matches is a broken pin, not a
hint.
