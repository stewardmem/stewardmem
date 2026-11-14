// IMPROVE16.1 S2 — the vlm shim's pins: the fence, the two-file contract,
// the independent pages count, the 1:1 raster law, the atomicity law.
//
// These pins run the shim DIRECTLY (`scripts/glm-ocr-kernel.sh`) against the
// STUB engine (the improve05/06 dual: a recording stub the kernel cannot
// tell from the real one) — zero live model calls anywhere in CI. The
// real-tool run is the plans repo's evidence (improve161-vlm-gate/): the S1
// lane run, and the shim's own recorded run whose doc.md was BYTE-IDENTICAL
// to the S1 lane's on the same subset (an honest observation — DI-1 still
// says by-record, never claimed byte-stable).

use std::path::PathBuf;
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/documents");
const SHIM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/glm-ocr-kernel.sh");

/// Every env var the shim's fence refuses — the vlm shim's named surface
/// plus the docling.rs remote-LLM class it doubly refuses.
const FENCE_VARS: [&str; 6] = [
    "GLM_OCR_ENDPOINT",
    "GLM_OCR_API_KEY",
    "GLM_OCR_BASE_URL",
    "VLM_ENDPOINT",
    "VLM_API_KEY",
    "DOCLING_RS_VLM_ENDPOINT",
];

/// The canary stub engine: proves it RAN by writing STUB_RAN into its
/// out-dir argument ($1) and then the minimal page/stamp file set. The
/// fence pins assert the canary never fired — the refusal happens BEFORE
/// any work.
fn write_canary_engine(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("glm-ocr-canary-engine");
    std::fs::write(
        &path,
        r#"#!/bin/sh
out="$1"
mkdir -p "$out"
touch "$out/STUB_RAN"
printf 'stub page one body\n' > "$out/page-000.txt"
printf 'stub page two body\n' > "$out/page-001.txt"
printf 'glm-ocr-stub@mlx-vlm-0.0.0-stub\n' > "$out/stamp.txt"
"#,
    )
    .expect("stub engine written");
    make_executable(&path);
    path
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("stub engine executable");
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {
    // the shim itself needs /bin/sh; the stub's exec bit rides the shim's
    // invocation (sh <engine>) on non-unix, where these pins do not run.
    panic!("the vlm shim pins are unix pins (POSIX sh + poppler)")
}

/// The fence pins: each named var, set, refuses exit 5 BY NAME before any
/// work — the canary never fires, the out-dir never gains a file.
#[test]
fn gsv_the_fence_refuses_every_named_var_before_any_work() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = write_canary_engine(dir.path());
    let input = PathBuf::from(format!("{FIXTURES}/scanned.pdf"));
    for var in FENCE_VARS {
        let out = tempfile::TempDir::new().expect("out dir");
        let (code, stderr) = shim_run_with_engine(
            &input,
            out.path(),
            &engine,
            Some((var, "http://stub-remote.example")),
        );
        assert_eq!(code, Some(5), "{var} must refuse exit 5");
        assert!(
            stderr.contains(var),
            "the refusal must name the variable {var}: {stderr}"
        );
        assert!(
            stderr.contains("local-only"),
            "the refusal must state the local-only law: {stderr}"
        );
        assert!(
            !out.path().join("STUB_RAN").exists(),
            "{var}: the canary fired — the refusal happened after work started"
        );
        assert!(
            !out.path().join("doc.md").exists(),
            "{var}: the refused run must write no doc.md"
        );
    }
}

/// Run the shim with the canary engine SUBSTITUTED for the real one: the
/// pins copy the shim into a temp dir so its `dirname "$0"` engine lookup
/// resolves to the canary (the shim resolves its engine beside itself — the
/// same layout law the shipped bundle uses).
fn shim_run_with_engine(
    input: &std::path::Path,
    out: &std::path::Path,
    engine: &std::path::Path,
    fence: Option<(&str, &str)>,
) -> (Option<i32>, String) {
    let dir = tempfile::TempDir::new().expect("shim home");
    let shim_copy = dir.path().join("glm-ocr-kernel.sh");
    // The shim is staged under a sibling name and RENAMED into place (the
    // house atomic-write idiom): exec then lands on a dentry that never
    // carried an open write handle. The ubuntu 26.04 runner image
    // intermittently answers the direct write-then-exec shape with ETXTBSY
    // under the suite's parallel load (the poppler CI run on 13b732ee: 4 of
    // 6 spawns green in the same second, two "Text file busy" at the spawn
    // below — macOS has never shown it), so the spawn carries a bounded,
    // named retry on exactly that errno; a file genuinely held busy still
    // fails, loudly, at the ceiling.
    let staged = dir.path().join("glm-ocr-kernel.sh.part");
    std::fs::copy(SHIM, &staged).expect("shim staged");
    make_executable(&staged);
    std::fs::rename(&staged, &shim_copy).expect("shim renamed into place");
    std::fs::copy(engine, dir.path().join("glm-ocr-mlx.py")).expect("engine copied");

    let mut cmd = Command::new(&shim_copy);
    cmd.arg(input).arg(out).env("BRAIN_VLM_PYTHON", "/bin/sh");
    for var in FENCE_VARS {
        cmd.env(var, "");
    }
    if let Some((var, value)) = fence {
        cmd.env(var, value);
    }
    let output = spawn_retrying_text_file_busy(&mut cmd);
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// ETXTBSY is errno 26 on every unix this harness runs on; the ErrorKind
/// variant is still unstable, so the match is on the raw code.
fn spawn_retrying_text_file_busy(cmd: &mut Command) -> std::process::Output {
    const CEILING: usize = 5;
    const BACKOFF_MS: u64 = 20;
    let mut last = None;
    for _ in 0..CEILING {
        match cmd.output() {
            Ok(out) => return out,
            Err(err) if err.raw_os_error() == Some(26) => {
                last = Some(err);
                std::thread::sleep(std::time::Duration::from_millis(BACKOFF_MS));
            }
            Err(err) => panic!("spawn the shim: {err}"),
        }
    }
    panic!(
        "spawn the shim: the runner kept the freshly staged script busy \
         (ETXTBSY) past the {CEILING}-try ceiling: {:?}",
        last
    );
}

/// The two-file contract on the stub engine: exit 0, the marker between the
/// two pages, meta.json exactly the contract's object with pages from the
/// PDF's own count (the INDEPENDENT source — the stub engine cannot move it).
#[test]
fn gsv_the_shim_honors_the_two_file_contract_on_the_stub_engine() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = write_canary_engine(dir.path());
    let input = PathBuf::from(format!("{FIXTURES}/scanned.pdf")); // 2 pages, 2 rasters
    let out = tempfile::TempDir::new().expect("out dir");
    let (code, stderr) = shim_run_with_engine(&input, out.path(), &engine, None);
    assert_eq!(code, Some(0), "the contract run must succeed: {stderr}");

    let doc = std::fs::read_to_string(out.path().join("doc.md")).expect("doc.md");
    assert_eq!(
        doc.matches("<!-- page-break -->").count(),
        1,
        "2 pages → exactly 1 marker: {doc}"
    );
    assert!(doc.contains("stub page one body"), "page one first: {doc}");
    assert!(doc.contains("stub page two body"), "page two after: {doc}");
    let one = doc.find("stub page one body").expect("page one");
    let marker = doc.find("<!-- page-break -->").expect("marker");
    let two = doc.find("stub page two body").expect("page two");
    assert!(
        one < marker && marker < two,
        "the marker separates the pages"
    );

    let meta = std::fs::read_to_string(out.path().join("meta.json")).expect("meta.json");
    let v: serde_json::Value = serde_json::from_str(&meta).expect("meta.json is the object");
    assert_eq!(v["tool"], "glm-ocr", "the shim's tool name");
    assert_eq!(v["pages"], 2, "pages from pdfinfo — the independent source");
    assert!(
        v["version"]
            .as_str()
            .is_some_and(|s| s.ends_with("@mlx-vlm-0.0.0-stub")),
        "the version is the engine's stamp verbatim: {meta}"
    );
}

/// The 1:1 raster law: hybrid.pdf has 4 pages but rasters on only 2 — the
/// shim refuses to guess a page mapping, named, exit 3.
#[test]
fn gsv_the_raster_page_mismatch_refuses_named() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = write_canary_engine(dir.path());
    let input = PathBuf::from(format!("{FIXTURES}/hybrid.pdf"));
    let out = tempfile::TempDir::new().expect("out dir");
    let (code, stderr) = shim_run_with_engine(&input, out.path(), &engine, None);
    assert_eq!(code, Some(3), "the mismatch must refuse exit 3");
    assert!(
        stderr.contains("1:1"),
        "the refusal names the mapping law: {stderr}"
    );
    assert!(
        !out.path().join("doc.md").exists(),
        "a refused run writes no doc.md"
    );
}

/// The engine's completeness is checked against the PDF's own page count:
/// a stub that writes fewer page files than pages refuses, exit 3, named —
/// the marker/meta split can never smuggle a hole (VO-7's shim-side half).
#[test]
fn gsv_an_engine_that_writes_fewer_pages_refuses_named() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = dir.path().join("glm-ocr-mlx.py");
    std::fs::write(
        &engine,
        r#"#!/bin/sh
out="$1"
mkdir -p "$out"
printf 'stub page one body\n' > "$out/page-000.txt"
printf 'glm-ocr-stub@mlx-vlm-0.0.0-stub\n' > "$out/stamp.txt"
"#,
    )
    .expect("stub written");
    make_executable(&engine);
    let input = PathBuf::from(format!("{FIXTURES}/scanned.pdf"));
    let out = tempfile::TempDir::new().expect("out dir");
    let (code, stderr) = shim_run_with_engine(&input, out.path(), &engine, None);
    assert_eq!(code, Some(3));
    assert!(
        stderr.contains("no text for page 2"),
        "the refusal names the missing page: {stderr}"
    );
    assert!(!out.path().join("doc.md").exists());
}

/// The atomicity law: a failing engine leaves NO partial document — doc.md
/// is assembled in the shim's private temp dir and moved only on success.
#[test]
fn gsv_a_failing_engine_leaves_no_partial_document() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = dir.path().join("glm-ocr-mlx.py");
    std::fs::write(
        &engine,
        r#"#!/bin/sh
echo "stub engine: the model could not be loaded" >&2
exit 1
"#,
    )
    .expect("stub written");
    make_executable(&engine);
    let input = PathBuf::from(format!("{FIXTURES}/scanned.pdf"));
    let out = tempfile::TempDir::new().expect("out dir");
    let (code, stderr) = shim_run_with_engine(&input, out.path(), &engine, None);
    assert_eq!(code, Some(3));
    assert!(
        stderr.contains("the glm-ocr engine failed"),
        "the failure is named as the engine's: {stderr}"
    );
    assert!(
        stderr.contains("the model could not be loaded"),
        "the engine's bounded stderr tail rides the refusal: {stderr}"
    );
    assert!(!out.path().join("doc.md").exists());
    assert!(!out.path().join("meta.json").exists());
}

/// The missing-input arm: exit 3, named, before any work.
#[test]
fn gsv_a_missing_input_refuses_named() {
    if !cfg!(unix) {
        return;
    }
    let dir = tempfile::TempDir::new().expect("temp dir");
    let engine = write_canary_engine(dir.path());
    let input = dir.path().join("no-such.pdf");
    let out = tempfile::TempDir::new().expect("out dir");
    let (code, stderr) = shim_run_with_engine(&input, out.path(), &engine, None);
    assert_eq!(code, Some(3));
    assert!(
        stderr.contains("input is not a file"),
        "the refusal names the input: {stderr}"
    );
    assert!(!out.path().join("doc.md").exists());
}

/// The REAL-engine golden (presence-gated — the honest dual): where the
/// operator's venv exists (GLM_OCR_VENV_PYTHON, or .venv-mlx beside the
/// tree), the shim runs for real on the synthetic scan fixture and the
/// CONTRACT is asserted — exit 0, the tool stamp, markers+1 == pages, every
/// page block nonempty. DI-1 does not hold for this tier (preregistered):
/// the output is never compared byte-to-byte, only by contract and by the
/// recorded evidence. Where the toolchain is absent (CI), the pin records
/// the split and passes.
#[test]
#[ignore = "the real-engine golden: run it where the venv exists (GLM_OCR_VENV_PYTHON=<venv python> cargo test ... --ignored)"]
fn gsv_the_real_engine_golden_holds_the_contract_not_bytes() {
    if !cfg!(unix) {
        return;
    }
    let venv = std::env::var("GLM_OCR_VENV_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".venv-mlx/bin/python"));
    if !venv.is_file() {
        println!(
            "glm-ocr goldens: the operator venv is absent — the split is recorded, the pin passes"
        );
        return;
    }
    let dir = tempfile::TempDir::new().expect("shim home");
    let shim_copy = dir.path().join("glm-ocr-kernel.sh");
    std::fs::copy(SHIM, &shim_copy).expect("shim copied");
    make_executable(&shim_copy);
    // the real engine beside the copied shim
    std::fs::copy(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/glm-ocr-mlx.py"),
        dir.path().join("glm-ocr-mlx.py"),
    )
    .expect("engine copied");

    let input = PathBuf::from(format!("{FIXTURES}/scanned.pdf"));
    let out = tempfile::TempDir::new().expect("out dir");
    let mut cmd = Command::new(&shim_copy);
    cmd.arg(&input)
        .arg(out.path())
        .env("BRAIN_VLM_PYTHON", &venv);
    for var in FENCE_VARS {
        cmd.env(var, "");
    }
    let output = cmd.output().expect("spawn the shim");
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "the real run: {stderr}");

    let meta = std::fs::read_to_string(out.path().join("meta.json")).expect("meta.json");
    let v: serde_json::Value = serde_json::from_str(&meta).expect("meta.json is the object");
    assert_eq!(v["tool"], "glm-ocr");
    assert!(
        v["version"]
            .as_str()
            .is_some_and(|s| s.contains("@mlx-vlm-")),
        "the version is the stamp law's shape: {meta}"
    );
    let doc = std::fs::read_to_string(out.path().join("doc.md")).expect("doc.md");
    let pages = v["pages"].as_u64().expect("pages");
    assert_eq!(
        doc.matches("<!-- page-break -->").count() as u64,
        pages - 1,
        "markers+1 == pages (VO-7's cross-check, on the real run)"
    );
    for block in doc.split("<!-- page-break -->") {
        assert!(
            !block.trim().is_empty(),
            "every page block carries the model's reading (the 4x4 raster may be blank — \
             an AS-PRODUCED empty page is legal; this fixture's is not)"
        );
    }
}
