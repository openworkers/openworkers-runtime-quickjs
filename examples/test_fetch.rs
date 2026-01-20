use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            console.log('Handler called');
            try {
                console.log('About to fetch');
                const response = await fetch('https://httpbin.workers.rocks/get');
                console.log('Fetch completed, status:', response.status);
                const text = await response.text();
                console.log('Got text, length:', text.length);
                event.respondWith(new Response('OK: ' + response.status));
            } catch (e) {
                console.log('Error:', e.message);
                event.respondWith(new Response('Error: ' + e.message, { status: 500 }));
            }
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
        .await
        .expect("Worker should initialize");

    println!("Worker created, executing fetch...");

    let request = HttpRequest {
        method: HttpMethod::Get,
        url: "http://localhost/".to_string(),
        headers: HashMap::new(),
        body: RequestBody::None,
    };

    let (task, rx) = Event::fetch(request);
    worker.exec(task).await.expect("Task should execute");

    let response = rx.await.expect("Should receive response");
    println!("Response status: {}", response.status);

    let body = response.body.collect().await.expect("Should have body");
    println!("Response body: {}", String::from_utf8_lossy(&body));
}
