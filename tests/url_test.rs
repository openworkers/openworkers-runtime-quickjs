use openworkers_core::{Event, HttpMethod, HttpRequest, RequestBody, Script};
use openworkers_runtime_quickjs::Worker;
use std::collections::HashMap;

/// Evaluate a JS expression inside a worker and return it as JSON
async fn eval(expr: &str) -> serde_json::Value {
    let script = format!(
        "addEventListener('fetch', event => event.respondWith(new Response(JSON.stringify({}))));",
        expr
    );

    let mut worker = Worker::new(Script::new(script.as_str()), None)
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
async fn test_url_components() {
    let url = eval(
        "(u => ({
            href: u.href, origin: u.origin, protocol: u.protocol, host: u.host,
            hostname: u.hostname, port: u.port, pathname: u.pathname,
            search: u.search, hash: u.hash
        }))(new URL('https://user:pw@example.com:8443/a/b?x=1&y=2#frag'))",
    )
    .await;

    assert_eq!(
        url["href"],
        "https://user:pw@example.com:8443/a/b?x=1&y=2#frag"
    );
    assert_eq!(url["origin"], "https://example.com:8443");
    assert_eq!(url["protocol"], "https:");
    assert_eq!(url["host"], "example.com:8443");
    assert_eq!(url["hostname"], "example.com");
    assert_eq!(url["port"], "8443");
    assert_eq!(url["pathname"], "/a/b");
    assert_eq!(url["search"], "?x=1&y=2");
    assert_eq!(url["hash"], "#frag");
}

#[tokio::test]
async fn test_url_defaults_are_empty_strings() {
    let url = eval(
        "(u => ({ port: u.port, search: u.search, hash: u.hash, pathname: u.pathname }))(new URL('http://example.com:80'))",
    )
    .await;

    assert_eq!(url["port"], "");
    assert_eq!(url["search"], "");
    assert_eq!(url["hash"], "");
    assert_eq!(url["pathname"], "/");
}

#[tokio::test]
async fn test_url_resolves_against_a_base() {
    let hrefs = eval(
        "[
            new URL('/c', 'https://example.com/a/b').href,
            new URL('d', 'https://example.com/a/b').href,
            new URL('https://other.test/x', 'https://example.com/a/b').href
        ]",
    )
    .await;

    assert_eq!(hrefs[0], "https://example.com/c");
    assert_eq!(hrefs[1], "https://example.com/a/d");
    assert_eq!(hrefs[2], "https://other.test/x");
}

#[tokio::test]
async fn test_url_accepts_non_special_schemes() {
    let url = eval(
        "(u => ({ href: u.href, protocol: u.protocol, host: u.host, origin: u.origin }))(new URL('sveltekit-internal://'))",
    )
    .await;

    assert_eq!(url["href"], "sveltekit-internal://");
    assert_eq!(url["protocol"], "sveltekit-internal:");
    assert_eq!(url["host"], "");
    assert_eq!(url["origin"], "null");
}

#[tokio::test]
async fn test_invalid_url_throws_type_error() {
    let thrown = eval("(() => { try { new URL('not a url'); return 'no throw'; } catch (e) { return e.constructor.name; } })()").await;

    assert_eq!(thrown, "TypeError");
}

#[tokio::test]
async fn test_url_setters() {
    let url = eval(
        "(u => {
            u.pathname = '/z';
            u.search = '?q=2';
            u.hash = 'end';
            u.host = 'other.test:9000';
            return { href: u.href, port: u.port, search: u.search, hash: u.hash };
        })(new URL('http://example.com/a?x=1'))",
    )
    .await;

    assert_eq!(url["href"], "http://other.test:9000/z?q=2#end");
    assert_eq!(url["port"], "9000");
    assert_eq!(url["search"], "?q=2");
    assert_eq!(url["hash"], "#end");
}

#[tokio::test]
async fn test_empty_search_setter_drops_the_question_mark() {
    let href =
        eval("(u => { u.search = ''; return u.href; })(new URL('http://example.com/a?x=1'))").await;

    assert_eq!(href, "http://example.com/a");
}

#[tokio::test]
async fn test_search_params_accessors() {
    let params = eval(
        "(p => ({
            get: p.get('a'), getAll: p.getAll('a'), missing: p.get('zz'),
            has: p.has('b'), hasValue: p.has('a', '2'), size: p.size,
            entries: [...p]
        }))(new URLSearchParams('a=1&b=2&a=3'))",
    )
    .await;

    assert_eq!(params["get"], "1");
    assert_eq!(params["getAll"], serde_json::json!(["1", "3"]));
    assert_eq!(params["missing"], serde_json::Value::Null);
    assert_eq!(params["has"], true);
    assert_eq!(params["hasValue"], false);
    assert_eq!(params["size"], 3);
    assert_eq!(
        params["entries"],
        serde_json::json!([["a", "1"], ["b", "2"], ["a", "3"]])
    );
}

#[tokio::test]
async fn test_search_params_mutation() {
    let result = eval(
        "(p => {
            p.append('c', '4');
            p.set('a', '9');
            p.delete('b');
            return p.toString();
        })(new URLSearchParams('a=1&b=2&a=3'))",
    )
    .await;

    assert_eq!(result, "a=9&c=4");
}

#[tokio::test]
async fn test_search_params_percent_decoding() {
    let params = eval(
        "(p => ({ value: p.get('name'), plus: p.get('q') }))(new URLSearchParams('?name=%C3%A9t%C3%A9&q=a+b'))",
    )
    .await;

    assert_eq!(params["value"], "\u{e9}t\u{e9}");
    assert_eq!(params["plus"], "a b");
}

#[tokio::test]
async fn test_search_params_serialization_escapes() {
    let serialized = eval(
        "(p => { p.append('k', 'a b&c=d/\u{e9}'); return p.toString(); })(new URLSearchParams())",
    )
    .await;

    assert_eq!(serialized, "k=a+b%26c%3Dd%2F%C3%A9");
}

#[tokio::test]
async fn test_search_params_constructors() {
    let strings = eval(
        "[
            new URLSearchParams({ a: '1', b: '2' }).toString(),
            new URLSearchParams([['a', '1'], ['a', '2']]).toString(),
            new URLSearchParams(new URLSearchParams('x=1')).toString()
        ]",
    )
    .await;

    assert_eq!(strings[0], "a=1&b=2");
    assert_eq!(strings[1], "a=1&a=2");
    assert_eq!(strings[2], "x=1");
}

#[tokio::test]
async fn test_search_params_sort() {
    let sorted =
        eval("(p => { p.sort(); return p.toString(); })(new URLSearchParams('c=3&a=1&b=2'))").await;

    assert_eq!(sorted, "a=1&b=2&c=3");
}

#[tokio::test]
async fn test_search_params_write_back_to_the_url() {
    let result = eval(
        "(u => {
            u.searchParams.set('page', '2');
            u.searchParams.delete('x');
            return { href: u.href, search: u.search };
        })(new URL('http://example.com/a?x=1'))",
    )
    .await;

    assert_eq!(result["href"], "http://example.com/a?page=2");
    assert_eq!(result["search"], "?page=2");
}

#[tokio::test]
async fn test_url_search_setter_updates_search_params() {
    let result = eval(
        "(u => {
            const params = u.searchParams;
            u.search = 'b=2';
            return [params.get('a'), params.get('b')];
        })(new URL('http://example.com/?a=1'))",
    )
    .await;

    assert_eq!(result[0], serde_json::Value::Null);
    assert_eq!(result[1], "2");
}

#[tokio::test]
async fn test_url_is_stringifiable() {
    let result = eval(
        "(u => [String(u), u.toJSON(), JSON.stringify({ u })])(new URL('http://example.com/a'))",
    )
    .await;

    assert_eq!(result[0], "http://example.com/a");
    assert_eq!(result[1], "http://example.com/a");
    assert_eq!(result[2], "{\"u\":\"http://example.com/a\"}");
}
