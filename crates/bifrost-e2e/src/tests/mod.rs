pub mod curl_mock;
pub mod https;
pub mod matchers;
pub mod protocols;
pub mod public;
pub mod request;
pub mod response;
pub mod routing;
pub mod template;

use crate::runner::TestCase;

pub fn all_tests() -> Vec<TestCase> {
    let mut tests = Vec::new();
    tests.extend(routing::tests());
    tests.extend(request::tests());
    tests.extend(response::tests());
    tests.extend(template::tests());
    tests.extend(public::tests());
    tests.extend(curl_mock::get_all_tests());
    tests.extend(https::get_all_tests());
    tests.extend(protocols::get_all_tests());
    tests.extend(matchers::get_all_tests());
    tests
}
