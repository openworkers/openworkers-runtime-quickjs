# OpenWorkers Runtime QuickJS

Lightweight JavaScript runtime for serverless workers, built on [QuickJS](https://bellard.org/quickjs/).

It implements `openworkers_core::Worker`, so it plugs into the runner the same way the other engines do.

## Quick Start

```rust
use openworkers_runtime_quickjs::{Event, HttpMethod, HttpRequest, RequestBody, Script, Worker};
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

`Worker::new` uses `DefaultOps`, which stubs `fetch()` out and prints logs to stderr.
Pass your own `OperationsHandler` to `Worker::new_with_ops` to serve outbound requests
and collect logs. A runnable version of the above is `cargo run --example hello_world`.

## Implemented

- `fetch` and `scheduled` events via `addEventListener`
- console, Headers, Request, Response, TextEncoder/TextDecoder, `atob`/`btoa`
- `URL` and `URLSearchParams`, backed by the `url` crate
- `setTimeout`/`setInterval`, awaitable from inside a handler
- `fetch()` and console delegated to the runner through `OperationsHandler`
- `crypto.getRandomValues`, `crypto.randomUUID`, `crypto.subtle.digest` (SHA-1/256/384/512)
- ReadableStream response bodies, collected in the runtime before the response is sent

It renders the SvelteKit SSR bundle of openworkers-website byte for byte like V8:
`cargo run --release --example ssr_bench -- <bundle.js>` times the wake, serve and
sleep cycle.

## Not implemented

- ES modules (`export default { fetch() {} }`)
- Streaming request bodies (rejected with an error) and WebSocket
- AbortController, Blob, FormData, structuredClone, `queueMicrotask`
- KV, storage, database and worker bindings
- `RuntimeLimits`: the parameter is accepted and ignored

## Testing

```bash
cargo test
```

## Status

See [TODO.md](TODO.md) for current limitations and roadmap.

## License

MIT
