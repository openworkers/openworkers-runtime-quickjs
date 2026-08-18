# TODO

## High Priority

- [ ] **ES Modules support** - `export default { fetch() {} }` style handlers
- [ ] **RuntimeLimits enforcement** - the `limits` parameter is accepted and ignored: no heap cap, no CPU or wall clock deadline
- [ ] **Post-response work** - a timer left pending when a request ends only runs once the next task drives the context, and `waitUntil` on a fetch event does nothing
- [ ] **abort()** - only checked before a task starts, so it cannot stop a running script

## Medium Priority

- [ ] **Bindings JS API** - expose `env.KV`, `env.DB`, `env.STORAGE` and worker bindings
- [ ] **True streaming** - request bodies are rejected when streamed, and a response ReadableStream is fully drained before the first chunk is sent
- [ ] **Binary bodies** - request bodies reach JS through `String::from_utf8_lossy`, which corrupts non-UTF-8 payloads
- [ ] **Outbound request headers** - `HttpRequest.headers` is a `HashMap`, so repeated names are joined into one field value before the request leaves the runtime
- [ ] **crypto.subtle** - only `digest`; no importKey/sign/verify, HMAC, ECDSA, RSA or PBKDF2

## Low Priority

- [ ] **Web API coverage** - no AbortController, Blob/FormData, structuredClone, queueMicrotask
- [ ] **statusText** - unknown status codes are reported to JS as "OK"
- [ ] **Benchmark suite** - `examples/ssr_bench.rs` covers SvelteKit SSR; nothing automated

## Won't Do (N/A for QuickJS)

- Isolate pooling - not needed, Context creation is cheap
- Thread pinning - single-threaded interpreter
- GC tracking - automatic reference counting
- Snapshots - cold start is fast enough; `snapshot::create_runtime_snapshot` stays a stub
