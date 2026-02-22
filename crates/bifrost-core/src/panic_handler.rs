use std::panic::{self, PanicHookInfo};
use std::sync::atomic::{AtomicBool, Ordering};

static PANIC_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn install_panic_hook() {
    if PANIC_HOOK_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    let default_hook = panic::take_hook();

    panic::set_hook(Box::new(move |panic_info: &PanicHookInfo| {
        let location = panic_info
            .location()
            .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
            .unwrap_or_else(|| "unknown location".to_string());

        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };

        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("unnamed");

        tracing::error!(
            target: "bifrost::panic",
            thread = %thread_name,
            location = %location,
            message = %message,
            "PANIC occurred in thread"
        );

        eprintln!(
            "\n[PANIC] Thread '{}' panicked at {}:\n  {}",
            thread_name, location, message
        );

        let backtrace = std::backtrace::Backtrace::capture();
        if backtrace.status() == std::backtrace::BacktraceStatus::Captured {
            tracing::error!(
                target: "bifrost::panic",
                backtrace = %backtrace,
                "Panic backtrace"
            );
            eprintln!("\nBacktrace:\n{}", backtrace);
        }

        default_hook(panic_info);
    }));

    tracing::debug!("Panic hook installed successfully");
}

pub async fn spawn_with_panic_guard<F, T>(
    name: &'static str,
    future: F,
) -> tokio::task::JoinHandle<()>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    tokio::spawn(async move {
        let result = tokio::spawn(future).await;

        if let Err(e) = result {
            if e.is_panic() {
                tracing::error!(
                    target: "bifrost::panic",
                    task = %name,
                    error = %e,
                    "Task panicked and was caught by panic guard"
                );
            } else if e.is_cancelled() {
                tracing::debug!(
                    target: "bifrost::task",
                    task = %name,
                    "Task was cancelled"
                );
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_install_panic_hook_idempotent() {
        install_panic_hook();
        install_panic_hook();
        assert!(PANIC_HOOK_INSTALLED.load(Ordering::SeqCst));
    }
}
