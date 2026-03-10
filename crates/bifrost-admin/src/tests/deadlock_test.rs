#[cfg(test)]
mod deadlock_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::time::timeout;

    use crate::body_store::BodyStore;
    use crate::connection_monitor::{ConnectionMonitor, WebSocketFrameRecord};
    use crate::frame_store::FrameStore;
    use crate::traffic::{FrameDirection, FrameType, TrafficRecord};
    use crate::traffic_store::TrafficStore;
    use parking_lot::RwLock;

    fn create_temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bifrost_deadlock_test_{}_{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_temp_dir(dir: &std::path::PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_traffic_store_concurrent_access() {
        let dir = create_temp_dir("traffic_store");
        let store = Arc::new(TrafficStore::new(dir.clone(), 1000, None));

        let mut handles = vec![];

        for i in 0..10 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                for j in 0..50 {
                    let record = TrafficRecord::new(
                        format!("REQ-{}-{}", i, j),
                        "GET".to_string(),
                        format!("http://test{}.com/path{}", i, j),
                    );
                    store_clone.record(record);
                }
            });
            handles.push(handle);
        }

        for i in 0..5 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..20 {
                    let _ = store_clone.filter(&Default::default());
                    let _ = store_clone.get_by_id(&format!("REQ-{}-0", i));
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(10), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Traffic store operations should not deadlock"
        );
        cleanup_temp_dir(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_frame_store_concurrent_access() {
        let dir = create_temp_dir("frame_store");
        let store = Arc::new(FrameStore::new(dir.clone(), None));

        let mut handles = vec![];

        for i in 0..10 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                for j in 0..20 {
                    let frame = WebSocketFrameRecord {
                        frame_id: j as u64,
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        direction: FrameDirection::Send,
                        frame_type: FrameType::Text,
                        payload_size: 100,
                        payload_is_text: true,
                        payload_preview: Some(format!("test message {} {}", i, j)),
                        payload_ref: None,
                        raw_payload_size: None,
                        raw_payload_is_text: None,
                        raw_payload_preview: None,
                        raw_payload_ref: None,
                        is_fin: true,
                        is_masked: false,
                    };
                    let _ = store_clone.append_frame(&format!("CONN-{}", i), &frame);
                }
            });
            handles.push(handle);
        }

        for i in 0..5 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..10 {
                    let _ = store_clone.load_frames(&format!("CONN-{}", i), None, 100);
                    let _ = store_clone.get_last_frame_id(&format!("CONN-{}", i));
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(10), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(result.is_ok(), "Frame store operations should not deadlock");
        cleanup_temp_dir(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_body_store_concurrent_access() {
        let dir = create_temp_dir("body_store");
        let store = Arc::new(RwLock::new(BodyStore::new(
            dir.clone(),
            100,
            7,
            64 * 1024,
            Duration::from_millis(200),
        )));

        let mut handles = vec![];

        for i in 0..10 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                for j in 0..20 {
                    let data = format!(
                        "test body content {} {} with some padding to make it longer",
                        i, j
                    );
                    let body_ref = {
                        let s = store_clone.read();
                        s.store(&format!("REQ-{}-{}", i, j), "req", data.as_bytes())
                    };
                    if let Some(ref br) = body_ref {
                        let s = store_clone.read();
                        let _ = s.load(br);
                    }
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(10), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(result.is_ok(), "Body store operations should not deadlock");
        cleanup_temp_dir(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_connection_monitor_concurrent_access() {
        let monitor = Arc::new(ConnectionMonitor::new());

        let mut handles = vec![];

        for i in 0..10 {
            let monitor_clone = monitor.clone();
            let handle = tokio::spawn(async move {
                let conn_id = format!("CONN-{}", i);
                monitor_clone.register_connection(&conn_id);

                for j in 0..20 {
                    let payload = format!("test {}", j);
                    monitor_clone.record_frame(
                        &conn_id,
                        FrameDirection::Send,
                        FrameType::Text,
                        payload.as_bytes(),
                        true,
                        None,
                        false,
                        true,
                        None,
                        None,
                        None,
                    );
                }

                monitor_clone.set_connection_closed(&conn_id, None, None, None, None);
            });
            handles.push(handle);
        }

        for i in 0..5 {
            let monitor_clone = monitor.clone();
            let handle = tokio::spawn(async move {
                for _ in 0..20 {
                    let _ = monitor_clone.get_frames(&format!("CONN-{}", i), None, 100);
                    let _ = monitor_clone.get_status(&format!("CONN-{}", i));
                    let _ = monitor_clone.get_connection_status(&format!("CONN-{}", i));
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(10), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "WebSocket monitor operations should not deadlock"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_spawn_blocking_with_file_io() {
        let dir = create_temp_dir("spawn_blocking");
        let store = Arc::new(RwLock::new(BodyStore::new(
            dir.clone(),
            10,
            7,
            64 * 1024,
            Duration::from_millis(200),
        )));

        let mut handles = vec![];

        for i in 0..20 {
            let store_clone = store.clone();
            let handle = tokio::spawn(async move {
                let data = format!(
                    "This is a large body content for testing {} with lots of padding to exceed the memory limit and force file storage",
                    i
                );

                let body_ref = tokio::task::spawn_blocking({
                    let store = store_clone.clone();
                    let data = data.clone();
                    move || {
                        let s = store.read();
                        s.store(&format!("REQ-{}", i), "req", data.as_bytes())
                    }
                })
                .await
                .unwrap();

                if let Some(ref br) = body_ref {
                    let loaded = tokio::task::spawn_blocking({
                        let store = store_clone.clone();
                        let br = br.clone();
                        move || {
                            let s = store.read();
                            s.load(&br)
                        }
                    })
                    .await
                    .unwrap();

                    assert!(loaded.is_some(), "Should be able to load stored body");
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(15), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "spawn_blocking file I/O should not deadlock"
        );
        cleanup_temp_dir(&dir);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_mixed_concurrent_operations() {
        let traffic_dir = create_temp_dir("traffic_mixed");
        let frame_dir = create_temp_dir("frame_mixed");
        let body_dir = create_temp_dir("body_mixed");

        let traffic_store = Arc::new(TrafficStore::new(traffic_dir.clone(), 1000, None));
        let frame_store = Arc::new(FrameStore::new(frame_dir.clone(), None));
        let body_store = Arc::new(RwLock::new(BodyStore::new(
            body_dir.clone(),
            100,
            7,
            64 * 1024,
            Duration::from_millis(200),
        )));
        let monitor = Arc::new(ConnectionMonitor::new());

        let mut handles = vec![];

        for i in 0..5 {
            let ts = traffic_store.clone();
            let fs = frame_store.clone();
            let bs = body_store.clone();
            let mon = monitor.clone();

            let handle = tokio::spawn(async move {
                let conn_id = format!("CONN-{}", i);
                mon.register_connection(&conn_id);

                for j in 0..10 {
                    let record = TrafficRecord::new(
                        format!("REQ-{}-{}", i, j),
                        "GET".to_string(),
                        format!("http://test{}.com/path{}", i, j),
                    );
                    ts.record(record);

                    let payload = format!("msg {}", j);
                    mon.record_frame(
                        &conn_id,
                        FrameDirection::Send,
                        FrameType::Text,
                        payload.as_bytes(),
                        true,
                        None,
                        false,
                        true,
                        None,
                        None,
                        None,
                    );

                    let frame = WebSocketFrameRecord {
                        frame_id: j as u64,
                        timestamp: chrono::Utc::now().timestamp_millis() as u64,
                        direction: FrameDirection::Send,
                        frame_type: FrameType::Text,
                        payload_size: 10,
                        payload_is_text: true,
                        payload_preview: Some(format!("msg {}", j)),
                        payload_ref: None,
                        raw_payload_size: None,
                        raw_payload_is_text: None,
                        raw_payload_preview: None,
                        raw_payload_ref: None,
                        is_fin: true,
                        is_masked: false,
                    };
                    let _ = fs.append_frame(&conn_id, &frame);

                    let body_data = format!("body {} {}", i, j);
                    let _ = {
                        let s = bs.read();
                        s.store(&format!("REQ-{}-{}", i, j), "req", body_data.as_bytes())
                    };
                }

                mon.set_connection_closed(&conn_id, None, None, None, None);
            });
            handles.push(handle);
        }

        for i in 0..3 {
            let ts = traffic_store.clone();
            let fs = frame_store.clone();
            let bs = body_store.clone();
            let mon = monitor.clone();

            let handle = tokio::spawn(async move {
                for _ in 0..15 {
                    let _ = ts.filter(&Default::default());
                    let _ = ts.get_by_id(&format!("REQ-{}-0", i));
                    let _ = fs.load_frames(&format!("CONN-{}", i), None, 100);
                    let _ = mon.get_frames(&format!("CONN-{}", i), None, 100);
                    let _ = {
                        let s = bs.read();
                        s.stats()
                    };
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            });
            handles.push(handle);
        }

        let result = timeout(Duration::from_secs(15), async {
            for handle in handles {
                handle.await.unwrap();
            }
        })
        .await;

        assert!(
            result.is_ok(),
            "Mixed concurrent operations should not deadlock"
        );

        cleanup_temp_dir(&traffic_dir);
        cleanup_temp_dir(&frame_dir);
        cleanup_temp_dir(&body_dir);
    }
}
