use rquickjs::{Ctx, Function, Result, prelude::Async};
use std::time::Duration;

/// Setup `setTimeout`, `setInterval` and their clear counterparts
///
/// Delays are futures spawned on the QuickJS job queue, so a handler that awaits
/// a timer resumes while the dispatch promise is still pending.
pub fn setup_timers(ctx: &Ctx<'_>) -> Result<()> {
    let sleep = Function::new(
        ctx.clone(),
        Async(|delay: u64| async move {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }),
    )?;
    ctx.globals().set("__sleep", sleep)?;

    ctx.eval::<(), _>(TIMERS_JS)?;

    Ok(())
}

const TIMERS_JS: &str = r#"
    (function () {
        const timers = new Map();
        let nextId = 1;

        function schedule(callback, delay, args, repeat) {
            const id = nextId++;
            timers.set(id, { callback, args });

            const arm = () => __sleep(delay).then(() => {
                const timer = timers.get(id);

                if (!timer) {
                    return;
                }

                if (!repeat) {
                    timers.delete(id);
                }

                try {
                    timer.callback(...timer.args);
                } catch (e) {
                    console.error('Timer callback error:', e);
                }

                if (repeat && timers.has(id)) {
                    arm();
                }
            });

            arm();

            return id;
        }

        globalThis.setTimeout = function setTimeout(callback, delay, ...args) {
            return schedule(callback, delay || 0, args, false);
        };

        globalThis.setInterval = function setInterval(callback, delay, ...args) {
            return schedule(callback, delay || 0, args, true);
        };

        globalThis.clearTimeout = function clearTimeout(id) {
            timers.delete(id);
        };

        globalThis.clearInterval = globalThis.clearTimeout;
    })();
"#;
