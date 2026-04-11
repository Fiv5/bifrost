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
}

