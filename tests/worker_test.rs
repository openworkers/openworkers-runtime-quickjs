use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;

#[tokio::test]
async fn test_simple_response() {
    let script = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Hello from QuickJS!'));
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    let body = response.body.as_bytes().expect("Should have body");
    assert_eq!(String::from_utf8_lossy(body), "Hello from QuickJS!");
}

#[tokio::test]
async fn test_json_response() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const data = { message: 'Hello', value: 42 };
            event.respondWith(new Response(JSON.stringify(data), {
                headers: { 'Content-Type': 'application/json' }
            }));
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.headers.get("content-type"),
        Some(&"application/json".to_string())
    );

    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["message"], "Hello");
    assert_eq!(json["value"], 42);
}

#[tokio::test]
async fn test_custom_status() {
    let script = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Not Found', { status: 404 }));
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 404);
}

#[tokio::test]
async fn test_request_method() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const method = event.request.method;
            event.respondWith(new Response(method));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "POST".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let body = response.body.as_bytes().expect("Should have body");
    assert_eq!(String::from_utf8_lossy(body), "POST");
}

#[tokio::test]
async fn test_request_url() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const url = new URL(event.request.url);
            event.respondWith(new Response(url.pathname));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/api/test".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let body = response.body.as_bytes().expect("Should have body");
    assert_eq!(String::from_utf8_lossy(body), "/api/test");
}

#[tokio::test]
async fn test_async_handler() {
    // Use respondWith with a Promise that resolves to Response
    let script = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(
                Promise.resolve({ async: true })
                    .then(data => new Response(JSON.stringify(data)))
            );
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

    let response = rx.await.expect("Should receive response");
    let body = response.body.as_bytes().expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(body).expect("Should be valid JSON");
    assert_eq!(json["async"], true);
}

#[tokio::test]
async fn test_console_log() {
    let script = r#"
        addEventListener('fetch', (event) => {
            console.log('Hello from console!');
            console.warn('Warning message');
            console.error('Error message');
            event.respondWith(new Response('logged'));
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);
}
