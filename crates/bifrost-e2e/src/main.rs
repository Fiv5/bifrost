use bifrost_e2e::{Reporter, TestRunner, TestStatus};
use clap::Parser;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "bifrost-e2e")]
#[command(about = "Bifrost E2E Test Runner", long_about = None)]
struct Args {
    #[arg(short, long, help = "Filter tests by category (routing, request, response, template, public)")]
    category: Option<String>,

    #[arg(short, long, help = "Filter tests by name pattern")]
    test: Option<String>,

    #[arg(short, long, help = "List all available tests without running")]
    list: bool,

    #[arg(short, long, help = "Output JSON report to file")]
    output: Option<String>,

    #[arg(short, long, default_value = "18080", help = "Base port for proxy instances")]
    port: u16,

    #[arg(short, long, help = "Verbose output")]
    verbose: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    let reporter = Reporter::new(args.verbose);
    let mut runner = TestRunner::new(args.port, reporter);

    runner.load_all_tests();

    if let Some(ref category) = args.category {
        runner.filter_by_category(category);
    }

    if let Some(ref pattern) = args.test {
        runner.filter_by_name(pattern);
    }

    if args.list {
        println!("\nAvailable tests:\n");
        for (category, tests) in runner.list_tests() {
            println!("  [{}]", category);
            for test in tests {
                println!("    - {}", test);
            }
            println!();
        }
        return ExitCode::SUCCESS;
    }

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║           Bifrost E2E Test Runner                            ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let results = runner.run_all().await;

    let passed = results.iter().filter(|r| r.status == TestStatus::Passed).count();
    let failed = results.iter().filter(|r| r.status == TestStatus::Failed).count();
    let skipped = results.iter().filter(|r| r.status == TestStatus::Skipped).count();
    let total = results.len();

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║                      Test Summary                            ║");
    println!("╠══════════════════════════════════════════════════════════════╣");
    println!("║  Total:   {:4}                                               ║", total);
    println!("║  Passed:  {:4}  ✓                                            ║", passed);
    println!("║  Failed:  {:4}  ✗                                            ║", failed);
    println!("║  Skipped: {:4}  ○                                            ║", skipped);
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    if let Some(output_path) = args.output {
        match runner.reporter().export_json(&results, &output_path) {
            Ok(_) => println!("Report saved to: {}", output_path),
            Err(e) => eprintln!("Failed to save report: {}", e),
        }
    }

    if failed > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
