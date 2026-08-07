//! Sequencer-backed end-to-end integration test.
//!
//! This is the real E2E check the assessment asked for: it launches the
//! standalone LEZ sequencer itself (on an ephemeral port, with a fresh temp
//! data dir), then drives the full airdrop flow (enroll, deploy, fund, claim,
//! double-claim) through the actual CLI binaries and asserts the same
//! invariants the demo asserts.
//!
//! Prerequisites (see `scripts/run_integration_tests.sh`):
//!   - the pinned `sequencer_service` binary built with `--features standalone`,
//!     reachable at `$LEZ_SEQUENCER_BIN` (or the default cache path),
//!   - the deploy artifact built (`methods/.../airdrop.bin`) and the host bins
//!     built (`cargo build --bins`),
//!   - `RISC0_DEV_MODE=1` set so the guest runs with a dev (non-succinct) prover.
//!
//! The test is `#[ignore]`d by default so plain `cargo test` stays fast and
//! dependency-free; the script runs it explicitly with `-- --ignored`.

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

const ENROLL: &str = env!("CARGO_BIN_EXE_airdrop_enroll");
const DEPLOY: &str = env!("CARGO_BIN_EXE_airdrop_deploy");
const FUND: &str = env!("CARGO_BIN_EXE_airdrop_fund");
const CLAIM: &str = env!("CARGO_BIN_EXE_airdrop_claim");
const STATUS: &str = env!("CARGO_BIN_EXE_airdrop_status");

const DEFAULT_SEQUENCER_BIN: &str =
    "/home/badman/.cache/logos-lez-rln/sequencer-src/target/release/sequencer_service";

fn sequencer_bin() -> PathBuf {
    std::env::var("LEZ_SEQUENCER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(DEFAULT_SEQUENCER_BIN))
}

/// Derive the standalone config from the bin's source root
/// (`<src>/target/release/sequencer_service` -> `<src>/lez/sequencer/service/
/// configs/debug/sequencer_config.json`), overridable via LEZ_SEQUENCER_CONFIG.
fn sequencer_config() -> PathBuf {
    if let Ok(cfg) = std::env::var("LEZ_SEQUENCER_CONFIG") {
        return PathBuf::from(cfg);
    }
    // release -> target -> <src-root>
    sequencer_bin()
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|src| src.join("lez/sequencer/service/configs/debug/sequencer_config.json"))
        .unwrap_or_else(|| {
            panic!("cannot derive sequencer config from bin; set LEZ_SEQUENCER_CONFIG")
        })
}

/// Wait until the sequencer RPC health check succeeds.
fn wait_for_health(port: u16) {
    for _ in 0..120 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("sequencer on port {port} did not become reachable within 60s");
}

struct SequencerGuard {
    child: Child,
    port: u16,
    data_dir: PathBuf,
}

impl SequencerGuard {
    fn spawn() -> Self {
        let bin = sequencer_bin();
        assert!(
            bin.exists(),
            "sequencer binary not found at {bin:?}; set LEZ_SEQUENCER_BIN or run scripts/run_integration_tests.sh"
        );
        let config = sequencer_config();
        assert!(
            config.exists(),
            "sequencer config not found at {config:?}; set LEZ_SEQUENCER_CONFIG"
        );

        // A free port: bind a socket, grab its port, then drop it. Slight race
        // with the bind in the child, acceptable for CI.
        let port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind probe port");
            listener.local_addr().expect("probe addr").port()
        };

        // Fresh, empty data dir so the sequencer creates genesis state from scratch.
        let data_dir = std::env::temp_dir().join(format!("airdrop-seqtest-{}", std::process::id()));
        std::fs::create_dir_all(&data_dir).expect("create sequencer data dir");

        eprintln!("Starting sequencer {bin:?} on port {port}, data in {data_dir:?}...");
        let child = Command::new(&bin)
            .arg(&config)
            .arg("--port")
            .arg(port.to_string())
            .current_dir(&data_dir)
            .env("RISC0_DEV_MODE", "1")
            .spawn()
            .unwrap_or_else(|e| panic!("failed to spawn sequencer {bin:?}: {e}"));

        Self {
            child,
            port,
            data_dir,
        }
    }
}

impl Drop for SequencerGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.data_dir);
        eprintln!("Sequencer stopped, temp data dir removed.");
    }
}

/// Run one airdrop bin with the given wallet dir and args, returning stdout.
fn run_bin(bin: &str, wallet_dir: &Path, args: &[&str]) -> String {
    let out = Command::new(bin)
        .args(args)
        .env("LEE_WALLET_HOME_DIR", wallet_dir)
        .env("RISC0_DEV_MODE", "1")
        // Tune block-seal and account-wait polling so the test is fast but
        // still tolerant of slow CI machines.
        .env("LEZ_AIRDROP_BLOCK_SEAL_SECS", "3")
        .env("LEZ_AIRDROP_ACCOUNT_WAIT_ATTEMPTS", "120")
        .output()
        .expect("run bin");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        panic!("{bin} failed\n--- stdout ---\n{stdout}\n--- stderr ---\n{stderr}");
    }
    stdout
}

fn wallet_dir(base: &Path, name: &str) -> PathBuf {
    let d = base.join(name);
    std::fs::create_dir_all(&d).expect("create wallet dir");
    d
}

fn write_wallet_config(wallet_dir: &Path, port: u16) {
    // The wallet reads `sequencer_addr` from wallet_config.json; the default is
    // 127.0.0.1:3040, so we must point it at our ephemeral port before the bin
    // connects. `init_wallet()` in airdrop_enroll creates the file only if it
    // does not exist, so pre-seed it here.
    let cfg = format!(
        r#"{{"sequencer_addr":"http://127.0.0.1:{port}/","seq_poll_timeout":"12s","seq_tx_poll_max_blocks":5,"seq_poll_max_retries":5,"seq_block_poll_max_amount":100}}"#
    );
    std::fs::write(wallet_dir.join("wallet_config.json"), cfg).expect("write wallet config");
}

/// Runs the airdrop bins from the crate root (their relative `.logos-airdrop`
/// and `methods/target` paths require that), so the stale local demo data must
/// be moved aside for the duration of the test and restored afterwards.
struct LocalStateGuard {
    moved: Option<PathBuf>,
}

impl LocalStateGuard {
    fn park() -> Self {
        let local = PathBuf::from(".logos-airdrop");
        if local.exists() {
            let parked = std::env::temp_dir()
                .join(format!("airdrop-localstate-parked-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&parked);
            std::fs::rename(&local, &parked).expect("move aside local .logos-airdrop");
            Self {
                moved: Some(parked),
            }
        } else {
            Self { moved: None }
        }
    }
}

impl Drop for LocalStateGuard {
    fn drop(&mut self) {
        let local = PathBuf::from(".logos-airdrop");
        let _ = std::fs::remove_dir_all(&local);
        if let Some(parked) = self.moved.take() {
            let _ = std::fs::rename(parked, local);
        }
    }
}

/// Assert the demo's invariants against a real sequencer.
#[test]
#[ignore = "requires a running LEZ standalone sequencer; use scripts/run_integration_tests.sh"]
fn full_flow_against_sequencer() {
    // Park any pre-existing local demo state so the run starts clean and the
    // demo data is restored afterwards.
    let _local = LocalStateGuard::park();

    // Fresh work dir for per-wallet configs; enrollments + manifest are written
    // by the bins into `.logos-airdrop` under the crate-root CWD.
    let work = std::env::temp_dir().join(format!("airdrop-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    // --- Start the sequencer on an ephemeral port ---
    let guard = SequencerGuard::spawn();
    let port = guard.port;
    wait_for_health(port);
    eprintln!("Sequencer healthy on port {port}.");

    // --- 1. Recipients enroll ---
    let wallets = work.join("wallets");
    for name in ["alice", "bob", "carol"] {
        let wd = wallet_dir(&wallets, name);
        write_wallet_config(&wd, port);
        let out = run_bin(ENROLL, &wd, &[name, "1000000"]);
        assert!(
            out.contains("Enrollment written"),
            "enroll {name} did not write enrollment:\n{out}"
        );
    }

    // --- 2. Distributor deploys + commits the root ---
    let distributor = wallet_dir(&wallets, "distributor");
    write_wallet_config(&distributor, port);
    let out = run_bin(DEPLOY, &distributor, &["1"]);
    assert!(
        out.contains("Manifest written"),
        "deploy did not write manifest:\n{out}"
    );

    // --- 3. Distributor funds each D_i ---
    let out = run_bin(FUND, &distributor, &[]);
    assert!(
        out.contains("All 3 allocations minted"),
        "fund did not mint all allocations:\n{out}"
    );

    // --- 4. Recipients claim, and the double claim must fail ---
    for name in ["alice", "bob", "carol"] {
        let wd = wallets.join(name);
        let out = run_bin(CLAIM, &wd, &["--name", name]);
        assert!(
            out.contains("claim tx hash"),
            "claim {name} did not produce a tx hash:\n{out}"
        );
        assert!(
            out.contains("OK: double claim prevented"),
            "claim {name} did not reject the double claim:\n{out}"
        );
    }

    // --- 5. Status verifies the on-chain root against the enrollments ---
    let out = run_bin(STATUS, &distributor, &[]);
    assert!(out.contains("=== OK ==="), "status verify failed:\n{out}");

    drop(guard);
    let _ = std::fs::remove_dir_all(&work);
    eprintln!("Full sequencer flow OK.");
}
