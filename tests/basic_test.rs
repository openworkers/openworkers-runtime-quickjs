use rquickjs::{Context, Function, Runtime};

#[test]
fn test_basic_eval() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: i32 = ctx.eval("1 + 2").unwrap();
        assert_eq!(result, 3);
    });
}

#[test]
fn test_string_eval() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: String = ctx.eval("'Hello' + ' ' + 'World'").unwrap();
        assert_eq!(result, "Hello World");
    });
}

#[test]
fn test_arrow_functions() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: i32 = ctx
            .eval(
                r#"
            const add = (a, b) => a + b;
            add(10, 20)
        "#,
            )
            .unwrap();
        assert_eq!(result, 30);
    });
}

#[test]
fn test_template_literals() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: String = ctx
            .eval(
                r#"
            const name = "QuickJS";
            `Hello, ${name}!`
        "#,
            )
            .unwrap();
        assert_eq!(result, "Hello, QuickJS!");
    });
}

#[test]
fn test_destructuring() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: i32 = ctx
            .eval(
                r#"
            const { x, y } = { x: 10, y: 20 };
            x + y
        "#,
            )
            .unwrap();
        assert_eq!(result, 30);
    });
}

#[test]
fn test_array_methods() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: i32 = ctx
            .eval(
                r#"
            [1, 2, 3, 4, 5].reduce((a, b) => a + b, 0)
        "#,
            )
            .unwrap();
        assert_eq!(result, 15);
    });
}

#[test]
fn test_json_stringify() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: String = ctx
            .eval(
                r#"
            JSON.stringify({ name: "test", value: 42 })
        "#,
            )
            .unwrap();
        assert_eq!(result, r#"{"name":"test","value":42}"#);
    });
}

#[test]
fn test_json_parse() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let result: i32 = ctx
            .eval(
                r#"
            const obj = JSON.parse('{"value": 42}');
            obj.value
        "#,
            )
            .unwrap();
        assert_eq!(result, 42);
    });
}

#[test]
fn test_native_function_binding() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let global = ctx.globals();

        // Register native function
        let add_fn = Function::new(ctx.clone(), |a: i32, b: i32| a + b).unwrap();
        global.set("nativeAdd", add_fn).unwrap();

        let result: i32 = ctx.eval("nativeAdd(100, 200)").unwrap();
        assert_eq!(result, 300);
    });
}

#[test]
fn test_console_log_setup() {
    let rt = Runtime::new().unwrap();
    let ctx = Context::full(&rt).unwrap();

    ctx.with(|ctx| {
        let global = ctx.globals();

        // Setup console
        let log_fn = Function::new(ctx.clone(), |_msg: String| {}).unwrap();
        global.set("__native_log", log_fn).unwrap();

        ctx.eval::<(), _>(
            r#"
            globalThis.console = {
                log: (...args) => __native_log(args.join(' '))
            };
        "#,
        )
        .unwrap();

        // Should not panic
        ctx.eval::<(), _>("console.log('test')").unwrap();
    });
}
