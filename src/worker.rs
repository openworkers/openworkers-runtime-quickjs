use crate::compat::{RuntimeLimits, Script, TerminationReason};
use crate::task::{HttpRequest, HttpResponse, ResponseBody, Task};
use bytes::Bytes;
use rquickjs::{
    AsyncContext, AsyncRuntime, Function, Object, async_with, prelude::Async, promise::Promise,
};
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

    // ReadableStream implementation (simplified WHATWG spec)
    globalThis.ReadableStream = class ReadableStream {
        constructor(underlyingSource = {}) {
            this._underlyingSource = underlyingSource;
            this._controller = null;
            this._reader = null;
            this._state = 'readable'; // 'readable', 'closed', 'errored'
            this._storedError = null;

            // Create controller
            const controller = new ReadableStreamDefaultController(this);
            this._controller = controller;

            // Start the stream
            if (underlyingSource.start) {
                const startPromise = Promise.resolve(underlyingSource.start(controller));
                startPromise.catch(e => {
                    controller.error(e);
                });
            }
        }

        getReader() {
            if (this._reader) {
                throw new TypeError('ReadableStream is locked to a reader');
            }
            const reader = new ReadableStreamDefaultReader(this);
            this._reader = reader;
            return reader;
        }

        cancel(reason) {
            if (this._state === 'closed') {
                return Promise.resolve();
            }
            if (this._state === 'errored') {
                return Promise.reject(this._storedError);
            }

            this._state = 'closed';

            if (this._reader) {
                this._reader._closePending();
                this._reader = null;
            }

            if (this._underlyingSource.cancel) {
                return Promise.resolve(this._underlyingSource.cancel(reason));
            }

            return Promise.resolve();
        }

        get locked() {
            return this._reader !== null;
        }
    };

    // ReadableStreamDefaultController
    globalThis.ReadableStreamDefaultController = class ReadableStreamDefaultController {
        constructor(stream) {
            this._stream = stream;
            this._queue = [];
            this._closeRequested = false;
        }

        enqueue(chunk) {
            if (this._closeRequested) {
                throw new TypeError('Cannot enqueue after close');
            }
            if (this._stream._state !== 'readable') {
                throw new TypeError('Stream is not in readable state');
            }

            this._queue.push({ type: 'chunk', value: chunk });
            this._processQueue();
        }

        close() {
            if (this._closeRequested) {
                throw new TypeError('Stream is already closing');
            }
            if (this._stream._state !== 'readable') {
                throw new TypeError('Stream is not in readable state');
            }

            this._closeRequested = true;
            this._queue.push({ type: 'close' });
            this._processQueue();
        }

        error(error) {
            if (this._stream._state !== 'readable') {
                return;
            }

            this._stream._state = 'errored';
            this._stream._storedError = error;

            if (this._stream._reader) {
                this._stream._reader._errorPending(error);
            }

            this._queue = [];
        }

        _processQueue() {
            if (this._stream._reader) {
                this._stream._reader._processQueue();
            }
        }

        get desiredSize() {
            if (this._stream._state === 'errored') {
                return null;
            }
            if (this._stream._state === 'closed') {
                return 0;
            }
            return Math.max(0, 1 - this._queue.length);
        }
    };

    // ReadableStreamDefaultReader
    globalThis.ReadableStreamDefaultReader = class ReadableStreamDefaultReader {
        constructor(stream) {
            if (stream._reader) {
                throw new TypeError('Stream is already locked');
            }

            this._stream = stream;
            this._readRequests = [];
            this._closedPromise = null;
            this._closedPromiseResolve = null;
            this._closedPromiseReject = null;

            this._closedPromise = new Promise((resolve, reject) => {
                this._closedPromiseResolve = resolve;
                this._closedPromiseReject = reject;
            });
        }

        read() {
            if (!this._stream) {
                return Promise.reject(new TypeError('Reader is released'));
            }

            if (this._stream._state === 'errored') {
                return Promise.reject(this._stream._storedError);
            }

            const controller = this._stream._controller;

            if (controller._queue.length > 0) {
                const item = controller._queue.shift();

                if (item.type === 'close') {
                    this._stream._state = 'closed';
                    this._closePending();
                    return Promise.resolve({ done: true, value: undefined });
                }

                return Promise.resolve({ done: false, value: item.value });
            }

            if (this._stream._state === 'closed') {
                return Promise.resolve({ done: true, value: undefined });
            }

            const underlyingSource = this._stream._underlyingSource;
            if (underlyingSource && underlyingSource.pull) {
                return new Promise((resolve, reject) => {
                    this._readRequests.push({ resolve, reject });
                    const pullPromise = underlyingSource.pull(controller);
                    if (pullPromise && typeof pullPromise.then === 'function') {
                        pullPromise.catch(e => {
                            controller.error(e);
                        });
                    }
                });
            }

            return new Promise((resolve, reject) => {
                this._readRequests.push({ resolve, reject });
            });
        }

        _processQueue() {
            const controller = this._stream._controller;

            while (this._readRequests.length > 0 && controller._queue.length > 0) {
                const request = this._readRequests.shift();
                const item = controller._queue.shift();

                if (item.type === 'close') {
                    this._stream._state = 'closed';
                    request.resolve({ done: true, value: undefined });
                    this._closePending();
                    break;
                } else {
                    request.resolve({ done: false, value: item.value });
                }
            }

            if (this._stream._state === 'closed' && this._readRequests.length > 0) {
                while (this._readRequests.length > 0) {
                    const request = this._readRequests.shift();
                    request.resolve({ done: true, value: undefined });
                }
            }
        }

        _closePending() {
            if (this._closedPromiseResolve) {
                this._closedPromiseResolve();
                this._closedPromiseResolve = null;
            }
        }

        _errorPending(error) {
            while (this._readRequests.length > 0) {
                const request = this._readRequests.shift();
                request.reject(error);
            }

            if (this._closedPromiseReject) {
                this._closedPromiseReject(error);
                this._closedPromiseReject = null;
            }
        }

        releaseLock() {
            if (!this._stream) {
                return;
            }

            if (this._readRequests.length > 0) {
                throw new TypeError('Cannot release lock while read requests are pending');
            }

            this._stream._reader = null;
            this._stream = null;
        }

        cancel(reason) {
            if (!this._stream) {
                return Promise.reject(new TypeError('Reader is released'));
            }

            const cancelPromise = this._stream.cancel(reason);
            this.releaseLock();
            return cancelPromise;
        }

        get closed() {
            return this._closedPromise;
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
            this._bodyUsed = false;
            this._isStream = false;

            // Handle body
            if (body === null || body === undefined) {
                this._body = null;
            } else if (body instanceof ReadableStream) {
                this._body = body;
                this._isStream = true;
            } else if (typeof body === 'string') {
                this._body = body;
            } else if (body instanceof Uint8Array) {
                this._body = body;
            } else {
                this._body = String(body);
            }
        }

        get body() {
            if (this._isStream) {
                return this._body;
            }
            // Convert non-stream body to ReadableStream
            if (this._body === null) {
                return null;
            }
            const content = this._body;
            return new ReadableStream({
                start(controller) {
                    if (typeof content === 'string') {
                        controller.enqueue(new TextEncoder().encode(content));
                    } else if (content instanceof Uint8Array) {
                        controller.enqueue(content);
                    }
                    controller.close();
                }
            });
        }

        get bodyUsed() {
            return this._bodyUsed;
        }

        async text() {
            if (this._bodyUsed) {
                throw new TypeError('Body has already been consumed');
            }
            this._bodyUsed = true;

            if (this._body === null) return '';

            if (this._isStream) {
                // Read entire stream
                const reader = this._body.getReader();
                const chunks = [];
                while (true) {
                    const { done, value } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                // Concatenate chunks
                const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
                const result = new Uint8Array(totalLength);
                let offset = 0;
                for (const chunk of chunks) {
                    result.set(chunk, offset);
                    offset += chunk.length;
                }
                return new TextDecoder().decode(result);
            }

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
            if (this._bodyUsed) {
                throw new TypeError('Body has already been consumed');
            }
            this._bodyUsed = true;

            if (this._body === null) return new ArrayBuffer(0);

            if (this._isStream) {
                const reader = this._body.getReader();
                const chunks = [];
                while (true) {
                    const { done, value } = await reader.read();
                    if (done) break;
                    chunks.push(value);
                }
                const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
                const result = new Uint8Array(totalLength);
                let offset = 0;
                for (const chunk of chunks) {
                    result.set(chunk, offset);
                    offset += chunk.length;
                }
                return result.buffer;
            }

            if (this._body instanceof Uint8Array) {
                return this._body.buffer;
            }
            const encoder = new TextEncoder();
            const text = typeof this._body === 'string' ? this._body : String(this._body);
            return encoder.encode(text).buffer;
        }

        clone() {
            if (this._bodyUsed) {
                throw new TypeError('Cannot clone a used body');
            }
            return new Response(this._body, {
                status: this.status,
                statusText: this.statusText,
                headers: this.headers
            });
        }

        // Helper to check if response is streaming
        _isStreamingResponse() {
            return this._isStream;
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

    // ScheduledEvent class
    class ScheduledEvent {
        constructor(scheduledTime) {
            this.type = 'scheduled';
            this.scheduledTime = scheduledTime;
            this._waitUntilPromises = [];
        }

        waitUntil(promise) {
            this._waitUntilPromises.push(promise);
        }
    }

    globalThis.ScheduledEvent = ScheduledEvent;

    // Global fetch function
    globalThis.fetch = async function(url, options) {
        options = options || {};

        // Handle Request object
        if (url instanceof Request) {
            options = {
                method: url.method,
                headers: url.headers,
                body: url._body,
                ...options
            };
            url = url.url;
        }

        const method = (options.method || 'GET').toUpperCase();
        const headers = {};

        if (options.headers) {
            if (options.headers instanceof Headers) {
                options.headers.forEach((value, key) => {
                    headers[key] = value;
                });
            } else {
                Object.assign(headers, options.headers);
            }
        }

        const body = options.body || null;

        // Call native fetch
        const result = await __native_fetch(JSON.stringify({
            url: url,
            method: method,
            headers: headers,
            body: body
        }));

        const data = JSON.parse(result);

        if (data.error) {
            throw new Error(data.error);
        }

        return new Response(data.body, {
            status: data.status,
            statusText: data.statusText || 'OK',
            headers: data.headers
        });
    };

    // Dispatch fetch event
    globalThis.__dispatchFetch = async function(request) {
        const event = new FetchEvent(new Request(request.url, {
            method: request.method,
            headers: request.headers,
            body: request.body
        }));

        for (const handler of globalThis.__eventListeners.fetch) {
            // Await the handler in case it's async
            await handler(event);
        }

        if (event._responded && event._response) {
            return await event._response;
        }

        return new Response('No response from worker', { status: 500 });
    };

    // Dispatch scheduled event
    globalThis.__dispatchScheduled = async function(scheduledTime) {
        const event = new ScheduledEvent(scheduledTime);

        for (const handler of globalThis.__eventListeners.scheduled) {
            // Await the handler in case it's async
            await handler(event);
        }

        // Wait for all waitUntil promises
        if (event._waitUntilPromises.length > 0) {
            await Promise.all(event._waitUntilPromises);
        }

        return { success: true };
    };
"#;

/// Native fetch implementation
async fn native_fetch(options_json: String) -> String {
    #[derive(serde::Deserialize)]
    struct FetchOptions {
        url: String,
        method: String,
        headers: HashMap<String, String>,
        body: Option<String>,
    }

    #[derive(serde::Serialize)]
    struct FetchResult {
        status: u16,
        #[serde(rename = "statusText")]
        status_text: String,
        headers: HashMap<String, String>,
        body: String,
        error: Option<String>,
    }

    let options: FetchOptions = match serde_json::from_str(&options_json) {
        Ok(o) => o,
        Err(e) => {
            return serde_json::to_string(&FetchResult {
                status: 0,
                status_text: String::new(),
                headers: HashMap::new(),
                body: String::new(),
                error: Some(format!("Invalid fetch options: {}", e)),
            })
            .unwrap();
        }
    };

    let client = reqwest::Client::new();

    let mut request = match options.method.as_str() {
        "GET" => client.get(&options.url),
        "POST" => client.post(&options.url),
        "PUT" => client.put(&options.url),
        "DELETE" => client.delete(&options.url),
        "PATCH" => client.patch(&options.url),
        "HEAD" => client.head(&options.url),
        _ => client.get(&options.url),
    };

    // Add headers
    for (key, value) in &options.headers {
        request = request.header(key, value);
    }

    // Add body if present
    if let Some(body) = options.body {
        request = request.body(body);
    }

    // Execute request
    match request.send().await {
        Ok(response) => {
            let status = response.status().as_u16();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or("OK")
                .to_string();

            // Extract headers
            let mut headers = HashMap::new();
            for (key, value) in response.headers() {
                if let Ok(v) = value.to_str() {
                    headers.insert(key.to_string(), v.to_string());
                }
            }

            // Get body
            match response.text().await {
                Ok(body) => serde_json::to_string(&FetchResult {
                    status,
                    status_text,
                    headers,
                    body,
                    error: None,
                })
                .unwrap(),
                Err(e) => serde_json::to_string(&FetchResult {
                    status: 0,
                    status_text: String::new(),
                    headers: HashMap::new(),
                    body: String::new(),
                    error: Some(format!("Failed to read response body: {}", e)),
                })
                .unwrap(),
            }
        }
        Err(e) => serde_json::to_string(&FetchResult {
            status: 0,
            status_text: String::new(),
            headers: HashMap::new(),
            body: String::new(),
            error: Some(format!("Fetch failed: {}", e)),
        })
        .unwrap(),
    }
}

/// Worker that executes JavaScript code
pub struct Worker {
    runtime: AsyncRuntime,
    context: AsyncContext,
}

impl Worker {
    /// Create a new worker with the given script
    pub async fn new(
        script: Script,
        _log_tx: Option<std::sync::mpsc::Sender<crate::compat::LogEvent>>,
        _limits: Option<RuntimeLimits>,
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

            // Setup native fetch function (returns a Promise)
            let fetch_fn = Function::new(ctx.clone(), Async(native_fetch))
                .map_err(|e| format!("Failed to create fetch function: {}", e))?;
            global.set("__native_fetch", fetch_fn)
                .map_err(|e| format!("Failed to set __native_fetch: {}", e))?;

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
    pub async fn exec(&mut self, mut task: Task) -> Result<TerminationReason, String> {
        match &mut task {
            Task::Fetch(init_opt) => {
                let init = init_opt
                    .take()
                    .ok_or_else(|| "FetchInit already taken".to_string())?;
                match self.handle_fetch(init.req).await {
                    Ok(response) => {
                        let _ = init.res_tx.send(response);
                        Ok(TerminationReason::Success)
                    }
                    Err(_) => Ok(TerminationReason::Exception),
                }
            }
            Task::Scheduled(init_opt) => {
                let init = init_opt
                    .take()
                    .ok_or_else(|| "ScheduledInit already taken".to_string())?;
                match self.handle_scheduled(init.time).await {
                    Ok(()) => {
                        let _ = init.res_tx.send(());
                        Ok(TerminationReason::Success)
                    }
                    Err(_) => Ok(TerminationReason::Exception),
                }
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

            // Extract headers as Vec<(String, String)>
            let mut headers = Vec::new();
            if let Ok(headers_obj) = response.get::<_, Object>("headers") {
                if let Ok(internal) = headers_obj.get::<_, Object>("_headers") {
                    for key in internal.keys::<String>() {
                        if let Ok(key) = key {
                            if let Ok(value) = internal.get::<_, String>(&key) {
                                headers.push((key, value));
                            }
                        }
                    }
                }
            }

            // Check if response is a stream
            let is_stream: bool = response.get("_isStream").unwrap_or(false);

            let body = if is_stream {
                // For streaming responses, read the stream using JS helper
                // Set the response on globalThis temporarily for the helper to access
                let global = ctx.globals();
                global.set("__streamResponse", response.clone())
                    .map_err(|e| format!("Failed to set __streamResponse: {}", e))?;

                let read_stream_code = r#"
                    (async () => {
                        const stream = __streamResponse._body;
                        const reader = stream.getReader();
                        const chunks = [];
                        while (true) {
                            const { done, value } = await reader.read();
                            if (done) break;
                            if (value) chunks.push(value);
                        }
                        // Concatenate all chunks
                        const totalLength = chunks.reduce((acc, chunk) => acc + chunk.length, 0);
                        const result = new Uint8Array(totalLength);
                        let offset = 0;
                        for (const chunk of chunks) {
                            result.set(chunk, offset);
                            offset += chunk.length;
                        }
                        return result;
                    })()
                "#;

                let read_promise: Promise = ctx.eval(read_stream_code.as_bytes())
                    .map_err(|e| format!("Failed to eval stream read: {}", e))?;
                let result_array: rquickjs::TypedArray<u8> = read_promise.into_future().await
                    .map_err(|e| format!("Failed to read stream: {}", e))?;

                let all_data: Vec<u8> = result_array.as_bytes().unwrap_or(&[]).to_vec();

                // Clean up
                global.remove("__streamResponse")
                    .map_err(|e| format!("Failed to remove __streamResponse: {}", e))?;

                ResponseBody::Bytes(Bytes::from(all_data))
            } else if let Ok(raw_body) = response.get::<_, String>("_body") {
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
    async fn handle_scheduled(&self, time: u64) -> Result<(), String> {
        async_with!(self.context => |ctx| {
            let dispatch_code = format!(r#"__dispatchScheduled({})"#, time);

            // Dispatch and await the scheduled event
            let promise: Promise = ctx.eval(dispatch_code.as_bytes())
                .map_err(|e| format!("Failed to dispatch scheduled: {}", e))?;

            let _result: Object = promise.into_future().await
                .map_err(|e| format!("Scheduled handler failed: {}", e))?;

            Ok(())
        })
        .await
    }
}
