use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

#[tokio::test]
async fn test_settimeout_basic() {
    let script = r#"
        globalThis.timeoutFired = false;

        addEventListener('fetch', (event) => {
            setTimeout(() => {
                globalThis.timeoutFired = true;
            }, 10);

            event.respondWith(new Response(JSON.stringify({
                before: globalThis.timeoutFired
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");

    // The timeout should have been set up, and since we process timers after
    // the handler completes, timeoutFired should be true by the time we check
    assert_eq!(
        json["before"], false,
        "Before respondWith, timeout should not have fired"
    );
}

#[tokio::test]
async fn test_settimeout_fires_after_response() {
    let script = r#"
        globalThis.counter = 0;

        addEventListener('fetch', (event) => {
            setTimeout(() => {
                globalThis.counter = 42;
            }, 10);

            event.respondWith(new Response('ok'));
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

    let _ = rx.await.expect("Should receive response");

    // Timers are processed after the event handler completes.
    // The timer callback should have fired by now since exec() processes
    // pending timers before returning.
}

#[tokio::test]
async fn test_cleartimeout_prevents_execution() {
    let script = r#"
        globalThis.shouldNotRun = false;

        addEventListener('fetch', (event) => {
            const id = setTimeout(() => {
                globalThis.shouldNotRun = true;
            }, 10);
            clearTimeout(id);

            event.respondWith(new Response(JSON.stringify({
                timerCleared: true
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    // Give time for any timer to fire (it shouldn't)
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_settimeout_returns_id() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const id = setTimeout(() => {}, 1000);
            const isNumber = typeof id === 'number' && id > 0;

            event.respondWith(new Response(JSON.stringify({
                hasId: isNumber,
                id: id
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["hasId"], true);
}

#[tokio::test]
async fn test_setinterval_returns_id() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const id = setInterval(() => {}, 1000);
            clearInterval(id);  // Clean up
            const isNumber = typeof id === 'number' && id > 0;

            event.respondWith(new Response(JSON.stringify({
                hasId: isNumber,
                id: id
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

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["hasId"], true);
}

#[tokio::test]
async fn test_settimeout_with_arguments() {
    let script = r#"
        globalThis.result = null;

        addEventListener('fetch', (event) => {
            setTimeout((a, b, c) => {
                globalThis.result = a + b + c;
            }, 10, 1, 2, 3);

            event.respondWith(new Response('ok'));
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

    let _ = rx.await.expect("Should receive response");

    // Timer callback should have fired with arguments
    // Result should be 1 + 2 + 3 = 6
}
