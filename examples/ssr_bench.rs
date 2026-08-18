//! Render a SvelteKit SSR bundle and time the wake, serve and sleep cycle.
//!
//! Usage: `cargo run --release --example ssr_bench -- <bundle.js> [path] [--diag]`
//! The bundle must be a classic script (lower ESM with openworkers-transform);
//! `--diag` reports a top-level throw and the response headers.

use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;
use std::time::Instant;

/// The adapter calls `default.fetch(request, env, ctx)`; static assets are not
/// available here, so the ASSETS binding answers 404.
const SHIM: &str = r#"
globalThis.__assetRequests = [];
if (globalThis.__initError) { console.error('INIT ERROR: ' + globalThis.__initError); }
addEventListener('fetch', (event) => {
    const env = {
        ASSETS: {
            fetch(input) {
                globalThis.__assetRequests.push(String(input && input.url ? input.url : input));
                return Promise.resolve(new Response('', { status: 404 }));
            }
        }
    };
    const ctx = { waitUntil() {}, passThroughOnException() {} };
    event.respondWith((async () => {
        try {
            const res = await globalThis.default.fetch(event.request, env, ctx);
            if (globalThis.__diag) {
                console.error('response status=' + res.status +
                    ' headers=' + JSON.stringify([...res.headers.entries()]) +
                    ' assets=' + JSON.stringify(globalThis.__assetRequests));
            }
            return res;
        } catch (e) {
            const detail = e && e.stack ? (e.name + ': ' + e.message + '\n' + e.stack) : String(e);
            return new Response('SSR THREW: ' + detail, { status: 599 });
        }
    })());
});
"#;

async fn render(worker: &mut Worker, path: &str) -> (u16, Vec<u8>) {
    let req = HttpRequest {
        method: HttpMethod::Get,
        url: format!("http://localhost{}", path),
        headers: HashMap::new(),
        body: RequestBody::None,
    };

    let (task, rx) = Event::fetch(req);
    worker.exec(task).await.expect("dispatch failed");
    let res = rx.await.expect("no response");
    let body = res.body.collect().await.unwrap_or_default();

    (res.status, body.to_vec())
}

fn rss_kb() -> u64 {
    let out = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .expect("ps failed");

    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
}

fn stats(mut samples: Vec<f64>) -> (f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());

    (samples[0], samples[samples.len() / 2])
}

#[tokio::main]
async fn main() {
    let bundle = std::env::args()
        .nth(1)
        .expect("usage: ssr_bench <lowered.js> [request-path] [--diag]");
    let path = std::env::args()
        .nth(2)
        .filter(|a| a.starts_with('/'))
        .unwrap_or("/ssr-bench".to_string());
    let source = std::fs::read_to_string(&bundle).expect("cannot read bundle");
    let diag = std::env::args().any(|a| a == "--diag");

    // Wrapping in a function lets a top-level throw be reported instead of aborting worker creation
    let code = if diag {
        format!(
            "globalThis.__diag = true;\nglobalThis.__initError = null;\ntry {{\n(function(){{\n{}\n}})();\n}} catch (e) {{ globalThis.__initError = (e && e.stack) ? (e.name + ': ' + e.message + '\\n' + e.stack) : String(e); }}\n{}",
            source, SHIM
        )
    } else {
        format!("{}\n{}", source, SHIM)
    };
    println!("bundle {} bytes (shim included)", code.len());

    let start = Instant::now();
    let mut worker = Worker::new(Script::new(code.as_str()), None)
        .await
        .expect("worker creation failed");
    println!(
        "worker creation: {:.2} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );

    let start = Instant::now();
    let (status, body) = render(&mut worker, &path).await;
    println!(
        "first render: {:.2} ms, status {}, {} bytes",
        start.elapsed().as_secs_f64() * 1000.0,
        status,
        body.len()
    );

    let rendered = format!("{}.rendered.html", bundle);
    std::fs::write(&rendered, &body).expect("cannot write body");
    println!("body written to {}", rendered);
    println!(
        "body head: {}",
        String::from_utf8_lossy(&body[..body.len().min(120)])
    );

    if !body.windows(5).any(|w| w == b"<html") {
        println!("no markup rendered, stopping");
        return;
    }

    let mut warm = Vec::new();
    for _ in 0..20 {
        let start = Instant::now();
        let (_, again) = render(&mut worker, &path).await;
        warm.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(again, body, "warm render diverged");
    }
    let (min, med) = stats(warm);
    println!("warm render x20: min {:.2} ms, median {:.2} ms", min, med);

    let mut cold = Vec::new();
    let mut creations = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        let mut worker = Worker::new(Script::new(code.as_str()), None)
            .await
            .expect("worker creation failed");
        creations.push(start.elapsed().as_secs_f64() * 1000.0);
        let (_, again) = render(&mut worker, &path).await;
        cold.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(again, body, "cold render diverged");
    }
    let (min, med) = stats(cold);
    println!("cold cycle x10: min {:.2} ms, median {:.2} ms", min, med);
    let (min, med) = stats(creations);
    println!(
        "  of which creation: min {:.2} ms, median {:.2} ms",
        min, med
    );

    let mut empty = Vec::new();
    for _ in 0..10 {
        let start = Instant::now();
        Worker::new(Script::new("addEventListener('fetch', () => {})"), None)
            .await
            .expect("worker creation failed");
        empty.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let (min, med) = stats(empty);
    println!(
        "  empty worker creation: min {:.2} ms, median {:.2} ms",
        min, med
    );

    let before = rss_kb();
    let mut idle = Vec::new();

    for _ in 0..10 {
        idle.push(
            Worker::new(Script::new(code.as_str()), None)
                .await
                .expect("worker creation failed"),
        );
    }

    println!(
        "RSS: {} MB, {} KB per idle worker",
        rss_kb() / 1024,
        (rss_kb() - before) / idle.len() as u64
    );
}
