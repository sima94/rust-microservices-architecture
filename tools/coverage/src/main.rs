use clap::Parser;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

// ── CLI ──────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "coverage-tool", about = "Run code coverage for Rust microservices")]
struct Cli {
    /// Run only a specific service (auth, user). Default: all services.
    #[arg(short, long)]
    service: Option<String>,

    /// Generate HTML report (opens in browser)
    #[arg(long)]
    html: bool,

    /// Generate LCOV output for CI pipelines
    #[arg(long)]
    lcov: bool,

    /// Minimum coverage percentage required (exit 1 if below)
    #[arg(short, long)]
    threshold: Option<f64>,

    /// Project root directory
    #[arg(long)]
    root: Option<PathBuf>,
}

// ── cargo-llvm-cov JSON output types ─────────────────────────────────

#[derive(Deserialize)]
struct CoverageReport {
    data: Vec<CoverageData>,
}

#[derive(Deserialize)]
struct CoverageData {
    totals: CoverageTotals,
}

#[derive(Deserialize)]
struct CoverageTotals {
    lines: CoverageMetric,
    functions: CoverageMetric,
    regions: CoverageMetric,
}

#[derive(Deserialize)]
struct CoverageMetric {
    count: u64,
    covered: u64,
    percent: f64,
}

// ── Service config ───────────────────────────────────────────────────

struct ServiceInfo {
    name: &'static str,
    dir: &'static str,
}

const SERVICES: &[ServiceInfo] = &[
    ServiceInfo { name: "auth-service", dir: "auth-service" },
    ServiceInfo { name: "user-service", dir: "user-service" },
];

// ── Main ─────────────────────────────────────────────────────────────

fn main() -> ExitCode {
    let cli = Cli::parse();

    let project_root = cli.root.clone().unwrap_or_else(|| {
        // Walk up from CARGO_MANIFEST_DIR to find project root (has docker-compose.yml)
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        while dir.parent().is_some() {
            if dir.join("docker-compose.yml").exists() {
                return dir;
            }
            dir = dir.parent().unwrap().to_path_buf();
        }
        panic!("Cannot find project root (no docker-compose.yml found)");
    });

    // Check cargo-llvm-cov is installed
    if !check_tool_installed() {
        eprintln!("ERROR: cargo-llvm-cov is not installed.");
        eprintln!("  Install: cargo install cargo-llvm-cov");
        eprintln!("  Also:    rustup component add llvm-tools-preview");
        return ExitCode::FAILURE;
    }

    // Select services
    let services: Vec<&ServiceInfo> = if let Some(ref name) = cli.service {
        SERVICES
            .iter()
            .filter(|s| s.name.starts_with(name) || s.dir.starts_with(name))
            .collect()
    } else {
        SERVICES.iter().collect()
    };

    if services.is_empty() {
        eprintln!("ERROR: No matching service found for '{}'", cli.service.unwrap());
        eprintln!("  Available: auth-service, user-service");
        return ExitCode::FAILURE;
    }

    println!("╔══════════════════════════════════════════╗");
    println!("║       Code Coverage Report               ║");
    println!("╚══════════════════════════════════════════╝\n");

    let mut all_passed = true;
    let mut results: Vec<(&str, Option<CoverageTotals>)> = Vec::new();

    for svc in &services {
        println!("── {} ──────────────────────────────", svc.name);
        let svc_dir = project_root.join(svc.dir);

        if !svc_dir.exists() {
            eprintln!("  SKIP: directory not found: {}", svc_dir.display());
            results.push((svc.name, None));
            continue;
        }

        // Run cargo llvm-cov --json
        let totals = run_coverage(&svc_dir, &cli);

        match &totals {
            Some(t) => {
                print_totals(svc.name, t);

                // Threshold check
                if let Some(min) = cli.threshold {
                    if t.lines.percent < min {
                        println!(
                            "  ✗ FAIL: line coverage {:.1}% < threshold {:.1}%",
                            t.lines.percent, min
                        );
                        all_passed = false;
                    } else {
                        println!(
                            "  ✓ PASS: line coverage {:.1}% >= threshold {:.1}%",
                            t.lines.percent, min
                        );
                    }
                }
            }
            None => {
                eprintln!("  ERROR: coverage run failed");
                all_passed = false;
            }
        }

        results.push((svc.name, totals));
        println!();
    }

    // Summary table
    print_summary(&results, cli.threshold);

    // Generate HTML if requested
    if cli.html {
        println!("\nGenerating HTML reports...");
        for svc in &services {
            generate_html(&project_root.join(svc.dir));
        }
    }

    // Generate LCOV if requested
    if cli.lcov {
        println!("\nGenerating LCOV reports...");
        for svc in &services {
            generate_lcov(&project_root.join(svc.dir), svc.name);
        }
    }

    if all_passed { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

// ── Tool check ───────────────────────────────────────────────────────

fn check_tool_installed() -> bool {
    Command::new("cargo")
        .args(["llvm-cov", "--version"])
        .output()
        .is_ok_and(|o| o.status.success())
}

// ── Run coverage ─────────────────────────────────────────────────────

fn run_coverage(svc_dir: &Path, _cli: &Cli) -> Option<CoverageTotals> {
    println!("  Running tests with coverage instrumentation...");

    let mut cmd = Command::new("cargo");
    cmd.args(["llvm-cov", "--json"]);
    cmd.current_dir(svc_dir);

    // Suppress test output noise
    cmd.env("CARGO_TERM_QUIET", "true");

    let output = cmd.output().ok()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("  cargo llvm-cov failed:\n{stderr}");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // cargo llvm-cov --json outputs multiple JSON objects, we need the coverage one
    // It outputs test results first, then the coverage JSON
    // Find the last valid JSON object that has "data" field
    let report: Option<CoverageReport> = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<CoverageReport>(line).ok());

    // If line-by-line didn't work, try parsing the whole stdout
    let report = report.or_else(|| serde_json::from_str::<CoverageReport>(&stdout).ok());

    report.and_then(|r| r.data.into_iter().next().map(|d| d.totals))
}

// ── Print helpers ────────────────────────────────────────────────────

fn print_totals(_name: &str, t: &CoverageTotals) {
    println!("  Lines:     {:>5}/{:<5} ({:.1}%) {}", t.lines.covered, t.lines.count, t.lines.percent, bar(t.lines.percent));
    println!("  Functions: {:>5}/{:<5} ({:.1}%) {}", t.functions.covered, t.functions.count, t.functions.percent, bar(t.functions.percent));
    println!("  Regions:   {:>5}/{:<5} ({:.1}%) {}", t.regions.covered, t.regions.count, t.regions.percent, bar(t.regions.percent));
}

fn bar(percent: f64) -> String {
    let filled = (percent / 5.0).round() as usize;
    let empty = 20_usize.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn print_summary(results: &[(&str, Option<CoverageTotals>)], threshold: Option<f64>) {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║  Service          │ Lines    │ Functions │ Regions          ║");
    println!("╠══════════════════════════════════════════════════════════════╣");

    for (name, totals) in results {
        match totals {
            Some(t) => {
                let status = threshold
                    .map(|min| if t.lines.percent >= min { "✓" } else { "✗" })
                    .unwrap_or(" ");
                println!(
                    "║ {status} {:<15} │ {:>5.1}%   │ {:>5.1}%     │ {:>5.1}%            ║",
                    name, t.lines.percent, t.functions.percent, t.regions.percent
                );
            }
            None => {
                println!("║   {:<15} │  ERROR   │  ERROR    │  ERROR           ║", name);
            }
        }
    }

    println!("╚══════════════════════════════════════════════════════════════╝");

    if let Some(min) = threshold {
        println!("  Threshold: {min:.1}%");
    }
}

// ── HTML report ──────────────────────────────────────────────────────

fn generate_html(svc_dir: &Path) {
    let name = svc_dir.file_name().unwrap().to_str().unwrap();
    println!("  {name}: generating HTML...");

    let status = Command::new("cargo")
        .args(["llvm-cov", "--html"])
        .current_dir(svc_dir)
        .status();

    match status {
        Ok(s) if s.success() => {
            let html_path = svc_dir.join("target/llvm-cov/html/index.html");
            println!("  {name}: {}", html_path.display());

            // Open in browser on macOS
            Command::new("open").arg(&html_path).status().ok();
        }
        _ => eprintln!("  {name}: HTML generation failed"),
    }
}

// ── LCOV report (for CI) ─────────────────────────────────────────────

fn generate_lcov(svc_dir: &Path, name: &str) {
    let lcov_path = svc_dir.join(format!("{name}.lcov"));
    println!("  {name}: generating LCOV → {}", lcov_path.display());

    let status = Command::new("cargo")
        .args([
            "llvm-cov",
            "--lcov",
            "--output-path",
            lcov_path.to_str().unwrap(),
        ])
        .current_dir(svc_dir)
        .status();

    match status {
        Ok(s) if s.success() => println!("  {name}: LCOV saved"),
        _ => eprintln!("  {name}: LCOV generation failed"),
    }
}
