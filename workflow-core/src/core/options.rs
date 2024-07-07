use std::path::PathBuf;

use crate::{postgres::PostgresConfig, RabbitMQOptions, TaskJobDesc};

/// The common options for the worker and controller.
#[derive(Debug, Clone)]
pub struct Options {
    /// The root directory of the all the tasks and jobs.
    pub root_dir: PathBuf,
    /// The description of the tasks and jobs.
    pub task_job_desc: TaskJobDesc,
    /// The address where to expose the controller REST API.
    pub address: String,
    /// The RabbitMQ options.
    pub rabbitmq: RabbitMQOptions,
    /// The Postgres config.
    pub postgres: PostgresConfig,
}
