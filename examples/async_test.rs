use rquickjs::{AsyncContext, AsyncRuntime, Function, async_with, promise::Promise};

#[tokio::main]
async fn main() {
    println!("--- Testing QuickJS Async/Await ---\n");

    // Create async runtime
    let rt = AsyncRuntime::new().unwrap();
    let ctx = AsyncContext::full(&rt).await.unwrap();

    async_with!(ctx => |ctx| {
        // Setup console.log
        let global = ctx.globals();
        let log_fn = Function::new(ctx.clone(), |msg: String| {
            println!("[JS] {}", msg);
        }).unwrap();
        global.set("__native_log", log_fn).unwrap();

        ctx.eval::<(), _>(r#"
            globalThis.console = {
                log: (...args) => __native_log(args.map(a =>
                    typeof a === 'object' ? JSON.stringify(a) : String(a)
                ).join(' '))
            };
        "#).unwrap();

        // Test 1: Basic Promise
        println!("Test 1: Basic Promise");
        let promise: Promise = ctx.eval(r#"
            new Promise((resolve) => {
                resolve(42);
            })
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        println!("Promise resolved: {}\n", result);

        // Test 2: Promise.resolve
        println!("Test 2: Promise.resolve");
        let promise: Promise = ctx.eval(r#"
            Promise.resolve("Hello from Promise!")
        "#).unwrap();
        let result: String = promise.into_future().await.unwrap();
        println!("Result: {}\n", result);

        // Test 3: Async function
        println!("Test 3: Async function");
        let promise: Promise = ctx.eval(r#"
            async function asyncAdd(a, b) {
                return a + b;
            }
            asyncAdd(10, 20)
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        println!("Async add: {}\n", result);

        // Test 4: Await in async function
        println!("Test 4: Await in async function");
        let promise: Promise = ctx.eval(r#"
            async function fetchData() {
                const data = await Promise.resolve({ message: "async works!" });
                return JSON.stringify(data);
            }
            fetchData()
        "#).unwrap();
        let result: String = promise.into_future().await.unwrap();
        println!("Fetched data: {}\n", result);

        // Test 5: Promise chain
        println!("Test 5: Promise chain");
        let promise: Promise = ctx.eval(r#"
            Promise.resolve(1)
                .then(x => x + 1)
                .then(x => x * 2)
                .then(x => x + 10)
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        println!("Chain result (1+1)*2+10 = {}\n", result);

        // Test 6: Promise.all
        println!("Test 6: Promise.all");
        let promise: Promise = ctx.eval(r#"
            Promise.all([
                Promise.resolve(1),
                Promise.resolve(2),
                Promise.resolve(3)
            ]).then(arr => JSON.stringify(arr))
        "#).unwrap();
        let result: String = promise.into_future().await.unwrap();
        println!("Promise.all: {}\n", result);

        // Test 7: Async/await with multiple awaits
        println!("Test 7: Multiple awaits");
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
        println!("Multi-await sum: {}\n", result);

        // Test 8: Promise.race
        println!("Test 8: Promise.race");
        let promise: Promise = ctx.eval(r#"
            Promise.race([
                Promise.resolve(42),
                new Promise(r => r(100))
            ])
        "#).unwrap();
        let result: i32 = promise.into_future().await.unwrap();
        println!("Promise.race winner: {}\n", result);
    })
    .await;

    println!("QuickJS Async/Await test complete!");
}
