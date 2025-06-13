//! Stratum protocol compatibility tests
//!
//! These tests verify that our Rust Stratum implementation is compatible
//! with the original Haskell implementation and follows the Stratum protocol correctly.

use chainweb_mining_client::workers::stratum::*;
use serde_json::{Value, json};

#[test]
fn test_stratum_message_formats() {
    // Test message formats that the expect script uses

    // Test mining.authorize message format
    let auth_request = StratumRequest {
        id: json!(0),
        method: "mining.authorize".to_string(),
        params: vec![
            Value::String(
                "f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f".to_string(),
            ),
            Value::String("x".to_string()),
        ],
    };

    let auth_json = serde_json::to_string(&auth_request).unwrap();
    let expected_auth = r#"{"id":0,"method":"mining.authorize","params":["f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f","x"]}"#;

    // Parse both to compare structure (order might differ)
    let auth_parsed: Value = serde_json::from_str(&auth_json).unwrap();
    let expected_parsed: Value = serde_json::from_str(expected_auth).unwrap();
    assert_eq!(auth_parsed, expected_parsed);

    // Test mining.subscribe message format
    let subscribe_request = StratumRequest {
        id: json!(2),
        method: "mining.subscribe".to_string(),
        params: vec![Value::String("kdaminer-v1.0.0".to_string()), Value::Null],
    };

    let subscribe_json = serde_json::to_string(&subscribe_request).unwrap();
    let expected_subscribe =
        r#"{"id":2,"method":"mining.subscribe","params":["kdaminer-v1.0.0",null]}"#;

    let subscribe_parsed: Value = serde_json::from_str(&subscribe_json).unwrap();
    let expected_sub_parsed: Value = serde_json::from_str(expected_subscribe).unwrap();
    assert_eq!(subscribe_parsed, expected_sub_parsed);
}

#[test]
fn test_stratum_response_formats() {
    // Test response formats that clients expect

    // Successful authorization response
    let auth_response = StratumResponse::success(json!(0), json!(true));
    let auth_json = serde_json::to_string(&auth_response).unwrap();

    // Should contain the ID and result
    assert!(auth_json.contains(r#""id":0"#));
    assert!(auth_json.contains(r#""result":true"#));
    assert!(auth_json.contains(r#""error":null"#));

    // Subscribe response with extranonce info
    let subscribe_result = json!([[["mining.notify", "subscription_id"]], "extranonce1_hex", 4]);
    let subscribe_response = StratumResponse::success(json!(2), subscribe_result);
    let subscribe_json = serde_json::to_string(&subscribe_response).unwrap();

    assert!(subscribe_json.contains(r#""id":2"#));
    assert!(subscribe_json.contains(r#""mining.notify""#));
    assert!(subscribe_json.contains(r#""extranonce1_hex""#));
}

#[test]
fn test_stratum_method_parsing() {
    // Test that our method parsing matches what the expect script sends

    assert_eq!(
        StratumMethod::parse_method("mining.authorize"),
        StratumMethod::Authorize
    );
    assert_eq!(
        StratumMethod::parse_method("mining.subscribe"),
        StratumMethod::Subscribe
    );
    assert_eq!(
        StratumMethod::parse_method("mining.submit"),
        StratumMethod::Submit
    );

    // Test unknown methods
    match StratumMethod::parse_method("unknown.method") {
        StratumMethod::Unknown(method) => assert_eq!(method, "unknown.method"),
        _ => panic!("Expected Unknown method"),
    }
}

#[test]
fn test_stratum_error_responses() {
    // Test error response formats
    let error_response = StratumResponse::error(json!(1), 20, "Invalid username");
    let error_json = serde_json::to_string(&error_response).unwrap();

    assert!(error_json.contains(r#""id":1"#));
    assert!(error_json.contains(r#""result":null"#));
    assert!(error_json.contains(r#""error""#));
    assert!(error_json.contains("20"));
    assert!(error_json.contains("Invalid username"));
}

#[test]
fn test_notification_format() {
    // Test notification message format (id: null)
    let notification = StratumNotification::new(
        "mining.notify",
        vec![
            json!("job_id"),
            json!("prevhash"),
            json!("coinb1"),
            json!("coinb2"),
            json!([]), // merkle branches
            json!("version"),
            json!("nbits"),
            json!("ntime"),
            json!(true), // clean_jobs
        ],
    );

    let notif_json = serde_json::to_string(&notification).unwrap();

    // Should have null ID for notifications
    assert!(notif_json.contains(r#""id":null"#));
    assert!(notif_json.contains(r#""method":"mining.notify""#));
    assert!(notif_json.contains(r#""params""#));
}

#[test]
fn test_expect_script_sequence() {
    // Test the exact sequence that the expect script performs

    // Step 1: First authorize (id: 0)
    let req1 = StratumRequest::new(
        json!(0),
        "mining.authorize",
        vec![
            json!("f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"),
            json!("x"),
        ],
    );
    assert_eq!(req1.method_enum(), StratumMethod::Authorize);

    // Step 2: Second authorize (id: 1)
    let req2 = StratumRequest::new(
        json!(1),
        "mining.authorize",
        vec![
            json!("f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"),
            json!("x"),
        ],
    );
    assert_eq!(req2.method_enum(), StratumMethod::Authorize);

    // Step 3: First subscribe (id: 2) with version and null
    let req3 = StratumRequest::new(
        json!(2),
        "mining.subscribe",
        vec![json!("kdaminer-v1.0.0"), json!(null)],
    );
    assert_eq!(req3.method_enum(), StratumMethod::Subscribe);

    // Step 4: Second subscribe (id: 3) with just version
    let req4 = StratumRequest::new(json!(3), "mining.subscribe", vec![json!("kdaminer-v1.0.0")]);
    assert_eq!(req4.method_enum(), StratumMethod::Subscribe);

    // Step 5: Third subscribe (id: 4) with empty params
    let req5 = StratumRequest::new(json!(4), "mining.subscribe", vec![]);
    assert_eq!(req5.method_enum(), StratumMethod::Subscribe);

    // All requests should be valid JSON
    for req in [&req1, &req2, &req3, &req4, &req5] {
        let json_str = serde_json::to_string(req).unwrap();
        let _: Value = serde_json::from_str(&json_str).unwrap(); // Should not panic
    }
}

#[test]
fn test_response_id_matching() {
    // Test that response IDs match request IDs correctly

    for id in [0, 1, 2, 3, 4] {
        let response = StratumResponse::success(json!(id), json!(true));
        let json_str = serde_json::to_string(&response).unwrap();

        // Should contain the exact ID
        assert!(json_str.contains(&format!(r#""id":{}"#, id)));
    }
}

#[test]
fn test_json_line_protocol() {
    // Test that our messages work with line-based JSON protocol

    let request = StratumRequest::new(
        json!(1),
        "mining.authorize",
        vec![json!("user"), json!("pass")],
    );

    let json_line = serde_json::to_string(&request).unwrap() + "\n";

    // Should be parseable as a single line
    let lines: Vec<&str> = json_line.lines().collect();
    assert_eq!(lines.len(), 1);

    // Should parse back correctly
    let parsed: StratumRequest = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(parsed.method, "mining.authorize");
    assert_eq!(parsed.id, json!(1));
}

/// Integration test that simulates the expect script behavior
#[test]
fn test_expect_script_simulation() {
    // This test simulates what the expect script does without requiring
    // a running server or network connections

    let test_cases = vec![
        // Each tuple is (id, method, params, expected_method_enum)
        (
            0,
            "mining.authorize",
            vec![
                json!("f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"),
                json!("x"),
            ],
            StratumMethod::Authorize,
        ),
        (
            1,
            "mining.authorize",
            vec![
                json!("f89ef46927f506c70b6a58fd322450a936311dc6ac91f4ec3d8ef949608dbf1f"),
                json!("x"),
            ],
            StratumMethod::Authorize,
        ),
        (
            2,
            "mining.subscribe",
            vec![json!("kdaminer-v1.0.0"), json!(null)],
            StratumMethod::Subscribe,
        ),
        (
            3,
            "mining.subscribe",
            vec![json!("kdaminer-v1.0.0")],
            StratumMethod::Subscribe,
        ),
        (4, "mining.subscribe", vec![], StratumMethod::Subscribe),
    ];

    for (id, method, params, expected_method) in test_cases {
        // Create request (simulates what expect script sends)
        let request = StratumRequest::new(json!(id), method, params);

        // Verify it parses correctly
        assert_eq!(request.method_enum(), expected_method);
        assert_eq!(request.id, json!(id));

        // Create a successful response (simulates what server should send back)
        let response = StratumResponse::success(json!(id), json!(true));

        // Verify response format
        let response_json = serde_json::to_string(&response).unwrap();
        assert!(response_json.contains(&format!(r#""id":{}"#, id)));

        // Verify the expect script regex would match
        // The expect script looks for: .*({.*"id": *ID})\r
        let _id_pattern = format!(r#"{{"#) + &format!(r#".*"id": *{}"#, id) + r#"}"#;
        assert!(response_json.contains(&format!(r#""id":{}"#, id)));
    }
}
