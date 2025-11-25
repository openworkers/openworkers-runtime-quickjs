use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;

#[tokio::test]
async fn test_fetch_basic_get() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.org/get');
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                hasUrl: !!data.url
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["status"], 200);
    assert_eq!(json["hasUrl"], true);
}

#[tokio::test]
async fn test_fetch_post_with_body() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.org/post', {
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
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["status"], 200);
    assert_eq!(json["receivedData"]["hello"], "world");
}

#[tokio::test]
async fn test_fetch_with_headers() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.org/headers', {
                headers: { 'X-Custom-Header': 'test-value' }
            });
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                customHeader: data.headers['X-Custom-Header']
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["customHeader"], "test-value");
}

#[tokio::test]
async fn test_fetch_forward() {
    // Test fetch forward - direct pass-through of fetch response
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.org/get');
            event.respondWith(response);
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    assert_eq!(response.status, 200);

    let body = response.body.as_bytes().expect("Should have body");
    let body_str = String::from_utf8_lossy(body);
    assert!(
        body_str.contains("httpbin.org"),
        "Should contain httpbin response"
    );
}

#[tokio::test]
async fn test_fetch_404() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://httpbin.org/status/404');
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                ok: response.ok
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = tokio::time::timeout(std::time::Duration::from_secs(10), rx)
        .await
        .expect("Should receive response within timeout")
        .expect("Channel should not close");

    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["status"], 404);
    assert_eq!(json["ok"], false);
}
