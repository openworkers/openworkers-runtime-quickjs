use bytes::Bytes;
use openworkers_core::{
    Event, HttpMethod, HttpRequest, HttpResponse, OpFuture, OperationsHandler, RequestBody,
    ResponseBody, Script,
};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;
use std::sync::Arc;

/// Handler answering every fetch with two Set-Cookie headers
struct TwoCookieOps;

impl OperationsHandler for TwoCookieOps {
    fn handle_fetch(&self, _request: HttpRequest) -> OpFuture<'_, Result<HttpResponse, String>> {
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                headers: vec![
                    ("set-cookie".to_string(), "up=1".to_string()),
                    ("set-cookie".to_string(), "up=2".to_string()),
                ],
                body: ResponseBody::Bytes(Bytes::from("upstream")),
            })
        })
    }
}

/// Handler answering with the request headers it received, as JSON
struct EchoHeadersOps;

impl OperationsHandler for EchoHeadersOps {
    fn handle_fetch(&self, request: HttpRequest) -> OpFuture<'_, Result<HttpResponse, String>> {
        Box::pin(async move {
            let body = serde_json::to_vec(&request.headers).expect("headers should serialize");

            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: ResponseBody::Bytes(Bytes::from(body)),
            })
        })
    }
}

async fn respond(script: &str, ops: Arc<dyn OperationsHandler>) -> Vec<(String, String)> {
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

    rx.await.expect("Should receive response").headers
}

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

fn values_of<'a>(headers: &'a [(String, String)], name: &str) -> Vec<&'a str> {
    headers
        .iter()
        .filter(|(header, _)| header == name)
        .map(|(_, value)| value.as_str())
        .collect()
}

#[tokio::test]
async fn test_appended_duplicate_headers_are_preserved() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const headers = new Headers();
            headers.append('Set-Cookie', 'a=1');
            headers.append('Set-Cookie', 'b=2');
            headers.append('Content-Type', 'text/plain');
            event.respondWith(new Response('ok', { headers }));
        });
    "#;

    let headers = respond(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(values_of(&headers, "set-cookie"), vec!["a=1", "b=2"]);
    assert_eq!(values_of(&headers, "content-type"), vec!["text/plain"]);
}

#[tokio::test]
async fn test_duplicate_headers_from_init_list_are_preserved() {
    let script = r#"
        addEventListener('fetch', (event) => {
            event.respondWith(new Response('ok', {
                headers: [['Set-Cookie', 'a=1'], ['Set-Cookie', 'b=2']]
            }));
        });
    "#;

    let headers = respond(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(values_of(&headers, "set-cookie"), vec!["a=1", "b=2"]);
}

#[tokio::test]
async fn test_duplicate_headers_survive_a_fetch_round_trip() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const upstream = await fetch('https://example.com/');
            event.respondWith(new Response('ok', { headers: upstream.headers }));
        });
    "#;

    let headers = respond(script, Arc::new(TwoCookieOps)).await;

    assert_eq!(values_of(&headers, "set-cookie"), vec!["up=1", "up=2"]);
}

#[tokio::test]
async fn test_outbound_duplicate_headers_are_combined() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const headers = new Headers();
            headers.append('X-Trace', 'one');
            headers.append('X-Trace', 'two');
            headers.append('Cookie', 'a=1');
            headers.append('Cookie', 'b=2');

            const upstream = await fetch('https://example.com/', { headers });
            event.respondWith(new Response(await upstream.text()));
        });
    "#;

    let json = respond_json(script, Arc::new(EchoHeadersOps)).await;

    assert_eq!(json["x-trace"], "one, two");
    assert_eq!(json["cookie"], "a=1; b=2");
}

#[tokio::test]
async fn test_outbound_header_names_are_lowercased() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const upstream = await fetch('https://example.com/', {
                headers: { 'X-Custom-Header': 'value' }
            });
            event.respondWith(new Response(await upstream.text()));
        });
    "#;

    let json = respond_json(script, Arc::new(EchoHeadersOps)).await;

    assert_eq!(json["x-custom-header"], "value");
}

#[tokio::test]
async fn test_get_set_cookie_returns_every_value() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const headers = new Headers([['Set-Cookie', 'a=1'], ['Set-Cookie', 'b=2']]);
            event.respondWith(new Response(JSON.stringify({
                cookies: headers.getSetCookie(),
                combined: headers.get('set-cookie')
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(json["cookies"][0], "a=1");
    assert_eq!(json["cookies"][1], "b=2");
    assert_eq!(json["combined"], "a=1, b=2");
}

#[tokio::test]
async fn test_headers_are_iterable() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const headers = new Headers([['X-One', '1'], ['X-Two', '2']]);

            const loop = [];
            for (const [name, value] of headers) {
                loop.push(name + '=' + value);
            }

            const forEached = [];
            headers.forEach(function (value, name) {
                forEached.push(name + '=' + value + '=' + this.tag);
            }, { tag: 'ctx' });

            event.respondWith(new Response(JSON.stringify({
                spread: [...headers],
                loop: loop,
                object: Object.fromEntries(headers),
                keys: [...headers.keys()],
                values: [...headers.values()],
                entries: [...headers.entries()],
                forEached: forEached
            })));
        });
    "#;

    let json = respond_json(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(
        json["spread"],
        serde_json::json!([["x-one", "1"], ["x-two", "2"]])
    );
    assert_eq!(json["loop"], serde_json::json!(["x-one=1", "x-two=2"]));
    assert_eq!(
        json["object"],
        serde_json::json!({"x-one": "1", "x-two": "2"})
    );
    assert_eq!(json["keys"], serde_json::json!(["x-one", "x-two"]));
    assert_eq!(json["values"], serde_json::json!(["1", "2"]));
    assert_eq!(json["entries"], json["spread"]);
    assert_eq!(
        json["forEached"],
        serde_json::json!(["x-one=1=ctx", "x-two=2=ctx"])
    );
}

#[tokio::test]
async fn test_iteration_yields_every_duplicate() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const headers = new Headers([['Set-Cookie', 'a=1'], ['Set-Cookie', 'b=2']]);
            event.respondWith(new Response(JSON.stringify([...headers])));
        });
    "#;

    let json = respond_json(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(
        json,
        serde_json::json!([["set-cookie", "a=1"], ["set-cookie", "b=2"]])
    );
}

#[tokio::test]
async fn test_set_replaces_every_value_of_a_header() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const headers = new Headers([['Set-Cookie', 'a=1'], ['Set-Cookie', 'b=2']]);
            headers.set('Set-Cookie', 'only=1');
            event.respondWith(new Response('ok', { headers }));
        });
    "#;

    let headers = respond(script, Arc::new(openworkers_core::DefaultOps)).await;

    assert_eq!(values_of(&headers, "set-cookie"), vec!["only=1"]);
}
