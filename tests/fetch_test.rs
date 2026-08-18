use bytes::Bytes;
use openworkers_core::{
    Event, HttpMethod, HttpRequest, HttpResponse, OpFuture, OperationsHandler, RequestBody,
    ResponseBody, Script,
};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;
use std::sync::Arc;

/// Mock handler that returns fixed responses for testing
struct MockOps;

impl OperationsHandler for MockOps {
    fn handle_fetch(&self, request: HttpRequest) -> OpFuture<'_, Result<HttpResponse, String>> {
        Box::pin(async move {
            // Return mock responses based on URL
            if request.url.contains("/get") {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![("content-type".to_string(), "application/json".to_string())],
                    body: ResponseBody::Bytes(Bytes::from(
                        r#"{"url":"https://example.com/get","data":"test"}"#,
                    )),
                })
            } else if request.url.contains("/post") {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![("content-type".to_string(), "application/json".to_string())],
                    body: ResponseBody::Bytes(Bytes::from(r#"{"json":{"hello":"world"}}"#)),
                })
            } else if request.url.contains("/headers") {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![("content-type".to_string(), "application/json".to_string())],
                    body: ResponseBody::Bytes(Bytes::from(
                        r#"{"headers":{"x-custom-header":"test-value"}}"#,
                    )),
                })
            } else if request.url.contains("/status/404") {
                Ok(HttpResponse {
                    status: 404,
                    headers: vec![],
                    body: ResponseBody::Bytes(Bytes::from("Not Found")),
                })
            } else {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![],
                    body: ResponseBody::Bytes(Bytes::from("OK")),
                })
            }
        })
    }
}

/// Handler sending the request body back as the response body
struct EchoOps;

impl OperationsHandler for EchoOps {
    fn handle_fetch(&self, request: HttpRequest) -> OpFuture<'_, Result<HttpResponse, String>> {
        Box::pin(async move {
            let body = match request.body {
                RequestBody::Bytes(bytes) => bytes,
                RequestBody::None => Bytes::new(),
                RequestBody::Stream(_) => return Err("streamed request body".to_string()),
            };

            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: ResponseBody::Bytes(body),
            })
        })
    }
}

/// Handler answering with the status code named at the end of the URL
struct StatusOps;

impl OperationsHandler for StatusOps {
    fn handle_fetch(&self, request: HttpRequest) -> OpFuture<'_, Result<HttpResponse, String>> {
        Box::pin(async move {
            let status = request
                .url
                .rsplit('/')
                .next()
                .and_then(|status| status.parse().ok())
                .expect("url should end with a status code");

            Ok(HttpResponse {
                status,
                headers: vec![],
                body: ResponseBody::None,
            })
        })
    }
}

/// Run a script and parse the JSON body it responds with
async fn respond_json(script: &str, ops: Arc<dyn OperationsHandler>) -> serde_json::Value {
    let mut worker = Worker::new_with_ops(Script::new(script), None, ops)
        .await
        .expect("Worker should initialize");

    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: RequestBody::None,
    };

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let body = response.body.collect().await.expect("Should have body");

    serde_json::from_slice(&body).expect("Should be valid JSON")
}

#[tokio::test]
async fn test_fetch_basic_get() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://example.com/get');
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                hasUrl: !!data.url
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(MockOps)).await;

    assert_eq!(json["status"], 200);
    assert_eq!(json["hasUrl"], true);
}

#[tokio::test]
async fn test_fetch_post_with_body() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://example.com/post', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ hello: 'world' })
            });
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                receivedData: data.json
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(MockOps)).await;

    assert_eq!(json["status"], 200);
    assert_eq!(json["receivedData"]["hello"], "world");
}

#[tokio::test]
async fn test_fetch_with_headers() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://example.com/headers', {
                headers: { 'X-Custom-Header': 'test-value' }
            });
            const data = await response.json();
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                customHeader: data.headers['x-custom-header']
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(MockOps)).await;

    assert_eq!(json["customHeader"], "test-value");
}

#[tokio::test]
async fn test_fetch_404() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://example.com/status/404');
            event.respondWith(new Response(JSON.stringify({
                status: response.status,
                ok: response.ok
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(MockOps)).await;

    assert_eq!(json["status"], 404);
    assert_eq!(json["ok"], false);
}

#[tokio::test]
async fn test_fetch_round_trips_binary_bodies() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const echo = async (body) => {
                const response = await fetch('https://example.com/echo', { method: 'POST', body });
                return Array.from(new Uint8Array(await response.arrayBuffer()));
            };

            const stream = new ReadableStream({
                start(controller) {
                    controller.enqueue(new Uint8Array([1, 2, 3]));
                    controller.enqueue(new Uint8Array([250, 251]));
                    controller.close();
                }
            });

            event.respondWith(new Response(JSON.stringify({
                bytes: await echo(new Uint8Array([0, 159, 146, 150, 255, 10])),
                view: await echo(new Uint8Array([7, 8, 9, 10]).subarray(1, 3)),
                buffer: await echo(new Uint8Array([200, 201]).buffer),
                stream: await echo(stream),
                text: await echo('h\u00e9llo')
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(EchoOps)).await;

    assert_eq!(
        json["bytes"],
        serde_json::json!([0, 159, 146, 150, 255, 10])
    );
    assert_eq!(json["view"], serde_json::json!([8, 9]));
    assert_eq!(json["buffer"], serde_json::json!([200, 201]));
    assert_eq!(json["stream"], serde_json::json!([1, 2, 3, 250, 251]));
    assert_eq!(
        json["text"],
        serde_json::json!([104, 195, 169, 108, 108, 111])
    );
}

#[tokio::test]
async fn test_status_text_comes_from_the_status_code() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const texts = {};

            for (const status of [200, 201, 302, 404, 418, 503, 599]) {
                const response = await fetch('https://example.com/status/' + status);
                texts[status] = response.statusText;
            }

            event.respondWith(new Response(JSON.stringify(texts)));
        });
    "#;

    let json = respond_json(script, Arc::new(StatusOps)).await;

    assert_eq!(json["200"], "OK");
    assert_eq!(json["201"], "Created");
    assert_eq!(json["302"], "Found");
    assert_eq!(json["404"], "Not Found");
    assert_eq!(json["418"], "");
    assert_eq!(json["503"], "Service Unavailable");
    assert_eq!(json["599"], "");
}

#[tokio::test]
async fn test_fetch_without_body_sends_none() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const response = await fetch('https://example.com/echo');
            const buffer = await response.arrayBuffer();
            event.respondWith(new Response(JSON.stringify({ length: buffer.byteLength })));
        });
    "#;

    let json = respond_json(script, Arc::new(EchoOps)).await;

    assert_eq!(json["length"], 0);
}
