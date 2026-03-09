use super::client::{PerformanceConfigResponse, TlsConfigResponse, WhitelistResponse};
use super::keys::{format_size, ConfigKey};

pub fn print_full_config(
    tls: &TlsConfigResponse,
    perf: &PerformanceConfigResponse,
    whitelist: &WhitelistResponse,
) {
    println!("Bifrost Configuration");
    println!("=====================\n");

    print_tls_config(tls);
    println!();
    print_traffic_config(perf);
    println!();
    print_access_config(whitelist);
}

pub fn print_tls_config(tls: &TlsConfigResponse) {
    println!("TLS Configuration");
    println!("  Enabled:              {}", tls.enable_tls_interception);
    println!("  Unsafe SSL:           {}", tls.unsafe_ssl);
    println!(
        "  Disconnect on Change: {}",
        tls.disconnect_on_config_change
    );
    println!();
    println!("  Exclude Domains:");
    if tls.intercept_exclude.is_empty() {
        println!("    (none)");
    } else {
        for domain in &tls.intercept_exclude {
            println!("    - {}", domain);
        }
    }
    println!();
    println!("  Include Domains:");
    if tls.intercept_include.is_empty() {
        println!("    (none)");
    } else {
        for domain in &tls.intercept_include {
            println!("    - {}", domain);
        }
    }
    println!();
    println!("  Exclude Apps:");
    if tls.app_intercept_exclude.is_empty() {
        println!("    (none)");
    } else {
        for app in &tls.app_intercept_exclude {
            println!("    - {}", app);
        }
    }
    println!();
    println!("  Include Apps:");
    if tls.app_intercept_include.is_empty() {
        println!("    (none)");
    } else {
        for app in &tls.app_intercept_include {
            println!("    - {}", app);
        }
    }
}

pub fn print_traffic_config(perf: &PerformanceConfigResponse) {
    println!("Traffic Configuration");
    println!("  Max Records:          {}", perf.traffic.max_records);
    println!(
        "  Max DB Size:          {}",
        format_size(perf.traffic.max_db_size_bytes as usize)
    );
    println!(
        "  Max Body Size:        {}",
        format_size(perf.traffic.max_body_memory_size)
    );
    println!(
        "  Max Buffer Size:      {}",
        format_size(perf.traffic.max_body_buffer_size)
    );
    println!(
        "  Retention Days:       {}",
        perf.traffic.file_retention_days
    );
    println!(
        "  SSE Flush Bytes:      {}",
        format_size(perf.traffic.sse_stream_flush_bytes)
    );
    println!(
        "  SSE Flush Interval:   {} ms",
        perf.traffic.sse_stream_flush_interval_ms
    );
    println!(
        "  WS Flush Bytes:       {}",
        format_size(perf.traffic.ws_payload_flush_bytes)
    );
    println!(
        "  WS Flush Interval:    {} ms",
        perf.traffic.ws_payload_flush_interval_ms
    );
    println!(
        "  WS Max Open Files:    {}",
        perf.traffic.ws_payload_max_open_files
    );

    if perf.body_store_stats.is_some()
        || perf.traffic_store_stats.is_some()
        || perf.frame_store_stats.is_some()
        || perf.ws_payload_store_stats.is_some()
    {
        println!();
        println!("  Storage Stats:");
        if let Some(ref stats) = perf.body_store_stats {
            println!(
                "    Body Cache:         {} ({} files)",
                format_size(stats.total_size as usize),
                stats.file_count
            );
        }
        if let Some(ref stats) = perf.traffic_store_stats {
            println!(
                "    Traffic Records:    {} records, {} processed",
                stats.record_count, stats.total_records_processed
            );
        }
        if let Some(ref stats) = perf.frame_store_stats {
            println!(
                "    Frame Store:        {} ({} connections)",
                format_size(stats.total_size as usize),
                stats.connection_count
            );
        }
        if let Some(ref stats) = perf.ws_payload_store_stats {
            println!(
                "    WS Payload Store:   {} ({} files)",
                format_size(stats.total_size as usize),
                stats.file_count
            );
        }
    }
}

pub fn print_access_config(whitelist: &WhitelistResponse) {
    println!("Access Control");
    println!("  Mode:                 {}", whitelist.mode);
    println!("  Allow LAN:            {}", whitelist.allow_lan);
}

pub fn print_config_value(key: &ConfigKey, value: &serde_json::Value) {
    match value {
        serde_json::Value::Bool(b) => println!("{} = {}", key, b),
        serde_json::Value::Number(n) => {
            if key.is_size() {
                if let Some(bytes) = n.as_u64() {
                    println!("{} = {} ({})", key, bytes, format_size(bytes as usize));
                } else {
                    println!("{} = {}", key, n);
                }
            } else {
                println!("{} = {}", key, n);
            }
        }
        serde_json::Value::String(s) => println!("{} = {}", key, s),
        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                println!("{} = []", key);
            } else {
                println!("{} =", key);
                for item in arr {
                    if let serde_json::Value::String(s) = item {
                        println!("  - {}", s);
                    }
                }
            }
        }
        _ => println!("{} = {}", key, value),
    }
}
