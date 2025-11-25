use rquickjs::{AsyncContext, AsyncRuntime, Function, async_with, promise::Promise};

#[tokio::test]
async fn test_basic_promise() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            new Promise((resolve) => resolve(42))
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 42);
    })
    .await;
}

#[tokio::test]
async fn test_promise_resolve() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            Promise.resolve("hello")
        "#).unwrap();
        let result: String = promise.into_future().await.unwrap();
        assert_eq!(result, "hello");
    })
    .await;
}

#[tokio::test]
async fn test_async_function() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            async function add(a, b) {
                return a + b;
            }
            add(10, 20)
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 30);
    })
    .await;
}

#[tokio::test]
async fn test_await_in_async() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            async function getData() {
                const value = await Promise.resolve(100);
                return value * 2;
            }
            getData()
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 200);
    })
    .await;
}

#[tokio::test]
async fn test_promise_chain() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            Promise.resolve(1)
                .then(x => x + 1)
                .then(x => x * 2)
                .then(x => x + 10)
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 14); // (1+1)*2+10 = 14
    })
    .await;
}

#[tokio::test]
async fn test_promise_all() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            Promise.all([
                Promise.resolve(1),
                Promise.resolve(2),
                Promise.resolve(3)
            ]).then(arr => arr.reduce((a, b) => a + b, 0))
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 6);
    })
    .await;
}

#[tokio::test]
async fn test_promise_race() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            Promise.race([
                Promise.resolve(42),
                new Promise(r => r(100))
            ])
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 42);
    })
    .await;
}

#[tokio::test]
async fn test_multiple_awaits() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            async function multiAwait() {
                const a = await Promise.resolve(10);
                const b = await Promise.resolve(20);
                const c = await Promise.resolve(30);
                return a + b + c;
            }
            multiAwait()
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 60);
    })
    .await;
}

#[tokio::test]
async fn test_async_with_json() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let promise: Promise = ctx.eval(r#"
            async function fetchData() {
                const data = await Promise.resolve({ name: "test", value: 42 });
                return JSON.stringify(data);
            }
            fetchData()
        "#).unwrap();
        let result: String = promise.into_future().await.unwrap();
        assert_eq!(result, r#"{"name":"test","value":42}"#);
    })
    .await;
}

#[tokio::test]
async fn test_native_async_function() {
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        let global = ctx.globals();

        // Register native async function
        let async_add = Function::new(ctx.clone(), |a: i32, b: i32| a + b).unwrap();
        global.set("nativeAdd", async_add).unwrap();

        let promise: Promise = ctx.eval(r#"
            async function test() {
                const result = nativeAdd(100, 200);
                return result;
            }
            test()
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        assert_eq!(result, 300);
    })
    .await;
}
