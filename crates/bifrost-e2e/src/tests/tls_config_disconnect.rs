use crate::runner::TestCase;

pub fn get_all_tests() -> Vec<TestCase> {
    vec![
        TestCase::standalone(
            "tls_config_disconnect_connection_registry",
            "Test: ConnectionRegistry unit tests",
            "tls_config_disconnect",
            test_connection_registry_unit,
        ),
        TestCase::standalone(
            "tls_config_disconnect_global_switch",
            "Test: global TLS switch change should disconnect affected HTTPS connections",
            "tls_config_disconnect",
            test_global_switch_disconnect,
        ),
        TestCase::standalone(
            "tls_config_disconnect_exclude_list",
            "Test: exclude list change should disconnect matching HTTPS connections",
            "tls_config_disconnect",
            test_exclude_list_disconnect,
        ),
        TestCase::standalone(
            "tls_config_disconnect_include_list",
            "Test: include list change should disconnect matching HTTPS connections",
            "tls_config_disconnect",
            test_include_list_disconnect,
        ),
    ]
}

async fn test_connection_registry_unit() -> Result<(), String> {
    use bifrost_admin::{ConnectionInfo, ConnectionRegistry};
    use tokio::sync::oneshot;

    println!("Testing ConnectionRegistry...");

    let registry = ConnectionRegistry::new(true);

    let (tx1, mut rx1) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-001".to_string(),
        "api.example.com".to_string(),
        443,
        true,
        tx1,
    ));

    let (tx2, mut rx2) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-002".to_string(),
        "api.other.com".to_string(),
        443,
        true,
        tx2,
    ));

    let (tx3, mut rx3) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-003".to_string(),
        "cdn.example.com".to_string(),
        443,
        false,
        tx3,
    ));

    assert_eq!(registry.active_count(), 3);
    println!("✓ Registered 3 connections");

    let disconnected = registry.disconnect_by_host_pattern(&["*.example.com".to_string()]);
    assert_eq!(disconnected.len(), 2);
    assert!(disconnected.contains(&"req-001".to_string()));
    assert!(disconnected.contains(&"req-003".to_string()));
    println!(
        "✓ Disconnected {} connections matching *.example.com",
        disconnected.len()
    );

    assert!(rx1.try_recv().is_ok(), "req-001 should be cancelled");
    assert!(rx2.try_recv().is_err(), "req-002 should not be cancelled");
    assert!(rx3.try_recv().is_ok(), "req-003 should be cancelled");
    println!("✓ Cancel signals verified");

    assert_eq!(registry.active_count(), 1);
    println!("✓ Only 1 connection remaining");

    registry.unregister("req-002");
    assert_eq!(registry.active_count(), 0);
    println!("✓ All connections cleaned up");

    let registry2 = ConnectionRegistry::new(false);

    let (tx4, _rx4) = oneshot::channel();
    registry2.register(ConnectionInfo::new(
        "req-004".to_string(),
        "test.local".to_string(),
        443,
        true,
        tx4,
    ));

    let disconnected2 = registry2.disconnect_affected(|_| true);
    assert!(
        disconnected2.is_empty(),
        "Should not disconnect when disabled"
    );
    println!("✓ Disconnect disabled mode works");

    println!("✓ All ConnectionRegistry tests passed!");
    Ok(())
}

async fn test_global_switch_disconnect() -> Result<(), String> {
    use bifrost_admin::{ConnectionInfo, ConnectionRegistry};
    use tokio::sync::oneshot;

    println!("Testing global TLS switch disconnect...");

    let registry = ConnectionRegistry::new(true);

    let (tx1, mut rx1) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-001".to_string(),
        "api.example.com".to_string(),
        443,
        true,
        tx1,
    ));

    let (tx2, mut rx2) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-002".to_string(),
        "api.other.com".to_string(),
        443,
        false,
        tx2,
    ));

    assert_eq!(registry.active_count(), 2);
    println!("✓ Registered 2 connections (1 intercept, 1 passthrough)");

    println!("Simulating global switch: enable_tls_interception = true -> false");

    let disconnected = registry.disconnect_all_with_mode(true);
    assert_eq!(disconnected.len(), 1);
    assert!(disconnected.contains(&"req-001".to_string()));
    println!(
        "✓ Disconnected {} intercepted connections",
        disconnected.len()
    );

    assert!(
        rx1.try_recv().is_ok(),
        "req-001 (intercept) should be cancelled"
    );
    assert!(
        rx2.try_recv().is_err(),
        "req-002 (passthrough) should not be cancelled"
    );
    println!("✓ Cancel signals verified correctly");

    assert_eq!(registry.active_count(), 1);
    println!("✓ 1 passthrough connection remaining");

    println!("Simulating global switch: enable_tls_interception = false -> true");

    let (tx3, _rx3) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-003".to_string(),
        "cdn.test.com".to_string(),
        443,
        false,
        tx3,
    ));

    let disconnected2 = registry.disconnect_all_with_mode(false);
    assert_eq!(disconnected2.len(), 2);
    println!(
        "✓ Disconnected {} passthrough connections",
        disconnected2.len()
    );

    println!("✓ Test passed: global switch disconnect works correctly");
    Ok(())
}

async fn test_exclude_list_disconnect() -> Result<(), String> {
    use bifrost_admin::{ConnectionInfo, ConnectionRegistry};
    use tokio::sync::oneshot;

    println!("Testing exclude list change disconnect...");

    let registry = ConnectionRegistry::new(true);

    let (tx1, mut rx1) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-001".to_string(),
        "api.example.com".to_string(),
        443,
        true,
        tx1,
    ));

    let (tx2, mut rx2) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-002".to_string(),
        "cdn.example.com".to_string(),
        443,
        true,
        tx2,
    ));

    let (tx3, mut rx3) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-003".to_string(),
        "api.other.net".to_string(),
        443,
        true,
        tx3,
    ));

    assert_eq!(registry.active_count(), 3);
    println!("✓ Registered 3 intercept connections");

    println!("Simulating exclude list change: added *.example.com");

    let disconnected = registry.disconnect_by_host_pattern(&["*.example.com".to_string()]);
    assert_eq!(disconnected.len(), 2);
    assert!(disconnected.contains(&"req-001".to_string()));
    assert!(disconnected.contains(&"req-002".to_string()));
    println!(
        "✓ Disconnected {} connections matching *.example.com",
        disconnected.len()
    );

    assert!(rx1.try_recv().is_ok(), "req-001 should be cancelled");
    assert!(rx2.try_recv().is_ok(), "req-002 should be cancelled");
    assert!(rx3.try_recv().is_err(), "req-003 should not be cancelled");
    println!("✓ Cancel signals verified correctly");

    assert_eq!(registry.active_count(), 1);
    println!("✓ 1 non-matching connection remaining");

    println!("✓ Test passed: exclude list disconnect works correctly");
    Ok(())
}

async fn test_include_list_disconnect() -> Result<(), String> {
    use bifrost_admin::{ConnectionInfo, ConnectionRegistry};
    use tokio::sync::oneshot;

    println!("Testing include list (force intercept) change disconnect...");

    let registry = ConnectionRegistry::new(true);

    let (tx1, mut rx1) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-001".to_string(),
        "api.force.local".to_string(),
        443,
        false,
        tx1,
    ));

    let (tx2, mut rx2) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-002".to_string(),
        "cdn.force.local".to_string(),
        443,
        false,
        tx2,
    ));

    let (tx3, mut rx3) = oneshot::channel();
    registry.register(ConnectionInfo::new(
        "req-003".to_string(),
        "api.other.net".to_string(),
        443,
        false,
        tx3,
    ));

    assert_eq!(registry.active_count(), 3);
    println!("✓ Registered 3 passthrough connections");

    println!("Simulating include list change: added *.force.local (force intercept)");

    let disconnected = registry.disconnect_by_host_pattern(&["*.force.local".to_string()]);
    assert_eq!(disconnected.len(), 2);
    assert!(disconnected.contains(&"req-001".to_string()));
    assert!(disconnected.contains(&"req-002".to_string()));
    println!(
        "✓ Disconnected {} connections matching *.force.local",
        disconnected.len()
    );

    assert!(rx1.try_recv().is_ok(), "req-001 should be cancelled");
    assert!(rx2.try_recv().is_ok(), "req-002 should be cancelled");
    assert!(rx3.try_recv().is_err(), "req-003 should not be cancelled");
    println!("✓ Cancel signals verified correctly");

    assert_eq!(registry.active_count(), 1);
    println!("✓ 1 non-matching connection remaining");

    println!("✓ Test passed: include list disconnect works correctly");
    Ok(())
}
