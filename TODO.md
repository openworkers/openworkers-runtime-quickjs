# TODO

## High Priority

- [x] **Wire fetch() to OperationsHandle** — Runner can now intercept fetch via handle_fetch()
- [x] **Wire console to OperationsHandle** — Runner can now collect logs via handle_log()
- [ ] **ES Modules support** — `export default { fetch() {} }` style handlers

## Medium Priority

- [ ] **Bindings JS API** — Expose `env.KV`, `env.DB`, `env.STORAGE` to JS
- [ ] **CPU/memory limits** — Currently `limits` parameter is ignored
- [ ] **Improve streaming** — Don't buffer entire RequestBody for streams

## Low Priority

- [ ] **TextEncoder/TextDecoder** — Currently using workarounds
- [ ] **Benchmark suite** — Automated perf comparison with V8

## Won't Do (N/A for QuickJS)

- Isolate pooling — Not needed, Context creation is cheap
- Thread pinning — Single-threaded interpreter
- GC tracking — Automatic reference counting
