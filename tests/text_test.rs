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
async fn test_encode_utf8() {
    let bytes = eval("[...new TextEncoder().encode('h\u{e9}\u{20ac}')]").await;

    assert_eq!(bytes, serde_json::json!([104, 195, 169, 226, 130, 172]));
}

#[tokio::test]
async fn test_encode_without_argument_is_empty() {
    let length = eval("new TextEncoder().encode().length").await;

    assert_eq!(length, 0);
}

#[tokio::test]
async fn test_encoding_property() {
    let names = eval("[new TextEncoder().encoding, new TextDecoder().encoding]").await;

    assert_eq!(names, serde_json::json!(["utf-8", "utf-8"]));
}

#[tokio::test]
async fn test_decode_accepts_every_buffer_source() {
    let texts = eval(
        "(bytes => [
            new TextDecoder().decode(bytes),
            new TextDecoder().decode(bytes.buffer),
            new TextDecoder().decode(new DataView(bytes.buffer))
        ])(new TextEncoder().encode('caf\u{e9}'))",
    )
    .await;

    assert_eq!(
        texts,
        serde_json::json!(["caf\u{e9}", "caf\u{e9}", "caf\u{e9}"])
    );
}

#[tokio::test]
async fn test_decode_without_argument_is_empty() {
    let text = eval("new TextDecoder().decode()").await;

    assert_eq!(text, "");
}

#[tokio::test]
async fn test_invalid_bytes_become_replacement_characters() {
    let text = eval("new TextDecoder().decode(new Uint8Array([104, 255, 105]))").await;

    assert_eq!(text, "h\u{fffd}i");
}

#[tokio::test]
async fn test_fatal_decoder_throws_on_invalid_bytes() {
    let thrown = eval(
        "(() => { try { new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array([255])); return 'no throw'; } catch (e) { return e.constructor.name; } })()",
    )
    .await;

    assert_eq!(thrown, "TypeError");
}

#[tokio::test]
async fn test_unknown_encoding_is_rejected() {
    let thrown = eval(
        "(() => { try { new TextDecoder('latin1'); return 'no throw'; } catch (e) { return e.constructor.name; } })()",
    )
    .await;

    assert_eq!(thrown, "RangeError");
}

#[tokio::test]
async fn test_byte_order_mark_is_stripped_unless_kept() {
    let texts = eval(
        "(bytes => [
            new TextDecoder().decode(bytes),
            new TextDecoder('utf-8', { ignoreBOM: true }).decode(bytes)
        ])(new Uint8Array([239, 187, 191, 104, 105]))",
    )
    .await;

    assert_eq!(texts[0], "hi");
    assert_eq!(texts[1], "\u{feff}hi");
}

#[tokio::test]
async fn test_streaming_decode_joins_a_split_sequence() {
    let texts = eval(
        "(decoder => [
            decoder.decode(new Uint8Array([104, 226, 130]), { stream: true }),
            decoder.decode(new Uint8Array([172, 105]))
        ])(new TextDecoder())",
    )
    .await;

    assert_eq!(texts[0], "h");
    assert_eq!(texts[1], "\u{20ac}i");
}
