use crate::compat::{RuntimeLimits, Script};
use crate::task::{HttpRequest, HttpResponse, ResponseBody, Task};
use bytes::Bytes;
use rquickjs::{AsyncContext, AsyncRuntime, Function, Object, async_with, promise::Promise};
use std::collections::HashMap;

/// JavaScript bindings for the worker
const RUNTIME_JS: &str = r#"
    // Event listeners storage
    globalThis.__eventListeners = {
        fetch: [],
        scheduled: []
    };

    // addEventListener implementation
    globalThis.addEventListener = function(type, handler) {
        if (globalThis.__eventListeners[type]) {
            globalThis.__eventListeners[type].push(handler);
        }
    };

    // Console implementation
    globalThis.console = {
        log: (...args) => __native_log(args.map(a =>
            typeof a === 'object' ? JSON.stringify(a) : String(a)
        ).join(' ')),
        warn: (...args) => __native_log('[WARN] ' + args.map(a =>
            typeof a === 'object' ? JSON.stringify(a) : String(a)
        ).join(' ')),
        error: (...args) => __native_log('[ERROR] ' + args.map(a =>
            typeof a === 'object' ? JSON.stringify(a) : String(a)
        ).join(' ')),
        info: (...args) => __native_log('[INFO] ' + args.map(a =>
            typeof a === 'object' ? JSON.stringify(a) : String(a)
        ).join(' ')),
        debug: (...args) => __native_log('[DEBUG] ' + args.map(a =>
            typeof a === 'object' ? JSON.stringify(a) : String(a)
        ).join(' '))
    };

    // Headers class
    globalThis.Headers = class Headers {
        constructor(init) {
            this._headers = {};
            if (init) {
                if (init instanceof Headers) {
                    for (const [key, value] of init.entries()) {
                        this._headers[key.toLowerCase()] = value;
                    }
                } else if (typeof init === 'object') {
                    for (const key in init) {
                        this._headers[key.toLowerCase()] = init[key];
                    }
                }
            }
        }

        get(name) {
            return this._headers[name.toLowerCase()] || null;
        }

        set(name, value) {
            this._headers[name.toLowerCase()] = value;
        }

        has(name) {
            return name.toLowerCase() in this._headers;
        }

        delete(name) {
            delete this._headers[name.toLowerCase()];
        }

        append(name, value) {
            const key = name.toLowerCase();
            if (this._headers[key]) {
                this._headers[key] += ', ' + value;
            } else {
                this._headers[key] = value;
            }
        }

        *entries() {
            for (const key in this._headers) {
                yield [key, this._headers[key]];
            }
        }

        forEach(callback) {
            for (const key in this._headers) {
                callback(this._headers[key], key, this);
            }
        }
    };

    // Response class
    globalThis.Response = class Response {
        constructor(body, init) {
            init = init || {};
            this.status = init.status || 200;
            this.statusText = init.statusText || 'OK';
            this.headers = new Headers(init.headers);
            this.ok = this.status >= 200 && this.status < 300;

            // Handle body
            if (body === null || body === undefined) {
                this._body = null;
            } else if (typeof body === 'string') {
                this._body = body;
            } else if (body instanceof Uint8Array) {
                this._body = body;
            } else {
                this._body = String(body);
            }
        }

        async text() {
            if (this._body === null) return '';
            if (typeof this._body === 'string') return this._body;
            if (this._body instanceof Uint8Array) {
                return new TextDecoder().decode(this._body);
            }
            return String(this._body);
        }

        async json() {
            const text = await this.text();
            return JSON.parse(text);
        }

        async arrayBuffer() {
            if (this._body === null) return new ArrayBuffer(0);
            if (this._body instanceof Uint8Array) {
                return this._body.buffer;
            }
            const encoder = new TextEncoder();
            return encoder.encode(await this.text()).buffer;
        }

        clone() {
            return new Response(this._body, {
                status: this.status,
                statusText: this.statusText,
                headers: this.headers
            });
        }

        // Helper to get raw body for Rust extraction
        _getRawBody() {
            return this._body;
        }
    };

    // Request class
    globalThis.Request = class Request {
        constructor(input, init) {
            init = init || {};
            if (input instanceof Request) {
                this.url = input.url;
                this.method = init.method || input.method;
                this.headers = new Headers(init.headers || input.headers);
                this._body = init.body !== undefined ? init.body : input._body;
            } else {
                this.url = String(input);
                this.method = (init.method || 'GET').toUpperCase();
                this.headers = new Headers(init.headers);
                this._body = init.body || null;
            }
        }

        async text() {
            if (this._body === null) return '';
            return String(this._body);
        }

        async json() {
            const text = await this.text();
            return JSON.parse(text);
        }

        clone() {
            return new Request(this.url, {
                method: this.method,
                headers: this.headers,
                body: this._body
            });
        }
    };

    // URL class
    globalThis.URL = class URL {
        constructor(url, base) {
            // Simple URL parsing
            const fullUrl = base ? new URL(base).origin + url : url;
            const match = fullUrl.match(/^(\w+):\/\/([^\/\?#]+)(\/[^\?#]*)?(\?[^#]*)?(#.*)?$/);
            if (match) {
                this.protocol = match[1] + ':';
                this.host = match[2];
                this.hostname = match[2].split(':')[0];
                this.port = match[2].split(':')[1] || '';
                this.pathname = match[3] || '/';
                this.search = match[4] || '';
                this.hash = match[5] || '';
                this.origin = this.protocol + '//' + this.host;
                this.href = fullUrl;
            } else {
                throw new Error('Invalid URL: ' + url);
            }
        }

        toString() {
            return this.href;
        }
    };

    // TextEncoder/TextDecoder
    globalThis.TextEncoder = class TextEncoder {
        encode(str) {
            const utf8 = unescape(encodeURIComponent(str));
            const result = new Uint8Array(utf8.length);
            for (let i = 0; i < utf8.length; i++) {
                result[i] = utf8.charCodeAt(i);
            }
            return result;
        }
    };

    globalThis.TextDecoder = class TextDecoder {
        decode(bytes) {
            if (!bytes) return '';
            let str = '';
            for (let i = 0; i < bytes.length; i++) {
                str += String.fromCharCode(bytes[i]);
            }
            return decodeURIComponent(escape(str));
        }
    };

    // FetchEvent class
    class FetchEvent {
        constructor(request) {
            this.type = 'fetch';
            this.request = request;
            this._response = null;
            this._responded = false;
        }

        respondWith(response) {
            this._responded = true;
            if (response instanceof Promise) {
                this._response = response;
            } else {
                this._response = Promise.resolve(response);
            }
        }

        waitUntil(promise) {
            // For now, just let it run
        }
    }

    globalThis.FetchEvent = FetchEvent;

    // Dispatch fetch event
    globalThis.__dispatchFetch = async function(request) {
        const event = new FetchEvent(new Request(request.url, {
            method: request.method,
            headers: request.headers,
            body: request.body
        }));

        for (const handler of globalThis.__eventListeners.fetch) {
            handler(event);
        }

        if (event._responded && event._response) {
            return await event._response;
        }

        return new Response('No response from worker', { status: 500 });
    };
"#;

/// Worker that executes JavaScript code
pub struct Worker {
    runtime: AsyncRuntime,
    context: AsyncContext,
}

impl Worker {
    /// Create a new worker with the given script
    pub async fn new(
        script: Script,
        _limits: Option<RuntimeLimits>,
        _log_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::compat::LogEvent>>,
    ) -> Result<Self, String> {
        let runtime =
            AsyncRuntime::new().map_err(|e| format!("Failed to create runtime: {}", e))?;
        let context = AsyncContext::full(&runtime)
            .await
            .map_err(|e| format!("Failed to create context: {}", e))?;

        // Initialize runtime bindings and evaluate script
        async_with!(context => |ctx| {
            // Setup native log function
            let global = ctx.globals();
            let log_fn = Function::new(ctx.clone(), |msg: String| {
                println!("[JS] {}", msg);
            }).map_err(|e| format!("Failed to create log function: {}", e))?;
            global.set("__native_log", log_fn).map_err(|e| format!("Failed to set __native_log: {}", e))?;

            // Evaluate runtime bindings
            ctx.eval::<(), _>(RUNTIME_JS)
                .map_err(|e| format!("Failed to evaluate runtime JS: {}", e))?;

            // Evaluate user script
            ctx.eval::<(), _>(script.code.as_str())
                .map_err(|e| format!("Failed to evaluate user script: {}", e))?;

            Ok::<(), String>(())
        })
        .await?;

        Ok(Self { runtime, context })
    }

    /// Execute a task
    pub async fn exec(&mut self, task: Task) -> Result<(), String> {
        match task {
            Task::Fetch {
                request,
                response_tx,
            } => {
                let response = self.handle_fetch(request).await?;
                let _ = response_tx.send(response);
                Ok(())
            }
            Task::Scheduled { cron, response_tx } => {
                let result = self.handle_scheduled(&cron).await;
                let _ = response_tx.send(result);
                Ok(())
            }
        }
    }

    /// Handle a fetch event
    async fn handle_fetch(&self, request: HttpRequest) -> Result<HttpResponse, String> {
        async_with!(self.context => |ctx| {
            // Build request object for JS
            let headers_json: String = serde_json::to_string(&request.headers)
                .unwrap_or_else(|_| "{}".to_string());

            let body_str = request.body
                .as_ref()
                .map(|b| String::from_utf8_lossy(b).to_string())
                .unwrap_or_default();

            let dispatch_code = format!(
                r#"__dispatchFetch({{
                    method: "{}",
                    url: "{}",
                    headers: {},
                    body: {}
                }})"#,
                request.method,
                request.url,
                headers_json,
                if body_str.is_empty() { "null".to_string() } else { format!("\"{}\"", body_str.replace("\"", "\\\"")) }
            );

            // Dispatch and get response
            let promise: Promise = ctx.eval(dispatch_code.as_bytes())
                .map_err(|e| format!("Failed to dispatch fetch: {}", e))?;

            let response: Object = promise.into_future().await
                .map_err(|e| format!("Fetch handler failed: {}", e))?;

            // Extract response properties
            let status: i32 = response.get("status")
                .map_err(|e| format!("Failed to get status: {}", e))?;

            // Extract headers
            let mut headers = HashMap::new();
            if let Ok(headers_obj) = response.get::<_, Object>("headers") {
                if let Ok(internal) = headers_obj.get::<_, Object>("_headers") {
                    for key in internal.keys::<String>() {
                        if let Ok(key) = key {
                            if let Ok(value) = internal.get::<_, String>(&key) {
                                headers.insert(key, value);
                            }
                        }
                    }
                }
            }

            // Extract body
            let body = if let Ok(raw_body) = response.get::<_, String>("_body") {
                ResponseBody::Bytes(Bytes::from(raw_body))
            } else {
                ResponseBody::None
            };

            Ok(HttpResponse {
                status: status as u16,
                headers,
                body,
            })
        })
        .await
    }

    /// Handle a scheduled event
    async fn handle_scheduled(&self, _cron: &str) -> Result<(), String> {
        // TODO: Implement scheduled event handling
        Ok(())
    }
}
