use std::{collections::HashMap, str::FromStr};

use chrono::Local;
use dockertest::{DockerTest, Image, TestBodySpecification};
use workflow_core::{
    postgres::{PostgresBackend, PostgresConfig},
    Id, Secret, StatesBackend, TimeStamp,
};

struct JobTestData {
    job_type: String,
    created_at: workflow_core::TimeStamp,
    input: workflow_core::ParameterSet,
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
        },
        JobTestData {
            job_type: search_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-12 13:13:15 +01:00").unwrap(),
            input: Default::default(),
        },
        // add some jobs of the type compress
        JobTestData {
            job_type: compress_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-13 14:00:00 +01:00").unwrap(),
            input: HashMap::from_iter(vec![
                ("input_file".to_string(), "file1.txt".to_string()),
                ("compression_level".to_string(), "9".to_string()),
            ]),
        },
        JobTestData {
            job_type: compress_job_type.to_string(),
            created_at: TimeStamp::from_str("2020-03-13 14:10:00 +01:00").unwrap(),
            input: HashMap::from_iter(vec![
                ("input_file".to_string(), "file2.txt".to_string()),
                ("compression_level".to_string(), "8".to_string()),
            ]),
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
        assert_eq!(job_state.status, workflow_core::Status::NotStarted);
        assert_eq!(job_state.stage, 0);
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
