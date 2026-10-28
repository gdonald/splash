use splash::correlate::{groups, ids, Group};

#[test]
fn test_uuids_and_trace_ids_are_identifiers() {
    assert_eq!(
        ids("span 4bf92f3577b34da6a3ce929d0e0e4736 of order 0f8fad5b-d9cb-469f-a165-70867728950e"),
        vec![
            "4bf92f3577b34da6a3ce929d0e0e4736",
            "0f8fad5b-d9cb-469f-a165-70867728950e"
        ]
    );
}

#[test]
fn test_the_values_of_id_fields_are_identifiers() {
    assert_eq!(
        ids(
            r#"request_id=r-1 {"traceId": "t.2"} X-Request-ID: x3 correlation-id:c4. session_id = s5 txnid=6"#
        ),
        vec!["r-1", "t.2", "x3", "c4", "s5", "6"]
    );
}

#[test]
fn test_an_identifier_found_twice_in_a_text_is_listed_once() {
    assert_eq!(
        ids("request_id=0f8fad5b-d9cb-469f-a165-70867728950e retried request_id=0f8fad5b-d9cb-469f-a165-70867728950e"),
        vec!["0f8fad5b-d9cb-469f-a165-70867728950e"]
    );
}

#[test]
fn test_fields_that_are_not_id_fields_hold_no_identifiers() {
    assert!(ids("pid=42 user_id=7 order 1001 placed").is_empty());
}

#[test]
fn test_entries_sharing_an_identifier_form_a_group_in_the_order_they_first_appear() {
    let entries = [
        "request_id=b accepted",
        "request_id=a accepted",
        "request_id=a done",
        "health check passed",
        "request_id=b done",
        "request_id=c done",
    ];

    assert_eq!(
        groups(&entries),
        vec![
            Group {
                id: "b".to_string(),
                members: vec![0, 4]
            },
            Group {
                id: "a".to_string(),
                members: vec![1, 2]
            },
        ]
    );
}

#[test]
fn test_an_entry_holding_two_identifiers_is_in_both_groups() {
    let entries = [
        "trace_id=t1 request_id=r1 accepted",
        "trace_id=t1 queued",
        "request_id=r1 done",
    ];

    assert_eq!(
        groups(&entries),
        vec![
            Group {
                id: "t1".to_string(),
                members: vec![0, 1]
            },
            Group {
                id: "r1".to_string(),
                members: vec![0, 2]
            },
        ]
    );
}
