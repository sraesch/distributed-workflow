use std::{io::Read, path::PathBuf};

use anyhow::{Context, Result};
use clap::{arg, value_parser, Command};
use log::{info, LevelFilter};
use serde::Deserialize;
use workflow_core::{postgres::PostgresConfig, Options, RabbitMQOptions, TaskJobDesc};

use crate::{initialize_logging, LogLevel};

/// Parses the program arguments and returns the program options.
///
/// # Arguments
/// * `app_name` - The name of the application.
/// * `version` - The version of the application.
/// * `about` - The description of the application.
pub fn parse_args_and_init_logging(
    app_name: &'static str,
    version: &'static str,
    about: &'static str,
) -> Result<Options> {
    // parse program arguments
    let matches = Command::new(app_name)
        .version(version)
        .about(about)
        .arg(
            arg!(
                -c --config <FILE> "Path to the configuration file."
            )
            .required(true)
            .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let config_path = matches.get_one::<PathBuf>("config").unwrap().clone();

    // load the configuration file, initialize logging and print the configuration
    let program_config = ProgramConfig::try_from(ProgramOptions { config_path })?;
    initialize_logging(LevelFilter::from(program_config.log));
    program_config.print_to_log();

    // try to load the referenced tasks and jobs description
    let task_job_desc =
        TaskJobDesc::from_reader(std::fs::File::open(&program_config.task_job_desc)?)
            .with_context(|| {
                format!(
                    "Failed to open file {}",
                    program_config.task_job_desc.display()
                )
            })?;

    Ok(Options {
        root_dir: program_config.root_dir,
        task_job_desc,
        address: program_config.address,
        rabbitmq: program_config.rabbitmq,
        postgres: program_config.postgres,
    })
}

/// The program options of the CLI.
struct ProgramOptions {
    /// The path to the configuration file.
    pub config_path: PathBuf,
}

/// The configuration for the worker agent.
#[derive(Debug, Deserialize)]
pub struct ProgramConfig {
    pub log: LogLevel,
    /// The root directory of the all the tasks and jobs.
    pub root_dir: PathBuf,
    /// The description of the tasks and jobs.
    pub task_job_desc: PathBuf,
    /// The address where to expose the controller REST API.
    pub address: String,
    /// The RabbitMQ options.
    pub rabbitmq: RabbitMQOptions,
    /// The Postgres config.
    pub postgres: PostgresConfig,
}

impl ProgramConfig {
    pub fn print_to_log(&self) {
        info!("Configuration:");
        info!("Log level: {}", self.log);
        info!("Root directory: {}", self.root_dir.display());
        info!("Jobs config File: {}", self.task_job_desc.display());
        info!("RabbitMQ:");
        info!("RabbitMQ Host: {}", self.rabbitmq.host);
        info!("RabbitMQ Port: {}", self.rabbitmq.port);
        info!("RabbitMQ User: {}", self.rabbitmq.user);
        info!("RabbitMQ Password: {}", self.rabbitmq.password);
        info!("Postgres:");
        info!("Postgres Host: {}", self.postgres.host);
        info!("Postgres Port: {}", self.postgres.port);
        info!("Postgres User: {}", self.postgres.user);
        info!("Postgres Password: {}", self.postgres.password);
        info!("Postgres Database: {}", self.postgres.dbname);
    }

    pub fn from_reader<R: Read>(r: R) -> Result<Self> {
        let mut s = String::new();

        let mut r = r;
        r.read_to_string(&mut s)?;

        let config: Self = toml::from_str(&s)?;

        Ok(config)
    }
}

impl TryFrom<ProgramOptions> for ProgramConfig {
    type Error = anyhow::Error;

    fn try_from(value: ProgramOptions) -> Result<Self, Self::Error> {
        let config_path = value.config_path.as_path();
        let r = std::fs::File::open(config_path)
            .with_context(|| format!("Failed to open file {}", config_path.display()))?;
        ProgramConfig::from_reader(r)
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr};

    use crate::LogLevel;

    use super::ProgramConfig;

    #[test]
    fn test_loading_worker_config() {
        let data = include_bytes!("../../example/config.toml");
        let c = ProgramConfig::from_reader(data.as_slice()).unwrap();

        assert_eq!(c.log, LogLevel::Info);
        assert_eq!(c.root_dir.to_string_lossy(), "./task_volume");
        assert_eq!(
            c.task_job_desc,
            PathBuf::from_str("example/task_job_config.yaml").unwrap()
        );
        assert_eq!(c.rabbitmq.host, "localhost");
        assert_eq!(c.rabbitmq.port, 5672);
        assert_eq!(c.rabbitmq.user, "guest");
        assert_eq!(c.rabbitmq.password.secret(), "guest");
    }
}
