//! IMPROVE16 S3 — the DepEd witness runner.
//!
//! Drives the REAL `/ingest/document` route (the real migrated schema, the
//! real doors, the REAL local corrector endpoint) over the DepEd Order 003,
//! s. 2026 page-range subset (pages 27–29 of the operator's searchable
//! variant — the CPG pattern for over-cap scans), once with `correct: true`.
//! The response JSON (both artifacts verbatim + corrected) prints to
//! stdout; the transcript is the round's evidence file.
//!
//! THIS IS NOT A CI TEST — it is the scripted live witness. It needs the
//! local model served at BRAIN_DOC_CORRECTOR_BASE_URL (the witness run used
//! ollama + qwen2.5:3b on 127.0.0.1:11434) and the subset PDF on disk.
//!
//! Usage: cargo run --example deped_witness -- <pdf-path> <out-json-path>

use std::sync::Arc;

use axum::body::Body;
use axum::http::Request;
use base64::Engine as _;
use brain_server::pool::SqliteConnectionManager;
use brain_server::server::router::app;
use brain_server::server::router::auth::JwtMiddlewareState;
use tower::ServiceExt;

const OP_TOKEN: &str = "improve16-witness-op-token";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "usage: deped_witness <pdf-path> <out-json-path> [model] [--verbatim] [--url URL]"
        );
        std::process::exit(2);
    }
    let pdf_path = &args[1];
    let out_path = &args[2];
    let model_name = args
        .get(3)
        .map(|s| s.trim_start_matches("--").to_string())
        .unwrap_or_else(|| "qwen2.5:3b".to_string());
    let verbatim_only = args.iter().any(|a| a == "--verbatim");
    let base_url = args
        .iter()
        .position(|a| a == "--url")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:11434".to_string());

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

    // The corrector env: the operator's LOCAL ollama (the local-endpoint
    // posture's exact shape). The secret: a scratch 0600 file (ollama
    // ignores the Authorization header, but the secret-file law's SHAPE is
    // all-or-nothing and is exercised verbatim).
    let secret_dir = dir.path().join("corrector-root");
    std::fs::create_dir_all(&secret_dir).expect("secret root");
    let secret = secret_dir.join("corrector.key");
    std::fs::write(&secret, "witness-scratch-secret").expect("secret");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).expect("chmod");
    }
    // SAFETY: witness process; the env is the configuration.
    unsafe {
        std::env::set_var("BRAIN_DOC_CORRECTOR_BASE_URL", &base_url);
        std::env::set_var("BRAIN_DOC_CORRECTOR_MODEL", &model_name);
        std::env::set_var("BRAIN_DOC_CORRECTOR_SECRET_FILE", "corrector.key");
        std::env::set_var(
            "BRAIN_DOC_CORRECTOR_SECRET_ROOT",
            secret_dir.display().to_string(),
        );
    }

    let pdf = std::fs::read(pdf_path).expect("the witness subset pdf");
    eprintln!("witness: {} bytes of PDF ({})", pdf.len(), pdf_path);
    let body = serde_json::json!({
        "data": base64::engine::general_purpose::STANDARD.encode(&pdf),
        "title": "DepEd Order No. 003, s. 2026 (witness: pages 27-29, searchable variant)",
        "tier": "text-layer",
        "correct": !verbatim_only,
    })
    .to_string();

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let started = std::time::Instant::now();
    let result = runtime.block_on(async {
        let req = Request::builder()
            .method("POST")
            .uri("/ingest/document")
            .header("authorization", format!("Bearer {OP_TOKEN}"))
            .header("content-type", "application/json")
            .body(Body::from(body))
            .expect("request");
        let res = app(state.clone()).oneshot(req).await.expect("infallible");
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .expect("body");
        (status, bytes)
    });
    let elapsed = started.elapsed();
    let (status, bytes) = result;
    eprintln!(
        "witness: route answered {} in {:.1}s",
        status,
        elapsed.as_secs_f32()
    );

    // The transcript: status + body + the rows as the DB holds them + the
    // audit rows (the doc-correction call's evidence).
    let body_json: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    let conn = state.pool.get().expect("conn");
    let mut rows_out = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, sha256, tier, extractor, extractor_version, pages, markdown_sha256,
                    extracted_utc FROM documents ORDER BY id",
        )
        .expect("stmt");
    let mut rows = stmt.query([]).expect("rows");
    while let Ok(Some(r)) = rows.next() {
        rows_out.push(serde_json::json!({
            "id": r.get::<_, i64>(0).unwrap_or(-1),
            "sha256": r.get::<_, String>(1).unwrap_or_default(),
            "tier": r.get::<_, String>(2).unwrap_or_default(),
            "extractor": r.get::<_, String>(3).unwrap_or_default(),
            "extractor_version": r.get::<_, String>(4).unwrap_or_default(),
            "pages": r.get::<_, i64>(5).unwrap_or(-1),
            "markdown_sha256": r.get::<_, String>(6).unwrap_or_default(),
            "extracted_utc": r.get::<_, String>(7).unwrap_or_default(),
        }));
    }
    let audit_rows: Vec<serde_json::Value> = {
        let mut out = Vec::new();
        let mut stmt = conn
            .prepare("SELECT id, kind, status FROM audit_events WHERE kind = 'doc-correction' ORDER BY id")
            .expect("audit stmt");
        let mut rows = stmt.query([]).expect("audit rows");
        while let Ok(Some(r)) = rows.next() {
            out.push(serde_json::json!({
                "id": r.get::<_, i64>(0).unwrap_or(-1),
                "kind": r.get::<_, String>(1).unwrap_or_default(),
                "status": r.get::<_, String>(2).unwrap_or_default(),
            }));
        }
        out
    };
    let transcript = serde_json::json!({
        "witness": {
            "pdf_bytes": pdf.len(),
            "route_status": status.as_u16(),
            "route_seconds": elapsed.as_secs_f64(),
            "response": body_json,
            "documents_rows": rows_out,
            "doc_correction_audit_rows": audit_rows,
            "model": format!("{model_name} (served at {base_url} — the local endpoint)"),
            "corpus": "DepEd Order No. 003, s. 2026 — pages 27-29, the operator's searchable variant (the inherited OCR layer), the CPG page-range pattern",
            "verbatim_only": verbatim_only,
            "reference": "DO_s2026_003r-1.md — the operator's corrected transcription (pages 27-29)",
        }
    });
    std::fs::write(
        out_path,
        serde_json::to_string_pretty(&transcript).expect("json"),
    )
    .expect("write transcript");
    eprintln!("witness: transcript → {out_path}");
}
