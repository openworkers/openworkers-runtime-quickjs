use bytes::Bytes;
use openworkers_core::{
    Event, HttpRequest, HttpResponse, LogLevel, OperationsHandle, RequestBody, ResponseBody,
    RuntimeLimits, Script, TaskResult, TerminationReason,
};
use rquickjs::{
    Array, AsyncContext, AsyncRuntime, Ctx, Function, IntoJs, Object, TypedArray, Value,
    async_with, prelude::Async, promise::Promise,
};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const RESPONSE_STREAM_BUFFER_SIZE: usize = 16;

/// JavaScript bindings for the worker
const RUNTIME_JS: &str = r#"
    // Event listeners storage
    globalThis.__eventListeners = {
        fetch: [],
        scheduled: []
    };

    globalThis.__pendingWork = [];

    // addEventListener implementation
    globalThis.addEventListener = function(type, handler) {
        if (globalThis.__eventListeners[type]) {
            globalThis.__eventListeners[type].push(handler);
        }
    };

    // Console implementation - each method calls its native function
    const __formatArgs = (args) => args.map(a =>
        typeof a === 'object' ? JSON.stringify(a) : String(a)
    ).join(' ');

    globalThis.console = {
        log: (...args) => __console_log(__formatArgs(args)),
        warn: (...args) => __console_warn(__formatArgs(args)),
        error: (...args) => __console_error(__formatArgs(args)),
        info: (...args) => __console_info(__formatArgs(args)),
        debug: (...args) => __console_debug(__formatArgs(args))
    };

    globalThis.DOMException = class DOMException extends Error {
        constructor(message, name) {
            super(message);
            this.name = name === undefined ? 'Error' : String(name);
        }
    };

    // Headers class; entries are kept as a list so duplicates (Set-Cookie) survive
    globalThis.Headers = class Headers {
        constructor(init) {
            this._list = [];
            if (init instanceof Headers) {
                for (const [name, value] of init._list) {
                    this._list.push([name, value]);
                }
            } else if (Array.isArray(init)) {
                for (const [name, value] of init) {
                    this.append(name, value);
                }
            } else if (init && typeof init === 'object') {
                for (const name in init) {
                    this.append(name, init[name]);
                }
            }
        }

        get(name) {
            const key = String(name).toLowerCase();
            const values = this._list.filter(e => e[0] === key).map(e => e[1]);
            return values.length > 0 ? values.join(', ') : null;
        }

        set(name, value) {
            const key = String(name).toLowerCase();
            this._list = this._list.filter(e => e[0] !== key);
            this._list.push([key, String(value)]);
        }

        has(name) {
            const key = String(name).toLowerCase();
            return this._list.some(e => e[0] === key);
        }

        delete(name) {
            const key = String(name).toLowerCase();
            this._list = this._list.filter(e => e[0] !== key);
        }

        append(name, value) {
            this._list.push([String(name).toLowerCase(), String(value)]);
        }

        getSetCookie() {
            return this._list.filter(e => e[0] === 'set-cookie').map(e => e[1]);
        }

        // Insertion order rather than the spec sorted, combined view, so duplicates stay separate
        *entries() {
            for (const [name, value] of this._list) {
                yield [name, value];
            }
        }

        *keys() {
            for (const [name] of this._list) {
                yield name;
            }
        }

        *values() {
            for (const [, value] of this._list) {
                yield value;
            }
        }

        [Symbol.iterator]() {
            return this.entries();
        }

        forEach(callback, thisArg) {
            for (const [name, value] of this._list) {
                callback.call(thisArg, value, name, this);
            }
        }
    };

    // Entry list rather than a map, so a repeated field name keeps every value
    globalThis.FormData = class FormData {
        constructor() {
            this._list = [];
        }

        append(name, value) {
            this._list.push([String(name), String(value)]);
        }

        set(name, value) {
            name = String(name);
            const index = this._list.findIndex(entry => entry[0] === name);

            if (index === -1) {
                this._list.push([name, String(value)]);
                return;
            }

            this._list[index][1] = String(value);
            this._list = this._list.filter((entry, i) => i <= index || entry[0] !== name);
        }

        get(name) {
            name = String(name);
            const entry = this._list.find(entry => entry[0] === name);
            return entry ? entry[1] : null;
        }

        getAll(name) {
            name = String(name);
            return this._list.filter(entry => entry[0] === name).map(entry => entry[1]);
        }

        has(name) {
            name = String(name);
            return this._list.some(entry => entry[0] === name);
        }

        delete(name) {
            name = String(name);
            this._list = this._list.filter(entry => entry[0] !== name);
        }

        *entries() {
            for (const [name, value] of this._list) {
                yield [name, value];
            }
        }

        *keys() {
            for (const [name] of this._list) {
                yield name;
            }
        }

        *values() {
            for (const [, value] of this._list) {
                yield value;
            }
        }

        [Symbol.iterator]() {
            return this.entries();
        }

        forEach(callback, thisArg) {
            for (const [name, value] of this._list) {
                callback.call(thisArg, value, name, this);
            }
        }
    };

    // multipart is out: without Blob and File a file part has nothing to land in
    const __formData = (contentType, text) => {
        const type = String(contentType || '').split(';')[0].trim().toLowerCase();

        if (type !== 'application/x-www-form-urlencoded') {
            throw new TypeError('formData() supports application/x-www-form-urlencoded only, got ' +
                (type || 'no content-type'));
        }

        const form = new FormData();

        for (const [name, value] of __urlencoded_parse(text)) {
            form.append(name, value);
        }

        return form;
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
            this.statusText = init.statusText === undefined ? '' : String(init.statusText);
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
            } else if (body instanceof ArrayBuffer) {
                this._body = new Uint8Array(body);
            } else if (ArrayBuffer.isView(body)) {
                this._body = new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
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

        async formData() {
            return __formData(this.headers.get('content-type'), await this.text());
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
                // Slice, or a view over part of a buffer would hand out the whole buffer
                const bytes = this._body;
                return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
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

            this._bodyUsed = false;
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

            if (this._body instanceof Uint8Array) {
                return new TextDecoder().decode(this._body);
            }

            return String(this._body);
        }

        async json() {
            const text = await this.text();
            return JSON.parse(text);
        }

        async formData() {
            return __formData(this.headers.get('content-type'), await this.text());
        }

        async arrayBuffer() {
            if (this._bodyUsed) {
                throw new TypeError('Body has already been consumed');
            }
            this._bodyUsed = true;

            if (this._body === null) return new ArrayBuffer(0);

            if (this._body instanceof Uint8Array) {
                // Slice, or a view over part of a buffer would hand out the whole buffer
                const bytes = this._body;
                return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
            }

            return new TextEncoder().encode(String(this._body)).buffer;
        }

        clone() {
            if (this._bodyUsed) {
                throw new TypeError('Cannot clone a used body');
            }
            return new Request(this.url, {
                method: this.method,
                headers: this.headers,
                body: this._body
            });
        }
    };

    // FetchEvent class
    class FetchEvent {
        constructor(request) {
            this.type = 'fetch';
            this.request = request;
            this._response = null;
            this._responded = false;
            this._waitUntilPromises = [];
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
            this._waitUntilPromises.push(promise);
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

    // Bytes for the native call; a stream is drained because ops take one buffer
    const __fetchBody = async (body) => {
        if (body === null || body === undefined) {
            return null;
        }

        if (body instanceof Uint8Array) {
            return body;
        }

        if (body instanceof ArrayBuffer) {
            return new Uint8Array(body);
        }

        if (ArrayBuffer.isView(body)) {
            return new Uint8Array(body.buffer, body.byteOffset, body.byteLength);
        }

        if (body instanceof ReadableStream) {
            const reader = body.getReader();
            const chunks = [];
            let length = 0;

            while (true) {
                const { done, value } = await reader.read();

                if (done) {
                    break;
                }

                const chunk = typeof value === 'string' ? new TextEncoder().encode(value) : value;
                chunks.push(chunk);
                length += chunk.length;
            }

            const joined = new Uint8Array(length);
            let offset = 0;

            for (const chunk of chunks) {
                joined.set(chunk, offset);
                offset += chunk.length;
            }

            return joined;
        }

        return new TextEncoder().encode(String(body));
    };

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

        // A pair list, so appended duplicates reach the host instead of overwriting each other
        const headers = [];

        if (options.headers instanceof Headers || Array.isArray(options.headers)) {
            for (const [name, value] of options.headers) {
                headers.push([String(name), String(value)]);
            }
        } else if (options.headers) {
            for (const name of Object.keys(options.headers)) {
                headers.push([name, String(options.headers[name])]);
            }
        }

        const result = await __native_fetch(JSON.stringify({
            url: url,
            method: method,
            headers: headers
        }), await __fetchBody(options.body));

        if (result.error) {
            throw new Error(result.error);
        }

        return new Response(result.body, {
            status: result.status,
            statusText: result.statusText,
            headers: result.headers
        });
    };

    // The body travels beside the init, where JSON cannot corrupt it
    globalThis.__dispatchFetch = async function(init, body) {
        const event = new FetchEvent(new Request(init.url, {
            method: init.method,
            headers: init.headers,
            body: body ?? null
        }));

        globalThis.__pendingWork = event._waitUntilPromises;

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

    // Called once the result is out; a timer left armed would otherwise fire inside the next task
    globalThis.__drainPendingWork = async function() {
        const pending = globalThis.__pendingWork;
        globalThis.__pendingWork = [];

        try {
            await Promise.all(pending);
        } catch (e) {
            console.error('waitUntil rejected:', e);
        }

        __cancelTimers();
    };
"#;

/// Fetch options from JS; the body travels beside them, where JSON cannot corrupt it
#[derive(serde::Deserialize)]
struct FetchOptions {
    url: String,
    method: String,
    headers: Vec<(String, String)>,
}

/// Core carries one value per request header, so repeats are joined as HTTP joins them
fn combine_headers(pairs: Vec<(String, String)>) -> HashMap<String, String> {
    let mut headers: HashMap<String, String> = HashMap::new();

    for (name, value) in pairs {
        match headers.entry(name.to_lowercase()) {
            Entry::Occupied(mut entry) => {
                let separator = if entry.key() == "cookie" { "; " } else { ", " };
                let combined = entry.get_mut();

                combined.push_str(separator);
                combined.push_str(&value);
            }
            Entry::Vacant(entry) => {
                entry.insert(value);
            }
        }
    }

    headers
}

/// Fetch result for JS; headers are name/value pairs so duplicates survive.
struct FetchResult {
    status: u16,
    status_text: String,
    headers: Vec<(String, String)>,
    body: Option<Bytes>,
    error: Option<String>,
}

impl FetchResult {
    fn failed(error: String) -> Self {
        Self {
            status: 0,
            status_text: String::new(),
            headers: Vec::new(),
            body: None,
            error: Some(error),
        }
    }
}

impl<'js> IntoJs<'js> for FetchResult {
    fn into_js(self, ctx: &Ctx<'js>) -> rquickjs::Result<Value<'js>> {
        let result = Object::new(ctx.clone())?;

        result.set("status", self.status)?;
        result.set("statusText", self.status_text)?;
        result.set("error", self.error)?;

        let headers = Array::new(ctx.clone())?;

        for (index, (name, value)) in self.headers.into_iter().enumerate() {
            let pair = Array::new(ctx.clone())?;
            pair.set(0, name)?;
            pair.set(1, value)?;
            headers.set(index, pair)?;
        }

        result.set("headers", headers)?;

        let body = self
            .body
            .map(|bytes| TypedArray::new(ctx.clone(), bytes.to_vec()))
            .transpose()?;
        result.set("body", body)?;

        Ok(result.into_value())
    }
}

/// The registered reason phrase, empty for a status code that has none
fn status_text(status: u16) -> &'static str {
    match status {
        100 => "Continue",
        101 => "Switching Protocols",
        102 => "Processing",
        103 => "Early Hints",
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        203 => "Non-Authoritative Information",
        204 => "No Content",
        205 => "Reset Content",
        206 => "Partial Content",
        207 => "Multi-Status",
        208 => "Already Reported",
        226 => "IM Used",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        305 => "Use Proxy",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        401 => "Unauthorized",
        402 => "Payment Required",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        406 => "Not Acceptable",
        407 => "Proxy Authentication Required",
        408 => "Request Timeout",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        412 => "Precondition Failed",
        413 => "Content Too Large",
        414 => "URI Too Long",
        415 => "Unsupported Media Type",
        416 => "Range Not Satisfiable",
        417 => "Expectation Failed",
        421 => "Misdirected Request",
        422 => "Unprocessable Content",
        423 => "Locked",
        424 => "Failed Dependency",
        425 => "Too Early",
        426 => "Upgrade Required",
        428 => "Precondition Required",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        451 => "Unavailable For Legal Reasons",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        505 => "HTTP Version Not Supported",
        506 => "Variant Also Negotiates",
        507 => "Insufficient Storage",
        508 => "Loop Detected",
        510 => "Not Extended",
        511 => "Network Authentication Required",
        _ => "",
    }
}

/// Native fetch implementation using OperationsHandle
async fn do_fetch(ops: OperationsHandle, options_json: String, body: Option<Bytes>) -> FetchResult {
    let options: FetchOptions = match serde_json::from_str(&options_json) {
        Ok(o) => o,
        Err(e) => return FetchResult::failed(format!("Invalid fetch options: {}", e)),
    };

    // Convert to HttpRequest for OperationsHandle
    let method = match options.method.as_str() {
        "GET" => openworkers_core::HttpMethod::Get,
        "POST" => openworkers_core::HttpMethod::Post,
        "PUT" => openworkers_core::HttpMethod::Put,
        "DELETE" => openworkers_core::HttpMethod::Delete,
        "PATCH" => openworkers_core::HttpMethod::Patch,
        "HEAD" => openworkers_core::HttpMethod::Head,
        "OPTIONS" => openworkers_core::HttpMethod::Options,
        _ => openworkers_core::HttpMethod::Get,
    };

    let request = HttpRequest {
        method,
        url: options.url,
        headers: combine_headers(options.headers),
        body: match body {
            Some(bytes) => RequestBody::Bytes(bytes),
            None => RequestBody::None,
        },
    };

    let response = match ops.handle_fetch(request).await {
        Ok(response) => response,
        Err(e) => return FetchResult::failed(e),
    };

    FetchResult {
        status: response.status,
        status_text: status_text(response.status).to_string(),
        headers: response.headers,
        body: response.body.collect().await,
        error: None,
    }
}

/// Bytes of a non-streaming response body, which JS holds as a string or a Uint8Array
fn buffered_body(response: &Object<'_>) -> Option<Bytes> {
    if let Ok(view) = response.get::<_, TypedArray<u8>>("_body") {
        return view.as_bytes().map(Bytes::copy_from_slice);
    }

    response.get::<_, String>("_body").ok().map(Bytes::from)
}

/// Worker that executes JavaScript code
pub struct Worker {
    context: AsyncContext,
    aborted: Arc<AtomicBool>,
}

impl Worker {
    /// Create a new worker with an OperationsHandler
    pub async fn new_with_ops(
        script: Script,
        _limits: Option<RuntimeLimits>,
        ops: OperationsHandle,
    ) -> Result<Self, TerminationReason> {
        let runtime = AsyncRuntime::new().map_err(|e| {
            TerminationReason::InitializationError(format!("Failed to create runtime: {}", e))
        })?;
        let context = AsyncContext::full(&runtime).await.map_err(|e| {
            TerminationReason::InitializationError(format!("Failed to create context: {}", e))
        })?;

        // Clone ops for use in closures
        let ops_log = ops.clone();
        let ops_warn = ops.clone();
        let ops_error = ops.clone();
        let ops_info = ops.clone();
        let ops_debug = ops.clone();
        let ops_fetch = ops.clone();

        // Initialize runtime bindings and evaluate script
        async_with!(context => |ctx| {
            // Setup native console functions that wire to OperationsHandle
            let global = ctx.globals();

            let log_fn = Function::new(ctx.clone(), move |msg: String| {
                ops_log.handle_log(LogLevel::Log, msg);
            }).map_err(|e| TerminationReason::InitializationError(format!("Failed to create console.log: {}", e)))?;
            global.set("__console_log", log_fn).map_err(|e| TerminationReason::InitializationError(format!("Failed to set __console_log: {}", e)))?;

            let warn_fn = Function::new(ctx.clone(), move |msg: String| {
                ops_warn.handle_log(LogLevel::Warn, msg);
            }).map_err(|e| TerminationReason::InitializationError(format!("Failed to create console.warn: {}", e)))?;
            global.set("__console_warn", warn_fn).map_err(|e| TerminationReason::InitializationError(format!("Failed to set __console_warn: {}", e)))?;

            let error_fn = Function::new(ctx.clone(), move |msg: String| {
                ops_error.handle_log(LogLevel::Error, msg);
            }).map_err(|e| TerminationReason::InitializationError(format!("Failed to create console.error: {}", e)))?;
            global.set("__console_error", error_fn).map_err(|e| TerminationReason::InitializationError(format!("Failed to set __console_error: {}", e)))?;

            let info_fn = Function::new(ctx.clone(), move |msg: String| {
                ops_info.handle_log(LogLevel::Info, msg);
            }).map_err(|e| TerminationReason::InitializationError(format!("Failed to create console.info: {}", e)))?;
            global.set("__console_info", info_fn).map_err(|e| TerminationReason::InitializationError(format!("Failed to set __console_info: {}", e)))?;

            let debug_fn = Function::new(ctx.clone(), move |msg: String| {
                ops_debug.handle_log(LogLevel::Debug, msg);
            }).map_err(|e| TerminationReason::InitializationError(format!("Failed to create console.debug: {}", e)))?;
            global.set("__console_debug", debug_fn).map_err(|e| TerminationReason::InitializationError(format!("Failed to set __console_debug: {}", e)))?;

            // Setup native fetch function that uses OperationsHandle
            let fetch_fn = Function::new(ctx.clone(), Async(move |options_json: String, body: Option<TypedArray<'_, u8>>| {
                let ops = ops_fetch.clone();
                let body = body.and_then(|view| view.as_bytes().map(Bytes::copy_from_slice));
                async move {
                    do_fetch(ops, options_json, body).await
                }
            }))
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to create fetch function: {}", e)))?;
            global.set("__native_fetch", fetch_fn)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to set __native_fetch: {}", e)))?;

            crate::runtime::setup_crypto(&ctx)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to setup crypto: {}", e)))?;

            crate::runtime::setup_url(&ctx)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to setup URL: {}", e)))?;

            crate::runtime::setup_base64(&ctx)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to setup base64: {}", e)))?;

            crate::runtime::setup_timers(&ctx)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to setup timers: {}", e)))?;

            crate::runtime::setup_text(&ctx)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to setup text encoding: {}", e)))?;

            // Evaluate runtime bindings
            ctx.eval::<(), _>(RUNTIME_JS)
                .map_err(|e| TerminationReason::InitializationError(format!("Failed to evaluate runtime JS: {}", e)))?;

            // Evaluate user script
            let js_code = script.code.as_js().ok_or_else(|| {
                TerminationReason::InitializationError(
                    "QuickJS runtime only supports JavaScript code".to_string(),
                )
            })?;
            ctx.eval::<(), _>(js_code)
                .map_err(|e| TerminationReason::Exception(format!("Script evaluation failed: {}", e)))?;

            Ok::<(), TerminationReason>(())
        })
        .await?;

        Ok(Self {
            context,
            aborted: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a new worker whose operations are stubbed out by `DefaultOps`
    ///
    /// For real fetch support, use `new_with_ops()` with a custom OperationsHandler.
    pub async fn new(
        script: Script,
        limits: Option<RuntimeLimits>,
    ) -> Result<Self, TerminationReason> {
        let ops: OperationsHandle = Arc::new(openworkers_core::DefaultOps);
        Self::new_with_ops(script, limits, ops).await
    }

    /// Refuse any task started from now on
    ///
    /// A script that is already running keeps running: QuickJS has no interrupt
    /// mechanism wired up here.
    pub fn abort(&mut self) {
        self.aborted.store(true, Ordering::SeqCst);
    }

    /// Execute a task
    pub async fn exec(&mut self, mut task: Event) -> Result<(), TerminationReason> {
        // Check if aborted before starting
        if self.aborted.load(Ordering::SeqCst) {
            return Err(TerminationReason::Aborted);
        }

        match &mut task {
            Event::Fetch(init_opt) => {
                let init = init_opt.take().ok_or_else(|| {
                    TerminationReason::Other("FetchInit already taken".to_string())
                })?;
                let response = self.handle_fetch(init.req).await?;
                let _ = init.res_tx.send(response);
                self.drain_pending_work().await
            }
            Event::Task(init_opt) => {
                let init = init_opt.take().ok_or_else(|| {
                    TerminationReason::Other("TaskInit already taken".to_string())
                })?;

                // Extract scheduled time from source
                let scheduled_time = match &init.source {
                    Some(openworkers_core::TaskSource::Schedule { time }) => *time,
                    _ => 0,
                };

                match self.handle_scheduled(scheduled_time).await {
                    Ok(()) => {
                        let _ = init.res_tx.send(TaskResult::success());
                        self.drain_pending_work().await
                    }
                    Err(e) => {
                        let _ = init.res_tx.send(TaskResult::err(e.to_string()));
                        Err(e)
                    }
                }
            }
        }
    }

    /// Settle the `waitUntil` promises of the task that just answered, then cancel its timers
    ///
    /// The result is already on its way out, so this runs after it rather than delaying it.
    async fn drain_pending_work(&self) -> Result<(), TerminationReason> {
        async_with!(self.context => |ctx| {
            let promise: Promise = ctx.eval(b"__drainPendingWork()")
                .map_err(|e| TerminationReason::Exception(format!("Failed to drain pending work: {}", e)))?;

            promise.into_future::<()>().await
                .map_err(|e| TerminationReason::Exception(format!("Pending work failed: {}", e)))
        })
        .await
    }

    /// Handle a fetch event
    async fn handle_fetch(&self, request: HttpRequest) -> Result<HttpResponse, TerminationReason> {
        async_with!(self.context => |ctx| {
            let body = match &request.body {
                RequestBody::Bytes(b) => Some(b.clone()),
                RequestBody::None => None,
                RequestBody::Stream(_) => {
                    return Err(TerminationReason::Other(
                        "Streaming request bodies are not supported".to_string(),
                    ));
                }
            };

            let init_json = serde_json::json!({
                "method": request.method.to_string(),
                "url": request.url,
                "headers": request.headers,
            });

            let init = ctx.json_parse(init_json.to_string())
                .map_err(|e| TerminationReason::Exception(format!("Failed to build request init: {}", e)))?;

            // A typed array rather than a string, or a non-UTF-8 body would be mangled
            let body = body
                .map(|bytes| TypedArray::new(ctx.clone(), bytes.to_vec()))
                .transpose()
                .map_err(|e| TerminationReason::Exception(format!("Failed to build request body: {}", e)))?;

            let dispatch: Function = ctx.globals().get("__dispatchFetch")
                .map_err(|e| TerminationReason::Exception(format!("Failed to get __dispatchFetch: {}", e)))?;

            // Dispatch and get response
            let promise: Promise = dispatch.call((init, body))
                .map_err(|e| TerminationReason::Exception(format!("Failed to dispatch fetch: {}", e)))?;

            let response: Object = promise.into_future().await
                .map_err(|e| TerminationReason::Exception(format!("Fetch handler failed: {}", e)))?;

            // Extract response properties
            let status: i32 = response.get("status")
                .map_err(|e| TerminationReason::Exception(format!("Failed to get status: {}", e)))?;

            let mut headers: Vec<(String, String)> = Vec::new();
            if let Ok(headers_obj) = response.get::<_, Object>("headers") {
                let entries = headers_obj.get::<_, Vec<Array>>("_list").unwrap_or_default();

                for entry in entries {
                    let (Ok(name), Ok(value)) = (entry.get::<String>(0), entry.get::<String>(1)) else {
                        continue;
                    };

                    headers.push((name, value));
                }
            }

            // Check if response is a stream
            let is_stream: bool = response.get("_isStream").unwrap_or(false);

            let body = if is_stream {
                // Collect all chunks first (QuickJS context is not Send)
                let mut chunks: Vec<Vec<u8>> = Vec::new();

                // Set the response on globalThis temporarily for the helper to access
                let global = ctx.globals();
                global.set("__streamResponse", response.clone())
                    .map_err(|e| TerminationReason::Exception(format!("Failed to set __streamResponse: {}", e)))?;

                // Read chunks one by one
                let read_chunk_code = r#"
                    (async () => {
                        if (!globalThis.__streamReader) {
                            const stream = __streamResponse._body;
                            globalThis.__streamReader = stream.getReader();
                        }
                        const { done, value } = await globalThis.__streamReader.read();
                        if (done) {
                            delete globalThis.__streamReader;
                            return { done: true };
                        }
                        return { done: false, value: value };
                    })()
                "#;

                // Read all chunks into memory
                loop {
                    let read_promise: Promise = ctx.eval(read_chunk_code.as_bytes())
                        .map_err(|e| TerminationReason::Exception(format!("Failed to eval chunk read: {}", e)))?;
                    let result: Object = read_promise.into_future().await
                        .map_err(|e| TerminationReason::Exception(format!("Failed to read chunk: {}", e)))?;

                    let done: bool = result.get("done").unwrap_or(true);
                    if done {
                        break;
                    }

                    // Get value (Uint8Array)
                    if let Ok(value) = result.get::<_, TypedArray<u8>>("value") {
                        let chunk_data: Vec<u8> = value.as_bytes().unwrap_or(&[]).to_vec();
                        chunks.push(chunk_data);
                    }
                }

                // Clean up JS state
                global.remove("__streamResponse").ok();
                global.remove("__streamReader").ok();

                // Create channel and spawn task to send chunks
                let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(RESPONSE_STREAM_BUFFER_SIZE);

                tokio::spawn(async move {
                    for chunk in chunks {
                        if tx.send(Ok(Bytes::from(chunk))).await.is_err() {
                            break; // Receiver dropped
                        }
                    }
                    // tx drops here, closing the channel
                });

                ResponseBody::Stream(rx)
            } else if let Some(body_bytes) = buffered_body(&response) {
                // Convert buffered body to stream for consistency
                let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, String>>(1);

                tokio::spawn(async move {
                    let _ = tx.send(Ok(body_bytes)).await;
                    // tx drops here, closing the channel
                });

                ResponseBody::Stream(rx)
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
    async fn handle_scheduled(&self, time: u64) -> Result<(), TerminationReason> {
        async_with!(self.context => |ctx| {
            let dispatch_code = format!(r#"__dispatchScheduled({})"#, time);

            // Dispatch and await the scheduled event
            let promise: Promise = ctx.eval(dispatch_code.as_bytes())
                .map_err(|e| TerminationReason::Exception(format!("Failed to dispatch scheduled: {}", e)))?;

            let _result: Object = promise.into_future().await
                .map_err(|e| TerminationReason::Exception(format!("Scheduled handler failed: {}", e)))?;

            Ok(())
        })
        .await
    }
}

impl openworkers_core::Worker for Worker {
    async fn new(script: Script, limits: Option<RuntimeLimits>) -> Result<Self, TerminationReason> {
        Worker::new(script, limits).await
    }

    async fn exec(&mut self, task: Event) -> Result<(), TerminationReason> {
        Worker::exec(self, task).await
    }

    fn abort(&mut self) {
        Worker::abort(self)
    }
}
