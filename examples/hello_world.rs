use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

const SCRIPT: &str = r#"
    addEventListener('fetch', async (event) => {
        console.log('handling', event.request.url);
        const body = JSON.stringify({ hello: 'from QuickJS', url: event.request.url });
        event.respondWith(new Response(body, {
            headers: { 'content-type': 'application/json' }
        }));
    });
"#;

#[tokio::main]
async fn main() {
    let mut worker = Worker::new(Script::new(SCRIPT), None)
        .await
        .expect("worker should initialize");

    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://localhost/hello".to_string(),
        headers: HashMap::new(),
        body: RequestBody::None,
    };

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("task should execute");

    let response = rx.await.expect("worker should respond");
    let body = response.body.collect().await.unwrap_or_default();

    println!("status: {}", response.status);
    println!("headers: {:?}", response.headers);
    println!("body: {}", String::from_utf8_lossy(&body));
}
