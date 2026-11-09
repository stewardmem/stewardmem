// IMPROVE07 S1 — the SA pins, red-first: each written against the pre-pin
// tree, each reds BY NAME, then green.
//
// ## The red ledger (measured 2026-11-08, against the pre-pin tree 2bcaa5e7
// with the S1 implementation stashed — the src/ edits stashed, this file
// compiled against the unmodified tree, run, recorded, unstashed)
//
// - SA-8 persist red (×3: the persist pin, the null pin, the re-serve pin):
//   `scholar_meta column exists: SqlInputError { … msg: "no such column:
//   scholar_meta", sql: "SELECT scholar_meta FROM knowledge WHERE
//   document_id = ?1 LIMIT 1" }` — the guarded ALTER had not landed; the
//   door's first scholar ingest red on the SELECT itself. Green: the
//   migration + the door persist landed with the pin.
// - SA-8 stamp red: `assertion left == right failed: the guarded ALTER
//   landed` (the pragma probe found 0 columns; the stamp assert never
//   ran). Green: the stamp moved with the ALTER (1.32.30 → 1.32.31;
//   re-pinned to the current stamp by each later schema round — 1.32.32 at
//   IMPROVE08).
// - SA-6 single-normalizer red: panicked at the `find("fn
//   scholar_meta_from_frontmatter")` expect — the helper (and its
//   normalize_doi call) did not exist. Green:
//   `scholar_meta_from_frontmatter` calls the ONE named normalizer and
//   never lowercases on its own.
// - SA-8 vocabulary red: panicked at the match-arm assert — `src/vault.rs`
//   carried no scholarly keys; the unknown-keys-still-drop sink arm was the
//   WHOLE vocabulary. Green: the six keys are named in one match arm, the
//   sink arm unchanged (the behavioral unknown-key-drop is the first pin's
//   `scholar_extra` assertion).
// - The parse-layer pins (src/scholar.rs unit tests) are green-on-arrival —
//   their red proof is BY MUTATION (the improve04 DI-2/DI-3 pattern,
//   recorded in IMPROVE07_EVIDENCE_2026-11-08.md): the trap pin reds against
//   a mutated floor derivation that accepts ANY `updated-by` entry; the
//   corpus pin reds against a flipped fixture byte (SHA256SUMS catches it).
//
// SA-1..SA-3 land with the S2 adapter lane (the allowlist/client/config/
// route pins); SA-2's end-to-end floor pin lands with S3; SA-4's
// CI-never-calls pin lands with the replay client in S2.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

mod common;

/// Serializes the pins in this file: each builds its own TempDir (isolated
/// storage) but the suite also touches process-global state (the screen
/// policy env removal in `server()`), and the crate's other live-tool pins
/// read the same env space — one tokio mutex, every test, no flakes.
static FILE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

const OP_TOKEN: &str = "improve07-sa-pins-op-token";

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

fn server() -> Server {
    brain_server::register_sqlite_vec::register_sqlite_vec();
    let dir = tempfile::TempDir::new().expect("temp dir");
    let db_path = dir.path().join("brain.db");
    let mgr = SqliteConnectionManager::file(&db_path);
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
    std::fs::write(&tok_file, format!("{OP_TOKEN}\n")).expect("token file");
    let token_store = brain_server::auth::TokenStore::from_file(Some(tok_file));
    token_store.reload_parts_from(vec![OP_TOKEN.to_string()], None);
    let state = Arc::new(brain_server::AppState {
        token_store,
        jwt_middleware_state: Arc::new(JwtMiddlewareState::opaque_for_tests(
            pool.clone(),
            db_path.clone(),
        )),
        cors: brain_server::server::bootstrap::cors_layer_from_config(),
        chat_answer_cache: std::sync::Arc::new(
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
    Server { _dir: dir, state }
}

async fn post_json(
    state: &Arc<brain_server::AppState>,
    uri: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value) {
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {OP_TOKEN}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request");
    let res = app(state.clone()).oneshot(req).await.expect("infallible");
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .expect("body");
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    };
    (status, json)
}

fn scholar_meta(state: &Server, document_id: &str) -> Option<String> {
    let conn = state.state.pool.get().expect("conn");
    conn.query_row(
        "SELECT scholar_meta FROM knowledge WHERE document_id = ?1 LIMIT 1",
        [document_id],
        |r| r.get::<_, Option<String>>(0),
    )
    .expect("scholar_meta column exists")
}

// ── SA-8: the frontmatter extension is backward compatible; the scholarly
//    keys persist through the markdown door into the row's scholar column ──

/// THE persist pin. The door's extended frontmatter vocabulary lands in the
/// row's `scholar_meta` JSON column; an unknown key STILL drops. Red-first:
/// pre-pin this reds with `no such column: scholar_meta` (the ALTER had not
/// landed).
#[tokio::test]
async fn sa8_scholarly_frontmatter_persists_through_the_markdown_door() {
    let _guard = FILE_LOCK.lock().await;
    let server = server();
    let content = "---\ntitle: The trap record\ndoi: HTTPS://DOI.ORG/10.1016/S0140-6736(97)11096-0\n\
                  pmid: 9500320\nsource_api: crossref\nfetched_at: 2026-11-08T00:00:00Z\n\
                  license: cc by\nretracted: 2010-02-06\nscholar_extra: MUST DROP\n---\n\n\
                  # The trap record\n\nIleal-lymphoid-nodular hyperplasia.\n";
    let (status, body) = post_json(
        &server.state,
        "/ingest/markdown",
        serde_json::json!({ "content": content }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let doc_id = body["document_id"]
        .as_str()
        .expect("document_id")
        .to_string();

    let meta = scholar_meta(&server, &doc_id);
    let meta = meta.expect("the scholarly keys persisted through the door");
    let v: serde_json::Value = serde_json::from_str(&meta).expect("scholar_meta is JSON");
    // SA-6: the DOI identity is the NORMALIZED form — lowercased, resolver
    // prefix stripped, through the ONE named normalizer.
    assert_eq!(
        v["doi"].as_str(),
        Some("10.1016/s0140-6736(97)11096-0"),
        "the stored DOI is the normalized identity"
    );
    assert_eq!(v["pmid"].as_str(), Some("9500320"));
    assert_eq!(v["source_api"].as_str(), Some("crossref"));
    assert_eq!(v["fetched_at"].as_str(), Some("2026-11-08T00:00:00Z"));
    assert_eq!(v["license"].as_str(), Some("cc by"));
    assert_eq!(v["retracted"].as_str(), Some("2010-02-06"));
    // THE BACKWARD-COMPATIBILITY HALF: the unknown key dropped — the
    // extension is the named act, not a vocabulary opening.
    assert!(
        v.get("scholar_extra").is_none(),
        "unknown frontmatter keys STILL drop (SA-8)"
    );
}

/// A document without scholarly frontmatter persists NULL — byte-compatible
/// with every row stored before the extension (the corpus-compat law).
#[tokio::test]
async fn sa8_documents_without_scholarly_frontmatter_persist_null() {
    let _guard = FILE_LOCK.lock().await;
    let server = server();
    let (status, body) = post_json(
        &server.state,
        "/ingest/markdown",
        serde_json::json!({ "content": "# Plain note\n\nNo scholarly keys here.\n",
                            "title": "Plain note" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let doc_id = body["document_id"]
        .as_str()
        .expect("document_id")
        .to_string();
    assert_eq!(
        scholar_meta(&server, &doc_id),
        None,
        "no scholarly keys → no scholar_meta value (never an empty JSON object)"
    );
}

/// The retraction signal survives its own no-op re-serve: the same record
/// re-ingested (identical chunks, the dedupe path) still lands the flag —
/// persist runs by document_id, not by the insert path.
#[tokio::test]
async fn sa8_the_flag_survives_a_deduped_re_serve() {
    let _guard = FILE_LOCK.lock().await;
    let server = server();
    // First serve: plain body, no scholarly keys.
    let (status, body) = post_json(
        &server.state,
        "/ingest/markdown",
        serde_json::json!({ "content": "# Wakefield 1998\n\nThe abstract body.\n",
                            "title": "Wakefield 1998" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Second serve: same title (same document_id) + the scholarly vocabulary.
    let (status, body) = post_json(
        &server.state,
        "/ingest/markdown",
        serde_json::json!({ "content": "---\ntitle: Wakefield 1998\ndoi: 10.1016/S0140-6736(97)11096-0\nretracted: 2010-02-06\n---\n\nThe abstract body.\n" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let doc_id = body["document_id"]
        .as_str()
        .expect("document_id")
        .to_string();
    let meta = scholar_meta(&server, &doc_id).expect("the flag lands on the re-serve");
    let v: serde_json::Value = serde_json::from_str(&meta).unwrap();
    assert_eq!(v["retracted"].as_str(), Some("2010-02-06"));
}

// ── the guarded ALTER + the stamp move ───────────────────────────────────

/// The ONE guarded ALTER: the column exists after `run_migration` and the
/// stamp moved with it. Red-first: pre-pin the stamp read 1.32.30 and the
/// column was absent.
#[test]
fn sa8_the_migration_adds_the_scholar_column_and_moves_the_stamp() {
    let mut conn = rusqlite::Connection::open_in_memory().expect("memory db");
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("migration");
    let present: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('knowledge') WHERE name='scholar_meta'",
            [],
            |r| r.get(0),
        )
        .expect("pragma");
    assert_eq!(present, 1, "the guarded ALTER landed");
    let stamp: String = conn
        .query_row(
            "SELECT value FROM schema_meta WHERE key='schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("stamp");
    assert_eq!(
        stamp, "1.32.33",
        "run_migration must stamp the CURRENT ceiling — the stamp and the column moved \
         together at 1.32.31, and each later schema round re-pins the stamp (1.32.32 at \
         IMPROVE08, 1.32.33 at IMPROVE11's hitl_decision_events)"
    );
    // The guard is REAL: a second run is a no-op (the pragma probe skips).
    brain_server::migration::run_migration(&mut conn, brain_server::config::DB_MMAP_SIZE_MIB)
        .expect("re-migration");
}

// ── SA-6: the DOI normalizer is single ──────────────────────────────────

/// The single-normalizer law, structurally: the door's scholar path calls
/// the ONE named normalizer — a second lowercase/strip point in the door is
/// a found bug. Reads the production source (comment-stripped via the
/// shared stripper), so a comment naming the law cannot satisfy the pin.
#[test]
fn sa6_the_door_normalizes_through_the_one_named_normalizer() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/server/router/memory.rs"
    ))
    .expect("the door source");
    let prod = common::code_only(&src);
    let helper_at = prod
        .find("fn scholar_meta_from_frontmatter")
        .expect("the scholar_meta builder exists in the door");
    let helper = &prod[helper_at..];
    assert!(
        helper.contains("crate::scholar::normalize_doi"),
        "the door's DOI handling goes through scholar::normalize_doi (SA-6: \
         total and single — a second normalizer is a found bug)"
    );
    // …and the builder never lowercases on its own.
    let helper_body = helper
        .split("fn persist_scholar_meta")
        .next()
        .expect("split always yields a first slice");
    assert!(
        !helper_body.contains("to_lowercase()"),
        "the builder must not re-normalize — the identity law is ONE function"
    );
}

/// The extension is the named act: the scholarly keys are spelled in the
/// vault match and the unknown-key sink arm is UNTOUCHED (SA-8 structural).
#[test]
fn sa8_the_vocabulary_extension_is_additive_and_the_sink_arm_untouched() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vault.rs"))
        .expect("the vault source");
    let prod = common::code_only(&src);
    let key = "\"doi\" | \"pmid\" | \"source_api\" | \"fetched_at\" | \"license\" | \"retracted\"";
    assert!(
        prod.contains(key),
        "the scholarly vocabulary is named in one match arm: {key}"
    );
    assert!(
        prod.contains("_ => {}"),
        "the unknown-key sink arm survives the extension — unknown keys STILL drop"
    );
    // The six fields exist on the struct.
    for field in [
        "doi",
        "pmid",
        "source_api",
        "fetched_at",
        "license",
        "retracted",
    ] {
        assert!(
            prod.contains(&format!("pub {field}: Option<String>")),
            "Frontmatter carries the scholarly key {field}"
        );
    }
}
