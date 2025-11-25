use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;
use std::time::{Duration, Instant};

async fn bench_buffered_response(iterations: u32) -> Duration {
    let code = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Hello World from buffered response!'));
        });
    "#;

    let script = Script::new(code);
    let mut worker = Worker::new(script, None, None).await.unwrap();

    let start = Instant::now();

    for _ in 0..iterations {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "http://localhost/".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let (task, rx) = Task::fetch(req);
        worker.exec(task).await.unwrap();
        let _ = rx.await.unwrap();
    }

    start.elapsed()
}

async fn bench_json_response(iterations: u32) -> Duration {
    let code = r#"
        addEventListener('fetch', (event) => {
            const data = { message: 'Hello', timestamp: Date.now(), count: 42 };
            event.respondWith(new Response(JSON.stringify(data), {
                headers: { 'Content-Type': 'application/json' }
            }));
        });
    "#;

    let script = Script::new(code);
    let mut worker = Worker::new(script, None, None).await.unwrap();

    let start = Instant::now();

    for _ in 0..iterations {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "http://localhost/".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let (task, rx) = Task::fetch(req);
        worker.exec(task).await.unwrap();
        let _ = rx.await.unwrap();
    }

    start.elapsed()
}

async fn bench_worker_creation(iterations: u32) -> Duration {
    let code = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Hello'));
        });
    "#;

    let start = Instant::now();

    for _ in 0..iterations {
        let script = Script::new(code);
        let _worker = Worker::new(script, None, None).await.unwrap();
    }

    start.elapsed()
}

async fn bench_url_parsing(iterations: u32) -> Duration {
    let code = r#"
        addEventListener('fetch', (event) => {
            const url = new URL(event.request.url);
            event.respondWith(new Response(url.pathname));
        });
    "#;

    let script = Script::new(code);
    let mut worker = Worker::new(script, None, None).await.unwrap();

    let start = Instant::now();

    for _ in 0..iterations {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "http://localhost/api/v1/users/123".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let (task, rx) = Task::fetch(req);
        worker.exec(task).await.unwrap();
        let _ = rx.await.unwrap();
    }

    start.elapsed()
}

async fn bench_async_response(iterations: u32) -> Duration {
    let code = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(
                Promise.resolve({ ok: true })
                    .then(data => new Response(JSON.stringify(data)))
            );
        });
    "#;

    let script = Script::new(code);
    let mut worker = Worker::new(script, None, None).await.unwrap();

    let start = Instant::now();

    for _ in 0..iterations {
        let req = HttpRequest {
            method: "GET".to_string(),
            url: "http://localhost/".to_string(),
            headers: HashMap::new(),
            body: None,
        };

        let (task, rx) = Task::fetch(req);
        worker.exec(task).await.unwrap();
        let _ = rx.await.unwrap();
    }

    start.elapsed()
}

#[tokio::main]
async fn main() {
    println!("🚀 OpenWorkers QuickJS Streaming Benchmark\n");
    println!("========================================\n");

    // Warmup
    println!("Warming up...");
    let _ = bench_buffered_response(10).await;
    println!();

    // Benchmark 1: Worker creation
    println!("🏗️  Worker Creation:");
    let iterations = 100;
    let elapsed = bench_worker_creation(iterations).await;
    let per_worker = elapsed / iterations;
    println!(
        "  {} iterations in {:.2?} ({:.2?}/worker, {:.0} workers/s)\n",
        iterations,
        elapsed,
        per_worker,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Benchmark 2: Buffered responses
    println!("📦 Buffered Response (local, no network):");
    let iterations = 1000;
    let elapsed = bench_buffered_response(iterations).await;
    let per_request = elapsed / iterations;
    println!(
        "  {} iterations in {:.2?} ({:.2?}/req, {:.0} req/s)\n",
        iterations,
        elapsed,
        per_request,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Benchmark 3: JSON responses
    println!("📄 JSON Response (local, no network):");
    let iterations = 1000;
    let elapsed = bench_json_response(iterations).await;
    let per_request = elapsed / iterations;
    println!(
        "  {} iterations in {:.2?} ({:.2?}/req, {:.0} req/s)\n",
        iterations,
        elapsed,
        per_request,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Benchmark 4: URL parsing
    println!("🔗 URL Parsing:");
    let iterations = 1000;
    let elapsed = bench_url_parsing(iterations).await;
    let per_request = elapsed / iterations;
    println!(
        "  {} iterations in {:.2?} ({:.2?}/req, {:.0} req/s)\n",
        iterations,
        elapsed,
        per_request,
        iterations as f64 / elapsed.as_secs_f64()
    );

    // Benchmark 5: Async responses
    println!("⚡ Async Response (Promise.resolve):");
    let iterations = 1000;
    let elapsed = bench_async_response(iterations).await;
    let per_request = elapsed / iterations;
    println!(
        "  {} iterations in {:.2?} ({:.2?}/req, {:.0} req/s)\n",
        iterations,
        elapsed,
        per_request,
        iterations as f64 / elapsed.as_secs_f64()
    );

    println!("========================================");
    println!("📝 Summary:");
    println!("  - QuickJS is lightweight (~200KB binary impact)");
    println!("  - Good for embedded/edge use cases");
    println!("  - ES2023 support with async/await");
    println!("\n✅ Benchmark complete!");
}
