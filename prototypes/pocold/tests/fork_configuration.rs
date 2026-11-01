use pocold::configured_forks;
use serde_json::json;

#[test]
fn trusted_local_schedule_is_explicit_and_fails_closed() {
    assert_eq!(configured_forks(&json!({})).unwrap(), vec![(1, 0)]);
    for value in [json!(null), json!(1), json!([]), json!("metadata")] {
        assert!(configured_forks(&value).is_err());
    }
    assert_eq!(configured_forks(&json!({"forks": [[1, 0], [2, 123456]]})).unwrap(), vec![(1, 0), (2, 123456)]);
    for value in [json!(null), json!([]), json!([[2, 0]]), json!([[1, 0], [2, 0]]),
        json!([[1, 0], [3, 20]]), json!([[1, 0], [2, 20], [1, 30]]),
        json!([[1, 0], [2, "20"]]), json!([[1, 0], [2, -1]]),
        json!([[1, 0], [1, 10]]), json!([[1, 0], [1, 10], [2, 20]])] {
        assert!(configured_forks(&json!({"forks": value})).is_err());
    }
}
