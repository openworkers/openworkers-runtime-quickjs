# OpenWorkers Runtime QuickJS

Lightweight JavaScript runtime for serverless workers, built on [QuickJS](https://bellard.org/quickjs/).

## Quick Start

```rust
use openworkers_runtime_quickjs::{Worker, Script, Event, HttpRequest, HttpMethod, RequestBody};
use std::collections::HashMap;

let script = Script::new(r#"
    addEventListener('fetch', event => {
        event.respondWith(new Response('Hello from QuickJS!'));
    });
"#);

let mut worker = Worker::new(script, None).await?;

let req = HttpRequest {
    method: HttpMethod::Get,
    url: "http://localhost/".to_string(),
    headers: HashMap::new(),
    body: RequestBody::None,
};

let (task, rx) = Event::fetch(req);
worker.exec(task).await?;

let response = rx.await?;
```

## Features

- **Lightweight** — Small binary footprint (~200KB)
- **Fast cold start** — Sub-millisecond worker creation
- **Web APIs** — fetch, setTimeout, Response, Request, URL, console
- **Async/await** — Full Promise support
- **Streaming** — ReadableStream support

## Testing

```bash
cargo test
```

## Status

See [TODO.md](TODO.md) for current limitations and roadmap.

## License

MIT
