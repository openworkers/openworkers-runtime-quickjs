use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

#[tokio::test]
async fn test_crypto_random_uuid() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const uuid = crypto.randomUUID();
            // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
            const isValid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(uuid);
            event.respondWith(new Response(JSON.stringify({
                uuid: uuid,
                isValid: isValid,
                length: uuid.length
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["isValid"], true);
    assert_eq!(json["length"], 36);
}

#[tokio::test]
async fn test_crypto_get_random_values() {
    let script = r#"
        addEventListener('fetch', (event) => {
            const array = new Uint8Array(16);

            const allZerosBefore = array.every(b => b === 0);

            const result = crypto.getRandomValues(array);

            const sameArray = result === array;

            const hasNonZero = array.some(b => b !== 0);

            event.respondWith(new Response(JSON.stringify({
                allZerosBefore: allZerosBefore,
                sameArray: sameArray,
                hasNonZero: hasNonZero,
                length: array.length
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["allZerosBefore"], true);
    assert_eq!(json["sameArray"], true);
    assert_eq!(json["hasNonZero"], true);
    assert_eq!(json["length"], 16);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha256() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-256', data);
            const hashArray = new Uint8Array(hashBuffer);

            const hashHex = Array.from(hashArray)
                .map(b => b.toString(16).padStart(2, '0'))
                .join('');

            // Known SHA-256 of "hello"
            const expected = '2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824';

            event.respondWith(new Response(JSON.stringify({
                hash: hashHex,
                expected: expected,
                matches: hashHex === expected,
                length: hashArray.length
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["matches"], true);
    assert_eq!(json["length"], 32);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha1() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-1', data);
            const hashArray = new Uint8Array(hashBuffer);

            const hashHex = Array.from(hashArray)
                .map(b => b.toString(16).padStart(2, '0'))
                .join('');

            // Known SHA-1 of "hello"
            const expected = 'aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d';

            event.respondWith(new Response(JSON.stringify({
                hash: hashHex,
                expected: expected,
                matches: hashHex === expected,
                length: hashArray.length
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["matches"], true);
    assert_eq!(json["length"], 20);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha512() {
    let script = r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-512', data);
            const hashArray = new Uint8Array(hashBuffer);

            event.respondWith(new Response(JSON.stringify({
                length: hashArray.length
            })));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None)
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
    assert_eq!(response.status, 200);

    let body = response.body.collect().await.expect("Should have body");
    let json: serde_json::Value = serde_json::from_slice(&body).expect("Should be valid JSON");
    assert_eq!(json["length"], 64);
}
