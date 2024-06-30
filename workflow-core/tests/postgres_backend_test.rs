use std::{collections::HashMap, str::FromStr};

use dockertest::{DockerTest, Image, TestBodySpecification};
use workflow_core::{
    postgres::{PostgresBackend, PostgresConfig},
    Error, Id, JobState, ParameterSet, Secret, StatesBackend, Status, TimeStamp, ToParameterSet,
};

struct Tasks {
    created_at: TimeStamp,
    task_type: String,
    input_sets: Vec<ParameterSet>,
}

struct JobTestData {
    job_type: String,
    created_at: workflow_core::TimeStamp,
    input: workflow_core::ParameterSet,
    /// The tasks that are part of the job for the respective job stages
    tasks: Vec<Tasks>,
}

/// Create a list of test jobs.
fn create_list_of_test_jobs() -> Vec<JobTestData> {
    let search_job_type = "search_job";
    let compress_job_type = "compress_job";

    vec![
        // add some jobs of the type search
        JobTestData {
            job_type: search_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-12 13:12:41 +01:00").unwrap(),
            input: Default::default(),
            tasks: vec![
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-12 13:13:00 +01:00").unwrap(),
                    task_type: "search".to_string(),
                    input_sets: vec![
                        [("key", "foobar")].to_params(),
                        [("key", "foobar2")].to_params(),
                    ],
                },
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-12 13:14:10 +01:00").unwrap(),
                    task_type: "report".to_string(),
                    input_sets: vec![],
                },
            ],
        },
        JobTestData {
            job_type: search_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-12 13:13:15 +01:00").unwrap(),
            input: Default::default(),
            tasks: vec![
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-12 13:14:00 +01:00").unwrap(),
                    task_type: "search".to_string(),
                    input_sets: vec![[("key", "42")].to_params()],
                },
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-12 13:14:10 +01:00").unwrap(),
                    task_type: "report".to_string(),
                    input_sets: vec![],
                },
            ],
        },
        // add some jobs of the type compress
        JobTestData {
            job_type: compress_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-13 14:00:00 +01:00").unwrap(),
            input: HashMap::from_iter(vec![
                ("input_files".to_string(), "file1.txt".to_string()),
                ("compression_level".to_string(), "9".to_string()),
            ]),
            tasks: vec![
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-13 14:02:00 +01:00").unwrap(),
                    task_type: "compress".to_string(),
                    input_sets: vec![
                        [("input_file", "file1.txt"), ("compression_level", "9")].to_params()
                    ],
                },
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-13 14:03:00 +01:00").unwrap(),
                    task_type: "send".to_string(),
                    input_sets: vec![[("url", "https://foobar.blub")].to_params()],
                },
            ],
        },
        JobTestData {
            job_type: compress_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-13 14:10:00 +01:00").unwrap(),
            input: HashMap::from_iter(vec![
                (
                    "input_files".to_string(),
                    "file2.txt, file3.txt, file4.txt, file5.txt".to_string(),
                ),
                ("compression_level".to_string(), "8".to_string()),
            ]),
            tasks: vec![
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-13 14:12:00 +01:00").unwrap(),
                    task_type: "compress".to_string(),
                    input_sets: vec![
                        [("input_file", "file2.txt"), ("compression_level", "8")].to_params(),
                        [("input_file", "file3.txt"), ("compression_level", "8")].to_params(),
                        [("input_file", "file4.txt"), ("compression_level", "8")].to_params(),
                        [("input_file", "file5.txt"), ("compression_level", "8")].to_params(),
                    ],
                },
                Tasks {
                    created_at: TimeStamp::from_str("2020-03-13 14:13:00 +01:00").unwrap(),
                    task_type: "send".to_string(),
                    input_sets: vec![[("url", "https://my-server:8080")].to_params()],
                },
            ],
        },
    ]
}

/// Test the states backend.
///
/// # Arguments
/// * `backend` - The states backend to test.
async fn states_backend_test<B: StatesBackend>(backend: B) {
    let jobs = backend.list_jobs(0, 1).await.unwrap();
    assert_eq!(jobs.total_count, 0);
    assert_eq!(jobs.jobs.len(), 0);

    // register all test jobs
    let test_jobs = create_list_of_test_jobs();
    let mut job_ids = Vec::new();
    for test_job in test_jobs.iter() {
        let job_id = backend
            .register_new_job_with_timestamp(
                &test_job.job_type,
                test_job.created_at,
                test_job.input.clone(),
            )
            .await
            .unwrap();

        job_ids.push(job_id);
    }

    // check that the total number of jobs is 4
    let list_jobs = backend.list_jobs(0, 10).await.unwrap();
    assert_eq!(list_jobs.total_count, 4);

    // make sure the initial state of the jobs is correct
    for job_id in job_ids.iter() {
        let job_state = backend.job_state(job_id).await.unwrap().unwrap();
        assert_eq!(job_state.status, Status::NotStarted);
        assert_eq!(job_state.stage, 0);
    }

    // update the state of the jobs to queued
    for (job_id, test_job) in job_ids.iter().zip(test_jobs.iter()) {
        let new_updated_at = test_job.created_at + chrono::Duration::seconds(1);

        let new_job_state = JobState {
            status: Status::Queued,
            stage: 0,
        };

        backend
            .update_job_state_with_timestamp(job_id, new_job_state, new_updated_at)
            .await
            .unwrap();
    }

    // the first two jobs should have 1 seconds difference between the created and updated
    // timestamp
    for job in backend.list_jobs(0, 2).await.unwrap().jobs.iter() {
        assert_eq!(
            job.created_at,
            job.updated_at - chrono::Duration::seconds(1)
        );
    }

    // try to create a task while the job is not in running status
    match backend
        .register_new_tasks(
            job_ids.first().unwrap(),
            "some_task_type",
            &[&ParameterSet::new()],
        )
        .await
    {
        Ok(_) => panic!("Creating a task for a job that is not running should fail"),
        Err(Error::JobNotRunning { .. }) => {}
        Err(e) => {
            panic!(
                "Creating a task for a job that is not running should return JobNotRunning error, but got {:?}",
                e
            );
        }
    }

    // try to create a task for job that does not exist
    match backend
        .register_new_tasks(&Id::default(), "some_task_type", &[&ParameterSet::new()])
        .await
    {
        Ok(_) => panic!("Creating a task for a job that does not exist should fail"),
        Err(Error::JobNotFound { .. }) => {}
        Err(e) => {
            panic!(
                "Creating a task for a job that does not exist should return JobNotFound error, but got {:?}",
                e
            );
        }
    }

    // update the state of the jobs to running
    for job_id in job_ids.iter() {
        let new_job_state = JobState {
            status: Status::Running,
            stage: 0,
        };

        backend
            .update_job_state(job_id, new_job_state)
            .await
            .unwrap();
    }

    // now create the tasks for the jobs of first stage
    let mut created_tasks: HashMap<Id, HashMap<Id, usize>> = HashMap::new();

    for (job_id, test_job) in job_ids.iter().zip(test_jobs.iter()) {
        let stage0_tasks = test_job.tasks.first().unwrap();

        let task_type = stage0_tasks.task_type.as_str();
        let timestamp = stage0_tasks.created_at;
        let task_parameter_sets: Vec<&ParameterSet> = stage0_tasks.input_sets.iter().collect();

        let task_ids = backend
            .register_new_tasks_with_timestamp(
                job_id,
                task_type,
                timestamp,
                task_parameter_sets.as_slice(),
            )
            .await
            .unwrap();

        let task_id_map: HashMap<Id, usize> =
            HashMap::from_iter(task_ids.iter().map(|id| (*id, 0usize)));
        created_tasks.insert(*job_id, task_id_map);
    }

    // check if the tasks were created correctly and if the number of active tasks is correct
    for job_id in job_ids.iter() {
        let tasks = backend.job_tasks(job_id, 0, 10, None).await.unwrap();
        let task_ids = created_tasks.get(job_id).unwrap();

        let num_active = backend.num_active_tasks(job_id).await.unwrap();
        assert_eq!(num_active, task_ids.len());

        assert_eq!(tasks.total_count, task_ids.len() as u64);

        for task in tasks.tasks.iter() {
            assert!(task_ids.contains_key(&task.task_id));
            assert_eq!(task.stage, 0);

            // make sure the state of the task is not started
            let task_status = backend.task_state(&task.task_id).await.unwrap().unwrap();
            assert_eq!(task_status, Status::NotStarted);
        }
    }

    // try to switch the state of an unknown task
    match backend
        .update_task_state(&Id::default(), Status::Running)
        .await
    {
        Ok(_) => panic!("Switching the state of an unknown task should fail"),
        Err(Error::TaskNotFound { .. }) => {}
        Err(e) => {
            panic!(
                "Switching the state of an unknown task should return TaskNotFound error, but got {:?}",
                e
            );
        }
    }

    // try to switch the state of a task to an invalid state
    let some_task_id = *created_tasks
        .values()
        .next()
        .unwrap()
        .keys()
        .next()
        .unwrap();
    match backend
        .update_task_state(&some_task_id, Status::NotStarted)
        .await
    {
        Ok(_) => panic!("Switching the state of a task to an invalid state should fail"),
        Err(Error::TaskInvalidStateSwitch { .. }) => {}
        Err(e) => {
            panic!(
                "Switching the state of a task to an invalid state should return TaskInvalidStateSwitch error, but got {:?}",
                e
            );
        }
    }

    // switch the state of the tasks to running
    for task_ids in created_tasks.values() {
        for task_id in task_ids.keys() {
            assert!(!backend
                .update_task_state(task_id, Status::Running)
                .await
                .unwrap());
        }
    }

    // now switch the state of the tasks to finished
    for task_ids in created_tasks.values() {
        for (i, task_id) in task_ids.keys().enumerate() {
            assert_eq!(
                backend
                    .update_task_state(task_id, Status::Finished)
                    .await
                    .unwrap(),
                i + 1 == task_ids.len()
            );
        }
    }

    // check the jobs
    for (job_id, test_job) in job_ids.iter().zip(test_jobs.iter()) {
        let job = backend.job(job_id).await.unwrap().unwrap();
        assert_eq!(job.created_at, test_job.created_at);
        assert_eq!(job.input, test_job.input);
        assert_eq!(job.job_type, test_job.job_type);
        assert_eq!(job.job_state.stage, 0);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_postgres_backend() {
    // Define our test instance
    let mut test = DockerTest::new();

    let image: Image = Image::with_repository("postgres")
        .pull_policy(dockertest::PullPolicy::IfNotPresent)
        .source(dockertest::Source::DockerHub)
        .tag("16");

    // define the postgres container
    let mut postgres = TestBodySpecification::with_image(image).set_publish_all_ports(true);

    // set the environment variables for the postgres container
    postgres
        .modify_env("POSTGRES_USER", "postgres")
        .modify_env("POSTGRES_PASSWORD", "password");

    // run the postgres container
    test.provide_container(postgres);

    test.run_async(|ops| async move {
        let container = ops.handle("postgres");

        // wait about 5 seconds for postgres to start
        println!("Waiting for postgres to start...");
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        println!("Waiting for postgres to start...DONE");

        let (ip, port) = container.host_port(5432).unwrap();
        println!("postgres running at {}:{}", ip, port);

        let options = PostgresConfig {
            host: "localhost".to_string(),
            port: *port as u16,
            dbname: "postgres".to_string(),
            user: "postgres".to_string(),
            password: Secret::from_str("password").unwrap(),
        };

        println!("Creating PostgresBackend instance...");
        let postgres_backend = PostgresBackend::new(options).await.unwrap();
        println!("Creating PostgresBackend instance...DONE");

        states_backend_test(postgres_backend).await;
    })
    .await;
}
