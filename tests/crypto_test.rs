use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Run a script and parse the JSON body it responds with
async fn respond_json(script: &str) -> serde_json::Value {
    let mut worker = Worker::new(Script::new(script), None)
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

    serde_json::from_slice(&body).expect("Should be valid JSON")
}

#[tokio::test]
async fn test_crypto_random_uuid() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            const uuid = crypto.randomUUID();
            const isValid = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(uuid);
            event.respondWith(new Response(JSON.stringify({
                isValid: isValid,
                length: uuid.length
            })));
        });
    "#,
    )
    .await;

    assert_eq!(json["isValid"], true);
    assert_eq!(json["length"], 36);
}

#[tokio::test]
async fn test_crypto_get_random_values() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            const array = new Uint8Array(16);

            const allZerosBefore = array.every(b => b === 0);
            const result = crypto.getRandomValues(array);

            event.respondWith(new Response(JSON.stringify({
                allZerosBefore: allZerosBefore,
                sameArray: result === array,
                hasNonZero: array.some(b => b !== 0),
                length: array.length
            })));
        });
    "#,
    )
    .await;

    assert_eq!(json["allZerosBefore"], true);
    assert_eq!(json["sameArray"], true);
    assert_eq!(json["hasNonZero"], true);
    assert_eq!(json["length"], 16);
}

#[tokio::test]
async fn test_get_random_values_fills_every_integer_view() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            const names = ['Int8Array', 'Uint8Array', 'Uint8ClampedArray', 'Int16Array',
                'Uint16Array', 'Int32Array', 'Uint32Array', 'BigInt64Array', 'BigUint64Array'];
            const filled = {};

            for (const name of names) {
                const array = new globalThis[name](8);
                filled[name] = crypto.getRandomValues(array) === array && array.some(v => v != 0);
            }

            event.respondWith(new Response(JSON.stringify(filled)));
        });
    "#,
    )
    .await;

    for name in [
        "Int8Array",
        "Uint8Array",
        "Uint8ClampedArray",
        "Int16Array",
        "Uint16Array",
        "Int32Array",
        "Uint32Array",
        "BigInt64Array",
        "BigUint64Array",
    ] {
        assert_eq!(json[name], true, "{} was not filled", name);
    }
}

#[tokio::test]
async fn test_get_random_values_rejects_non_integer_views() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            const rejected = {};

            for (const value of [new Float32Array(4), new Float64Array(4), new DataView(new ArrayBuffer(4)), [0, 0], 'nope']) {
                try {
                    crypto.getRandomValues(value);
                    rejected[typeof value + ':' + (value.constructor && value.constructor.name)] = 'accepted';
                } catch (e) {
                    rejected[typeof value + ':' + (value.constructor && value.constructor.name)] = e.name;
                }
            }

            event.respondWith(new Response(JSON.stringify(rejected)));
        });
    "#,
    )
    .await;

    assert_eq!(
        json,
        serde_json::json!({
            "object:Float32Array": "TypeMismatchError",
            "object:Float64Array": "TypeMismatchError",
            "object:DataView": "TypeMismatchError",
            "object:Array": "TypeMismatchError",
            "string:String": "TypeMismatchError"
        })
    );
}

#[tokio::test]
async fn test_get_random_values_enforces_the_quota() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            let atQuota = 'threw';
            try {
                atQuota = crypto.getRandomValues(new Uint8Array(65536)).length;
            } catch (e) {
                atQuota = e.name;
            }

            let overQuota = 'accepted';
            try {
                crypto.getRandomValues(new Uint8Array(65537));
            } catch (e) {
                overQuota = e.name;
            }

            let overQuotaWide = 'accepted';
            try {
                crypto.getRandomValues(new Uint32Array(16385));
            } catch (e) {
                overQuotaWide = e.name;
            }

            event.respondWith(new Response(JSON.stringify({ atQuota, overQuota, overQuotaWide })));
        });
    "#,
    )
    .await;

    assert_eq!(json["atQuota"], 65536);
    assert_eq!(json["overQuota"], "QuotaExceededError");
    assert_eq!(json["overQuotaWide"], "QuotaExceededError");
}

#[tokio::test]
async fn test_get_random_values_fills_only_the_view() {
    let json = respond_json(
        r#"
        addEventListener('fetch', (event) => {
            const buffer = new ArrayBuffer(16);
            crypto.getRandomValues(new Uint8Array(buffer, 8, 4));

            const bytes = Array.from(new Uint8Array(buffer));

            event.respondWith(new Response(JSON.stringify({
                outside: bytes.slice(0, 8).concat(bytes.slice(12)).every(b => b === 0),
                inside: bytes.slice(8, 12).some(b => b !== 0)
            })));
        });
    "#,
    )
    .await;

    assert_eq!(json["outside"], true);
    assert_eq!(json["inside"], true);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha256() {
    let json = respond_json(
        r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-256', data);
            const hashArray = new Uint8Array(hashBuffer);

            const hashHex = Array.from(hashArray)
                .map(b => b.toString(16).padStart(2, '0'))
                .join('');

            event.respondWith(new Response(JSON.stringify({
                hash: hashHex,
                length: hashArray.length
            })));
        });
    "#,
    )
    .await;

    assert_eq!(
        json["hash"],
        "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
    );
    assert_eq!(json["length"], 32);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha1() {
    let json = respond_json(
        r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-1', data);
            const hashArray = new Uint8Array(hashBuffer);

            const hashHex = Array.from(hashArray)
                .map(b => b.toString(16).padStart(2, '0'))
                .join('');

            event.respondWith(new Response(JSON.stringify({
                hash: hashHex,
                length: hashArray.length
            })));
        });
    "#,
    )
    .await;

    assert_eq!(json["hash"], "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d");
    assert_eq!(json["length"], 20);
}

#[tokio::test]
async fn test_crypto_subtle_digest_sha512() {
    let json = respond_json(
        r#"
        addEventListener('fetch', async (event) => {
            const data = new TextEncoder().encode('hello');
            const hashBuffer = await crypto.subtle.digest('SHA-512', data);

            event.respondWith(new Response(JSON.stringify({
                length: new Uint8Array(hashBuffer).length
            })));
        });
    "#,
    )
    .await;

    assert_eq!(json["length"], 64);
}
