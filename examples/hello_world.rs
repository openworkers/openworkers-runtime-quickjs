use rquickjs::{Context, Function, Runtime};

fn main() {
    // Create QuickJS runtime
    let rt = Runtime::new().unwrap();

    // Create execution context
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        // Setup console.log
        let global = ctx.globals();

        // Create console object with log function
        ctx.eval::<(), _>(r#"
            globalThis.console = {
                log: (...args) => __native_log(args.map(a =>
                    typeof a === 'object' ? JSON.stringify(a) : String(a)
                ).join(' ')),
                warn: (...args) => __native_log('[WARN] ' + args.join(' ')),
                error: (...args) => __native_log('[ERROR] ' + args.join(' '))
            };
        "#).ok();

        // Register native log function
        let log_fn = Function::new(ctx.clone(), |msg: String| {
            println!("[JS] {}", msg);
        }).unwrap();
        global.set("__native_log", log_fn).unwrap();

        // Now test!
        println!("--- Testing QuickJS ---\n");

        // Simple evaluation
        let result: String = ctx.eval(r#"
            const message = "Hello from QuickJS!";
            console.log(message);
            message
        "#).unwrap();
        println!("Result: {}\n", result);

        // Test arithmetic
        let result: i32 = ctx.eval(r#"
            const add = (a, b) => a + b;
            add(40, 2)
        "#).unwrap();
        println!("40 + 2 = {}\n", result);

        // Test objects
        let result: String = ctx.eval(r#"
            const obj = { name: "QuickJS", version: "2024" };
            JSON.stringify(obj)
        "#).unwrap();
        println!("Object: {}\n", result);

        // Test arrow functions and template literals
        let result: String = ctx.eval(r#"
            const greet = (name) => `Hello, ${name}!`;
            greet("OpenWorkers")
        "#).unwrap();
        println!("Greeting: {}\n", result);

        // Test destructuring
        let result: i32 = ctx.eval(r#"
            const { x, y } = { x: 10, y: 20 };
            x + y
        "#).unwrap();
        println!("Destructuring: x + y = {}\n", result);
    });

    println!("QuickJS Hello World complete!");
}
