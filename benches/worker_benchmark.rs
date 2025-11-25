use criterion::{Criterion, criterion_group, criterion_main};
use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn bench_worker_new(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    c.bench_function("Worker/new", |b| {
        b.iter(|| {
            rt.block_on(async {
                let script = Script::new(
                    r#"
                    addEventListener('fetch', (event) => {
                        event.respondWith(new Response('Hello'));
                    });
                "#,
                );
                Worker::new(script, None, None).await.unwrap()
            })
        })
    });
}

fn bench_exec_simple_response(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut worker = rt.block_on(async {
        let script = Script::new(
            r#"
            addEventListener('fetch', (event) => {
                event.respondWith(new Response('Hello World'));
            });
        "#,
        );
        Worker::new(script, None, None).await.unwrap()
    });

    c.bench_function("Worker/exec_simple_response", |b| {
        b.iter(|| {
            rt.block_on(async {
                let req = HttpRequest {
                    method: "GET".to_string(),
                    url: "http://localhost/".to_string(),
                    headers: HashMap::new(),
                    body: None,
                };
                let (task, rx) = Task::fetch(req);
                worker.exec(task).await.unwrap();
                rx.await.unwrap()
            })
        })
    });
}

fn bench_exec_json_response(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut worker = rt.block_on(async {
        let script = Script::new(
            r#"
            addEventListener('fetch', (event) => {
                const data = { message: 'Hello', timestamp: Date.now(), count: 42 };
                event.respondWith(new Response(JSON.stringify(data), {
                    headers: { 'Content-Type': 'application/json' }
                }));
            });
        "#,
        );
        Worker::new(script, None, None).await.unwrap()
    });

    c.bench_function("Worker/exec_json_response", |b| {
        b.iter(|| {
            rt.block_on(async {
                let req = HttpRequest {
                    method: "GET".to_string(),
                    url: "http://localhost/".to_string(),
                    headers: HashMap::new(),
                    body: None,
                };
                let (task, rx) = Task::fetch(req);
                worker.exec(task).await.unwrap();
                rx.await.unwrap()
            })
        })
    });
}

fn bench_exec_with_headers(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let mut worker = rt.block_on(async {
        let script = Script::new(
            r#"
            addEventListener('fetch', (event) => {
                event.respondWith(new Response('Hello', {
                    status: 200,
                    headers: {
                        'Content-Type': 'text/plain',
                        'X-Custom-Header': 'custom-value',
                        'Cache-Control': 'no-cache'
                    }
                }));
            });
        "#,
        );
        Worker::new(script, None, None).await.unwrap()
    });

    c.bench_function("Worker/exec_with_headers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let req = HttpRequest {
                    method: "GET".to_string(),
                    url: "http://localhost/".to_string(),
                    headers: HashMap::new(),
                    body: None,
                };
                let (task, rx) = Task::fetch(req);
                worker.exec(task).await.unwrap();
                rx.await.unwrap()
            })
        })
    });
}

criterion_group!(
    benches,
    bench_worker_new,
    bench_exec_simple_response,
    bench_exec_json_response,
    bench_exec_with_headers,
);

criterion_main!(benches);
