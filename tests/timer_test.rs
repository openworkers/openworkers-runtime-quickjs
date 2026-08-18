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
}

/// Run a handler body that responds with JSON and return the parsed result
async fn respond(handler: &str) -> serde_json::Value {
    let script = format!(
        "addEventListener('fetch', async (event) => {{ {} }});",
        handler
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
    let body = response.body.collect().await.expect("Should have body");

    serde_json::from_slice(&body).expect("Should be valid JSON")
}

#[tokio::test]
async fn test_awaited_timeout_resumes_the_handler() {
    let result = respond(
        "const start = Date.now();
         await new Promise(resolve => setTimeout(resolve, 20));
         event.respondWith(new Response(JSON.stringify({ elapsed: Date.now() - start })));",
    )
    .await;

    assert!(
        result["elapsed"].as_i64().unwrap() >= 20,
        "handler resumed too early: {}",
        result["elapsed"]
    );
}

#[tokio::test]
async fn test_awaited_timeouts_fire_in_delay_order() {
    let result = respond(
        "const order = [];
         const timer = (delay) => new Promise(resolve => setTimeout(() => { order.push(delay); resolve(); }, delay));
         await Promise.all([timer(30), timer(5), timer(15)]);
         event.respondWith(new Response(JSON.stringify(order)));",
    )
    .await;

    assert_eq!(result, serde_json::json!([5, 15, 30]));
}

#[tokio::test]
async fn test_cleared_timeout_never_runs() {
    let result = respond(
        "let fired = false;
         const id = setTimeout(() => { fired = true; }, 5);
         clearTimeout(id);
         await new Promise(resolve => setTimeout(resolve, 30));
         event.respondWith(new Response(JSON.stringify({ fired })));",
    )
    .await;

    assert_eq!(result["fired"], false);
}

#[tokio::test]
async fn test_interval_repeats_until_cleared() {
    let result = respond(
        "let ticks = 0;
         const id = setInterval(() => { ticks++; }, 5);
         await new Promise(resolve => setTimeout(resolve, 60));
         clearInterval(id);
         const afterClear = ticks;
         await new Promise(resolve => setTimeout(resolve, 30));
         event.respondWith(new Response(JSON.stringify({ ticks, afterClear })));",
    )
    .await;

    assert!(
        result["afterClear"].as_i64().unwrap() >= 3,
        "interval ticked {} times in 60 ms",
        result["afterClear"]
    );
    assert_eq!(result["ticks"], result["afterClear"]);
}

#[tokio::test]
async fn test_out_of_range_delays_fire_immediately() {
    let result = respond(
        "const fired = [];
         await Promise.all([
             new Promise(resolve => setTimeout(() => { fired.push('negative'); resolve(); }, -1)),
             new Promise(resolve => setTimeout(() => { fired.push('nan'); resolve(); }, NaN)),
             new Promise(resolve => setTimeout(() => { fired.push('string'); resolve(); }, '0')),
         ]);
         event.respondWith(new Response(JSON.stringify(fired.sort())));",
    )
    .await;

    assert_eq!(result, serde_json::json!(["nan", "negative", "string"]));
}

#[tokio::test]
async fn test_timer_arguments_reach_the_callback() {
    let result = respond(
        "const sum = await new Promise(resolve => setTimeout((a, b, c) => resolve(a + b + c), 5, 1, 2, 3));
         event.respondWith(new Response(JSON.stringify({ sum })));",
    )
    .await;

    assert_eq!(result["sum"], 6);
}
