use bytes::Bytes;
use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Echo the incoming request back as JSON
const ECHO_SCRIPT: &str = r#"
    addEventListener('fetch', async (event) => {
        event.respondWith(new Response(JSON.stringify({
            method: event.request.method,
            url: event.request.url,
            body: await event.request.text(),
            header: event.request.headers.get('x-test')
        })));
    });
"#;

async fn echo(request: HttpRequest) -> serde_json::Value {
    let mut worker = Worker::new(Script::new(ECHO_SCRIPT), None)
        .await
        .expect("Worker should initialize");

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

fn get(url: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: url.to_string(),
        headers: HashMap::new(),
        body: RequestBody::None,
    }
}

#[tokio::test]
async fn test_body_with_quotes_and_newlines_reaches_the_worker() {
    let payload = "line one\n\"quoted\" and \\backslash\\ and 'single'";

    let mut request = get("http://localhost/");
    request.method = HttpMethod::Post;
    request.body = RequestBody::Bytes(Bytes::from(payload));

    let echoed = echo(request).await;

    assert_eq!(echoed["method"], "POST");
    assert_eq!(echoed["body"], payload);
}

#[tokio::test]
async fn test_url_with_quotes_reaches_the_worker() {
    let url = "http://localhost/search?q=%22quoted%22&raw=\"unquoted\"";

    let echoed = echo(get(url)).await;

    assert_eq!(echoed["url"], url);
}

#[tokio::test]
async fn test_request_headers_reach_the_worker() {
    let mut request = get("http://localhost/");
    request
        .headers
        .insert("x-test".to_string(), "value \"with\" quotes".to_string());

    let echoed = echo(request).await;

    assert_eq!(echoed["header"], "value \"with\" quotes");
}

#[tokio::test]
async fn test_request_without_body_is_empty() {
    let echoed = echo(get("http://localhost/")).await;

    assert_eq!(echoed["body"], "");
}

/// Run a script against one POST carrying `payload` and return the body it answered with
async fn post(script: &str, payload: Vec<u8>) -> String {
    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    let mut request = get("http://localhost/");
    request.method = HttpMethod::Post;
    request.body = RequestBody::Bytes(Bytes::from(payload));

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let body = response
        .body
        .collect()
        .await
        .expect("Should read body")
        .unwrap_or_default();

    String::from_utf8(body.to_vec()).expect("Response should be UTF-8")
}

#[tokio::test]
async fn test_non_utf8_body_reaches_the_worker_intact() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const bytes = new Uint8Array(await event.request.arrayBuffer());
                return new Response(JSON.stringify([...bytes]));
            })());
        });
    "#;

    let body = post(script, vec![0xff, 0xfe, 0x00, 0x41]).await;

    assert_eq!(body, "[255,254,0,65]");
}

#[tokio::test]
async fn test_body_used_flips_once_the_body_is_read() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const before = event.request.bodyUsed;
                await event.request.text();
                return new Response(before + ',' + event.request.bodyUsed);
            })());
        });
    "#;

    let body = post(script, b"hello".to_vec()).await;

    assert_eq!(body, "false,true");
}

#[tokio::test]
async fn test_reading_a_body_twice_throws() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                await event.request.text();
                try {
                    await event.request.text();
                    return new Response('no throw');
                } catch (e) {
                    return new Response(e.name);
                }
            })());
        });
    "#;

    let body = post(script, b"hello".to_vec()).await;

    assert_eq!(body, "TypeError");
}
