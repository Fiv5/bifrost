use crate::RuntimeConfig;

pub struct TlsStatusInfo {
    pub enable_tls_interception: bool,
    pub intercept_exclude: Vec<String>,
    pub intercept_include: Vec<String>,
    pub app_intercept_exclude: Vec<String>,
    pub app_intercept_include: Vec<String>,
    pub unsafe_ssl: bool,
    pub disconnect_on_config_change: bool,
    pub active_connections: usize,
}

impl TlsStatusInfo {
    pub fn from_runtime_config(config: &RuntimeConfig, active_connections: usize) -> Self {
        Self {
            enable_tls_interception: config.enable_tls_interception,
            intercept_exclude: config.intercept_exclude.clone(),
            intercept_include: config.intercept_include.clone(),
            app_intercept_exclude: config.app_intercept_exclude.clone(),
            app_intercept_include: config.app_intercept_include.clone(),
            unsafe_ssl: config.unsafe_ssl,
            disconnect_on_config_change: config.disconnect_on_config_change,
            active_connections,
        }
    }

    pub fn print_status(&self) {
        println!("🔒 TLS/HTTPS INTERCEPTION");
        if self.enable_tls_interception {
            println!("   Status:        ✓ enabled");
            if !self.intercept_exclude.is_empty() {
                println!("   Excluded:      {:?}", self.intercept_exclude);
            }
            if !self.intercept_include.is_empty() {
                println!("   Included:      {:?}", self.intercept_include);
            }
            if !self.app_intercept_exclude.is_empty() {
                println!("   App Excluded:  {:?}", self.app_intercept_exclude);
            }
            if !self.app_intercept_include.is_empty() {
                println!("   App Included:  {:?}", self.app_intercept_include);
            }
            if self.unsafe_ssl {
                println!("   ⚠️  Upstream TLS verification: DISABLED (--unsafe-ssl)");
            }
        } else {
            println!("   Status:        ✗ disabled");
        }
        println!("   Active conns:  {}", self.active_connections);
    }

    pub fn print_update_banner(&self) {
        println!();
        println!("════════════════════════════════════════════════════════════════════════");
        println!("                    🔄 TLS CONFIG UPDATED");
        println!("════════════════════════════════════════════════════════════════════════");
        println!(
            "   HTTPS Interception: {}",
            if self.enable_tls_interception {
                "✓ ENABLED"
            } else {
                "✗ DISABLED"
            }
        );
        if !self.intercept_exclude.is_empty() {
            println!("   Exclude patterns:   {:?}", self.intercept_exclude);
        }
        if !self.intercept_include.is_empty() {
            println!("   Include patterns:   {:?}", self.intercept_include);
        }
        if !self.app_intercept_exclude.is_empty() {
            println!("   App exclude:        {:?}", self.app_intercept_exclude);
        }
        if !self.app_intercept_include.is_empty() {
            println!("   App include:        {:?}", self.app_intercept_include);
        }
        println!("   Active connections: {}", self.active_connections);
        println!("════════════════════════════════════════════════════════════════════════");
        println!();
    }

    pub fn log_update_banner(&self) {
        tracing::info!("════════════════════════════════════════════════════════════════════════");
        tracing::info!("                    🔄 TLS CONFIG UPDATED");
        tracing::info!("════════════════════════════════════════════════════════════════════════");
        tracing::info!(
            "   HTTPS Interception: {}",
            if self.enable_tls_interception {
                "✓ ENABLED"
            } else {
                "✗ DISABLED"
            }
        );
        if !self.intercept_exclude.is_empty() {
            tracing::info!("   Exclude patterns:   {:?}", self.intercept_exclude);
        }
        if !self.intercept_include.is_empty() {
            tracing::info!("   Include patterns:   {:?}", self.intercept_include);
        }
        if !self.app_intercept_exclude.is_empty() {
            tracing::info!("   App exclude:        {:?}", self.app_intercept_exclude);
        }
        if !self.app_intercept_include.is_empty() {
            tracing::info!("   App include:        {:?}", self.app_intercept_include);
        }
        tracing::info!("   Active connections: {}", self.active_connections);
        tracing::info!("════════════════════════════════════════════════════════════════════════");
    }
}
