//! Scholarly source adapters (IMPROVE07): Europe PMC / Crossref / PubMed.
//!
//! THE DETERMINISTIC ZERO-TOKEN LAW. The publishers already publish the
//! retraction signal in their metadata — Crossref's `updated-by` array (the
//! Retraction-Watch integration), Europe PMC's `pubTypeList`, PubMed's
//! `pubtype` — so this module READS that published signal at zero tokens.
//! No ML retraction detection exists in this plan (arXiv:2502.15429
//! (Pub-Guard-LLM) is the foil: a model that guesses retraction status is a
//! determinism violation by construction, not an alternative design).
//!
//! THE CORRECTION TRAP (the round's load-bearing fact, verified LIVE on
//! 2026-11-08 against DOI `10.1016/S0140-6736(97)11096-0`). A retracted
//! article's Crossref `updated-by` array carries BOTH a 2004
//! `type:"correction"` entry AND the 2010 `type:"retraction"` entry on the
//! SAME record. An adapter that floors on ANY `updated-by` entry demotes
//! every corrected paper in the corpus. The floor's vocabulary is therefore
//! EXACTLY [`CROSSREF_RETRACTION_TYPE`] for Crossref and exactly
//! [`RETRACTED_PUB_TYPE`] for Europe PMC/PubMed — every other update kind
//! (`correction`, `expression-of-concern`, …) is RECORDED
//! ([`SourceRecord::updates`]) and NEVER floors (SA-5; the honest-data
//! caution of arXiv:2403.13339 — upstream signals can be wrong, so the floor
//! is a recorded signal plus a HITL override, never a silent demotion).
//!
//! AN ADAPTER IS A FETCHER, NEVER A PROMOTER. Records produced here enter
//! the store only through the EXISTING markdown door's internals; this
//! module holds no ingest, no gate, no trust-tier write. The retrieval-side
//! consumer is the trust floor (`search::tiebreak_cmp`), which reads the
//! persisted retraction flag — the floor and the flag are the plan's S3.
//!
//! PARSERS ARE DETERMINISTIC. Each parser is a pure `(&str, &str) ->
//! Result<SourceRecord, ScholarError>` over RECORDED API bytes (the replay
//! discipline: CI never performs a live call, SA-4). The live-verified
//! fixture corpus lives at `tests/fixtures/scholar/` (SHA256SUMS is the
//! authority; the README carries the probe provenance). `fetched_at` is a
//! PARAMETER, not a clock read — a parser over fixed bytes with a fixed
//! timestamp is byte-reproducible.
//!
//! NO DATA LEAVES EXCEPT THE ALLOWLISTED HOSTS. This module performs no I/O
//! at all; the network lane is the adapter layer (plan S2), which is
//! operator-configured (the GDL all-or-nothing profile pattern), pinned to
//! `www.ebi.ac.uk | europepmc.org | eutils.ncbi.nlm.nih.gov |
//! api.crossref.org` by construction, and constructed with
//! `redirect::Policy::none()` — a redirect is an allowlist bypass.

use serde_json::Value;

/// The exact Europe PMC / PubMed pubType string that means retraction.
/// Verified live 2026-11-08 on the Wakefield record: both APIs carry
/// `"Retracted Publication"` verbatim. The floor's vocabulary is EXACTLY
/// this string — no substring match, no case folding (SA-5).
pub const RETRACTED_PUB_TYPE: &str = "Retracted Publication";

/// The exact Crossref `updated-by[].type` value that means retraction.
/// The trap discipline: `updated-by[].type` ALSO carries `"correction"` on
/// the same retracted record — flooring on any entry is the bug this
/// constant's narrowness exists to prevent.
pub const CROSSREF_RETRACTION_TYPE: &str = "retraction";

/// Which API produced a record. `as_str()` is the frontmatter
/// `source_api` vocabulary (lowercase, stable).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceApi {
    EuropePmc,
    Crossref,
    PubMed,
}

impl SourceApi {
    pub fn as_str(&self) -> &'static str {
        match self {
            SourceApi::EuropePmc => "europepmc",
            SourceApi::Crossref => "crossref",
            SourceApi::PubMed => "pubmed",
        }
    }
}

/// The identifier an adapter fetches. The DOI/Pmid forms go through
/// [`normalize_doi`] / digit validation at parse time — the identifier is
/// the dedupe key (plan-08's seen table) and must be canonical there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceIdentifier {
    Doi(String),
    Pmid(String),
    Query(String),
}

/// The kind of a RETRACTION. The closed vocabulary: only `Retraction`
/// exists today. A future signal class (`expression-of-concern`, …) is a
/// RECORDED update ([`SourceUpdate`]), never a `Retraction` — the type
/// system makes "floors" and "records" different constructions (SA-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetractionKind {
    Retraction,
}

impl RetractionKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            RetractionKind::Retraction => "retraction",
        }
    }
}

/// One update relationship on a scholarly record, RECORDED verbatim.
/// These never floor by themselves: the floor derivation filters the
/// EXACT retraction vocabulary out of this list ([`SourceRecord::retraction`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUpdate {
    /// The notice's DOI (Crossref `updated-by[].DOI`; Europe PMC
    /// `commentCorrectionList` references carry `doi:` tokens — extracted
    /// and normalized where present).
    pub doi: Option<String>,
    /// The API's OWN vocabulary for the update kind, kept verbatim
    /// (`"correction"`, `"retraction"`, `"Retraction in"`,
    /// `"Expression of concern in"`, …). The floor reads only the exact
    /// retraction values; everything else is provenance.
    pub kind: String,
    pub label: Option<String>,
    /// The relationship's source (`"retraction-watch"`, `"publisher"`, …)
    /// — the audit-grade witness field (arXiv:2506.21232's dynamics lane).
    pub source: Option<String>,
    /// ISO date (zero-padded) when the API dates the update.
    pub date: Option<String>,
}

/// The retraction relationship, derived ONLY by exact-vocabulary match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retraction {
    /// The retraction notice's DOI (the Crossref witness supplies it —
    /// the audit-grade field; Europe PMC/PubMed flag the status without
    /// naming the notice, so it stays `None` there).
    pub notice_doi: Option<String>,
    /// The retraction date (ISO), when the API carries it.
    pub date: Option<String>,
    pub kind: RetractionKind,
}

/// One scholarly record as an adapter produced it. Metadata is verbatim
/// provenance; `doi` is the NORMALIZED identity ([`normalize_doi`]); the
/// retraction pair (`updates` + `retraction`) is the trust-floor input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    pub doi: Option<String>,
    pub pmid: Option<String>,
    pub title: Option<String>,
    pub authors: Option<String>,
    pub journal: Option<String>,
    pub pub_year: Option<String>,
    /// The abstract, VERBATIM from the API (content, never corrected —
    /// the no-LLM law; presentation is the renderer's concern).
    pub abstract_text: Option<String>,
    /// The license, when the API publishes one. Option, never assumed:
    /// ABSENT on non-OA records (verified live — the Wakefield record
    /// carries none).
    pub license: Option<String>,
    pub open_access: Option<bool>,
    /// A full-text URL when the API offers one (the OA copy, deterministically
    /// preferred over the paywalled resolver link).
    pub fulltext_url: Option<String>,
    /// Every update relationship, RECORDED (never floored — SA-5).
    pub updates: Vec<SourceUpdate>,
    /// The retraction, derived ONLY by exact-vocabulary match against the
    /// published signal. `None` is the honest "no retraction signal".
    pub retraction: Option<Retraction>,
    /// RFC3339 fetch stamp (a parser PARAMETER — see the module doc).
    pub fetched_at: String,
    pub api: SourceApi,
}

impl SourceRecord {
    /// The deterministic status line the rendered body carries (the human
    /// reviewer's and the chunk text's view; the persisted flag is the
    /// machine's). Zero-token, pure formatting.
    pub fn status_line(&self) -> Option<String> {
        let r = self.retraction.as_ref()?;
        let date = r.date.as_deref().unwrap_or("date unknown");
        Some(match &r.notice_doi {
            Some(doi) => format!("Retracted: {} — notice DOI {}", date, doi),
            None => format!("Retracted: {}", date),
        })
    }
}

/// The named refusals. Every failure is a closed variant with a fixed
/// message — no API bytes flow into error strings (the GDL config law).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScholarError {
    /// The bytes are not JSON at all.
    MalformedJson { api: &'static str },
    /// Valid JSON, but not the API's record shape (an identity field is
    /// structurally absent).
    WrongSchema {
        api: &'static str,
        field: &'static str,
    },
    /// The envelope is well-formed but carries no records.
    EmptyResult { api: &'static str },
}

impl std::fmt::Display for ScholarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScholarError::MalformedJson { api } => {
                write!(
                    f,
                    "scholar_malformed_json: {api} response is not valid JSON"
                )
            }
            ScholarError::WrongSchema { api, field } => {
                write!(f, "scholar_wrong_schema: {api} response has no {field}")
            }
            ScholarError::EmptyResult { api } => {
                write!(f, "scholar_empty_result: {api} response carries no records")
            }
        }
    }
}

/// THE DOI NORMALIZER — total and single (SA-6). Lowercase; strip the
/// resolver prefixes (`https://doi.org/`, `http(s)://dx.doi.org/`,
/// `doi.org/`, `dx.doi.org/`, `doi:`); trim whitespace. Total: any input
/// yields a string (empty in → empty out); it never panics, never errors.
/// SINGLE: the record identity, the door's dedupe key, and plan-08's
/// seen-table key ALL go through this one function — a second normalizer
/// is a found bug (arXiv:2603.03126's cross-source norm).
pub fn normalize_doi(raw: &str) -> String {
    let mut s = raw.trim();
    const PREFIXES: [&str; 6] = [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
        "doi.org/",
        "doi:",
    ];
    // One pass is enough in practice, but loop so stacked prefixes
    // (`https://doi.org/doi:10.…`) still normalize — total means total.
    loop {
        let lower_prefix_scan = s.to_ascii_lowercase();
        let mut stripped = false;
        for p in PREFIXES {
            if lower_prefix_scan.starts_with(p) {
                s = s[p.len()..].trim_start_matches('/');
                stripped = true;
                break;
            }
        }
        if !stripped {
            break;
        }
    }
    s.trim().to_ascii_lowercase()
}

/// Flatten a Crossref-style date-parts array (`[[2010,2,6]]`) into a
/// zero-padded ISO date. Returns `None` for absent/empty parts.
fn date_parts_to_iso(v: Option<&Value>) -> Option<String> {
    let parts = v?.get(0)?.as_array()?;
    if parts.is_empty() {
        return None;
    }
    let nums: Vec<String> = parts
        .iter()
        .map(|p| p.as_u64().map(|n| format!("{:02}", n)).unwrap_or_default())
        .collect();
    if nums.iter().any(|s| s.is_empty()) {
        return None;
    }
    Some(nums.join("-"))
}

/// Extract a DOI from a free-text reference string carrying a
/// `doi: 10.xxxx/…` token (Europe PMC `commentCorrectionList` references),
/// normalized. Deterministic: the first whitespace-delimited token after
/// the LAST `doi:` occurrence (a DOI never contains whitespace), with
/// trailing sentence punctuation trimmed. No regex dependency (the deny
/// law). `to_ascii_lowercase` preserves byte offsets, so the scan indexes
/// the original string safely.
fn doi_from_reference(reference: &str) -> Option<String> {
    let lower = reference.to_ascii_lowercase();
    let idx = lower.rfind("doi:")?;
    let rest = reference[idx + 4..].trim_start();
    let token = rest.split_whitespace().next()?;
    let token = token.trim_end_matches(['.', ';', ')', ',']);
    if token.is_empty() {
        None
    } else {
        Some(normalize_doi(token))
    }
}

// ── Europe PMC ───────────────────────────────────────────────────────────

/// Parse one Europe PMC `resultType=core` search response (the PRIMARY
/// adapter's shape — no key). The response is a search envelope:
/// `resultList.result[]`; the FIRST record is the fetch target.
///
/// The retraction derivation: `pubTypeList.pubType` contains EXACTLY
/// [`RETRACTED_PUB_TYPE`] (no substring, no case fold — SA-5).
/// `commentCorrectionList.commentCorrection` entries are RECORDED as
/// [`SourceUpdate`]s (their `reference` strings carry `doi:` tokens — the
/// notice DOIs — and their `type` strings include `"Retraction in"` and
/// `"Expression of concern in"`; recording is not flooring).
pub fn parse_epmc_core(json: &str, fetched_at: &str) -> Result<SourceRecord, ScholarError> {
    const API: &str = "europepmc";
    let v: Value =
        serde_json::from_str(json).map_err(|_| ScholarError::MalformedJson { api: API })?;
    let results = v
        .get("resultList")
        .and_then(|rl| rl.get("result"))
        .and_then(|r| r.as_array())
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "resultList.result",
        })?;
    let rec = results
        .first()
        .ok_or(ScholarError::EmptyResult { api: API })?;
    // Identity presence: a result object with none of id/doi/title/pmid is
    // not a record — the wrong-schema refusal (the malformed-envelope line).
    if rec.get("id").and_then(|x| x.as_str()).is_none()
        && rec.get("doi").and_then(|x| x.as_str()).is_none()
        && rec.get("title").and_then(|x| x.as_str()).is_none()
        && rec.get("pmid").and_then(|x| x.as_str()).is_none()
    {
        return Err(ScholarError::WrongSchema {
            api: API,
            field: "resultList.result[0]",
        });
    }

    let doi = rec
        .get("doi")
        .and_then(|x| x.as_str())
        .map(normalize_doi)
        .filter(|s| !s.is_empty());
    let pmid = rec.get("pmid").and_then(|x| x.as_str()).map(str::to_string);
    let title = rec
        .get("title")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let authors = rec
        .get("authorString")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let journal = rec
        .get("journalInfo")
        .and_then(|ji| ji.get("journal"))
        .and_then(|j| j.get("title"))
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let pub_year = rec
        .get("pubYear")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let abstract_text = rec
        .get("abstractText")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    // `license` is ABSENT on non-OA records (verified live) — Option, never
    // assumed.
    let license = rec
        .get("license")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let open_access = rec
        .get("isOpenAccess")
        .and_then(|x| x.as_str())
        .map(|s| s == "Y");
    // Full-text preference is deterministic: the first "Open access" URL,
    // else the first listed URL.
    let fulltext_url = rec
        .get("fullTextUrlList")
        .and_then(|l| l.get("fullTextUrl"))
        .and_then(|u| u.as_array())
        .and_then(|urls| {
            urls.iter()
                .find(|u| u.get("availability").and_then(|a| a.as_str()) == Some("Open access"))
                .or_else(|| urls.first())
        })
        .and_then(|u| u.get("url"))
        .and_then(|x| x.as_str())
        .map(str::to_string);

    // RECORD, never floor: commentCorrectionList entries verbatim.
    let updates: Vec<SourceUpdate> = rec
        .get("commentCorrectionList")
        .and_then(|l| l.get("commentCorrection"))
        .and_then(|c| c.as_array())
        .map(|entries| {
            entries
                .iter()
                .map(|e| SourceUpdate {
                    doi: e
                        .get("reference")
                        .and_then(|x| x.as_str())
                        .and_then(doi_from_reference),
                    kind: e
                        .get("type")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    label: None,
                    source: e.get("source").and_then(|x| x.as_str()).map(str::to_string),
                    date: None,
                })
                .collect()
        })
        .unwrap_or_default();

    // THE FLOOR SIGNAL: the exact pubType string only.
    let retracted = rec
        .get("pubTypeList")
        .and_then(|l| l.get("pubType"))
        .and_then(|p| p.as_array())
        .map(|types| types.iter().any(|t| t.as_str() == Some(RETRACTED_PUB_TYPE)))
        .unwrap_or(false);
    let retraction = if retracted {
        Some(Retraction {
            notice_doi: None,
            date: None,
            kind: RetractionKind::Retraction,
        })
    } else {
        None
    };

    Ok(SourceRecord {
        doi,
        pmid,
        title,
        authors,
        journal,
        pub_year,
        abstract_text,
        license,
        open_access,
        fulltext_url,
        updates,
        retraction,
        fetched_at: fetched_at.to_string(),
        api: SourceApi::EuropePmc,
    })
}

// ── Crossref ─────────────────────────────────────────────────────────────

/// Parse one Crossref `works/{doi}` response — the RETRACTION WITNESS. The
/// audit-grade fields live in `updated-by[]`: the notice DOI, the
/// retraction date, and the Retraction-Watch `record-id`-backed
/// relationship.
///
/// THE TRAP (see the module doc): `updated-by[].type` carries BOTH
/// `"correction"` and `"retraction"` on the same retracted record. The
/// retraction derivation filters EXACTLY [`CROSSREF_RETRACTION_TYPE`]; every
/// other entry is recorded in [`SourceRecord::updates`] and never floors.
pub fn parse_crossref_works(json: &str, fetched_at: &str) -> Result<SourceRecord, ScholarError> {
    const API: &str = "crossref";
    let v: Value =
        serde_json::from_str(json).map_err(|_| ScholarError::MalformedJson { api: API })?;
    let msg = v
        .get("message")
        .and_then(|m| m.as_object())
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "message",
        })?;
    // Identity presence: a works record IS a DOI record.
    let doi = msg
        .get("DOI")
        .and_then(|x| x.as_str())
        .map(normalize_doi)
        .filter(|s| !s.is_empty())
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "message.DOI",
        })?;

    let title = msg
        .get("title")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let authors = msg.get("author").and_then(|a| a.as_array()).map(|list| {
        list.iter()
            .filter_map(|a| {
                let family = a.get("family").and_then(|x| x.as_str())?;
                let given = a.get("given").and_then(|x| x.as_str()).unwrap_or("");
                Some(if given.is_empty() {
                    family.to_string()
                } else {
                    format!("{given} {family}")
                })
            })
            .collect::<Vec<_>>()
            .join(", ")
    });
    let journal = msg
        .get("container-title")
        .and_then(|t| t.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let pub_year = date_parts_to_iso(msg.get("issued").and_then(|i| i.get("date-parts")))
        .map(|iso| iso[..4].to_string());
    let abstract_text = msg
        .get("abstract")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    // Crossref's license array is TDM-oriented: non-OA records carry TDM
    // entries too (verified live on the clean non-OA fixture). The recorded
    // license is a CREATIVE-COMMONS entry when present, else None — a TDM
    // license is not an OA license and must not be recorded as one.
    let license = msg
        .get("license")
        .and_then(|l| l.as_array())
        .and_then(|entries| {
            entries
                .iter()
                .filter_map(|e| e.get("URL").and_then(|u| u.as_str()))
                .find(|u| u.contains("creativecommons.org"))
        })
        .map(str::to_string);

    let updates: Vec<SourceUpdate> = msg
        .get("updated-by")
        .and_then(|u| u.as_array())
        .map(|entries| {
            entries
                .iter()
                .map(|e| SourceUpdate {
                    doi: e
                        .get("DOI")
                        .and_then(|x| x.as_str())
                        .map(normalize_doi)
                        .filter(|s| !s.is_empty()),
                    kind: e
                        .get("type")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default()
                        .to_string(),
                    label: e.get("label").and_then(|x| x.as_str()).map(str::to_string),
                    source: e.get("source").and_then(|x| x.as_str()).map(str::to_string),
                    date: date_parts_to_iso(e.get("updated").and_then(|u| u.get("date-parts"))),
                })
                .collect()
        })
        .unwrap_or_default();

    // THE TRAP FILTER: exactly `retraction`, nothing else.
    let retraction = updates
        .iter()
        .find(|u| u.kind == CROSSREF_RETRACTION_TYPE)
        .map(|u| Retraction {
            notice_doi: u.doi.clone(),
            date: u.date.clone(),
            kind: RetractionKind::Retraction,
        });

    Ok(SourceRecord {
        doi: Some(doi),
        pmid: None,
        title,
        authors,
        journal,
        pub_year,
        abstract_text,
        license,
        open_access: None,
        fulltext_url: None,
        updates,
        retraction,
        fetched_at: fetched_at.to_string(),
        api: SourceApi::Crossref,
    })
}

// ── PubMed ───────────────────────────────────────────────────────────────

/// Parse one PubMed E-utilities `esummary.fcgi?db=pubmed&retmode=json`
/// response (the key-configured FALLBACK adapter's shape). The envelope is
/// `result.uids[]` + `result["<uid>"]` records; the FIRST uid is the fetch
/// target. The retraction derivation: `pubtype` contains EXACTLY
/// [`RETRACTED_PUB_TYPE`].
pub fn parse_pubmed_esummary(json: &str, fetched_at: &str) -> Result<SourceRecord, ScholarError> {
    const API: &str = "pubmed";
    let v: Value =
        serde_json::from_str(json).map_err(|_| ScholarError::MalformedJson { api: API })?;
    let result = v
        .get("result")
        .and_then(|r| r.as_object())
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "result",
        })?;
    let uid = result
        .get("uids")
        .and_then(|u| u.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "result.uids",
        })?;
    let rec = result.get(uid).ok_or(ScholarError::WrongSchema {
        api: API,
        field: "result[uid]",
    })?;
    // Identity presence: a record with none of title/pubtype/articleids is
    // not a usable esummary record.
    if rec.get("title").and_then(|x| x.as_str()).is_none()
        && rec.get("pubtype").and_then(|x| x.as_array()).is_none()
        && rec.get("articleids").and_then(|x| x.as_array()).is_none()
    {
        return Err(ScholarError::WrongSchema {
            api: API,
            field: "result[uid]",
        });
    }

    let doi = rec
        .get("articleids")
        .and_then(|a| a.as_array())
        .and_then(|ids| {
            ids.iter()
                .find(|i| i.get("idtype").and_then(|t| t.as_str()) == Some("doi"))
        })
        .and_then(|i| i.get("value"))
        .and_then(|x| x.as_str())
        .map(normalize_doi)
        .filter(|s| !s.is_empty());
    let title = rec
        .get("title")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let authors = rec.get("authors").and_then(|a| a.as_array()).map(|list| {
        list.iter()
            .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    });
    let journal = rec
        .get("fulljournalname")
        .and_then(|x| x.as_str())
        .map(str::to_string);
    let pub_year = rec
        .get("pubdate")
        .and_then(|x| x.as_str())
        .and_then(|s| s.split_whitespace().next())
        .filter(|y| y.len() == 4 && y.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string);
    let abstract_text = None; // esummary carries no abstract — a PubMed
    // abstract needs the efetch shape, which this adapter does not speak.

    // THE FLOOR SIGNAL: the exact pubtype string only. `updates` stays
    // empty: the esummary shape carries no comment-correction array.
    let retracted = rec
        .get("pubtype")
        .and_then(|p| p.as_array())
        .map(|types| types.iter().any(|t| t.as_str() == Some(RETRACTED_PUB_TYPE)))
        .unwrap_or(false);
    let retraction = if retracted {
        Some(Retraction {
            notice_doi: None,
            date: None,
            kind: RetractionKind::Retraction,
        })
    } else {
        None
    };

    Ok(SourceRecord {
        doi,
        pmid: Some(uid.to_string()),
        title,
        authors,
        journal,
        pub_year,
        abstract_text,
        license: None,
        open_access: None,
        fulltext_url: None,
        updates: Vec::new(),
        retraction,
        fetched_at: fetched_at.to_string(),
        api: SourceApi::PubMed,
    })
}

/// Parse the PubMed E-utilities `esearch.fcgi` response — the FIRST leg of
/// the fallback adapter's two-step (a DOI term resolves to uids here; the
/// `esummary` leg fetches the records). Returns the uid list in order.
pub fn parse_pubmed_esearch(json: &str) -> Result<Vec<String>, ScholarError> {
    const API: &str = "pubmed";
    let v: Value =
        serde_json::from_str(json).map_err(|_| ScholarError::MalformedJson { api: API })?;
    v.get("esearchresult")
        .and_then(|r| r.get("idlist"))
        .and_then(|l| l.as_array())
        .map(|ids| {
            ids.iter()
                .filter_map(|i| i.as_str().map(str::to_string))
                .collect()
        })
        .ok_or(ScholarError::WrongSchema {
            api: API,
            field: "esearchresult.idlist",
        })
}

// ── The adapter contract ─────────────────────────────────────────────────

/// One scholarly source. A FETCHER, never a promoter: the trait's whole
/// contract is `identifier in, record out`; ingest, screening, HITL, and
/// the trust floor all live elsewhere (the doors that already exist).
/// The network lane is operator-configured and allowlist-pinned (plan S2);
/// CI runs this contract over the REPLAY path only (SA-4).
pub trait SourceAdapter {
    fn api(&self) -> SourceApi;
    fn fetch(&self, id: &SourceIdentifier) -> Result<SourceRecord, ScholarError>;
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scholar");
    const FETCHED_AT: &str = "2026-11-08T00:00:00Z";

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{FIXTURES}/{name}")).expect("fixture in the corpus")
    }

    fn expected(name: &str) -> Value {
        let bytes = std::fs::read_to_string(format!("{FIXTURES}/expected/{name}.json"))
            .expect("expected-metadata file in the corpus");
        serde_json::from_str(&bytes).expect("expected-metadata file parses")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(bytes);
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    // ── the corpus is intact (the SHA256SUMS discipline) ─────────────────

    #[test]
    fn the_fixture_corpus_round_trips_byte_identically() {
        let sums = std::fs::read_to_string(format!("{FIXTURES}/SHA256SUMS"))
            .expect("SHA256SUMS in the corpus");
        let lines = sums.lines().filter(|l| !l.trim().is_empty()).count();
        assert!(lines >= 15, "the corpus carries the full fixture set");
        for line in sums.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let (hex, name) = line
                .split_once("  ")
                .unwrap_or_else(|| panic!("SHA256SUMS line is malformed: {line}"));
            let bytes = std::fs::read(format!("{FIXTURES}/{name}"))
                .unwrap_or_else(|_| panic!("fixture {name} is present"));
            assert_eq!(
                sha256_hex(&bytes),
                hex,
                "{name} drifted from SHA256SUMS — the corpus is frozen at commit"
            );
        }
    }

    // ── the DOI normalizer (SA-6): total and single ─────────────────────

    #[test]
    fn the_doi_normalizer_strips_resolvers_and_lowercases() {
        assert_eq!(
            normalize_doi("10.1016/S0140-6736(97)11096-0"),
            "10.1016/s0140-6736(97)11096-0"
        );
        assert_eq!(
            normalize_doi("https://doi.org/10.1016/S0140-6736(97)11096-0"),
            "10.1016/s0140-6736(97)11096-0"
        );
        assert_eq!(
            normalize_doi("HTTP://DX.DOI.ORG/10.1016/S0140-6736(97)11096-0"),
            "10.1016/s0140-6736(97)11096-0"
        );
        assert_eq!(
            normalize_doi("doi:10.1016/S0140-6736(97)11096-0"),
            "10.1016/s0140-6736(97)11096-0"
        );
        assert_eq!(
            normalize_doi("  doi.org/10.1016/S0140-6736(97)11096-0  "),
            "10.1016/s0140-6736(97)11096-0"
        );
        assert_eq!(
            normalize_doi("https://doi.org/doi:10.1016/S0140-6736(97)11096-0"),
            "10.1016/s0140-6736(97)11096-0"
        );
    }

    #[test]
    fn the_doi_normalizer_is_total() {
        assert_eq!(normalize_doi(""), "");
        assert_eq!(normalize_doi("   "), "");
        assert_eq!(normalize_doi("not a doi at all"), "not a doi at all");
        assert_eq!(normalize_doi("doi:"), "");
        // Total means it never panics on any byte string:
        normalize_doi("10.1016/\u{00e9}trang\u{00e8}re(97)");
    }

    // ── THE CORRECTION TRAP (SA-5's parse half) ─────────────────────────

    #[test]
    fn the_trap_fixture_floors_on_the_exact_retraction_only() {
        let rec = parse_crossref_works(&fixture("cr-wakefield.json"), FETCHED_AT)
            .expect("the live-verified Crossref record parses");
        // The record holds BOTH entries.
        assert_eq!(
            rec.updates.len(),
            2,
            "correction AND retraction are recorded"
        );
        let correction = rec
            .updates
            .iter()
            .find(|u| u.kind == "correction")
            .expect("the 2004 correction entry is recorded");
        assert_eq!(
            correction.doi.as_deref(),
            Some("10.1016/s0140-6736(04)15715-2"),
            "the correction entry is recorded, NOT floored"
        );
        // …and the floor derives ONLY from the exact retraction entry.
        let r = rec.retraction.expect("the 2010 retraction floors");
        assert_eq!(r.kind, RetractionKind::Retraction);
        assert_eq!(
            r.notice_doi.as_deref(),
            Some("10.1016/s0140-6736(10)60175-4"),
            "the notice DOI is the audit-grade witness field"
        );
        assert_eq!(r.date.as_deref(), Some("2010-02-06"));
    }

    #[test]
    fn the_control_arm_never_floors_on_a_correction() {
        let rec = parse_crossref_works(&fixture("cr-correction-control.json"), FETCHED_AT)
            .expect("the control arm parses");
        assert_eq!(rec.updates.len(), 1);
        assert_eq!(rec.updates[0].kind, "correction");
        assert!(
            rec.retraction.is_none(),
            "a corrected-but-never-retracted record NEVER floors — flooring on any \
             updated-by entry is the trap the vocabulary exactness exists to prevent"
        );
    }

    #[test]
    fn expression_of_concern_is_recorded_never_floored() {
        let rec = parse_epmc_core(&fixture("epmc-wakefield-core.json"), FETCHED_AT)
            .expect("the EPMC record parses");
        let kinds: Vec<&str> = rec.updates.iter().map(|u| u.kind.as_str()).collect();
        assert!(
            kinds.contains(&"Expression of concern in"),
            "the EoC relationship is RECORDED (the honest-metadata law)"
        );
        assert!(
            rec.retraction.is_some(),
            "the pubType string floors; the EoC entry does not — its recorded kind \
             never feeds the floor derivation"
        );
    }

    // ── the three API shapes agree on the Wakefield record ──────────────

    #[test]
    fn all_three_api_shapes_derive_the_retraction() {
        let epmc = parse_epmc_core(&fixture("epmc-wakefield-core.json"), FETCHED_AT).unwrap();
        let cr = parse_crossref_works(&fixture("cr-wakefield.json"), FETCHED_AT).unwrap();
        let pm = parse_pubmed_esummary(&fixture("pm-wakefield.json"), FETCHED_AT).unwrap();
        for rec in [&epmc, &cr, &pm] {
            assert!(
                rec.retraction.is_some(),
                "{} carries the published retraction signal",
                rec.api.as_str()
            );
        }
        assert_eq!(
            epmc.doi, cr.doi,
            "the normalized identity agrees across lanes"
        );
        assert_eq!(cr.doi, pm.doi, "…in all three");
    }

    // ── the machine-checked expected-metadata contract ──────────────────

    #[test]
    fn the_expected_metadata_contract_holds_for_crossref() {
        let e = expected("cr-wakefield");
        let rec = parse_crossref_works(&fixture("cr-wakefield.json"), FETCHED_AT).unwrap();
        assert_eq!(rec.doi.as_deref(), e["expect"]["doi"].as_str());
        let r = rec.retraction.as_ref().unwrap();
        assert_eq!(
            r.notice_doi.as_deref(),
            e["expect"]["retraction_notice_doi"].as_str()
        );
        assert_eq!(r.date.as_deref(), e["expect"]["retraction_date"].as_str());
        assert_eq!(
            rec.updates.len(),
            e["expect"]["updates_recorded"].as_u64().unwrap() as usize
        );
    }

    #[test]
    fn the_expected_metadata_contract_holds_for_epmc() {
        let e = expected("epmc-wakefield");
        let rec = parse_epmc_core(&fixture("epmc-wakefield-core.json"), FETCHED_AT).unwrap();
        assert_eq!(rec.doi.as_deref(), e["expect"]["doi"].as_str());
        assert_eq!(rec.pmid.as_deref(), e["expect"]["pmid"].as_str());
        assert_eq!(
            rec.license, None,
            "license is ABSENT on non-OA — Option, never assumed"
        );
        assert_eq!(rec.open_access, Some(false));
        assert_eq!(
            rec.updates.len(),
            e["expect"]["updates_recorded"].as_u64().unwrap() as usize
        );
        let retraction_in: Vec<&str> = rec
            .updates
            .iter()
            .filter(|u| u.kind == "Retraction in")
            .filter_map(|u| u.doi.as_deref())
            .collect();
        let want: Vec<&str> = e["expect"]["retraction_in_dois"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(retraction_in, want, "the EPMC notice DOIs are recorded");
        assert!(rec.retraction.is_some());
    }

    #[test]
    fn the_expected_metadata_contract_holds_for_clean_records() {
        let oa = parse_epmc_core(&fixture("epmc-clean-oa.json"), FETCHED_AT).unwrap();
        assert_eq!(
            oa.license.as_deref(),
            Some("cc by"),
            "the OA shape carries the license"
        );
        assert_eq!(oa.open_access, Some(true));
        assert!(oa.retraction.is_none());
        assert!(oa.updates.is_empty());
        assert!(oa.fulltext_url.is_some(), "the OA copy's URL is recorded");

        let noa = parse_epmc_core(&fixture("epmc-clean-noa.json"), FETCHED_AT).unwrap();
        assert_eq!(noa.license, None, "the non-OA shape carries NO license");
        assert_eq!(noa.open_access, Some(false));
        assert!(noa.retraction.is_none());

        let pm = parse_pubmed_esummary(&fixture("pm-clean.json"), FETCHED_AT).unwrap();
        assert!(pm.retraction.is_none());
        assert_eq!(
            pm.pmid.as_deref(),
            Some("35476794"),
            "the first uid is the fetch target"
        );
    }

    #[test]
    fn the_expected_metadata_contract_holds_for_the_esearch_leg() {
        let e = expected("pm-esearch-wakefield");
        let uids = parse_pubmed_esearch(&fixture("pm-esearch-wakefield.json"))
            .expect("the esearch response parses");
        assert_eq!(uids.len(), e["expect"]["count"].as_u64().unwrap() as usize);
        let want: Vec<&str> = e["expect"]["uids"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(uids, want, "the DOI term resolves to the Wakefield uid");
    }

    // ── named refusals over the malformed set ───────────────────────────

    #[test]
    fn malformed_json_is_refused_by_name() {
        let err = parse_epmc_core(&fixture("malformed-raw-bytes.json"), FETCHED_AT).unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_malformed_json: europepmc response is not valid JSON"
        );
        let err =
            parse_epmc_core(&fixture("malformed-epmc-truncated.json"), FETCHED_AT).unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_malformed_json: europepmc response is not valid JSON"
        );
        let err =
            parse_crossref_works(&fixture("malformed-raw-bytes.json"), FETCHED_AT).unwrap_err();
        assert!(
            err.to_string()
                .starts_with("scholar_malformed_json: crossref")
        );
    }

    #[test]
    fn wrong_schema_is_refused_by_name() {
        let err = parse_epmc_core(&fixture("wrong-schema-epmc.json"), FETCHED_AT).unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_wrong_schema: europepmc response has no resultList.result[0]"
        );
        let err =
            parse_crossref_works(&fixture("wrong-schema-crossref.json"), FETCHED_AT).unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_wrong_schema: crossref response has no message.DOI"
        );
        let err =
            parse_pubmed_esummary(&fixture("wrong-schema-pubmed.json"), FETCHED_AT).unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_wrong_schema: pubmed response has no result[uid]"
        );
    }

    #[test]
    fn empty_envelopes_are_refused_by_name() {
        let err = parse_epmc_core(
            r#"{"hitCount": 0, "resultList": {"result": []}}"#,
            FETCHED_AT,
        )
        .unwrap_err();
        assert_eq!(
            err.to_string(),
            "scholar_empty_result: europepmc response carries no records"
        );
    }

    // ── the 429 transcript (the backoff pin's input, plan S2) ───────────
    #[test]
    fn the_429_transcript_parses_and_is_honestly_synthetic() {
        let t = expected_data(&fixture("transcript-429.json"));
        assert_eq!(t["status"].as_u64(), Some(429));
        assert_eq!(t["headers"]["retry-after"].as_str(), Some("1"));
        assert!(
            t["transcript"].as_str().unwrap().contains("SYNTHETIC"),
            "the transcript marks its own provenance: a live 429 was not triggered \
             (proving one would violate the rate etiquette)"
        );
    }

    fn expected_data(v: &str) -> Value {
        serde_json::from_str(v).expect("the transcript parses")
    }

    // ── the status line (the rendered body's deterministic view) ────────

    #[test]
    fn the_status_line_carries_the_notice_doi_when_the_witness_supplies_it() {
        let rec = parse_crossref_works(&fixture("cr-wakefield.json"), FETCHED_AT).unwrap();
        assert_eq!(
            rec.status_line().as_deref(),
            Some("Retracted: 2010-02-06 — notice DOI 10.1016/s0140-6736(10)60175-4")
        );
    }

    #[test]
    fn the_status_line_degrades_honestly_without_the_witness() {
        let rec = parse_epmc_core(&fixture("epmc-wakefield-core.json"), FETCHED_AT).unwrap();
        assert_eq!(
            rec.status_line().as_deref(),
            Some("Retracted: date unknown"),
            "the EPMC flag carries no notice DOI or date — the honest degrade, \
             never an invented field"
        );
        let clean = parse_epmc_core(&fixture("epmc-clean-oa.json"), FETCHED_AT).unwrap();
        assert!(clean.status_line().is_none());
    }

    // ── the adapter contract's trait shape compiles over a replay ───────

    struct ReplayCrossref;
    impl SourceAdapter for ReplayCrossref {
        fn api(&self) -> SourceApi {
            SourceApi::Crossref
        }
        fn fetch(&self, id: &SourceIdentifier) -> Result<SourceRecord, ScholarError> {
            match id {
                SourceIdentifier::Doi(d) if normalize_doi(d) == "10.1016/s0140-6736(97)11096-0" => {
                    parse_crossref_works(&fixture("cr-wakefield.json"), FETCHED_AT)
                }
                _ => Err(ScholarError::EmptyResult { api: "crossref" }),
            }
        }
    }

    #[test]
    fn the_adapter_trait_fetches_over_the_replay_path() {
        let a = ReplayCrossref;
        let rec = a
            .fetch(&SourceIdentifier::Doi(
                "HTTPS://DOI.ORG/10.1016/S0140-6736(97)11096-0".into(),
            ))
            .expect("the replay fetch resolves the normalized identity");
        assert_eq!(rec.api, SourceApi::Crossref);
        assert!(rec.retraction.is_some());
    }
}
