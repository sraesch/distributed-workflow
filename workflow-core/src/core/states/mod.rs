pub mod postgres;

use std::future::Future;

use serde::{Deserialize, Serialize};

use crate::{Id, JobState, ParameterSet, Result, Status, TimeStamp};

/// The result of a job list query.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobList {
    /// The total number of jobs in the workflow engine.
    pub total_count: u64,

    /// The jobs that match the query.
    pub jobs: Vec<JobListEntry>,
}

/// The details of a job.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobDetails {
    pub job_id: Id,
    pub job_type: String,
    pub job_state: JobState,
    pub stage: usize,
    pub tasks_summary: TaskSummary,
    pub created_at: TimeStamp,
    pub updated_at: TimeStamp,
    pub input: ParameterSet,
}

/// The summary of the tasks of a job.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskSummary {
    pub total_count: u64,
    pub not_started_count: u64,
    pub queued_count: u64,
    pub running_count: u64,
    pub finished_count: u64,
    pub failed_count: u64,
}

/// The details of a task.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskListEntry {
    pub task_id: Id,
    pub task_name: String,
    pub task_state: Status,
    pub stage: usize,
    pub created_at: TimeStamp,
    pub updated_at: TimeStamp,
}

/// The resulting list of tasks that match the query.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobTasks {
    /// The total number of tasks in related to the job.
    pub total_count: u64,
    /// The tasks that match the query.
    pub tasks: Vec<TaskListEntry>,
}

/// An entry in the result of a job list query.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JobListEntry {
    /// The id of the job.
    pub job_id: Id,

    /// The type of the job, e.g., the name of the job template.
    pub job_type: String,

    /// The state of the job.
    pub job_state: JobState,

    /// The timestamp when the job was created.
    pub created_at: TimeStamp,

    /// The timestamp when the job was last updated.
    pub updated_at: TimeStamp,
}

/// The backend where the states of jobs and tasks are stored.
pub trait StatesBackend: Send + Sync {
    /// Registers a new job with the given data and returns the id of the job.
    /// The job is initially in the not started state.
    ///
    /// # Arguments
    /// * `job_type` - The type of the job, e.g., the name of the job template.
    /// * `timestamp` - The timestamp when the job was created.
    /// * `parameters` - The input parameters for the job as key-value pairs.
    fn register_new_job_with_timestamp(
        &self,
        job_type: &str,
        timestamp: TimeStamp,
        parameters: ParameterSet,
    ) -> impl Future<Output = Result<Id>> + Send;

    /// Registers a new job with the given data and returns the id of the job.
    /// The job is initially in the not started state.
    ///
    /// # Arguments
    /// * `job_type` - The type of the job, e.g., the name of the job template.
    /// * `parameters` - The input parameters for the job as key-value pairs.
    fn register_new_job(
        &self,
        job_type: &str,
        parameters: ParameterSet,
    ) -> impl Future<Output = Result<Id>> + Send {
        let timestamp = chrono::Local::now();
        self.register_new_job_with_timestamp(job_type, timestamp, parameters)
    }

    /// Returns the state of the job with the given id or None if the job was not found.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    fn job_state(&self, job_id: &Id) -> impl Future<Output = Result<Option<JobState>>> + Send;

    /// Returns a list of jobs with the given offset and limit. Also returns the total number of
    /// jobs.
    ///
    /// # Arguments
    /// * `offset` - The offset of the first job to return.
    /// * `limit` - The maximum number of jobs to return.
    fn list_jobs(&self, offset: u64, limit: u64) -> impl Future<Output = Result<JobList>> + Send;

    /// Returns the details of the job with the given id or None if the job was not found.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    fn job(&self, job_id: &Id) -> impl Future<Output = Result<Option<JobDetails>>> + Send;

    /// Returns the tasks of the job that match the given offset, limit, and stage.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    /// * `offset` - The offset of the first task to return.
    /// * `limit` - The maximum number of tasks to return.
    /// * `stage` - The stage of the tasks to return or None to return tasks from all stages.
    fn job_tasks(
        &self,
        job_id: &Id,
        offset: u64,
        limit: u64,
        stage: Option<usize>,
    ) -> impl Future<Output = Result<JobTasks>> + Send;

    /// Updates the state of a job. Returns an error if the job with the given id was not found.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    /// * `state` - The new state of the job.
    /// * `update_timestamp` - The timestamp when the job was updated.
    fn update_job_state_with_timestamp(
        &self,
        job_id: &Id,
        state: JobState,
        update_timestamp: TimeStamp,
    ) -> impl Future<Output = Result<()>> + Send;

    /// Updates the state of a job. Returns an error if the job with the given id was not found.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    /// * `state` - The new state of the job.
    fn update_job_state(
        &self,
        job_id: &Id,
        state: JobState,
    ) -> impl Future<Output = Result<()>> + Send {
        let update_timestamp = chrono::Local::now();
        self.update_job_state_with_timestamp(job_id, state, update_timestamp)
    }

    /// Registers the given tasks. The tasks are initially in the not started state.
    /// Returns an error if either
    /// - the job with the given id was not found or
    /// - the job is not in the running state.
    /// The function returns the ids of the inserted tasks.
    ///
    /// # Arguments
    /// * `job_id` - The id of the owning job.
    /// * `task_type` - The type of the tasks.
    /// * `job_stage` - The stage of the job to which the tasks belong.
    /// * `timestamp` - The timestamp when the tasks were created.
    /// * `task_parameter_sets` - The input parameter sets for the tasks to insert.
    fn register_new_tasks_with_timestamp(
        &self,
        job_id: &Id,
        task_type: &str,
        timestamp: TimeStamp,
        task_parameter_sets: &[&ParameterSet],
    ) -> impl Future<Output = Result<Vec<Id>>> + Send;

    /// Registers the given tasks. The tasks are initially in the not started state.
    /// Returns an error if either
    /// - the job with the given id was not found or
    /// - the job is not in the running state.
    ///
    /// # Arguments
    /// * `job_id` - The id of the owning job.
    /// * `task_type` - The type of the tasks.
    /// * `job_stage` - The stage of the job to which the tasks belong.
    /// * `task_parameter_sets` - The input parameter sets for the tasks to insert.
    fn register_new_tasks(
        &self,
        job_id: &Id,
        task_type: &str,
        task_parameter_sets: &[&ParameterSet],
    ) -> impl Future<Output = Result<Vec<Id>>> + Send {
        let timestamp = chrono::Local::now();
        self.register_new_tasks_with_timestamp(job_id, task_type, timestamp, task_parameter_sets)
    }

    /// Updates the state of a task and also decrements the number of queued tasks in the owning
    /// job if the task is finished or failed. Returns true if the current stage of the job is
    /// finished.
    /// Returns an error if either
    /// - the task with the given id was not found or
    /// - the corresponding job is not in the running state.
    ///
    /// # Arguments
    /// * `task_id` - The id of the task.
    /// * `state` - The new state of the task.
    /// * `update_timestamp` - The timestamp when the task was updated.
    fn update_task_state_with_timestamp(
        &self,
        task_id: &Id,
        state: Status,
        update_timestamp: TimeStamp,
    ) -> impl Future<Output = Result<bool>> + Send;

    /// Updates the state of a task and also decrements the number of queued tasks in the owning
    /// job if the task is finished or failed. Returns true if the current stage of the job is
    /// finished.
    /// Returns an error if either
    /// - the task with the given id was not found or
    /// - the corresponding job is not in the running state.
    ///
    /// # Arguments
    /// * `task_id` - The id of the task.
    /// * `state` - The new state of the task.
    fn update_task_state(
        &self,
        task_id: &Id,
        state: Status,
    ) -> impl Future<Output = Result<bool>> + Send {
        let update_timestamp = chrono::Local::now();
        self.update_task_state_with_timestamp(task_id, state, update_timestamp)
    }

    /// Returns the number of tasks for the given job that nor net finished or failed.
    ///
    /// # Arguments
    /// * `job_id` - The id of the job.
    fn num_active_tasks(&self, job_id: &Id) -> impl Future<Output = Result<usize>> + Send;

    /// Returns the state of the task with the given id or None if the task was not found.
    ///
    /// # Arguments
    /// * `task_id` - The id of the task.
    fn task_state(&self, task_id: &Id) -> impl Future<Output = Result<Option<Status>>> + Send;
}
