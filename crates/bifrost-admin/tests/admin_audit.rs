use bifrost_admin::admin_audit;

#[test]
fn test_admin_audit_record_and_list_and_count_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    bifrost_storage::set_data_dir(tmp.path().join("bifrost-data"));

    admin_audit::record_login("admin", "127.0.0.1", "ua-1").expect("record login 1");
    admin_audit::record_login("admin", "127.0.0.1", "ua-2").expect("record login 2");

    let total = admin_audit::count_logins().expect("count logins");
    assert_eq!(total, 2);

    let items = admin_audit::list_logins(10, 0).expect("list logins");
    assert_eq!(items.len(), 2);
    assert!(items[0].id > items[1].id, "should be ordered by id desc");
    assert_eq!(items[0].username, "admin");
    assert_eq!(items[0].ip, "127.0.0.1");
    assert!(!items[0].ua.is_empty());

    let db_path = admin_audit::audit_db_path().expect("db path");
    assert!(db_path.exists(), "audit db file should exist");

    let before = chrono::Utc::now().timestamp();
    admin_audit::record_login("operator", "192.168.8.31", "Mozilla/5.0 Chrome")
        .expect("record login with remote ip");
    let after = chrono::Utc::now().timestamp();

    let recent = admin_audit::list_logins(1, 0).expect("list recent");
    assert_eq!(recent.len(), 1);
    let entry = &recent[0];
    assert_eq!(entry.username, "operator");
    assert_eq!(entry.ip, "192.168.8.31");
    assert_eq!(entry.ua, "Mozilla/5.0 Chrome");
    assert!(entry.ts >= before, "ts should be >= before");
    assert!(entry.ts <= after, "ts should be <= after");

    for i in 0..5 {
        admin_audit::record_login("admin", &format!("10.0.0.{i}"), &format!("agent-{i}"))
            .expect("record login for pagination");
    }

    let total = admin_audit::count_logins().expect("count after batch");
    assert_eq!(total, 8);

    let page1 = admin_audit::list_logins(3, 0).expect("page1");
    assert_eq!(page1.len(), 3);

    let page2 = admin_audit::list_logins(3, 3).expect("page2");
    assert_eq!(page2.len(), 3);

    assert!(page1[0].id > page1[2].id, "page1 should be desc");
    assert!(page1[2].id > page2[0].id, "page2 should follow page1");
}
