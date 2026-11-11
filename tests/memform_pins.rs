//! IMPROVE13 — the MemForm pins (MF-0..MF-1 in this step; MF-2..MF-4 land with
//! their steps): the formation-interface's approver eyes. The proposal queue's
//! human approval is the boundary the screen never claimed to be
//! (`src/screen.rs:23`), so these pins give the approver provenance the wire
//! cannot forge and hold the producers to a closed vocabulary.
//!
//! LAWS (the plan's gating laws, restated as what these pins enforce):
//! HITL sacred — a marker informs, it never gates; no model in the gates —
//! every check here is deterministic code; the wire cannot set the marker —
//! `ProposalRequest` gains NO `derived_from` field and the openapi request
//! schema declares none; every production INSERT stamps a closed-vocabulary
//! value at the kernel side; the matrix (docs/MEMFORM_MATRIX_2026-11-11.md)
//! committed FIRST (the git-order pin) and every cell of it is cited.
//!
//! RED-FIRST PROVENANCE: this file was written and RUN against the pre-step
//! tree (HEAD c51e5688, the matrix commit) before the S2 code landed; the
//! red transcript is recorded in the round evidence
//! (/tmp/i13-probe/s2_red_transcript.txt). The arms that read the not-yet-
//! landed surfaces red there by design — that is the non-vacuity proof.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use tower::ServiceExt;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// one call deep: a repo-shaped read helper (the census's shape (b))
fn read_repo(rel: &str) -> String {
    std::fs::read_to_string(repo().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ── MF-0: the git order + the matrix citation law ────────────────────────

/// MF-0, the git-order half: the matrix's ADD commit precedes this pins
/// file's ADD commit (the AA-1 git order, checkable in the checkout; the
/// no_engagement_name precedent for git-in-test).
#[test]
fn mf0_the_matrix_commit_precedes_this_pins_file() {
    let add_commit = |path: &str| -> Option<String> {
        let out = Command::new("git")
            .args(["log", "--diff-filter=A", "--format=%H", "--", path])
            .current_dir(repo())
            .output()
            .expect("git log must run — the MF-0 git order is checkable only in a checkout");
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.lines().last().map(|l| l.to_string())
    };
    let matrix = add_commit("docs/MEMFORM_MATRIX_2026-11-11.md")
        .expect("the matrix must be committed (the MF-0 git order)");
    let pins = add_commit("tests/memform_pins.rs")
        .expect("this pins file must be committed for the order to be checkable");
    let before = Command::new("git")
        .args(["merge-base", "--is-ancestor", &matrix, &pins])
        .current_dir(repo())
        .output()
        .expect("git merge-base must run");
    assert!(
        before.status.success(),
        "the matrix commit ({matrix}) must PRECEDE the pins-file commit ({pins}) — \
         the matrix is cited by these pins, so an assertion about it cannot land first"
    );
}

/// The matrix citation scan: every class cell (C1–C4) and every special row
/// (POSTURE / FATIGUE / ASI06) carries a backticked tree cite, and every
/// producer section carries all four cells. A cite is either a full
/// `src/…rs:NN` token or a section-relative `` `:NN `` token (the section
/// header names the file). The rule is written here so a disagreement about
/// a cell is a disagreement about this rule, not about taste.
fn scan_matrix_cells(doc: &str) -> Result<(), String> {
    let full_cite = |l: &str| l.contains("src/") && l.contains(".rs:");
    let rel_cite = |l: &str| l.contains("`:");
    let mut section: Option<String> = None;
    let mut seen_cells: Vec<String> = Vec::new();
    for line in doc.lines() {
        if line.starts_with("### P") {
            if let Some(prev) = &section {
                for c in ["C1", "C2", "C3", "C4"] {
                    if !seen_cells
                        .iter()
                        .any(|s| s.starts_with(&format!("- **{c}·")))
                    {
                        return Err(format!("section {prev} is missing its {c} cell"));
                    }
                }
            }
            section = Some(line.trim_start_matches("### ").to_string());
            seen_cells.clear();
            continue;
        }
        if line.starts_with("- **C1·")
            || line.starts_with("- **C2·")
            || line.starts_with("- **C3·")
            || line.starts_with("- **C4·")
        {
            seen_cells.push(line.to_string());
            if !full_cite(line) && !rel_cite(line) {
                return Err(format!("uncited cell: {}", &line[..line.len().min(90)]));
            }
            continue;
        }
        if ["- **POSTURE**", "- **FATIGUE**", "- **ASI06**"]
            .into_iter()
            .any(|marker| line.starts_with(marker))
            && !full_cite(line)
        {
            return Err(format!(
                "special row without a full tree cite: {}",
                &line[..line.len().min(90)]
            ));
        }
    }
    if let Some(prev) = &section {
        for c in ["C1", "C2", "C3", "C4"] {
            if !seen_cells
                .iter()
                .any(|s| s.starts_with(&format!("- **{c}·")))
            {
                return Err(format!("section {prev} is missing its {c} cell"));
            }
        }
    }
    Ok(())
}

/// MF-0, the citation half: zero uncited cells in the matrix, every producer
/// section complete (the plan's S1 acceptance, enforced from S2 on).
#[test]
fn mf0_matrix_cells_are_cited_and_complete() {
    let doc = read_repo("docs/MEMFORM_MATRIX_2026-11-11.md");
    scan_matrix_cells(&doc).expect("the matrix must have zero uncited cells");
}

/// The planted-miss arm of the citation scan — the CONTROL of the pair:
/// the SAME checker fed an uncited cell must red it NAMED (a checker that
/// cannot red is the one defect a structural scan must never have).
#[test]
fn mf0_control_an_uncited_cell_reds_the_scan() {
    let planted = "### P99 · planted producer (not real)\n\n\
- **C1·formation-interface** — a cell whose author forgot the cite entirely.\n\
- **C2·decomposition** — cited: `src/screen.rs:23`.\n\
- **C3·stale-fact** — cited: `src/screen.rs:23`.\n\
- **C4·goal-planting** — cited: `src/screen.rs:23`.\n";
    let err = scan_matrix_cells(planted).expect_err("the planted uncited cell must red");
    assert!(
        err.contains("uncited cell"),
        "the red must name the defect: {err}"
    );
    // and the completeness arm discriminates too:
    let planted_partial = "### P98 · planted producer\n\n\
- **C1·formation-interface** — cited: `src/screen.rs:23`.\n\
- **C2·decomposition** — cited: `src/screen.rs:23`.\n\
- **C4·goal-planting** — cited: `src/screen.rs:23`.\n";
    let err = scan_matrix_cells(planted_partial).expect_err("a section missing C3 must red");
    assert!(err.contains("missing its C3 cell"), "named: {err}");
}

// ── MF-1: the path-assigned provenance marker ────────────────────────────

/// Extract a top-level struct body from Rust source (from the `pub struct
/// NAME {` line to its matching closing brace) — the extractor the struct
/// scan and its planted arm share.
fn struct_body(src: &str, name: &str) -> Option<String> {
    let needle = format!("struct {name} {{");
    let start = src.find(&needle)? + needle.len();
    let mut depth = 1usize;
    let mut end = start;
    for (i, c) in src[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = start + i;
                    break;
                }
            }
            _ => {}
        }
    }
    Some(src[start..end].to_string())
}

/// MF-1, the spoofing law (the structural half): the REQUEST struct carries
/// NO `derived_from` field — request-borne provenance is the MemForm defeat,
/// so the marker must be PATH-ASSIGNED at the kernel side.
#[test]
fn mf1_request_struct_carries_no_derived_from_field() {
    let gate = read_repo("src/handlers/gate.rs");
    let body = struct_body(&gate, "ProposalRequest")
        .expect("ProposalRequest must exist in src/handlers/gate.rs");
    assert!(
        !body.contains("derived_from"),
        "ProposalRequest must NEVER gain a derived_from field — the wire cannot \
         set the marker (request-borne provenance is the MemForm defeat); if you \
         added the field, move the stamp to the producer's INSERT instead"
    );
}

/// The planted arm of the struct scan — the CONTROL of the pair: the SAME
/// extractor over a planted struct WITH the field must red NAMED (proving
/// the scan can fail).
#[test]
fn mf1_control_a_planted_field_reds_the_struct_scan() {
    let planted = "pub struct PlantedRequest {\n    pub content: String,\n    \
                   pub derived_from: String,\n}";
    let body = struct_body(planted, "PlantedRequest").expect("planted struct parses");
    assert!(
        body.contains("derived_from"),
        "the control must SEE the planted field (if this fails the extractor is broken)"
    );
    // the scan's rejection path, exercised on the planted body:
    let rejects = |b: &str| b.contains("derived_from");
    assert!(rejects(&body), "the planted field must trip the scan");
    let clean = struct_body(&read_repo("src/handlers/gate.rs"), "ProposalRequest")
        .expect("the real struct");
    assert!(!rejects(&clean), "the real struct must pass the same scan");
}

/// The two-lane token-file server the wire pins drive (the
/// agent_write_posture harness shape): one operator token, opaque mode.
struct WireServer {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
    op_token: String,
}

fn build_wire_server(op_token: &str) -> WireServer {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    let mgr = brain_server::pool::SqliteConnectionManager::file(&db_path);
    let pool: brain_server::Pool = r2d2::Pool::builder().max_size(4).build(mgr).expect("pool");
    brain_server::migration::run_migration(
        &mut pool.get().expect("conn"),
        brain_server::config::DB_MMAP_SIZE_MIB,
    )
    .expect("migration");
    let model: Arc<dyn brain_server::embed::Embedder> = Arc::new(
        brain_server::embed::StaticEmbedder::new(brain_server::config::MODEL_ID).expect("model"),
    );
    let tok_file = dir.path().join("tokens");
    std::fs::write(&tok_file, format!("{op_token}\n")).expect("token file");
    let token_store = brain_server::auth::TokenStore::from_file(Some(tok_file));
    token_store.reload_parts_from(vec![op_token.to_string()], None);
    let state = Arc::new(brain_server::AppState {
        token_store,
        jwt_middleware_state: Arc::new(
            brain_server::server::router::auth::JwtMiddlewareState::opaque_for_tests(
                pool.clone(),
                db_path.clone(),
            ),
        ),
        cors: tower_http::cors::CorsLayer::new(),
        chat_answer_cache: Arc::new(
            brain_server::service::chat_answer_cache::ChatAnswerCache::new(),
        ),
        durability: Default::default(),
        loom: Default::default(),
        model,
        registry: brain_server::domain_registry::DomainRegistry::new(pool.clone(), &db_path, false),
        pool,
        db_path,
        connection_tracker: Arc::new(brain_server::http_limit::ConnectionTracker::new()),
        rate_limiter: Arc::new(brain_server::http_limit::RateLimiter::new()),
        snapshot: brain_server::integrity::SnapshotState::default(),
        audit_chain_cache: Arc::new(std::sync::Mutex::new(None)),
        auth_mode: brain_server::auth::AuthMode::Opaque,
        key_store: brain_server::auth::jwks::KeyStore::default(),
        revocation_cache: Arc::new(brain_server::auth::revocation::RevocationCache::new()),
        jwt_issuer: String::new(),
        jwt_audience: String::new(),
        oidc_config: brain_server::handlers::well_known::OidcConfig::unconfigured(),
        ump_events: tokio::sync::broadcast::channel(brain_server::config::UMP_EVENT_BUFFER).0,
        alert_events: tokio::sync::broadcast::channel(brain_server::config::ALERT_EVENT_BUFFER).0,
        alert_seq: std::sync::atomic::AtomicU64::new(0),
        chain_watch: brain_server::alert::ChainWatchState::default(),
        concurrency: &brain_server::concurrency::CONCURRENCY,
    });
    WireServer {
        _dir: dir,
        state,
        op_token: op_token.to_string(),
    }
}

async fn wire_post(
    srv: &WireServer,
    token: &str,
    path: &str,
    body: serde_json::Value,
) -> (axum::http::StatusCode, serde_json::Value) {
    let req = axum::http::Request::builder()
        .method(axum::http::Method::POST)
        .uri(path)
        .header("content-type", "application/json")
        .header("authorization", format!("Bearer {token}"))
        .body(axum::body::Body::from(body.to_string()))
        .expect("request");
    let resp = brain_server::server::router::app(srv.state.clone())
        .oneshot(req)
        .await
        .expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

async fn wire_get(
    srv: &WireServer,
    token: &str,
    path: &str,
) -> (axum::http::StatusCode, serde_json::Value) {
    let req = axum::http::Request::builder()
        .method(axum::http::Method::GET)
        .uri(path)
        .header("authorization", format!("Bearer {token}"))
        .body(axum::body::Body::empty())
        .expect("request");
    let resp = brain_server::server::router::app(srv.state.clone())
        .oneshot(req)
        .await
        .expect("oneshot");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

/// Pull the pending rows out of a /proposals list response (array or
/// envelope — the pin holds across either shape).
fn list_rows(list: &serde_json::Value) -> Vec<serde_json::Value> {
    list.get("proposals")
        .and_then(|v| v.as_array())
        .or_else(|| list.as_array())
        .cloned()
        .unwrap_or_default()
}

/// MF-1, the wire law (the behavioral half): serialize-shape honesty — a
/// caller POSTing a `derived_from` key to `/ingest/proposal` cannot move the
/// marker (deserialization ignores unknown keys by design; the marker is
/// stamped from the calling context). Proven end-to-end below through the
/// REAL route.
#[tokio::test]
async fn mf1_the_marker_rides_the_real_wire_end_to_end() {
    let srv = build_wire_server("mf13-op-token");

    // THE SPOOF ATTEMPT: the caller claims `derived_from: "watch"` — a value
    // the caller has NO authority to set. The stored marker is PATH-assigned:
    // the operator ingest route stamps `operator`, the caller's claim is
    // ignored (no field consumes it).
    let (status, _) = wire_post(
        &srv,
        &srv.op_token,
        "/ingest/proposal",
        serde_json::json!({
            "content": "the memform wire pin's honest body",
            "kind": "fact",
            "source": "i-claim-i-am-the-watch",
            "derived_from": "watch",
            "producer_ref": "i-claim-a-ref"
        }),
    )
    .await;
    assert_eq!(
        status,
        axum::http::StatusCode::OK,
        "the filing must succeed (the spoofed keys are simply not consumed)"
    );

    // THE QUEUE READS THE PATH-ASSIGNED MARKER BESIDE THE CANONICAL CONTENT:
    let (status, list) = wire_get(&srv, &srv.op_token, "/proposals?status=pending").await;
    assert_eq!(status, axum::http::StatusCode::OK, "the list must serve");
    let rows = list_rows(&list);
    assert!(!rows.is_empty(), "the list shape must expose rows");
    let row = rows
        .iter()
        .find(|r| {
            r.get("content")
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains("the memform wire pin's honest body"))
        })
        .expect("the filed row must render in the pending list");
    assert_eq!(
        row.get("derived_from").and_then(|v| v.as_str()),
        Some("operator"),
        "the row's marker is PATH-assigned (the operator ingest route), never \
         what the caller claimed — the spoofed \"watch\" value must be ignored"
    );
    assert_eq!(
        row.get("producer_ref").and_then(|v| v.as_str()),
        Some("ingest-proposal"),
        "the route's producer reference rides the row"
    );
}

/// MF-1, the INSERT-site walk: EVERY production `INSERT INTO proposals` in
/// the tree stamps `derived_from` (the closed vocabulary is stamped at the
/// producer's INSERT, kernel-side; pre-marker rows read `legacy` by the
/// column's default). Test-fixture INSERTs below the file's `#[cfg(test)]`
/// are not producers and are excluded by the same rule the S1 walk used.
fn unstamped_production_inserts(files: &[(String, String)]) -> Vec<String> {
    let mut misses = Vec::new();
    for (name, content) in files {
        let cfg_test_line = content
            .lines()
            .position(|l| l.contains("#[cfg(test)]"))
            .map(|i| i + 1);
        let lines: Vec<&str> = content.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if !line.contains("INSERT INTO proposals") {
                continue;
            }
            let lineno = i + 1;
            if let Some(ct) = cfg_test_line
                && lineno > ct
            {
                continue; // a test fixture, not a producer
            }
            // the SQL literal window: the column list + VALUES ride the next
            // few lines in this tree's house style
            let window: String = lines[i..(i + 6).min(lines.len())].join("\n");
            if !window.contains("derived_from") {
                misses.push(format!("{name}:{lineno}"));
            }
        }
    }
    misses
}

fn collect_rs_sources() -> Vec<(String, String)> {
    let src = repo().join("src");
    let mut out = Vec::new();
    let mut stack = vec![src.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read_dir src") {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let rel = path
                    .strip_prefix(repo())
                    .expect("in-repo")
                    .to_string_lossy()
                    .to_string();
                out.push((rel, std::fs::read_to_string(&path).expect("read src")));
            }
        }
    }
    out
}

/// MF-1, the walk pin: zero unstamped production INSERT sites.
#[test]
fn mf1_every_production_insert_stamps_derived_from() {
    let files = collect_rs_sources();
    let misses = unstamped_production_inserts(&files);
    assert!(
        misses.is_empty(),
        "every production INSERT INTO proposals must stamp the path-assigned \
         derived_from marker (closed vocabulary operator|agent|watch|legacy); \
         unstamped sites: {misses:?}"
    );
}

/// The planted arm of the walk — the CONTROL of the pair: the SAME walker
/// over a planted unstamped INSERT must red it (named with the site).
#[test]
fn mf1_control_an_unstamped_insert_reds_the_walk() {
    let planted = vec![(
        "planted.rs".to_string(),
        "pub fn f(c: &rusqlite::Connection) {\n    c.execute(\n        \"INSERT INTO proposals(kind, content, source) VALUES ('fact', ?1, 'x')\",\n        [],\n    );\n}"
            .to_string(),
    )];
    let misses = unstamped_production_inserts(&planted);
    assert_eq!(
        misses,
        vec!["planted.rs:3".to_string()],
        "the planted unstamped site must red, named"
    );
    // and a stamped plant passes the same walker (it discriminates):
    let stamped = vec![(
        "stamped.rs".to_string(),
        "pub fn f(c: &rusqlite::Connection) {\n    c.execute(\n        \"INSERT INTO proposals(kind, content, derived_from) VALUES ('fact', ?1, 'agent')\",\n        [],\n    );\n}"
            .to_string(),
    )];
    assert!(unstamped_production_inserts(&stamped).is_empty());
}

/// MF-1, the openapi law: the queue VIEW schema declares the marker (the
/// approver's client renders it) and the REQUEST schema declares NO marker
/// field (the wire cannot set it).
#[test]
fn mf1_openapi_declares_the_marker_on_the_view_only() {
    let spec = read_repo("openapi.yaml");
    let view_start = spec
        .find("    ProposalView:")
        .expect("the ProposalView schema must exist");
    let view_end = spec[view_start..]
        .find("\n    IpassSection:")
        .map(|i| view_start + i)
        .unwrap_or(spec.len());
    let view = &spec[view_start..view_end];
    assert!(
        view.contains("derived_from"),
        "ProposalView must declare derived_from — no proposal renders for \
         approval without its path-assigned marker"
    );
    assert!(
        view.contains("operator") && view.contains("agent") && view.contains("watch"),
        "the marker's closed vocabulary (operator|agent|watch|legacy) must be \
         declared on the view schema"
    );
    assert!(
        view.contains("producer_ref"),
        "ProposalView must declare the producer/gate reference"
    );
    // the REQUEST schema must NOT carry the field: the /ingest/proposal
    // requestBody is an inline object in the spec — its path block must
    // declare no derived_from property (the wire cannot set the marker).
    let req_start = spec
        .find("  /ingest/proposal:")
        .expect("the /ingest/proposal path must exist");
    let req_end = req_start
        + spec[req_start..]
            .find("\n  /")
            .unwrap_or(spec.len() - req_start);
    let req = &spec[req_start..req_end];
    assert!(
        !req.contains("derived_from"),
        "the /ingest/proposal request schema must NEVER declare derived_from — \
         the wire cannot set the marker"
    );
}

/// MF-1, the generated client: the TypeScript contract carries the marker
/// fields (regenerated via `pnpm gen:api`, byte-stable).
#[test]
fn mf1_generated_schema_carries_the_marker() {
    let dts = read_repo("shell/src/lib/api/schema.d.ts");
    assert!(
        dts.contains("derived_from"),
        "schema.d.ts must carry derived_from — rerun `pnpm gen:api` after the \
         openapi change"
    );
    assert!(
        dts.contains("producer_ref"),
        "schema.d.ts must carry producer_ref"
    );
}

/// MF-1, the render: the review page consumes the marker keys beside the
/// canonical content (the approver SEES who wrote the text at decision time).
#[test]
fn mf1_review_page_renders_the_marker() {
    let page = read_repo("shell/src/routes/review/+page.svelte");
    assert!(
        page.contains("review.provenance."),
        "the review page must render the provenance marker (the \
         review.provenance.* keys) beside the canonical content"
    );
}

/// MF-1, the ×5 key law: the marker keys exist in ALL five locale catalogs —
/// paraglide's base-locale fallback would silently serve English for a
/// missing non-en key, so the pin checks each file by name.
#[test]
fn mf1_marker_keys_land_in_all_five_locales() {
    const KEYS: [&str; 6] = [
        "review.provenance.operator",
        "review.provenance.agent",
        "review.provenance.watch",
        "review.provenance.legacy",
        "review.provenance.ref",
        "review.provenance.batch",
    ];
    for loc in ["en", "de", "fr", "es", "nl"] {
        let raw = read_repo(&format!("shell/src/lib/i18n/{loc}.json"));
        let catalog: serde_json::Value =
            serde_json::from_str(&raw).expect("the catalog is valid JSON");
        for key in KEYS {
            assert!(
                catalog.get(key).is_some_and(|v| v.is_string()),
                "locale {loc} is missing the marker key {key} — keys land ×5 or \
                 the fallback silently serves English"
            );
        }
    }
}

/// MF-1, the behavioral half: the migration stamps 1.32.34 — the stamp,
/// the known-version bound and the columns move together (the lockstep law).
#[test]
fn mf1_migration_stamps_1_32_34() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    let stamp: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("stamp");
    assert_eq!(
        stamp, "1.32.34",
        "run_migration must stamp the CURRENT ceiling — 1.32.34 is the \
         MemformStamps round (IMPROVE13); the stamp and the column moved together"
    );
}

/// MF-1, the closed column: `derived_from` exists with the CHECK-closed
/// vocabulary (the hitl_decision_events CHECK precedent) and `producer_ref`
/// exists nullable; a raw write OUTSIDE the vocabulary refuses at the
/// storage layer.
#[test]
fn mf1_migration_adds_the_closed_column_with_its_check() {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let mut conn = rusqlite::Connection::open_in_memory().expect("db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    let derived_present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('proposals') WHERE name = 'derived_from'",
            [],
            |r| r.get(0),
        )
        .expect("pragma");
    let ref_present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('proposals') WHERE name = 'producer_ref'",
            [],
            |r| r.get(0),
        )
        .expect("pragma");
    assert_eq!(derived_present, 1, "derived_from must exist");
    assert_eq!(ref_present, 1, "producer_ref must exist");
    // pre-marker rows read `legacy` (provenance beats guessing):
    let legacy: (i64, String) = conn
        .query_row(
            "INSERT INTO proposals(kind, content, status, created_at, novelty, salience) \
             VALUES ('fact', 'a pre-marker row', 'pending', 1, 0.5, 0.5) RETURNING id, derived_from",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .expect("the default insert must succeed");
    assert_eq!(
        legacy.1, "legacy",
        "pre-marker rows read legacy, never a guess"
    );
    // the CHECK refuses an out-of-vocabulary value:
    let refused = conn.execute(
        "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from) \
         VALUES ('fact', 'spoofed', 'pending', 1, 0.5, 0.5, 'i-claim-operator')",
        [],
    );
    assert!(
        refused.is_err(),
        "the CHECK must refuse a value outside operator|agent|watch|legacy"
    );
}

/// MF-1, the structural half: the known-version bound and the migration's
/// stamp string agree — either side moving alone reds here. (The const-name
/// agreement is held by storage_layout's own probe test, which moves to
/// 1.32.35 in the same commit.)
#[test]
fn mf1_lockstep_ceiling_moves_with_the_stamp() {
    assert_eq!(
        brain_server::storage_layout::LATEST_KNOWN_SCHEMA,
        "1.32.34",
        "the ceiling is the MemformStamps round (IMPROVE13)"
    );
    let migration = read_repo("src/migration.rs");
    assert!(
        migration.contains("'schema_version', '1.32.34'"),
        "run_migration must stamp 1.32.34 — the stamp and the ceiling move together"
    );
}

/// The posture-default lane: the write-posture default stays `open` — the
/// round records the posture question, it never flips the default; this pin
/// holds the region's shape so a silent flip reds HERE, named.
#[test]
fn mf1_the_posture_default_stays_open() {
    let config = read_repo("src/config.rs");
    assert!(
        config.contains("_ => \"open\""),
        "write_posture's fallback must stay \"open\" — the shipped default is \
         the operator's product call (the matrix's POSTURE-DEFAULT-2026-11-11 \
         decision row records it); flipping it is a named decision, never a \
         drive-by"
    );
    assert!(
        config.contains("default stays open"),
        "the back-compat default test must stay (the verify lane the plan names)"
    );
}

// ── MF-2: the shared-provenance grouping (the GRAD arm) ─────────────────

/// MF-2, the behavioral half: ONE producer files N individually-innocuous
/// rows — three honest `fact` proposals through the SAME operator ingest
/// route (they share the path-assigned reference by construction) — and the
/// queue reports their shared cohort to every one of them. A poisoned batch
/// can no longer look like unrelated rows.
#[tokio::test]
async fn mf2_a_shared_reference_renders_its_batch() {
    let srv = build_wire_server("mf13-batch-token");
    for n in 1..=3 {
        let (status, _) = wire_post(
            &srv,
            &srv.op_token,
            "/ingest/proposal",
            serde_json::json!({
                "content": format!("the batch fixture's innocuous row {n}"),
                "kind": "fact"
            }),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "row {n} must file");
    }
    let (status, list) = wire_get(&srv, &srv.op_token, "/proposals?status=pending").await;
    assert_eq!(status, axum::http::StatusCode::OK, "the list must serve");
    let rows = list_rows(&list);
    let batch: Vec<_> = rows
        .iter()
        .filter(|r| {
            r.get("content")
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains("the batch fixture's innocuous row"))
        })
        .collect();
    assert_eq!(batch.len(), 3, "the three filings must all render");
    for r in &batch {
        assert_eq!(
            r.get("shared_pending_count").and_then(|v| v.as_i64()),
            Some(3),
            "each row of the batch reports the SHARED cohort (>= 2 renders the \
             batch badge in the page)"
        );
        assert!(
            r.get("producer_ref").and_then(|v| v.as_str()).is_some(),
            "the batch rows carry the shared reference the badge names"
        );
    }
}

/// MF-2, the planted arm: a LONE row — a unique producer/gate reference, no
/// cohort — reports exactly 1 and so renders NO batch badge (the grouping
/// discriminates: shared reference ⇒ visible batch, unique reference ⇒
/// nothing).
#[tokio::test]
async fn mf2_a_lone_row_reports_no_batch() {
    let srv = build_wire_server("mf13-lone-token");
    // one honest filing through the route, so the queue is not empty —
    // then one raw row whose reference is unique in the whole queue.
    let (status, _) = wire_post(
        &srv,
        &srv.op_token,
        "/ingest/proposal",
        serde_json::json!({ "content": "the lone-row fixture's neighbour", "kind": "fact" }),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let conn = srv.state.pool.get().expect("conn");
    conn.execute(
        "INSERT INTO proposals(kind, content, status, created_at, novelty, salience, derived_from, producer_ref)
         VALUES ('fact', 'the lone-row fixture itself', 'pending', 1, 0.5, 0.5, 'agent', 'lone-ref-no-cohort')",
        [],
    )
    .expect("the lone row");
    let (status, list) = wire_get(&srv, &srv.op_token, "/proposals?status=pending").await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let rows = list_rows(&list);
    let lone = rows
        .iter()
        .find(|r| {
            r.get("content")
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains("the lone-row fixture itself"))
        })
        .expect("the lone row renders");
    assert_eq!(
        lone.get("shared_pending_count").and_then(|v| v.as_i64()),
        Some(1),
        "a unique reference reads 1 — the page gates the badge at >= 2, so a \
         lone row never looks like a batch"
    );
}

/// MF-2, the structural half: the page renders the batch badge beside the
/// marker, and the batch key lands in ALL five locales (the fallback trap:
/// a missing non-en key silently serves English).
#[test]
fn mf2_the_render_exposes_the_batch() {
    let page = read_repo("shell/src/routes/review/+page.svelte");
    assert!(
        page.contains("review-row-shared-batch") && page.contains("review.provenance.batch"),
        "the review page must render the shared-provenance batch badge"
    );
    for loc in ["en", "de", "fr", "es", "nl"] {
        let raw = read_repo(&format!("shell/src/lib/i18n/{loc}.json"));
        let catalog: serde_json::Value =
            serde_json::from_str(&raw).expect("the catalog is valid JSON");
        assert!(
            catalog
                .get("review.provenance.batch")
                .is_some_and(|v| v.is_string()),
            "locale {loc} is missing the batch key — keys land ×5 or the \
             fallback silently serves English"
        );
    }
}
