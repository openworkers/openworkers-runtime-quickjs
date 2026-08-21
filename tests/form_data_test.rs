use bytes::Bytes;
use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Run a script against one POST and return the body it answered with
async fn post(script: &str, content_type: &str, body: &str) -> String {
    let mut worker = Worker::new(Script::new(script), None)
        .await
        .expect("Worker should initialize");

    let mut headers = HashMap::new();

    if !content_type.is_empty() {
        headers.insert("content-type".to_string(), content_type.to_string());
    }

    let request = HttpRequest {
        method: HttpMethod::Post,
        url: "http://localhost/".to_string(),
        headers,
        body: RequestBody::Bytes(Bytes::copy_from_slice(body.as_bytes())),
    };

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    let bytes = response
        .body
        .collect()
        .await
        .expect("Should read body")
        .unwrap_or_default();

    String::from_utf8(bytes.to_vec()).expect("Body should be UTF-8")
}

const URLENCODED: &str = "application/x-www-form-urlencoded";

#[tokio::test]
async fn test_form_data_decodes_percent_escapes() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const form = await event.request.formData();
                return new Response(form.get('name'));
            })());
        });
    "#;

    let body = post(script, URLENCODED, "name=Alice%20%26%20%3CBob%3E").await;

    assert_eq!(body, "Alice & <Bob>");
}

#[tokio::test]
async fn test_form_data_decodes_plus_as_space() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const form = await event.request.formData();
                return new Response(form.get('name'));
            })());
        });
    "#;

    let body = post(script, URLENCODED, "name=Jo+Ann").await;

    assert_eq!(body, "Jo Ann");
}

#[tokio::test]
async fn test_form_data_keeps_repeated_names() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const form = await event.request.formData();
                return new Response(JSON.stringify({
                    all: form.getAll('tag'),
                    first: form.get('tag'),
                    entries: [...form].length
                }));
            })());
        });
    "#;

    let body = post(script, URLENCODED, "tag=a&tag=b&other=c").await;

    assert_eq!(body, r#"{"all":["a","b"],"first":"a","entries":3}"#);
}

#[tokio::test]
async fn test_form_data_content_type_may_carry_a_charset() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const form = await event.request.formData();
                return new Response(form.get('name'));
            })());
        });
    "#;

    let body = post(
        script,
        "Application/X-WWW-Form-Urlencoded; charset=UTF-8",
        "name=Ada",
    )
    .await;

    assert_eq!(body, "Ada");
}

#[tokio::test]
async fn test_form_data_rejects_multipart() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                try {
                    await event.request.formData();
                    return new Response('no throw');
                } catch (e) {
                    return new Response(e.name);
                }
            })());
        });
    "#;

    let body = post(script, "multipart/form-data; boundary=x", "--x--").await;

    assert_eq!(body, "TypeError");
}

#[tokio::test]
async fn test_response_reads_a_form_body_too() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const res = new Response('a=1&b=2', {
                    headers: { 'content-type': 'application/x-www-form-urlencoded' }
                });
                const form = await res.formData();
                return new Response(form.get('b'));
            })());
        });
    "#;

    let body = post(script, URLENCODED, "").await;

    assert_eq!(body, "2");
}

#[tokio::test]
async fn test_form_data_set_replaces_every_value() {
    let script = r#"
        addEventListener('fetch', event => {
            event.respondWith((async () => {
                const form = new FormData();
                form.append('a', '1');
                form.append('a', '2');
                form.append('b', '3');
                form.set('a', '9');
                return new Response(JSON.stringify([...form]));
            })());
        });
    "#;

    let body = post(script, URLENCODED, "").await;

    assert_eq!(body, r#"[["a","9"],["b","3"]]"#);
}
