//! Run the SvelteKit conformance suite and diff against the recorded V8 oracle.
//!
//! Usage: `cargo run --release --example conformance -- [scenario-name...]`
//! The fixture lives in the sibling `openworkers-conformance` checkout.

use openworkers_core::{
    Event, HttpMethod, HttpRequest, LogLevel, OperationsHandler, RequestBody, RuntimeLimits, Script,
};
use openworkers_runtime_quickjs::Worker;
use openworkers_transform::{CodeLanguage, parse_worker_code};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;

/// The adapter calls `default.fetch(request, env, ctx)`; the fixture runs
/// without static assets, so every ASSETS fetch answers 404 like the oracle's.
const SHIM: &str = r#"
addEventListener('fetch', (event) => {
    const env = {
        ASSETS: {
            fetch: () => Promise.resolve(new Response(null, { status: 404 }))
        }
    };
    const ctx = { waitUntil: (p) => event.waitUntil(p), passThroughOnException() {} };

    event.respondWith((async () => {
        try {
            return await globalThis.default.fetch(event.request, env, ctx);
        } catch (e) {
            const detail = e && e.stack ? (e.name + ': ' + e.message + '\n' + e.stack) : String(e);
            return new Response('SSR THREW: ' + detail, { status: 599 });
        }
    })());
});
"#;

#[derive(Deserialize)]
struct Spec {
    bundle: String,
    base_url: String,
    scenarios: Vec<Scenario>,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    request: RequestSpec,
}

#[derive(Deserialize)]
struct RequestSpec {
    method: String,
    path: String,
    headers: Vec<String>,
    body: Option<String>,
}

#[derive(Deserialize)]
struct Oracle {
    lowered: Lowered,
    scenarios: Vec<Recorded>,
}

#[derive(Deserialize)]
struct Lowered {
    sha256: String,
}

#[derive(Deserialize)]
struct Recorded {
    name: String,
    response: RecordedResponse,
}

#[derive(Deserialize)]
struct RecordedResponse {
    status: u16,
    headers: Vec<String>,
    body_file: String,
    body_sha256: String,
    warm_identical: bool,
}

/// Logs are printed so a handler that threw says why
struct Ops;

impl OperationsHandler for Ops {
    fn handle_log(&self, level: LogLevel, message: String) {
        println!("  [{level:?}] {message}");
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

async fn dispatch(
    worker: &mut Worker,
    base_url: &str,
    spec: &RequestSpec,
) -> (u16, Vec<String>, Vec<u8>) {
    let headers: HashMap<String, String> = spec
        .headers
        .iter()
        .map(|h| {
            let (name, value) = h.split_once(": ").expect("header must be `name: value`");
            (name.to_string(), value.to_string())
        })
        .collect();

    let req = HttpRequest {
        method: spec.method.parse::<HttpMethod>().expect("bad method"),
        url: format!("{base_url}{}", spec.path),
        headers,
        body: match &spec.body {
            Some(b) => RequestBody::Bytes(b.clone().into_bytes().into()),
            None => RequestBody::None,
        },
    };

    let (task, rx) = Event::fetch(req);
    worker.exec(task).await.expect("exec failed");

    let res = rx.await.expect("no response");
    let body = res
        .body
        .collect()
        .await
        .expect("Should read body")
        .unwrap_or_default();

    let headers = res
        .headers
        .iter()
        .map(|(k, v)| format!("{k}: {v}"))
        .collect();

    (res.status, headers, body.to_vec())
}

/// The header lists side by side, marking every position that does not line up
fn header_diff(expected: &[String], actual: &[String]) -> Vec<String> {
    let mut lines = Vec::new();

    for index in 0..expected.len().max(actual.len()) {
        let want = expected.get(index).map(String::as_str);
        let got = actual.get(index).map(String::as_str);

        if want == got {
            continue;
        }

        lines.push(format!("      [{index}] want {}", want.unwrap_or("<none>")));
        lines.push(format!("      [{index}]  got {}", got.unwrap_or("<none>")));
    }

    lines
}

/// The first byte that differs, with the text around it on both sides
fn body_diff(expected: &[u8], actual: &[u8]) -> Vec<String> {
    let Some(at) =
        (0..expected.len().max(actual.len())).find(|i| expected.get(*i) != actual.get(*i))
    else {
        return Vec::new();
    };

    let from = at.saturating_sub(40);
    let window = |bytes: &[u8]| {
        String::from_utf8_lossy(&bytes[from.min(bytes.len())..(at + 60).min(bytes.len())])
            .replace('\n', "\\n")
    };

    vec![
        format!(
            "      first divergence at byte {at} ({} vs {} bytes)",
            expected.len(),
            actual.len()
        ),
        format!("      want ...{}", window(expected)),
        format!("       got ...{}", window(actual)),
    ]
}

fn main() {
    let filter: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../openworkers-conformance/fixtures/sveltekit-app");

    let spec: Spec = serde_json::from_slice(
        &std::fs::read(root.join("scenarios.json")).expect("no scenarios.json"),
    )
    .expect("bad scenarios.json");

    let oracle: Oracle =
        serde_json::from_slice(&std::fs::read(root.join("oracle.json")).expect("no oracle.json"))
            .expect("bad oracle.json");

    let bundle = std::fs::read(root.join(&spec.bundle)).expect("no bundle");
    let lowered = parse_worker_code(&bundle, CodeLanguage::JavaScript).expect("transform failed");

    let digest = sha256(lowered.as_bytes());

    println!(
        "lowered {} bytes, sha256 {digest} ({})",
        lowered.len(),
        if digest == oracle.lowered.sha256 {
            "same bytes as the oracle"
        } else {
            "DIFFERENT from the oracle"
        }
    );

    let code = format!("{lowered}\n{SHIM}");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();

    let failures = local.block_on(&rt, async {
        let mut failures = 0;

        for scenario in &spec.scenarios {
            if !filter.is_empty() && !filter.contains(&scenario.name) {
                continue;
            }

            let recorded = oracle
                .scenarios
                .iter()
                .find(|s| s.name == scenario.name)
                .expect("scenario missing from the oracle");
            let want_body =
                std::fs::read(root.join(&recorded.response.body_file)).unwrap_or_default();

            let mut worker = Worker::new_with_ops(
                Script::new(code.as_str()),
                Some(RuntimeLimits::default()),
                std::sync::Arc::new(Ops),
            )
            .await
            .expect("worker creation failed");

            let (status, headers, body) =
                dispatch(&mut worker, &spec.base_url, &scenario.request).await;
            let warm = dispatch(&mut worker, &spec.base_url, &scenario.request).await;
            let warm_identical = warm == (status, headers.clone(), body.clone());

            let status_ok = status == recorded.response.status;
            let headers_ok = headers == recorded.response.headers;
            let body_ok = sha256(&body) == recorded.response.body_sha256;
            let warm_ok = warm_identical == recorded.response.warm_identical;

            let verdict = match (status_ok, headers_ok, body_ok, warm_ok) {
                (true, true, true, true) => "PASS",
                (true, _, true, true) => "PARTIAL",
                _ => "FAIL",
            };

            println!("{:<24} {verdict}", scenario.name);

            if verdict != "PASS" {
                failures += 1;

                if !status_ok {
                    println!(
                        "      status want {} got {status}",
                        recorded.response.status
                    );
                }

                for line in header_diff(&recorded.response.headers, &headers) {
                    println!("{line}");
                }

                for line in body_diff(&want_body, &body) {
                    println!("{line}");
                }

                if !warm_ok {
                    println!(
                        "      warm_identical want {} got {warm_identical}",
                        recorded.response.warm_identical
                    );
                }
            }
        }

        failures
    });

    println!("\n{failures} scenario(s) differ from the oracle");
}
