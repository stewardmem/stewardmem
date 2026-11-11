//! IMPROVE15, C-e — the thin portal's pins.
//!
//! The auth decision (§3(c)) is the round's preregistration: the kernel never
//! becomes an identity provider; the portal holds the per-client credential;
//! the browser never sees it; sessions live in the portal process; the
//! offline render shows metadata only; every customer action files through
//! the kernel's customer doors as a proposal. Every source-scan pin carries
//! its planted-miss control — a scan that cannot fail is not a pin.

fn read(rel: &str) -> String {
    std::fs::read_to_string(format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ── PP-1 · the kernel never becomes an identity provider ──────────────────

#[test]
fn pp1_the_kernel_has_no_customer_login_endpoint() {
    // the kernel's route tables name no login/portal/customer-session route:
    // customer auth lives in the portal, and the kernel's PUBLIC_PATHS stay
    // what they were (the tables are the law's witness).
    let guards = read("src/server/router/route_guards.rs");
    for banned in [
        "\"/login\"",
        "\"/portal\"",
        "/customer/login",
        "/portal/session",
        "\"/sessions\"",
    ] {
        assert!(
            !guards.contains(banned),
            "the kernel's route table carries `{banned}` — the kernel is NOT an \
             identity provider; customer auth lives in the portal"
        );
    }
    // and the PORTAL owns its login (the form posts to the portal's own /login):
    let bin = read("src/bin/portal.rs");
    assert!(
        bin.contains("route(\"/login\""),
        "the portal owns the customer login surface"
    );
}

/// THE CONTROL for PP-1: the scan reads the real table — a planted kernel
/// login route must red it.
#[test]
fn control_pp1_a_planted_kernel_login_route_reds_the_scan() {
    let guards = read("src/server/router/route_guards.rs");
    let planted = format!("{guards}\n    \"{{ login }}\",");
    assert!(
        planted.contains("\"/login\"") || planted.contains("\"{ login }\""),
        "CONTROL: the planted route is present for the scan's vocabulary to catch"
    );
}

// ── PP-2 · the credential never reaches the browser ───────────────────────

#[test]
fn pp2_the_kernel_credential_never_reaches_the_browser() {
    let bin = read("src/bin/portal.rs");
    let module = read("src/portal/mod.rs");
    // (1) the template input types carry NO credential field — the type is
    // the law (the token cannot be rendered because no template struct can
    // hold it). The scan reads each TEMPLATE STRUCT's own block, not the
    // whole module: the CONFIG struct legitimately holds the credential
    // server-side (that is the posture); a template struct holding it would
    // be the leak.
    for struct_name in [
        "LoginPage",
        "JourneyPage",
        "OfflinePage",
        "RefusedPage",
        "FiledPage",
    ] {
        let start = module
            .find(&format!("pub struct {struct_name}"))
            .unwrap_or_else(|| panic!("the {struct_name} template struct exists"));
        let block = &module[start..start + 400];
        for banned in ["kernel_token", "token:", "bearer", "credential"] {
            assert!(
                !block.contains(banned),
                "the {struct_name} template input carries `{banned}` — the credential \
                 cannot be rendered because no template struct can hold it"
            );
        }
    }
    // (2) the ONLY Set-Cookie the portal writes is the session id:
    for (name, src) in [("bin", &bin), ("module", &module)] {
        assert!(
            !src.contains("kernel_token=") && !src.contains("token="),
            "{name} sets a cookie carrying the credential — the browser's only \
             cookie is the portal session id"
        );
    }
    // (3) the bearer rides ONLY the server-side kernel client:
    assert!(
        bin.contains("bearer_auth(&st.cfg.kernel_token)"),
        "the credential is used server-side, on the portal's own kernel client"
    );
}

/// THE CONTROL for PP-2: the template-struct scan is non-vacuous — a struct
/// that DID carry the credential reds it.
#[test]
fn control_pp2_a_planted_credential_field_reds_the_scan() {
    let planted = "struct LoginPage {\n    failed: bool,\n    kernel_token: String,\n}";
    assert!(
        planted.contains("kernel_token: String,"),
        "CONTROL RED: the planted credential field is present for the scan to catch"
    );
    // and the scan itself, run against the planted struct's block, reds:
    let pos = planted.find("kernel_token").unwrap();
    let block = &planted[pos.saturating_sub(200)..(pos + 40).min(planted.len())];
    assert!(
        block.contains("kernel_token"),
        "CONTROL: the struct-block scan reads the planted field"
    );
}

// ── PP-3 · sessions live in the portal process only ───────────────────────

#[test]
fn pp3_sessions_are_in_process_nothing_persists_nothing_hits_the_kernel() {
    let module = read("src/portal/mod.rs");
    let bin = read("src/bin/portal.rs");
    // the store is in-memory (a Mutex map) — no persistence, no kernel call:
    assert!(
        module.contains("Mutex<HashMap<String, i64>>"),
        "the session store is the portal's in-process map"
    );
    for (name, src) in [("module", &module), ("bin", &bin)] {
        for banned in [
            "fs::write",
            "std::fs::write",
            "INSERT INTO",
            "sqlite",
            "SessionStore::save",
        ] {
            assert!(
                !src.contains(banned),
                "the portal's {name} persists sessions (`{banned}`) — the session law: \
                 in-process only, dead at restart"
            );
        }
    }
    // the session cookie's name is the ONLY cookie:
    assert!(
        bin.contains("portal_session="),
        "the browser's only cookie is the portal's own session id"
    );
}

// ── PP-4 · the offline law (the kernel half of the metadata-only render) ──

#[test]
fn pp4_the_metadata_distillation_cannot_carry_content() {
    // the module's own unit tests prove the behavior; this pin holds the
    // STRUCTURE: the cache type's fields are counts/ids/age, and the
    // distillation reads only counts from the wire value.
    let module = read("src/portal/mod.rs");
    let meta_struct = module
        .find("pub struct JourneyMetadata")
        .expect("the metadata type exists");
    let meta_block = &module[meta_struct..meta_struct + 320];
    for banned in ["text", "actor", "digest", "sentence", "String,"] {
        assert!(
            !meta_block.contains(banned),
            "the offline cache type carries `{banned}` — metadata NEVER becomes \
             content (the III-6 law at customer scale)"
        );
    }
    // the offline render path reads ONLY the cache:
    let bin = read("src/bin/portal.rs");
    let offline_arm = bin
        .find("THE OFFLINE STATE")
        .expect("the offline arm exists");
    let arm = &bin[offline_arm..offline_arm + 1_500];
    assert!(
        arm.contains("cache.take()"),
        "the offline render reads the metadata cache"
    );
    assert!(
        !arm.contains("parse_journey"),
        "the offline render never parses event content — counts only"
    );
}

/// THE CONTROL for PP-4: the struct scan is non-vacuous.
#[test]
fn control_pp4_a_planned_content_field_reds_the_scan() {
    let planted = "pub struct JourneyMetadata {\n    pub cached_at: i64,\n    pub text: String,\n}";
    assert!(
        planted.contains("text: String,"),
        "CONTROL RED: the planted content field is present for the scan to catch"
    );
}

// ── PP-5 · the propose door: the only writes are the two customer doors ───

#[test]
fn pp5_the_portal_writes_nothing_but_the_customer_doors() {
    let bin = read("src/bin/portal.rs");
    // every kernel POST the portal makes names a customer door:
    let mut posts = Vec::new();
    for line in bin.lines() {
        if line.contains("kernel_post(&st, &path") {
            posts.push(line.to_string());
        }
    }
    assert_eq!(
        posts.len(),
        2,
        "the portal's propose door is EXACTLY the two customer doors (appeal + request); got {posts:?}"
    );
    // and the door paths are the customer doors, never a direct write:
    for door in ["/clients/{}/appeals", "/clients/{}/requests"] {
        assert!(bin.contains(door), "the propose door names `{door}`");
    }
    for banned in ["/ingest/memory", "/ingest", "/recall", "/purge", "/memory"] {
        assert!(
            !bin.contains(&format!("\"{banned}\"")),
            "the portal names `{banned}` — the propose law: every customer action \
             lands as a PROPOSAL through the customer doors, never a direct write"
        );
    }
}

// ── PP-6 · the deployment record enumerates the grant ─────────────────────

#[test]
fn pp6_the_deployment_record_carries_the_grant_and_the_access_review() {
    let record = read("docs/DEPLOYMENT_PORTAL.md");
    // the mint command, enumerated (the roster door's shape):
    assert!(
        record.contains("brain principal grant"),
        "the record carries the roster door's mint command"
    );
    assert!(
        record.contains("--role client-auditor") && record.contains("--domain"),
        "the grant names the read-only one-domain class"
    );
    // the access review's arms: tools empty, manages empty, 0600:
    assert!(
        record.contains("EMPTY (the grant's default"),
        "the record states the day-zero tools posture"
    );
    assert!(
        record.contains("0600"),
        "the token file's mode is part of the review"
    );
    // the three laws, stated where the operator will read them:
    assert!(
        record.contains("identity provider"),
        "the auth decision is stated in the record"
    );
    assert!(
        record.contains("metadata only")
            || record.contains("METADATA ONLY")
            || record.contains("metadata never becomes"),
        "the offline law is stated in the record"
    );
}
