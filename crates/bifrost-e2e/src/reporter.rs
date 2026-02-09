use crate::runner::{TestResult, TestStatus};
use colored::*;
use serde::Serialize;
use std::fs::File;
use std::io::Write;
use std::time::Instant;

pub struct Reporter {
    #[allow(dead_code)]
    verbose: bool,
    start_time: Option<Instant>,
}

impl Reporter {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            start_time: None,
        }
    }

    pub fn start(&mut self, total: usize) {
        self.start_time = Some(Instant::now());
        println!();
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════════".bright_blue()
        );
        println!(
            "{}",
            "                    Bifrost E2E Test Suite                     "
                .bright_blue()
                .bold()
        );
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════════".bright_blue()
        );
        println!();
        println!("  Running {} tests...", total.to_string().cyan());
        println!();
    }

    pub fn report_test(&self, result: &TestResult, current: usize, total: usize) {
        let status_str = match result.status {
            TestStatus::Passed => "✓ PASS".green(),
            TestStatus::Failed => "✗ FAIL".red(),
            TestStatus::Skipped => "○ SKIP".yellow(),
        };

        let duration_ms = result.duration.as_millis();
        let duration_str = format!("{}ms", duration_ms).dimmed();

        println!(
            "  [{}/{}] {} {} {}",
            current.to_string().dimmed(),
            total.to_string().dimmed(),
            status_str,
            result.name.white(),
            duration_str
        );

        if result.status == TestStatus::Failed {
            if let Some(ref error) = result.error {
                println!("          {}", error.red());
            }
        }
    }

    pub fn summary(&self, results: &[TestResult]) {
        let passed = results
            .iter()
            .filter(|r| r.status == TestStatus::Passed)
            .count();
        let failed = results
            .iter()
            .filter(|r| r.status == TestStatus::Failed)
            .count();
        let skipped = results
            .iter()
            .filter(|r| r.status == TestStatus::Skipped)
            .count();
        let total = results.len();

        let total_duration = self.start_time.map(|s| s.elapsed()).unwrap_or_default();

        println!();
        println!(
            "{}",
            "───────────────────────────────────────────────────────────────".dimmed()
        );
        println!();
        println!("  {}", "Test Summary".bold());
        println!();
        println!("    {} {} passed", "✓".green(), passed.to_string().green());

        if failed > 0 {
            println!("    {} {} failed", "✗".red(), failed.to_string().red());
        }

        if skipped > 0 {
            println!(
                "    {} {} skipped",
                "○".yellow(),
                skipped.to_string().yellow()
            );
        }

        println!();
        println!(
            "  Total: {} | Duration: {:.2}s",
            total.to_string().cyan(),
            total_duration.as_secs_f64()
        );

        println!();

        if failed == 0 {
            println!("  {}", "All tests passed! 🎉".green().bold());
        } else {
            println!("  {}", format!("{} test(s) failed", failed).red().bold());

            println!();
            println!("  {}", "Failed tests:".red());
            for result in results.iter().filter(|r| r.status == TestStatus::Failed) {
                println!("    - {}", result.name.red());
                if let Some(ref error) = result.error {
                    println!("      {}", error.dimmed());
                }
            }
        }

        println!();
        println!(
            "{}",
            "═══════════════════════════════════════════════════════════════".bright_blue()
        );
        println!();
    }

    pub fn export_json(&self, results: &[TestResult], path: &str) -> std::io::Result<()> {
        #[derive(Serialize)]
        struct JsonResult {
            name: String,
            category: String,
            status: String,
            duration_ms: u128,
            error: Option<String>,
        }

        #[derive(Serialize)]
        struct JsonReport {
            total: usize,
            passed: usize,
            failed: usize,
            skipped: usize,
            results: Vec<JsonResult>,
        }

        let json_results: Vec<JsonResult> = results
            .iter()
            .map(|r| JsonResult {
                name: r.name.clone(),
                category: r.category.clone(),
                status: match r.status {
                    TestStatus::Passed => "passed".to_string(),
                    TestStatus::Failed => "failed".to_string(),
                    TestStatus::Skipped => "skipped".to_string(),
                },
                duration_ms: r.duration.as_millis(),
                error: r.error.clone(),
            })
            .collect();

        let report = JsonReport {
            total: results.len(),
            passed: results
                .iter()
                .filter(|r| r.status == TestStatus::Passed)
                .count(),
            failed: results
                .iter()
                .filter(|r| r.status == TestStatus::Failed)
                .count(),
            skipped: results
                .iter()
                .filter(|r| r.status == TestStatus::Skipped)
                .count(),
            results: json_results,
        };

        let json = serde_json::to_string_pretty(&report).unwrap();
        let mut file = File::create(path)?;
        file.write_all(json.as_bytes())?;

        println!("  Report saved to: {}", path.cyan());

        Ok(())
    }
}

impl Default for Reporter {
    fn default() -> Self {
        Self::new(false)
    }
}
