//! Pin 18 — `the_crate_loads_no_model_and_makes_no_network_call`. The
//! crate's dependency list carries no model runner and no HTTP client; the
//! only encoder is the port, and the kernel fills it. The other half of the
//! pin (an ingest under a fake encoder completes with networking refused)
//! is proven by every fake-encoder ingest test: they run with no network
//! surface at all, because the crate has none to call.

#[test]
fn the_crate_carries_no_model_or_network_dependency() {
    let manifest = include_str!("../Cargo.toml");
    // Whole-token names: a model runner or an HTTP client under any alias
    // would still declare one of these crates.
    let banned_substrings = [
        "fastembed",
        "reqwest",
        "hf-hub",
        "model2vec",
        "tokenizers",
        "ort-sys",
    ];
    for banned in banned_substrings {
        assert!(
            !manifest.contains(banned),
            "brain-code-core's Cargo.toml declares `{banned}` — the crate loads no model and \
             makes no network call (pin 18); the encoder arrives through the port"
        );
    }
    // The line-form `ort = ...` / `ort-{...}` dependency — checked line-wise
    // so words like `sort` cannot false-positive.
    for line in manifest.lines() {
        let trimmed = line.trim();
        let banned_line = ["ort ", "ort=", "ort=\"", "ort {"]
            .iter()
            .any(|form| trimmed.starts_with(*form) || trimmed.starts_with(&format!("\"{form}")));
        assert!(
            !banned_line || trimmed.starts_with('#'),
            "brain-code-core's Cargo.toml declares an ONNX runtime line: {trimmed} (pin 18)"
        );
    }
}

#[test]
fn the_port_is_the_only_encoder_surface() {
    // The crate's src reaches no model runner or HTTP client by name: the
    // only encoder is the trait and its fake (cfg(test)). A direct encoder
    // dependency (the drill's mutation) would have to arrive with a real
    // encoder type somewhere in src/ — this scan names it.
    let src_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    let mut stack = vec![src_dir.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                for (idx, line) in text.lines().enumerate() {
                    let lower = line.to_lowercase();
                    for marker in [
                        "fastembed::",
                        "model2vec",
                        "ort::",
                        "reqwest::",
                        "hf_hub",
                        "tokenizers::",
                    ] {
                        if lower.contains(marker) {
                            offenders.push(format!(
                                "{}:{idx}: {marker}",
                                path.strip_prefix(&src_dir).unwrap().display()
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "the crate's source reaches a model or network surface directly: {}",
        offenders.join("; ")
    );
}
