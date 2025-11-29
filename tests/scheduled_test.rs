use openworkers_core::{Script, Task};
use openworkers_runtime_quickjs::Worker;

#[tokio::test]
async fn test_scheduled_basic() {
    let script = r#"
        let executed = false;
        addEventListener('scheduled', (event) => {
            executed = true;
            console.log('Scheduled event executed');
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1234567890);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}

#[tokio::test]
async fn test_scheduled_with_time() {
    let script = r#"
        addEventListener('scheduled', (event) => {
            console.log('Scheduled time:', event.scheduledTime);
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1700000000000);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}

#[tokio::test]
async fn test_scheduled_async_handler() {
    let script = r#"
        addEventListener('scheduled', async (event) => {
            console.log('Async handler started');
            await Promise.resolve();
            console.log('Async handler completed');
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1700000000000);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}

#[tokio::test]
async fn test_scheduled_wait_until() {
    let script = r#"
        addEventListener('scheduled', (event) => {
            event.waitUntil(Promise.resolve('done'));
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1700000000000);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}

#[tokio::test]
async fn test_scheduled_multiple_handlers() {
    let script = r#"
        let count = 0;

        addEventListener('scheduled', (event) => {
            count++;
            console.log('Handler 1, count:', count);
        });

        addEventListener('scheduled', (event) => {
            count++;
            console.log('Handler 2, count:', count);
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1700000000000);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}

#[tokio::test]
async fn test_scheduled_event_properties() {
    let script = r#"
        addEventListener('scheduled', (event) => {
            console.log('Event type:', event.type);
            console.log('Has scheduledTime:', typeof event.scheduledTime === 'number');

            if (event.type !== 'scheduled') {
                throw new Error('Wrong event type');
            }
            if (event.scheduledTime !== 1700000000000) {
                throw new Error('Wrong scheduledTime: ' + event.scheduledTime);
            }
        });
    "#;

    let script_obj = Script::new(script);
    let mut worker = Worker::new(script_obj, None, None)
        .await
        .expect("Worker should initialize");

    let (task, rx) = Task::scheduled(1700000000000);
    worker.exec(task).await.expect("Task should execute");

    rx.await.expect("Should receive result");
}
