use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Respond with `body` and return the bytes the runtime hands back
async fn respond_with(body: &str) -> Vec<u8> {
    let script = format!(
        "addEventListener('fetch', event => event.respondWith(new Response({})));",
        body
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

    response
        .body
        .collect()
        .await
        .expect("Should read body")
        .unwrap_or_default()
        .to_vec()
}

#[tokio::test]
async fn test_uint8array_body_reaches_the_client() {
    let body = respond_with("new TextEncoder().encode('hello bytes')").await;

    assert_eq!(body, b"hello bytes");
}

#[tokio::test]
async fn test_array_buffer_body_reaches_the_client() {
    let body = respond_with("new TextEncoder().encode('from buffer').buffer").await;

    assert_eq!(body, b"from buffer");
}

#[tokio::test]
async fn test_typed_array_view_respects_its_offset() {
    let body =
        respond_with("new Uint8Array(new TextEncoder().encode('0123456789').buffer, 3, 4)").await;

    assert_eq!(body, b"3456");
}

#[tokio::test]
async fn test_binary_body_survives_non_utf8_bytes() {
    let body = respond_with("new Uint8Array([0, 159, 146, 150, 255])").await;

    assert_eq!(body, [0, 159, 146, 150, 255]);
}

#[tokio::test]
async fn test_string_body_still_works() {
    let body = respond_with("'plain string'").await;

    assert_eq!(body, b"plain string");
}

#[tokio::test]
async fn test_array_buffer_keeps_the_bounds_of_a_view() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const view = new Uint8Array([1, 2, 3, 4, 5]).subarray(1, 3);
            const buffer = await new Response(view).arrayBuffer();
            event.respondWith(new Response(new Uint8Array(buffer)));
        });
    "#;

    let mut worker = Worker::new(Script::new(script), None)
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

    assert_eq!(body, [2, 3].as_slice());
}

#[tokio::test]
async fn test_status_text_defaults_to_empty() {
    let body = respond_with(
        "JSON.stringify([
            new Response('a').statusText,
            new Response('b', { statusText: 'Custom' }).statusText
        ])",
    )
    .await;

    assert_eq!(body, br#"["","Custom"]"#);
}
