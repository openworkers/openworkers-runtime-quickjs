use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;

#[tokio::test]
async fn test_readable_stream_basic() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const stream = new ReadableStream({
                start(controller) {
                    controller.enqueue(new TextEncoder().encode('Hello '));
                    controller.enqueue(new TextEncoder().encode('World!'));
                    controller.close();
                }
            });
            event.respondWith(new Response(stream));
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
    let result = worker.exec(task).await;
    println!("exec result: {:?}", result);
    result.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    assert_eq!(response.status, 200);

    // Stream is buffered into bytes
    let body = response.body.as_bytes().expect("Should have body");
    assert_eq!(String::from_utf8_lossy(body), "Hello World!");
}

#[tokio::test]
async fn test_readable_stream_with_pull() {
    let script = r#"
        addEventListener('fetch', (event) => {
            let count = 0;
            const stream = new ReadableStream({
                pull(controller) {
                    if (count < 3) {
                        controller.enqueue(new TextEncoder().encode(`Chunk ${count}\n`));
                        count++;
                    } else {
                        controller.close();
                    }
                }
            });
            event.respondWith(new Response(stream));
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
    let result = String::from_utf8_lossy(body);
    assert!(result.contains("Chunk 0"));
    assert!(result.contains("Chunk 1"));
    assert!(result.contains("Chunk 2"));
}

#[tokio::test]
async fn test_non_stream_response() {
    // Test that non-streaming responses still work
    let script = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Hello World'));
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
    assert_eq!(String::from_utf8_lossy(body), "Hello World");
}

#[tokio::test]
async fn test_response_body_property() {
    // Test that Response.body returns a ReadableStream
    let script = r#"
        addEventListener('fetch', (event) => {
            const response = new Response('Test');
            const hasBody = response.body !== null;
            const isStream = response.body instanceof ReadableStream;
            event.respondWith(new Response(JSON.stringify({ hasBody, isStream })));
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
    assert_eq!(json["hasBody"], true);
    assert_eq!(json["isStream"], true);
}

#[tokio::test]
async fn test_readable_stream_async_pull() {
    let script = r#"
        addEventListener('fetch', (event) => {
            let count = 0;
            const stream = new ReadableStream({
                async pull(controller) {
                    await Promise.resolve(); // Simulate async
                    if (count < 2) {
                        controller.enqueue(new TextEncoder().encode(`Async ${count} `));
                        count++;
                    } else {
                        controller.close();
                    }
                }
            });
            event.respondWith(new Response(stream));
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
    let result = String::from_utf8_lossy(body);
    assert!(result.contains("Async 0"));
    assert!(result.contains("Async 1"));
}
