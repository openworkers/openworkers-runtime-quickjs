# OpenWorkers QuickJS Runtime

A lightweight JavaScript runtime for OpenWorkers based on QuickJS, providing fast cold starts and efficient execution for edge computing workloads.

## Features

- **Fast Cold Start**: ~850µs worker creation time
- **High Throughput**: ~85,000 req/s for simple responses
- **Web APIs**: Fetch, Request, Response, Headers, URL, URLSearchParams
- **Streaming**: ReadableStream support for response bodies
- **Async/Await**: Full ES2023 async support with Promises
- **Scheduled Events**: Cron-style scheduled task execution

## Runtime Comparison (v0.5.0)

| Runtime | Engine | Worker::new() | exec_simple | exec_json | Tests |
|---------|--------|---------------|-------------|-----------|-------|
| **[QuickJS](https://github.com/openworkers/openworkers-runtime-quickjs)** | QuickJS | 738µs | **12.4µs** ⚡ | **13.7µs** | 16/17 |
| **[V8](https://github.com/openworkers/openworkers-runtime-v8)** | V8 | 790µs | 32.3µs | 34.3µs | **17/17** |
| **[JSC](https://github.com/openworkers/openworkers-runtime-jsc)** | JavaScriptCore | 1.07ms | 30.3µs | 28.3µs | 15/17 |
| **[Deno](https://github.com/openworkers/openworkers-runtime-deno)** | V8 + Deno | 2.56ms | 46.8µs | 38.7µs | **17/17** |
| **[Boa](https://github.com/openworkers/openworkers-runtime-boa)** | Boa | 738µs | 12.4µs | 13.7µs | 13/17 |

**QuickJS has the fastest exec time** (~12µs) - ideal for high-throughput scenarios.

## Usage

```rust
use openworkers_runtime_quickjs::{HttpRequest, Script, Task, Worker};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let script = Script::new(r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('Hello World!'));
        });
    "#);

    let mut worker = Worker::new(script, None, None)
        .await
        .expect("Failed to create worker");

    let request = HttpRequest {
        method: "GET".to_string(),
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: None,
    };

    let (task, rx) = Task::fetch(request);
    worker.exec(task).await.expect("Failed to execute");

    let response = rx.await.expect("Failed to receive response");
    println!("Status: {}", response.status);
}
```

## Streaming Responses

```javascript
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
```

## Scheduled Events

```javascript
addEventListener('scheduled', (event) => {
    console.log('Scheduled at:', event.scheduledTime);
    event.waitUntil(doSomeWork());
});
```

## Supported Web APIs

- `fetch()` - HTTP client
- `Request` / `Response` - HTTP primitives
- `Headers` - HTTP headers
- `URL` / `URLSearchParams` - URL parsing
- `TextEncoder` / `TextDecoder` - Text encoding
- `ReadableStream` - Streaming bodies
- `atob()` / `btoa()` - Base64 encoding
- `addEventListener()` - Event registration
- `Promise` - Async primitives

## Running Benchmarks

```bash
cargo run --example streaming_bench --release
```

## Running Tests

```bash
cargo test
```

## License

MIT License - see [LICENSE](LICENSE) file.
