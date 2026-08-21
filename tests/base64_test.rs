use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Evaluate a JS expression inside a worker and return it as JSON
async fn eval(expr: &str) -> serde_json::Value {
    let script = format!(
        "addEventListener('fetch', event => event.respondWith(new Response(JSON.stringify({}))));",
        expr
    );

    let mut worker = Worker::new(Script::new(script.as_str()), None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: RequestBody::None,
    };

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let body = response
        .body
        .collect()
        .await
        .expect("Should read body")
        .expect("Should have body");

    serde_json::from_slice(&body).expect("Should be valid JSON")
}

#[tokio::test]
async fn test_btoa_encodes() {
    let encoded = eval("[btoa(''), btoa('f'), btoa('fo'), btoa('foo'), btoa('hello world')]").await;

    assert_eq!(encoded[0], "");
    assert_eq!(encoded[1], "Zg==");
    assert_eq!(encoded[2], "Zm8=");
    assert_eq!(encoded[3], "Zm9v");
    assert_eq!(encoded[4], "aGVsbG8gd29ybGQ=");
}

#[tokio::test]
async fn test_btoa_encodes_high_bytes() {
    let encoded = eval("btoa(String.fromCharCode(0, 127, 128, 255))").await;

    assert_eq!(encoded, "AH+A/w==");
}

#[tokio::test]
async fn test_btoa_rejects_code_units_above_255() {
    let thrown = eval(
        "(() => { try { btoa('\\u0100'); return 'no throw'; } catch (e) { return e.constructor.name; } })()",
    )
    .await;

    assert_eq!(thrown, "TypeError");
}

#[tokio::test]
async fn test_atob_decodes() {
    let decoded = eval("[atob(''), atob('Zg=='), atob('Zm8='), atob('Zm9v')]").await;

    assert_eq!(decoded[0], "");
    assert_eq!(decoded[1], "f");
    assert_eq!(decoded[2], "fo");
    assert_eq!(decoded[3], "foo");
}

#[tokio::test]
async fn test_atob_accepts_missing_padding_and_whitespace() {
    let decoded = eval("[atob('Zm8'), atob(' Zm\\n9v ')]").await;

    assert_eq!(decoded[0], "fo");
    assert_eq!(decoded[1], "foo");
}

#[tokio::test]
async fn test_atob_yields_one_code_unit_per_byte() {
    let codes = eval("[...atob('AH+A/w==')].map(c => c.charCodeAt(0))").await;

    assert_eq!(codes, serde_json::json!([0, 127, 128, 255]));
}

#[tokio::test]
async fn test_atob_rejects_invalid_input() {
    let thrown = eval(
        "['Zm9v!', 'Z'].map(input => { try { atob(input); return 'no throw'; } catch (e) { return e.constructor.name; } })",
    )
    .await;

    assert_eq!(thrown, serde_json::json!(["TypeError", "TypeError"]));
}

#[tokio::test]
async fn test_base64_round_trip() {
    let round_trip = eval("atob(btoa('The quick brown fox'))").await;

    assert_eq!(round_trip, "The quick brown fox");
}
