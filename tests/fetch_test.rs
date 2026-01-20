use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

#[tokio::test]
async fn test_fetch_basic_get() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.workers.rocks/get');
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                hasUrl: !!data.url
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["status"], 200);
    assert_eq!(json["hasUrl"], true);
}

#[tokio::test]
async fn test_fetch_post_with_body() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.workers.rocks/post', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ hello: 'world' })
            });
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                receivedData: data.json
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["status"], 200);
    assert_eq!(json["receivedData"]["hello"], "world");
}

#[tokio::test]
async fn test_fetch_with_headers() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.workers.rocks/headers', {
                headers: { 'X-Custom-Header': 'test-value' }
            });
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                customHeader: data.headers['x-custom-header']
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["customHeader"], "test-value");
}

#[tokio::test]
async fn test_fetch_forward() {
    // Test fetch forward - direct pass-through of fetch response
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.workers.rocks/get');
            event.respondWith(response);
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let body_str = String::from_utf8_lossy(&body);
    assert!(
        body_str.contains("httpbin.workers.rocks"),
        "Should contain httpbin response"
    );
}

#[tokio::test]
async fn test_fetch_404() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.workers.rocks/status/404');
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                ok: response.ok
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["status"], 404);
    assert_eq!(json["ok"], false);
}
