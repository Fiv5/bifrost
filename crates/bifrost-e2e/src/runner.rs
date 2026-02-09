use crate::client::ProxyClient;
use crate::proxy::ProxyInstance;
use crate::reporter::Reporter;
use crate::tests;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::time::{Duration, Instant};

pub type TestFn = Box<
    dyn Fn(ProxyClient) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync,
>;

#[derive(Debug, Clone, PartialEq)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub category: String,
    pub status: TestStatus,
    pub duration: Duration,
    pub error: Option<String>,
}

pub struct TestCase {
    pub name: String,
    pub category: String,
    pub rules: Vec<String>,
    pub test_fn: TestFn,
}

impl TestCase {
    pub fn new<F, Fut>(name: &str, category: &str, rules: Vec<&str>, test_fn: F) -> Self
    where
        F: Fn(ProxyClient) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        Self {
            name: name.to_string(),
            category: category.to_string(),
            rules: rules.iter().map(|s| s.to_string()).collect(),
            test_fn: Box::new(move |client| Box::pin(test_fn(client))),
        }
    }
}

pub struct TestRunner {
    tests: Vec<TestCase>,
    base_port: u16,
    reporter: Reporter,
}

impl TestRunner {
    pub fn new(base_port: u16, reporter: Reporter) -> Self {
        Self {
            tests: Vec::new(),
            base_port,
            reporter,
        }
    }

    pub fn load_all_tests(&mut self) {
        self.tests = tests::all_tests();
    }

    pub fn add_test(&mut self, test: TestCase) {
        self.tests.push(test);
    }

    pub fn add_tests(&mut self, tests: Vec<TestCase>) {
        self.tests.extend(tests);
    }

    pub fn filter_by_category(&mut self, category: &str) {
        self.tests.retain(|t| t.category == category);
    }

    pub fn filter_by_name(&mut self, pattern: &str) {
        self.tests.retain(|t| t.name.contains(pattern));
    }

    pub fn list_tests(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for test in &self.tests {
            map.entry(test.category.clone())
                .or_default()
                .push(test.name.clone());
        }
        map
    }

    pub fn reporter(&self) -> &Reporter {
        &self.reporter
    }

    pub async fn run_all(&mut self) -> Vec<TestResult> {
        let mut results = Vec::new();
        let total = self.tests.len();

        self.reporter.start(total);

        for (i, test) in self.tests.iter().enumerate() {
            let port = self.base_port + (i as u16);
            let result = self.run_single_test(test, port).await;
            self.reporter.report_test(&result, i + 1, total);
            results.push(result);
        }

        self.reporter.summary(&results);
        results
    }

    async fn run_single_test(&self, test: &TestCase, port: u16) -> TestResult {
        let start = Instant::now();

        let rules: Vec<&str> = test.rules.iter().map(|s| s.as_str()).collect();

        let proxy = match ProxyInstance::start(port, rules).await {
            Ok(p) => p,
            Err(e) => {
                return TestResult {
                    name: test.name.clone(),
                    category: test.category.clone(),
                    status: TestStatus::Failed,
                    duration: start.elapsed(),
                    error: Some(format!("Failed to start proxy: {}", e)),
                };
            }
        };

        let client = match ProxyClient::new(&proxy.proxy_url()) {
            Ok(c) => c,
            Err(e) => {
                return TestResult {
                    name: test.name.clone(),
                    category: test.category.clone(),
                    status: TestStatus::Failed,
                    duration: start.elapsed(),
                    error: Some(format!("Failed to create client: {}", e)),
                };
            }
        };

        let result = (test.test_fn)(client).await;

        let duration = start.elapsed();

        TestResult {
            name: test.name.clone(),
            category: test.category.clone(),
            status: if result.is_ok() {
                TestStatus::Passed
            } else {
                TestStatus::Failed
            },
            duration,
            error: result.err(),
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new(18800, Reporter::new(false))
    }
}
