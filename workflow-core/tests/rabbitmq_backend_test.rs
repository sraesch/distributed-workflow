use std::{collections::HashMap, sync::Arc};

use dockertest::{DockerTest, Image, TestBodySpecification};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use workflow_core::{Id, IncomingMessages, MessageQueue, RabbitMQ, RabbitMQOptions, Secret};

const TASK_QUEUE_NAME: &str = "tasks";
const RESULT_QUEUE_NAME: &str = "results";

const QUEUES: [&str; 2] = [TASK_QUEUE_NAME, RESULT_QUEUE_NAME];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Task {
    job_id: Id,
    task_id: Id,
    task_class: String,
    parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskResult {
    job_id: Id,
    task_id: Id,
    success: bool,
}

/// Creates a list of random tasks.
///
/// # Arguments
/// * `num_tasks` - The number of tasks to create.
fn create_random_tasks(num_tasks: usize) -> Vec<Task> {
    let task_classes = ["task1", "task2", "task3"];

    let mut tasks = Vec::new();
    for _ in 0..num_tasks {
        // take a random task class for each task
        let task_class = {
            let idx = rand::random::<usize>() % task_classes.len();
            task_classes[idx].to_string()
        };

        tasks.push(Task {
            job_id: Id::default(),
            task_id: Id::default(),
            task_class,
            parameters: HashMap::new(),
        })
    }
    tasks
}

/// Function to test the given message queue backend.
///
/// # Arguments
/// * `mq` - The message queue to test.
async fn test_mq<MQ: MessageQueue + 'static>(mq: MQ) {
    const MAX_MS_WAIT: u64 = 10;
    const NUM_WORKERS: usize = 10;
    const NUM_TASKS: usize = 400;

    let mq: Arc<Mutex<MQ>> = Arc::new(Mutex::new(mq));

    // Collections to store all processed tasks and results, to check later
    let processed_tasks = Arc::new(Mutex::new(Vec::<(usize, Task)>::new()));
    let processed_results = Arc::new(Mutex::new(Vec::<TaskResult>::new()));

    // spawn a worker to process tasks
    for worker_id in 0..NUM_WORKERS {
        let mq = mq.clone();
        let processed_tasks = processed_tasks.clone();
        tokio::spawn(async move {
            let mut tasks_receiver = mq.lock().await.subscribe(TASK_QUEUE_NAME).await.unwrap();

            while let Some(task) = tasks_receiver.recv().await {
                let task: Task = task.unwrap();

                let job_id = task.job_id;
                let task_id = task.task_id;

                // wait a random amount of time between 0 and MAX_MS_WAIT ms to simulate processing
                let ms = rand::random::<u64>() % MAX_MS_WAIT;
                tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;

                processed_tasks.lock().await.push((worker_id, task));

                // publish a result for the task
                let result = TaskResult {
                    job_id,
                    task_id,
                    success: true,
                };

                mq.lock()
                    .await
                    .publish(RESULT_QUEUE_NAME, &result)
                    .await
                    .unwrap();
            }
        });
    }

    // spawn a worker to process results
    {
        let mut results_receiver = mq.lock().await.subscribe(RESULT_QUEUE_NAME).await.unwrap();
        let processed_results = processed_results.clone();
        tokio::spawn(async move {
            while let Some(result) = results_receiver.recv().await {
                let result = result.unwrap();
                processed_results.lock().await.push(result);
            }
        });
    }

    // spawn some tasks
    let tasks = create_random_tasks(NUM_TASKS);
    for task in tasks.iter() {
        mq.lock()
            .await
            .publish(TASK_QUEUE_NAME, task)
            .await
            .unwrap();

        // wait a random amount of time between 0 and MAX_MS_WAIT ms to simulate processing
        let ms = rand::random::<u64>() % MAX_MS_WAIT;
        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    }

    // wait some time to make sure the tasks are processed
    tokio::time::sleep(tokio::time::Duration::from_millis(MAX_MS_WAIT * 2)).await;

    // check if all tasks and results were processed
    assert_eq!(processed_tasks.lock().await.len(), NUM_TASKS);
    assert_eq!(processed_results.lock().await.len(), NUM_TASKS);

    // sort the tasks and results by task id to compare them with the original tasks
    processed_tasks.lock().await.sort_by_key(|t| t.1.task_id);

    processed_results.lock().await.sort_by_key(|r| r.task_id);

    let mut tasks = tasks;
    tasks.sort_by(|lhs, rhs| lhs.task_id.cmp(&rhs.task_id));

    // compare the processed tasks with the input tasks
    for (i, (lhs_task, rhs_task)) in tasks
        .iter()
        .zip(processed_tasks.lock().await.iter())
        .enumerate()
    {
        assert_eq!(
            lhs_task.task_id, rhs_task.1.task_id,
            "Task {} is not equal",
            i
        );
    }

    // compare the processed results with the input tasks
    for (i, (lhs_result, rhs_result)) in tasks
        .iter()
        .zip(processed_results.lock().await.iter())
        .enumerate()
    {
        assert_eq!(
            lhs_result.task_id, rhs_result.task_id,
            "Result {} is not equal",
            i
        );
    }

    // count the number of processed tasks for each worker
    let mut worker_counts = [0u64; NUM_WORKERS];
    for (worker_id, _) in processed_tasks.lock().await.iter() {
        worker_counts[*worker_id] += 1;
    }

    // print the number of processed tasks for each worker
    for (worker_id, count) in worker_counts.iter().enumerate() {
        println!("Worker {}: {} tasks", worker_id, count);
    }

    // check if all workers processed at least one task
    for count in worker_counts.iter() {
        assert!(*count > 0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_rabbit_mq() {
    // Define our test instance
    let mut test = DockerTest::new();

    let image: Image = Image::with_repository("rabbitmq")
        .pull_policy(dockertest::PullPolicy::IfNotPresent)
        .source(dockertest::Source::DockerHub)
        .tag("3.13-alpine");

    // define the RabbitMQ container
    let rabbit = TestBodySpecification::with_image(image).set_publish_all_ports(true);

    // run the RabbitMQ container
    test.provide_container(rabbit);

    test.run_async(|ops| async move {
        let container = ops.handle("rabbitmq");

        // wait about 10 seconds for RabbitMQ to start
        println!("Waiting for RabbitMQ to start...");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        println!("Waiting for RabbitMQ to start...DONE");

        let (ip, port) = container.host_port(5672).unwrap();
        println!("RabbitMQ running at {}:{}", ip, port);

        let options = RabbitMQOptions {
            host: "localhost".to_string(),
            port: *port as u16,
            user: "guest".to_string(),
            password: Secret::new("guest".to_string()),
        };

        let mq = RabbitMQ::new(&options, QUEUES.iter()).await.unwrap();
        test_mq(mq).await;
    })
    .await;
}
