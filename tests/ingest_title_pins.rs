// R147 — the ingest-time entity escape, driven end to end.
//
// ## Why this file exists
//
// The vault markdown ingest used to HTML-escape the title BEFORE storage
// (`Green Tea&#x27;s Secret Weapon` on the row), so the shell rendered the
// entity verbatim and every HTML emission that re-escaped a stored title
// double-escaped it. The write side now stores the RAW title — escaping is
// a rendering concern, and the emitters already escape. These pins hold
// all three faces of that decision: the round trip stores and returns the
// raw bytes through the REAL ingest route and the REAL recall handler; the
// legacy decode migration is idempotent byte-for-byte; and the public HTML
// emission escapes the stored title EXACTLY ONCE (the double-escape's
// death certificate). The escape-before-store defect is invisible to any
// unit test of the escaper itself — it only exists at the seams, so the
// claims are asserted from outside, through the shipped surfaces.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use rusqlite::Connection;
use tower::ServiceExt;

/// The opaque superuser token the fixture store accepts.
const OP_TOKEN: &str = "r147-title-pins-op-token";

struct Server {
    _dir: tempfile::TempDir,
    state: Arc<brain_server::AppState>,
}

/// An opaque-mode server on a real file-backed, real migrated database —
/// the same harness shape as `tests/trace_citation_pins.rs`; every seam
/// under test (the ingest route, the writer, the recall handler) is the
/// shipped one.
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
    body: &str,
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

/// THE ROUND-TRIP PIN: a vault markdown doc titled with an apostrophe
/// stores the RAW title on the row and returns it RAW from recall — the
/// ingest path never escapes, and the byte-faithful read seam never
/// decodes (there is nothing to decode).
#[tokio::test]
async fn a_vault_title_with_an_apostrophe_stores_raw() {
    let s = server();
    let raw_title = "Green Tea's Secret Weapon";
    let payload = serde_json::json!({
        "title": raw_title,
        "content": "# Green Tea's Secret Weapon\n\nThe leaves are steamed.\n"
    });
    let (status, resp) = post_json(&s.state, "/ingest/markdown", &payload.to_string()).await;
    assert_eq!(status, StatusCode::OK, "ingest failed: {resp}");

    // STORED raw: the row's title column holds the apostrophe, and no row
    // holds the escaped form.
    let conn = Connection::open(&s.state.db_path).expect("db");
    let stored: String = conn
        .query_row(
            "SELECT title FROM knowledge WHERE title = ?1 LIMIT 1",
            [raw_title],
            |r| r.get(0),
        )
        .expect("row with the raw title exists");
    assert_eq!(stored, raw_title);
    let escaped_rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM knowledge WHERE title LIKE '%&#x27;%'",
            [],
            |r| r.get(0),
        )
        .expect("count");
    assert_eq!(escaped_rows, 0, "the ingest path must store no entity");

    // RETURNED raw: the recall seam is byte-faithful over the raw row.
    let (rstatus, rjson) = post_json(
        &s.state,
        "/recall",
        r#"{"query": "green tea secret weapon steamed"}"#,
    )
    .await;
    assert_eq!(rstatus, StatusCode::OK, "recall failed: {rjson}");
    let hits = rjson
        .get("hits")
        .and_then(|v| v.as_array())
        .expect("hits array");
    assert!(
        hits.iter()
            .any(|h| h.get("title").and_then(|t| t.as_str()) == Some(raw_title)),
        "recall must return the raw title verbatim; got: {hits:?}"
    );
}

/// THE MIGRATION PIN: legacy escaped rows decode once, and a second run
/// changes nothing (byte-stable idempotence). Covers all three scoped
/// columns and the merge case (two escaped spellings decoding to the same
/// name rejoin into one node, edges re-pointed).
#[test]
fn the_migration_decodes_legacy_escaped_titles() {
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
    let conn = pool.get().expect("conn");

    // The fixture: the prompt's byte-level row, a sources row, and the two
    // spellings of one escaped entity (hex + decimal apostrophe) with an
    // edge on each — the merge machinery's whole surface.
    conn.execute_batch(
        r#"INSERT INTO knowledge (title, content, source, content_hash)
           VALUES ('Green Tea&#x27;s &amp; Spike''s', 'body', 'markdown', 'fixhash1');
           INSERT INTO sources (uri, kind, title) VALUES ('vault/green-tea.md', 'vault', 'Green Tea&#x27;s &amp; Spike''s');
           INSERT INTO entities (name) VALUES ('green tea&#x27;s secret weapon');
           INSERT INTO entities (name) VALUES ('green tea&#39;s secret weapon');
           INSERT INTO relationships (from_entity_id, to_entity_id, relation_type, knowledge_id)
             SELECT e1.id, e2.id, 'references', k.id FROM entities e1, entities e2, knowledge k
             WHERE e1.name = 'green tea&#x27;s secret weapon'
               AND e2.name = 'green tea&#39;s secret weapon' AND k.content_hash = 'fixhash1';"#,
    )
    .expect("fixture rows");

    let report = brain_server::migration::decode_legacy_entity_titles(&pool.get().expect("conn"))
        .expect("decode");
    assert_eq!(report.titles, 1, "one knowledge title decoded");
    assert_eq!(report.sources, 1, "one source title decoded");
    assert_eq!(report.merges, 1, "the second escaped spelling merges");
    assert_eq!(
        report.entities, 1,
        "the first-processed spelling decodes in place"
    );

    {
        let conn = pool.get().expect("conn");
        let title: String = conn
            .query_row(
                "SELECT title FROM knowledge WHERE content_hash = 'fixhash1'",
                [],
                |r| r.get(0),
            )
            .expect("row");
        assert_eq!(title, "Green Tea's & Spike's", "decoded once, honestly");
        let nodes: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM entities WHERE name = 'green tea''s secret weapon'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(nodes, 1, "the graph rejoins into ONE node");
        let edges: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM relationships r JOIN entities e ON e.id = r.from_entity_id
                 WHERE e.name = 'green tea''s secret weapon'",
                [],
                |r| r.get(0),
            )
            .expect("count");
        assert_eq!(edges, 1, "both edges re-pointed to the kept node");
    }

    // IDEMPOTENCE: a second pass over the decoded DB changes nothing.
    let first = brain_server::migration::decode_legacy_entity_titles(&pool.get().expect("conn"))
        .expect("second decode");
    assert_eq!(
        first,
        brain_server::migration::DecodeReport::default(),
        "the second run must rewrite nothing"
    );
    let second = {
        let conn = pool.get().expect("conn");
        conn.query_row(
            "SELECT title FROM knowledge WHERE content_hash = 'fixhash1'",
            [],
            |r| r.get::<_, String>(0),
        )
        .expect("row")
    };
    assert_eq!(second, "Green Tea's & Spike's", "byte-stable across runs");
}

/// THE SINGLE-ESCAPE PIN: a stored raw title reaches the public HTML
/// escaped EXACTLY ONCE — the emission site (kb.rs `esc`) is the ONLY
/// escaper between the row and the page. This is the double-escape's
/// death certificate; the escaper itself is pinned at kb.rs's own test.
#[test]
fn a_sitemap_title_is_escaped_exactly_once() {
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
    {
        let conn = pool.get().expect("conn");
        // The stored row is RAW (the ingest fix's product) and published.
        conn.execute_batch(
            r#"INSERT INTO knowledge (title, content, source, content_hash, kcs_state, public_slug)
               VALUES ('Green Tea''s Secret Weapon', 'body', 'markdown', 'smap-hash1', 'published', 'green-teas-secret-weapon');"#,
        )
        .expect("published fixture row");
    }
    let conn = pool.get().expect("conn");
    let (articles, _redirects) = brain_server::kb::collect_articles(&conn).expect("collect");
    assert_eq!(articles.len(), 1, "the published row is collected");
    let page = brain_server::kb::render_article_page(&articles[0], None);
    // kb.rs `esc` emits the DECIMAL apostrophe form (`&#39;`) — the ingest
    // escaper that stored the rows was the HEX form (`&#x27;`); the two are
    // distinguishable on the page. The title may APPEAR several times on
    // the page (head + body); the law is that EVERY occurrence is escaped
    // exactly once: every `&#39;` on the page belongs to a title
    // occurrence, and no double-escaped form appears anywhere.
    let title_occurrences = page.matches("Green Tea&#39;s Secret Weapon").count();
    assert!(
        title_occurrences >= 1,
        "the title renders human-readable (decoded by the browser)"
    );
    assert_eq!(
        page.matches("&#39;").count(),
        title_occurrences,
        "every apostrophe escape on the page belongs to one single-escaped title occurrence"
    );
    assert!(
        !page.contains("&amp;#39;") && !page.contains("&amp;#x27;") && !page.contains("&#x27;"),
        "no double escape and no foreign escape form may appear on the page"
    );
}
