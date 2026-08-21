use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;
use std::time::{Duration, Instant};

fn request(path: &str) -> HttpRequest {
    HttpRequest {
        method: HttpMethod::Get,
        url: format!("http://localhost{}", path),
        headers: HashMap::new(),
        body: RequestBody::None,
    }
}

async fn call(worker: &mut Worker, path: &str) -> Vec<u8> {
    let (task, rx) = Event::fetch(request(path));
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
async fn test_pending_timer_does_not_run_in_the_next_request() {
    let script = r#"
        globalThis.fired = 0;

        addEventListener('fetch', async (event) => {
            if (event.request.url.endsWith('/arm')) {
                setTimeout(() => { globalThis.fired++; }, 5);
                event.respondWith(new Response('armed'));
                return;
            }

            await new Promise(resolve => setTimeout(resolve, 30));
            event.respondWith(new Response(String(globalThis.fired)));
        });
    "#;

    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    assert_eq!(call(&mut worker, "/arm").await, b"armed");
    assert_eq!(call(&mut worker, "/check").await, b"0");
}

#[tokio::test]
async fn test_pending_interval_does_not_run_in_the_next_request() {
    let script = r#"
        globalThis.ticks = 0;

        addEventListener('fetch', async (event) => {
            if (event.request.url.endsWith('/arm')) {
                setInterval(() => { globalThis.ticks++; }, 5);
                event.respondWith(new Response('armed'));
                return;
            }

            await new Promise(resolve => setTimeout(resolve, 40));
            event.respondWith(new Response(String(globalThis.ticks)));
        });
    "#;

    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    assert_eq!(call(&mut worker, "/arm").await, b"armed");
    assert_eq!(call(&mut worker, "/check").await, b"0");
}

#[tokio::test]
async fn test_wait_until_runs_after_the_response() {
    let script = r#"
        globalThis.done = false;

        addEventListener('fetch', (event) => {
            if (event.request.url.endsWith('/start')) {
                event.waitUntil(new Promise(resolve => setTimeout(() => {
                    globalThis.done = true;
                    resolve();
                }, 60)));
                event.respondWith(new Response('started'));
                return;
            }

            event.respondWith(new Response(String(globalThis.done)));
        });
    "#;

    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Event::fetch(request("/start"));
    let started = Instant::now();
    let (executed, answered) = tokio::join!(worker.exec(task), async {
        rx.await.expect("Should receive response");
        started.elapsed()
    });

    executed.expect("Task should execute");

    assert!(
        answered < Duration::from_millis(40),
        "the response waited for waitUntil: {:?}",
        answered
    );
    assert!(
        started.elapsed() >= Duration::from_millis(60),
        "exec returned before waitUntil settled"
    );
    assert_eq!(call(&mut worker, "/check").await, b"true");
}

#[tokio::test]
async fn test_rejected_wait_until_does_not_fail_the_task() {
    let script = r#"
        addEventListener('fetch', (event) => {
            event.waitUntil(Promise.reject(new Error('background failed')));
            event.respondWith(new Response('ok'));
        });
    "#;

    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    assert_eq!(call(&mut worker, "/").await, b"ok");
}
