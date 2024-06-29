use std::{collections::HashMap, io::Read, sync::Arc};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// The description of jobs and tasks that can be executed by a worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskJobDesc {
    /// The tasks that can be executed by a job.
    pub tasks: HashMap<String, TaskDefinition>,
    /// The jobs that can be executed by a worker.
    pub jobs: HashMap<String, JobDefinition>,
}

impl TaskJobDesc {
    /// Reads the task and job configuration from a reader and checks if the configuration is
    /// valid.
    ///
    /// # Arguments
    /// * `r` - The reader to read the configuration from.
    pub fn from_reader<R: Read>(r: R) -> Result<Self> {
        let config: Self =
            serde_yaml::from_reader(r).map_err(|e| Error::ParsingConfigError(Arc::new(e)))?;

        config.is_valid()?;

        Ok(config)
    }

    /// Checks if the configuration is valid.
    pub fn is_valid(&self) -> Result<()> {
        // create regex to find placeholders in the exec_command
        let re = Regex::new(r"\{\{([\w,\-]+)\}\}").unwrap();

        // first check if all tasks are valid
        for (task_name, task) in self.tasks.iter() {
            if task.exec_command.is_empty() {
                return Err(Error::InvalidConfigError(format!(
                    "Task {} has no exec_command",
                    task_name
                )));
            }

            // iterate over the arguments of the exec_command
            for arg in task.exec_args.iter() {
                // check if the argument is a placeholder
                for (_, [identifier]) in re.captures_iter(arg).map(|caps| caps.extract()) {
                    if !task.input.contains_key(identifier) {
                        return Err(Error::InvalidConfigError(format!(
                            "Task {} has an invalid input identifier in exec_args: {}",
                            task_name, identifier
                        )));
                    }
                }
            }
        }

        for (job_name, job_def) in self.jobs.iter() {
            for job_stage in job_def.stages.iter() {
                // try to find the task definition for the task class
                let task_def = if let Some(task_def) = self.tasks.get(job_stage.task_class.as_str())
                {
                    task_def
                } else {
                    return Err(Error::InvalidConfigError(
                        "No task definition for the task class 'task'".to_string(),
                    ));
                };

                // check the spawn task if defined
                if let Some(spawn_task_identifier) = job_stage.spawn_task.as_ref() {
                    if !self.tasks.contains_key(spawn_task_identifier) {
                        return Err(Error::InvalidConfigError(format!(
                            "Job {} has an invalid spawn task identifier: {}",
                            job_name, spawn_task_identifier
                        )));
                    }
                } else {
                    // if no spawn task is defined, only the job parameter can be used as input for the
                    // direct input parameters of the task.
                    for (input_name, input) in task_def.input.iter() {
                        if input.source_type == TaskInputSourceType::Parameter
                            && !job_def.input.contains_key(input_name)
                        {
                            return Err(Error::InvalidConfigError(format!(
                                "Task {} requires input parameter {} but job {} does not provide it.",
                                job_stage.task_class, job_name, input_name
                            )));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// The definition of a single task that can be executed by a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// A small description of what the task is doing.
    pub description: String,
    /// The inputs for the task.
    #[serde(default)]
    pub input: HashMap<String, TaskInput>,
    /// The command to execute on the worker
    pub exec_command: String,
    /// The arguments to pass to the command
    #[serde(default)]
    pub exec_args: Vec<String>,
}

/// A single input for a task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    /// A small description of what the input is used for.
    pub description: String,
    /// If true, the input value is a secret and should not be logged.
    #[serde(default)]
    pub secret: bool,
    /// The source type, where the input value is coming from.
    pub source_type: TaskInputSourceType,
}

/// The source, where the input value is coming from.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone, Copy)]
pub enum TaskInputSourceType {
    /// The input value is coming from a parameter that has to be explicitly be set, e.g., job
    /// input parameter or set by a spawn task.
    #[serde(rename = "parameter")]
    Parameter,
    /// The input value is coming from an environment variable.
    #[serde(rename = "environment")]
    Environment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDefinition {
    /// A small description of what the job is doing.
    pub description: String,
    /// The job inputs
    #[serde(default)]
    pub input: HashMap<String, JobInput>,
    /// The stages of the job
    pub stages: Vec<JobStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobInput {
    /// A small description of what the input is used for.
    pub description: String,
    /// The source, where the input value is coming from.
    pub source_type: JobInputSourceType,
    /// If true, the input value is a secret and should not be logged.
    #[serde(default)]
    pub secret: bool,
    /// Optionally, the key value if the source is http-headers.
    pub key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
pub enum JobInputSourceType {
    /// The input value is coming from a parameter that has to be explicitly be set through the job
    /// request.
    #[serde(rename = "parameter")]
    Parameter,

    /// The input value is coming from HTTP headers.
    #[serde(rename = "http-header")]
    HTTPHeader,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStage {
    /// The name of the stage.
    pub name: String,
    /// Optionally, a special task that should be executed before the stage to spawn the tasks.
    pub spawn_task: Option<String>,
    /// The class of task that should be executed in this stage.
    pub task_class: String,
    /// If true, the job will fail if the task fails.
    /// If false, the job will continue with the next stage.
    pub job_failed_on_error: bool,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_task_job_config_example() {
        let config_source = include_bytes!("../../example/task_job_config.yaml");
        let config_reader = std::io::Cursor::new(config_source);
        TaskJobDesc::from_reader(config_reader).unwrap();
    }
}
