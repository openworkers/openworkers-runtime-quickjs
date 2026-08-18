use rquickjs::{Ctx, Function, Result, prelude::Async};
use std::time::Duration;

/// Setup `setTimeout`, `setInterval` and their clear counterparts
///
/// Delays are futures spawned on the QuickJS job queue, so a handler that awaits
/// a timer resumes while the dispatch promise is still pending. `__cancelTimers`
/// drops every armed callback, which the worker uses to end a request.
pub fn setup_timers(ctx: &Ctx<'_>) -> Result<()> {
    let sleep = Function::new(
        ctx.clone(),
        Async(|delay: f64| async move {
            // Saturating cast, so a past deadline or an absurd delay clamps instead of throwing
            tokio::time::sleep(Duration::from_millis(delay as u64)).await;
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
            const ms = Number(delay) || 0;
            timers.set(id, { callback, args });

            const arm = () => __sleep(ms).then(() => {
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
            return schedule(callback, delay, args, false);
        };

        globalThis.setInterval = function setInterval(callback, delay, ...args) {
            return schedule(callback, delay, args, true);
        };

        globalThis.clearTimeout = function clearTimeout(id) {
            timers.delete(id);
        };

        globalThis.clearInterval = globalThis.clearTimeout;

        globalThis.__cancelTimers = function () {
            timers.clear();
        };
    })();
"#;
