//! W-T1's Tools-pkg pins — the selective installer's laws. The pkg is the
//! machine's half of "PROPOSE, never presume": its scripts are LABEL-BLIND
//! (the D1 facts arrive from the detection core's own binary), its write-set
//! is the user's home and nothing else, and the preserve law is drilled
//! behaviorally — a fake home, an operator-shaped service, and the refusal
//! observed as bytes that never moved.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// ── the harness ─────────────────────────────────────────────────────────────

const PKG_DIR_REL: &str = "../../scripts/tools-pkg";

fn pkg_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(PKG_DIR_REL)
}

fn read_pkg_file(name: &str) -> String {
    fs::read_to_string(pkg_dir().join(name))
        .unwrap_or_else(|error| panic!("{name} must exist and read: {error}"))
}

const VERSION: &str = "1.29.38";

/// A fake home for the behavioral drills: an operator-shaped service whose
/// plist references a REAL file with KNOWN bytes, an anchor with payload
/// files and a stub detector that reports the D1 facts, and a scripts dir
/// assembled exactly the way the builder assembles it (the version baked
/// into the library, the postinstall beside it).
struct FakeHome {
    root: PathBuf,
}

impl FakeHome {
    /// `preserve`: when true, the stub detector reports an existing install
    /// whose binaryPath IS `$HOME/.local/bin/brain-server` (the operator's
    /// plist shape, measured 2026-10-31).
    fn build(preserve: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tools-pkg-pin-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let anchor = root.join("Library/Application Support/StewardMem/tools");
        let versioned = anchor.join(VERSION);
        fs::create_dir_all(&versioned).expect("anchor fixture");
        // The payload, exactly what the builder assembles (core component).
        for name in ["brain-server", "brain", "mcp"] {
            fs::write(versioned.join(name), format!("new-{name}-bytes")).expect("payload");
        }
        // The referenced binary: a REAL file at the plist's ProgramArguments[0].
        let bin_dir = root.join(".local/bin");
        fs::create_dir_all(&bin_dir).expect("bin fixture");
        fs::write(
            bin_dir.join("brain-server"),
            b"THE OPERATOR'S OWN BRAIN-SERVER BYTES",
        )
        .expect("referenced binary");
        // The operator's plist, the documented launchd shape.
        let agents = root.join("Library/LaunchAgents");
        fs::create_dir_all(&agents).expect("agents fixture");
        fs::write(
            agents.join("com.brain.server.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <plist version=\"1.0\">\n<dict>\n\
                 <key>Label</key><string>com.brain.server</string>\n\
                 <key>ProgramArguments</key>\n<array>\n\
                 <string>{}/.local/bin/brain-server</string>\n\
                 </array>\n</dict>\n</plist>\n",
                root.display()
            ),
        )
        .expect("plist fixture");
        // The stub detector — the D1 facts the postinstall consumes, the
        // same `--fields` surface the real binary prints.
        let detector_body = if preserve {
            format!(
                "#!/bin/sh\n\
                 echo \"existing.binaryPath={}/.local/bin/brain-server\"\n\
                 echo \"existing.version=1.29.37\"\n\
                 echo \"existing.running=true\"\n",
                root.display()
            )
        } else {
            "#!/bin/sh\n".to_string()
        };
        fs::write(versioned.join("stewardmem-detect"), detector_body).expect("detector stub");
        let mut perms = fs::metadata(versioned.join("stewardmem-detect"))
            .expect("detector stub")
            .permissions();
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(0o755);
        fs::set_permissions(versioned.join("stewardmem-detect"), perms).expect("detector +x");
        // The scripts dir, assembled the builder's way: the version baked
        // into the library, the postinstall beside it.
        let lib_source = fs::read_to_string(pkg_dir().join("lib-postinstall.sh")).expect("the lib");
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).expect("scripts fixture");
        fs::write(
            scripts.join("lib-postinstall.sh"),
            lib_source.replace("__VERSION__", VERSION),
        )
        .expect("baked lib");
        fs::copy(
            pkg_dir().join("core-postinstall.sh"),
            scripts.join("postinstall"),
        )
        .expect("postinstall fixture");
        let mut perms = fs::metadata(scripts.join("postinstall"))
            .expect("postinstall fixture")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(scripts.join("postinstall"), perms).expect("postinstall +x");
        Self { root }
    }

    /// The service MIGRATED onto our install: the plist now references the
    /// versioned link, and the stub detector reports THAT path (the D1 read
    /// follows the plist, always).
    fn migrate_service_onto_our_link(&mut self) {
        let agents = self.root.join("Library/LaunchAgents");
        fs::write(
            agents.join("com.brain.server.plist"),
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
                 <plist version=\"1.0\">\n<dict>\n\
                 <key>Label</key><string>com.brain.server</string>\n\
                 <key>ProgramArguments</key>\n<array>\n\
                 <string>{}/.local/bin/brain-server-{VERSION}</string>\n\
                 </array>\n</dict>\n</plist>\n",
                self.root.display()
            ),
        )
        .expect("the migrated plist");
        let versioned = self
            .root
            .join("Library/Application Support/StewardMem/tools")
            .join(VERSION);
        fs::write(
            versioned.join("stewardmem-detect"),
            format!(
                "#!/bin/sh\necho \"existing.binaryPath={}/.local/bin/brain-server-{VERSION}\"\necho \"existing.version={VERSION}\"\n",
                self.root.display()
            ),
        )
        .expect("the migrated stub detector");
    }

    /// The brain-afm PAIR in the anchor payload (round 21): the binary,
    /// and — when `with_bundle` — the mlx-swift_Cmlx.bundle resource bundle
    /// beside it (the pair the metallib law requires; a bundle-less
    /// brain-afm is the broken shape the deployment shipped 2026-11-15).
    fn add_afm_payload(&self, with_bundle: bool) {
        let versioned = self
            .root
            .join("Library/Application Support/StewardMem/tools")
            .join(VERSION);
        fs::write(versioned.join("brain-afm"), b"new-brain-afm-bytes").expect("afm payload");
        if with_bundle {
            let metallib = versioned
                .join("mlx-swift_Cmlx.bundle")
                .join("Contents/Resources/default.metallib");
            fs::create_dir_all(metallib.parent().expect("bundle fixture")).expect("bundle dirs");
            fs::write(&metallib, b"THE DEFAULT METALLIB BYTES").expect("metallib fixture");
        }
    }

    fn run_core_postinstall(&self) -> String {
        let output = Command::new("/bin/bash")
            .arg(self.root.join("scripts/postinstall"))
            .env("HOME", &self.root)
            .output()
            .expect("the postinstall runs");
        assert!(
            output.status.success(),
            "the postinstall must succeed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }
}

impl Drop for FakeHome {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).ok();
    }
}

// ── the write-set, structurally ─────────────────────────────────────────────

/// The pkg is LABEL-BLIND and CANNOT ELEVATE: no script names the operator's
/// service label (the D1 facts ride the detector's `--fields` lines — a
/// script that cannot name a thing cannot clobber it), no script invokes
/// sudo or launchctl (the pkg creates no service and never touches one), and
/// no script names a system domain. The distribution REFUSES the system
/// targets structurally.
#[test]
fn the_tools_pkg_is_label_blind_and_cannot_elevate() {
    for name in [
        "lib-postinstall.sh",
        "core-postinstall.sh",
        "extras-postinstall.sh",
        "uninstall-stewardmem-tools.sh",
        "build-tools-pkg.sh",
        "welcome.html",
        "conclusion.html",
    ] {
        let source = read_pkg_file(name);
        for forbidden in ["com.brain.server", "sudo ", "launchctl", "/Library/Launch"] {
            assert!(
                !source.contains(forbidden),
                "{name} names {forbidden} — the pkg scripts are label-blind and cannot \
                 elevate; the D1 facts arrive from the detector, and a script that cannot \
                 name the operator's service cannot write to it"
            );
        }
    }
    // The distribution structurally refuses every target but the user's home.
    let distribution = read_pkg_file("distribution.xml");
    assert!(
        distribution.contains("enable_currentUserHome=\"true\"")
            && distribution.contains("enable_localSystem=\"false\"")
            && distribution.contains("enable_anywhere=\"false\"")
            && distribution.contains("rootVolumeOnly=\"false\""),
        "the distribution must allow ONLY the current user's home — no admin, no sudo, ever"
    );
    // The builder signs with the INSTALLER identity (a product archive
    // refuses the Application identity) and stages from a dir it does not
    // rebuild beside.
    let builder = read_pkg_file("build-tools-pkg.sh");
    assert!(
        builder.contains("Developer ID Installer:"),
        "the builder signs the product archive with the Developer ID Installer identity"
    );
    assert!(
        builder.contains("refusing to rebuild beside the release"),
        "the builder refuses a staged dir missing any binary — the same-build-pass law"
    );
}

// ── the preserve law, behaviorally ──────────────────────────────────────────

/// With an existing service whose plist references
/// `~/.local/bin/brain-server`: the referenced file is byte-identical after
/// the install, the referenced path is NOT taken even though it could be,
/// the new binary lands beside it as `brain-server-<version>`, the free
/// names take their plain links, and the hand-off names the preserved act
/// with the version and the path.
#[test]
fn the_preserve_law_never_touches_the_referenced_path() {
    let fixture = FakeHome::build(true);
    let referenced = fixture.root.join(".local/bin/brain-server");
    let before = sha256_of(&referenced);

    let output = fixture.run_core_postinstall();

    // The referenced bytes never moved.
    assert_eq!(
        sha256_of(&referenced),
        before,
        "the operator's referenced binary is byte-identical after the install"
    );
    let bin = fixture.root.join(".local/bin");
    // The referenced path was NOT re-created or linked over (it existed as
    // the operator's real file; nothing replaced it).
    assert!(
        !fs::symlink_metadata(&referenced)
            .expect("referenced path")
            .file_type()
            .is_symlink(),
        "the referenced path keeps being the operator's real file, not our symlink"
    );
    // The new binary landed BESIDE it.
    let beside = fs::read_link(bin.join(format!("brain-server-{VERSION}")))
        .expect("the versioned link exists");
    assert!(
        beside.ends_with(format!("tools/{VERSION}/brain-server")),
        "the versioned link points into the anchor: {beside:?}"
    );
    // The free names took their plain links.
    assert!(
        fs::read_link(bin.join("brain")).is_ok(),
        "brain took its plain link"
    );
    assert!(
        fs::read_link(bin.join("mcp")).is_ok(),
        "mcp took its plain link"
    );
    // The hand-off names the preserved act.
    assert!(
        output.contains("HAND-OFF") && output.contains("1.29.37"),
        "the hand-off names the existing version: {output}"
    );
    assert!(
        output.contains("never touched"),
        "the hand-off says the running service was never touched: {output}"
    );
}

/// An OCCUPIED path is never replaced either — even when no service
/// references it, the new link lands beside it and the existing bytes stay.
#[test]
fn an_occupied_path_is_never_replaced_only_beside() {
    let fixture = FakeHome::build(false); // no existing service
    let occupied = fixture.root.join(".local/bin/brain");
    fs::write(&occupied, b"THE USER'S OWN BRAIN BINARY").expect("occupied path");

    let output = fixture.run_core_postinstall();

    assert_eq!(
        fs::read(&occupied).expect("the user's binary"),
        b"THE USER'S OWN BRAIN BINARY",
        "an occupied path is never replaced"
    );
    let beside = fs::read_link(fixture.root.join(format!(".local/bin/brain-{VERSION}")));
    assert!(
        beside.is_ok(),
        "the new link landed beside the occupied path"
    );
    assert!(
        output.contains("HAND-OFF"),
        "the occupied path is named in the hand-off: {output}"
    );
}

// ── the metallib bundle law, round 21 ───────────────────────────────────────

/// The STATIC law: the builder demands the pair in the staged dir and packs
/// both into the core payload; the postinstall skips directory entries and
/// carries the named refusal; both tauri lanes bundle the resource beside
/// the sidecars. A build that could ship a bundle-less brain-afm must not
/// exist.
#[test]
fn the_builder_carries_the_metallib_bundle_law() {
    let builder = read_pkg_file("build-tools-pkg.sh");
    assert!(
        builder.contains("for tool in brain-server brain mcp brain-afm"),
        "the builder's payload list carries brain-afm"
    );
    assert!(
        builder.contains("the staged dir is missing mlx-swift_Cmlx.bundle"),
        "the builder refuses a staged dir without the bundle — fail-closed"
    );
    assert!(
        builder.contains("carries no Contents/Resources/default.metallib"),
        "the builder refuses a bundle without its metallib — the broken shape by name"
    );
    assert!(
        builder.contains("cp \"$STAGED_DIR/brain-afm-darwin-$ARCH\" \"$WORK/core/brain-afm\"")
            && builder.contains(
                "cp -R \"$STAGED_DIR/mlx-swift_Cmlx.bundle\" \"$WORK/core/mlx-swift_Cmlx.bundle\""
            ),
        "the core payload packs the PAIR — the binary and its resource bundle"
    );
    let postinstall = read_pkg_file("core-postinstall.sh");
    assert!(
        postinstall.contains("REFUSED: brain-afm staged without mlx-swift_Cmlx.bundle"),
        "the postinstall's refusal is named"
    );
    assert!(
        postinstall.contains("[ -f \"$tool_path\" ] || continue"),
        "the link loop skips directory entries (the bundle is not a tool)"
    );
}

/// The GREEN pair: an anchor carrying brain-afm WITH mlx-swift_Cmlx.bundle
/// beside it links the tool AND places the bundle beside the link — the
/// invoked-path law the child's loader measures (a symlink-invoked child
/// looks beside the link, never beside the real file). The bundle link is a
/// symlink into OUR anchor: one bundle, two paths, no copy.
#[test]
fn the_metallib_bundle_rides_beside_the_brain_afm_link() {
    let fixture = FakeHome::build(false); // no existing service
    fixture.add_afm_payload(true);

    let output = fixture.run_core_postinstall();

    let bin = fixture.root.join(".local/bin");
    let link = fs::read_link(bin.join("brain-afm")).expect("the brain-afm link exists");
    assert!(
        link.ends_with(format!("tools/{VERSION}/brain-afm")),
        "the brain-afm link points into the anchor: {link:?}"
    );
    let bundle = fs::read_link(bin.join("mlx-swift_Cmlx.bundle")).expect("the bundle link exists");
    assert!(
        bundle.ends_with(format!("tools/{VERSION}/mlx-swift_Cmlx.bundle")),
        "the bundle link points into the anchor: {bundle:?}"
    );
    // The loader's contract, drilled structurally: the metallib is reachable
    // through the link's own path.
    assert!(
        bin.join("mlx-swift_Cmlx.bundle/Contents/Resources/default.metallib")
            .exists(),
        "default.metallib is reachable beside the link"
    );
    assert!(
        output.contains("mlx-swift_Cmlx.bundle(beside-brain-afm)"),
        "the hand-off names the bundle placement: {output}"
    );
}

/// The RED pair refused LOUDLY: brain-afm staged WITHOUT the bundle beside
/// it gets NO link (a link would create a writer tier that dies at first MLX
/// use — "Failed to load the default metallib", exit 255, the exact
/// recurrence the deployment shipped 2026-11-15), the refusal is NAMED, and
/// the rest of the payload still installs.
#[test]
fn a_bundle_less_brain_afm_is_refused_loudly() {
    let fixture = FakeHome::build(false);
    fixture.add_afm_payload(false); // the binary alone — the broken shape

    let output = fixture.run_core_postinstall();

    let bin = fixture.root.join(".local/bin");
    assert!(
        !bin.join("brain-afm").exists(),
        "a bundle-less brain-afm takes NO link"
    );
    assert!(
        !bin.join("mlx-swift_Cmlx.bundle").exists(),
        "no bundle link without a bundle"
    );
    assert!(
        output.contains("REFUSED: brain-afm staged without mlx-swift_Cmlx.bundle"),
        "the refusal names the broken pair: {output}"
    );
    // The rest of the payload is untouched by the refusal (the fixture's
    // brain-server is the operator's occupied real file — it takes the
    // versioned beside-link per the preserve law; brain and mcp are free).
    assert!(
        fs::read_link(bin.join(format!("brain-server-{VERSION}"))).is_ok(),
        "brain-server still installs beside"
    );
    assert!(
        fs::read_link(bin.join("brain")).is_ok(),
        "brain still links"
    );
    assert!(fs::read_link(bin.join("mcp")).is_ok(), "mcp still links");
}

/// The preserve law extended to the bundle: an OCCUPIED
/// `~/.local/bin/mlx-swift_Cmlx.bundle` is never replaced — the hand-copy
/// this very machine carries is preserved and named.
#[test]
fn an_occupied_bundle_path_is_preserved_and_named() {
    let fixture = FakeHome::build(false);
    fixture.add_afm_payload(true);
    let occupied = fixture.root.join(".local/bin/mlx-swift_Cmlx.bundle");
    fs::create_dir_all(occupied.join("Contents/Resources")).expect("occupied bundle fixture");
    fs::write(
        occupied.join("Contents/Resources/default.metallib"),
        b"THE OPERATOR'S OWN METALLIB BYTES",
    )
    .expect("occupied metallib");

    let output = fixture.run_core_postinstall();

    assert_eq!(
        fs::read(occupied.join("Contents/Resources/default.metallib"))
            .expect("the operator's metallib"),
        b"THE OPERATOR'S OWN METALLIB BYTES",
        "an occupied bundle path is never replaced"
    );
    assert!(
        output.contains("PRESERVED") || output.contains("mlx-swift_Cmlx.bundle(existing)"),
        "the preserved bundle is named: {output}"
    );
}

// ── the escape hatch, behaviorally ──────────────────────────────────────────

/// The uninstaller removes ONLY symlinks that point into OUR anchor: the
/// user's own files stay, and a referenced path that is OURS is kept and
/// NAMED (an uninstall that breaks a running service is not an escape
/// hatch).
#[test]
fn the_uninstaller_removes_only_our_symlinks() {
    let fixture = FakeHome::build(true);
    fixture.run_core_postinstall();
    // The user's own binary, unrelated to ours.
    let theirs = fixture.root.join(".local/bin/theirs-tool");
    fs::write(&theirs, b"the user's own tool").expect("their tool");

    let output = Command::new("/bin/bash")
        .arg(pkg_dir().join("uninstall-stewardmem-tools.sh"))
        .env("HOME", &fixture.root)
        .output()
        .expect("the uninstaller runs");
    assert!(
        output.status.success(),
        "stdout/err: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = String::from_utf8_lossy(&output.stdout).to_string();

    // Ours are gone; the referenced path is the OPERATOR'S real file — it is
    // not ours to remove, and the uninstaller never touched it (there is no
    // KEPT, NAMED here: nothing of ours was referenced).
    assert!(
        !fs::symlink_metadata(fixture.root.join(format!(".local/bin/brain-{VERSION}"))).is_ok(),
        "our versioned links are removed"
    );
    assert!(
        !fs::symlink_metadata(fixture.root.join(".local/bin/brain")).is_ok(),
        "our plain links are removed"
    );
    assert_eq!(
        fs::read(&theirs).expect("their tool"),
        b"the user's own tool",
        "the user's own files are never touched"
    );
    assert!(
        fs::read(fixture.root.join(".local/bin/brain-server")).is_ok(),
        "the referenced path survives the uninstall untouched"
    );
    assert!(
        output.contains("never touched"),
        "the receipt says so: {output}"
    );
    assert!(
        !output.contains("REFUSED"),
        "nothing of ours was referenced — there is nothing to refuse: {output}"
    );
    // The anchor is gone.
    assert!(
        !fixture
            .root
            .join("Library/Application Support/StewardMem/tools")
            .exists(),
        "the anchor is removed"
    );
}

/// The FAIL-CLOSED guard: when the running service executes from OUR anchor
/// (a previous install became the service's binary), the uninstaller
/// REFUSES — the anchor stays, the referenced link stays, and the refusal
/// is named. An escape hatch that wrecks the service it sits beside is not
/// an escape hatch.
#[test]
fn the_uninstaller_refuses_when_the_service_runs_from_our_anchor() {
    let mut fixture = FakeHome::build(true);
    fixture.run_core_postinstall();
    // The service has MIGRATED onto our install: the plist now references
    // the versioned link the pkg created, and the detector reports THAT.
    fixture.migrate_service_onto_our_link();

    let output = Command::new("/bin/bash")
        .arg(pkg_dir().join("uninstall-stewardmem-tools.sh"))
        .env("HOME", &fixture.root)
        .output()
        .expect("the uninstaller runs");
    assert!(output.status.success());
    let output = String::from_utf8_lossy(&output.stdout).to_string();

    // The refusal, named.
    assert!(
        output.contains("REFUSED"),
        "the uninstall is refused while the service runs from our anchor: {output}"
    );
    // The anchor and the referenced link stay (fail-closed, not half-removed).
    assert!(
        fixture
            .root
            .join("Library/Application Support/StewardMem/tools")
            .exists(),
        "the anchor was NOT removed"
    );
    assert!(
        fs::read_link(
            fixture
                .root
                .join(format!(".local/bin/brain-server-{VERSION}"))
        )
        .is_ok(),
        "the referenced link survives"
    );
}

// ── the hash ────────────────────────────────────────────────────────────────

fn sha256_of(path: &Path) -> String {
    let output = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .expect("shasum runs");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string()
}
