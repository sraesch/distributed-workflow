use std::future::Future;

use chrono::{DateTime, Local};
use deadpool_postgres::{Config, Pool};
use log::{error, info, trace};
use serde::Deserialize;
use tokio_postgres::{NoTls, Row};
use uuid::Uuid;

use crate::{
    Error, Id, JobDetails, JobList, JobListEntry, JobState, JobTasks, ParameterSet,
    Result as WfResult, Secret, StatesBackend, Status, TaskListEntry, TaskSummary, TimeStamp,
};

/// Postgres based implementation of the state backend.
pub struct PostgresBackend {
    /// The postgres connection pool.
    pool: Pool,
}

/// The configuration for connecting to the postgres database.
#[derive(Clone, Debug, Deserialize)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Secret,
    pub dbname: String,
}

impl PostgresBackend {
    /// Create a new PostgresBackend instance.
    ///
    /// # Arguments
    /// * `config` - The configuration for the postgres connection.
    pub async fn new(config: PostgresConfig) -> WfResult<Self> {
        // create the connection pool configuration
        let mut pool_config = Config::new();
        pool_config.user = Some(config.user);
        pool_config.password = Some(config.password.secret().to_string());
        pool_config.dbname = Some(config.dbname);
        pool_config.host = Some(config.host);
        pool_config.port = Some(config.port);

        // create the connection pool
        let pool = match pool_config.create_pool(None, NoTls) {
            Ok(pool) => pool,
            Err(e) => return Err(Error::DBCreatePoolError(Box::new(e))),
        };

        // initialize the database schema
        info!("Initializing database schema...");
        if let Err(err) = Self::initialize_db(&pool).await {
            error!("Failed to initialize database schema: {}", err);
            return Err(err);
        }

        info!("Initializing database schema...DONE");

        Ok(Self { pool })
    }

    /// Initialize the database schema.
    ///
    /// # Arguments
    /// * `pool` - The connection pool to use for the database schema initialization.
    async fn initialize_db(pool: &Pool) -> WfResult<()> {
        let client = match pool.get().await {
            Ok(client) => client,
            Err(e) => return Err(Error::DBPoolError(Box::new(e))),
        };

        // Create database schema which is defined in the schema.sql file.
        // The schema has to be applied s.t. nothing is done if the schema already exists.
        let schema_sql = include_str!("schema.sql");
        trace!("Schema SQL: {}", schema_sql);

        if let Err(e) = client.batch_execute(schema_sql).await {
            return Err(Error::DBError(Box::new(e)));
        }

        Ok(())
    }

    /// Get a client from the connection pool.
    async fn get_client(&self) -> WfResult<deadpool_postgres::Client> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::DBPoolError(Box::new(e)))
    }
}

impl StatesBackend for PostgresBackend {
    async fn register_new_job_with_timestamp(
        &self,
        job_type: &str,
        timestamp: chrono::DateTime<chrono::Local>,
        parameters: std::collections::HashMap<String, String>,
    ) -> WfResult<Id> {
        let job_id = Id::new();

        // serialize the parameters to JSON
        let job_parameters = match serde_json::to_value(&parameters) {
            Ok(json) => json,
            Err(e) => {
                return Err(Error::InternalError(format!(
                    "Failed to serialize parameters: {}",
                    e
                )))
            }
        };

        let client = self.get_client().await?;

        client
            .execute_statement(
                "SELECT create_job($1, $2, $3, $4)",
                &[&job_id.into_inner(), &job_type, &job_parameters, &timestamp],
            )
            .await?;

        Ok(job_id)
    }

    async fn job_state(&self, job_id: &Id) -> WfResult<Option<JobState>> {
        let client = self.get_client().await?;
        let row = client
            .query_0_or_1(
                "SELECT job_state, job_stage FROM jobs WHERE job_id = $1",
                &[&job_id.into_inner()],
            )
            .await?;

        match row {
            Some(row) => {
                let status: i32 = row.get(0);
                let stage: i32 = row.get(1);

                let status = Status::try_from(status)?;
                let stage = stage as usize;

                Ok(Some(JobState { status, stage }))
            }
            None => Ok(None),
        }
    }

    async fn list_jobs(&self, offset: u64, limit: u64) -> WfResult<JobList> {
        let client = self.get_client().await?;

        let offset = offset as i64;
        let limit = limit as i64;
        let rows = client
            .query_n("SELECT * FROM get_jobs($1, $2);", &[&offset, &limit])
            .await?;

        let mut jobs = Vec::new();
        for row in rows {
            let job_id: Uuid = row.get(0);
            let job_id = Id::from(job_id);
            let status: i32 = row.get(1);
            let status = Status::try_from(status)?;
            let stage: i32 = row.get(2);
            let stage = stage as usize;
            let job_type: String = row.get(3);
            let created_at: DateTime<Local> = row.get(4);
            let updated_at: DateTime<Local> = row.get(5);

            let job_state = JobState { status, stage };

            jobs.push(JobListEntry {
                job_id,
                job_state,
                job_type,
                created_at,
                updated_at,
            });
        }

        // query the total amount of jobs
        let row = client.query_1("SELECT COUNT(*) FROM jobs", &[]).await?;
        let total_count: i64 = row.get(0);
        let total_count = total_count as u64;

        Ok(JobList { jobs, total_count })
    }

    async fn job(&self, job_id: &Id) -> WfResult<Option<JobDetails>> {
        let client = self.get_client().await?;

        let row = client
            .query_0_or_1("SELECT * FROM get_job_details($1)", &[&job_id.into_inner()])
            .await?;

        let row = match row {
            Some(row) => row,
            None => return Ok(None),
        };

        let job_id: Uuid = row.get(0);
        let job_id = Id::from(job_id);
        let status: i32 = row.get(1);
        let status = Status::try_from(status)?;
        let stage: i32 = row.get(2);
        let stage = stage as usize;
        let job_type: String = row.get(3);
        let created_at: DateTime<Local> = row.get(4);
        let updated_at: DateTime<Local> = row.get(5);
        let input_parameters: serde_json::Value = row.get(6);
        let input_parameters: ParameterSet = match serde_json::from_value(input_parameters) {
            Ok(parameters) => parameters,
            Err(e) => {
                return Err(Error::InternalError(format!(
                    "Failed to deserialize input parameters: {}",
                    e
                )))
            }
        };

        Ok(Some(JobDetails {
            job_id,
            job_state: JobState { status, stage },
            job_type,
            created_at,
            updated_at,
            input: input_parameters,
        }))
    }

    async fn job_tasks(
        &self,
        job_id: &Id,
        offset: u64,
        limit: u64,
        stage: Option<usize>,
    ) -> WfResult<JobTasks> {
        let client = self.get_client().await?;

        let offset = offset as i64;
        let limit = limit as i64;
        let rows = if let Some(job_stage) = stage {
            let job_stage = job_stage as i32;

            client
            .query_n(
                "SELECT t.task_id, t.task_type, t.task_state, t.job_stage, t.created_at, MAX(u.updated_at)
                        FROM tasks t, tasks_updates u
                        WHERE t.job_id = $1 AND t.job_stage = $2 AND t.task_id = u.task_id
                        GROUP BY t.task_id
                        ORDER BY t.created_at
                        OFFSET $3 LIMIT $4;",
                &[&job_id.into_inner(), &job_stage, &offset, &limit],
            )
            .await?
        } else {
            client
            .query_n(
                "SELECT t.task_id, t.task_type, t.task_state, t.job_stage, t.created_at, MAX(u.updated_at)
                        FROM tasks t, tasks_updates u
                        WHERE t.job_id = $1 AND t.task_id = u.task_id
                        GROUP BY t.task_id
                        ORDER BY t.created_at
                        OFFSET $2 LIMIT $3;",
                &[&job_id.into_inner(), &offset, &limit],
            )
            .await?
        };

        let mut tasks = Vec::new();
        for row in rows {
            let task_id: Uuid = row.get(0);
            let task_id = Id::from(task_id);
            let task_name: String = row.get(1);
            let task_state: i32 = row.get(2);
            let task_state = Status::try_from(task_state)?;
            let stage: i32 = row.get(3);
            let stage = stage as usize;
            let created_at: DateTime<Local> = row.get(4);
            let updated_at: DateTime<Local> = row.get(5);

            tasks.push(TaskListEntry {
                task_id,
                task_state,
                task_name,
                stage,
                created_at,
                updated_at,
            });
        }

        // query the total amount of tasks related to the given job
        let row = client
            .query_1(
                "SELECT COUNT(*) FROM tasks WHERE job_id = $1",
                &[&job_id.into_inner()],
            )
            .await?;
        let total_count: i64 = row.get(0);
        let total_count = total_count as u64;

        Ok(JobTasks { total_count, tasks })
    }

    async fn update_job_state_with_timestamp(
        &self,
        job_id: &Id,
        state: JobState,
        update_timestamp: TimeStamp,
    ) -> WfResult<()> {
        let client = self.get_client().await?;

        let new_status = i32::from(state.status);
        let new_stage = state.stage as i32;

        let anything_changed = client
            .query_1(
                "SELECT change_job_state($1, $2, $3, $4)",
                &[
                    &job_id.into_inner(),
                    &new_status,
                    &new_stage,
                    &update_timestamp,
                ],
            )
            .await?;

        let anything_changed: bool = anything_changed.get(0);

        // if no rows were changed, the job was not found
        if !anything_changed {
            return Err(Error::JobNotFound(*job_id));
        }

        Ok(())
    }

    async fn register_new_tasks_with_timestamp(
        &self,
        job_id: &Id,
        task_type: &str,
        timestamp: TimeStamp,
        task_parameter_sets: &[&ParameterSet],
    ) -> WfResult<Vec<Id>> {
        // insert tasks into the database
        let client = self.get_client().await?;
        let stmt: tokio_postgres::Statement = client
            .prepare("SELECT create_task($1, $2, $3, $4, $5);")
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;

        // insert the tasks into the database
        let mut task_ids = Vec::with_capacity(task_parameter_sets.len());
        for task_parameter_set in task_parameter_sets.iter() {
            let task_id = Id::new();
            task_ids.push(task_id);

            // encode the parameter set as json
            let task_parameters = match serde_json::to_value(task_parameter_set) {
                Ok(json) => json,
                Err(e) => {
                    return Err(Error::InternalError(format!(
                        "Failed to serialize task parameters: {}",
                        e
                    )))
                }
            };

            // insert the task into the DB
            let row = client
                .query_one(
                    &stmt,
                    &[
                        &job_id.into_inner(),
                        &task_id.into_inner(),
                        &task_type,
                        &task_parameters,
                        &timestamp,
                    ],
                )
                .await
                .map_err(|e| Error::DBError(Box::new(e)))?;

            // evaluate the return code of the function
            let ret_code: i32 = row.get(0);
            match ret_code {
                0 => {}
                1 => return Err(Error::JobNotFound(*job_id)),
                2 => {
                    return Err(Error::InternalError(format!(
                        "Task {} already exists",
                        task_id
                    )))
                }
                3 => return Err(Error::JobNotRunning(*job_id)),
                _ => return Err(Error::InternalError("Unknown return code".to_string())),
            }
        }

        Ok(task_ids)
    }

    async fn update_task_state_with_timestamp(
        &self,
        task_id: &Id,
        state: Status,
        update_timestamp: TimeStamp,
    ) -> WfResult<bool> {
        let client = self.get_client().await?;

        let row = client
            .query_1(
                "SELECT change_task_state($1, $2, $3)",
                &[&task_id.into_inner(), &i32::from(state), &update_timestamp],
            )
            .await?;

        let ret_code: i32 = row.get(0);

        match ret_code {
            -2 => Err(Error::TaskInvalidStateSwitch(*task_id, state)),
            -1 => Err(Error::TaskNotFound(*task_id)),
            0 => Ok(true),
            1 => Ok(false),
            _ => Err(Error::InternalError(format!(
                "Unknown return code: {}",
                ret_code
            ))),
        }
    }

    async fn num_active_tasks(&self, job_id: &Id) -> WfResult<usize> {
        let client = self.get_client().await?;
        let row = client
            .query_1("SELECT num_active_tasks($1)", &[&job_id.into_inner()])
            .await?;

        let count: i64 = row.get(0);

        Ok(count as usize)
    }

    async fn task_state(&self, task_id: &Id) -> WfResult<Option<Status>> {
        let client = self.get_client().await?;

        let row = client
            .query_0_or_1(
                "SELECT task_state FROM tasks WHERE task_id = $1",
                &[&task_id.into_inner()],
            )
            .await?;

        match row {
            Some(row) => {
                let task_state: i32 = row.get(0);
                Ok(Some(Status::try_from(task_state)?))
            }
            None => Ok(None),
        }
    }

    async fn job_stage_tasks(&self, job_id: &Id, stage: usize) -> WfResult<TaskSummary> {
        let client = self.get_client().await?;

        let stage = stage as i32;

        let row = client
            .query_1(
                "SELECT * FROM get_job_stage_tasks_summary($1, $2)",
                &[&job_id.into_inner(), &stage],
            )
            .await?;

        let total_count: i64 = row.get(0);
        let not_started_count: Option<i64> = row.get(1);
        let queued_count: Option<i64> = row.get(2);
        let running_count: Option<i64> = row.get(3);
        let finished_count: Option<i64> = row.get(4);
        let failed_count: Option<i64> = row.get(5);

        Ok(TaskSummary {
            total_count: total_count as u64,
            not_started_count: not_started_count.unwrap_or_default() as u64,
            queued_count: queued_count.unwrap_or_default() as u64,
            running_count: running_count.unwrap_or_default() as u64,
            finished_count: finished_count.unwrap_or_default() as u64,
            failed_count: failed_count.unwrap_or_default() as u64,
        })
    }
}

trait SQLClientFunctionalities {
    /// Executes a SQL statement that does not return a result.
    ///
    /// # Arguments
    /// * `query` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    fn execute_statement(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = WfResult<u64>> + Send;

    /// Queries 0 or 1 row from the database and returns an error if there are more than 1 rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_0_or_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = WfResult<Option<Row>>> + Send;

    /// Queries 1 row from the database and returns an error if there is less or more rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = WfResult<Row>> + Send;

    /// Queries rows from the database.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    fn query_n(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> impl Future<Output = WfResult<Vec<Row>>> + Send;
}

impl SQLClientFunctionalities for deadpool_postgres::Client {
    /// Executes a SQL statement that does not return a result.
    ///
    /// # Arguments
    /// * `query` - The SQL query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn execute_statement(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> WfResult<u64> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.execute(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries 0 or 1 row from the database and returns an error if there are more than 1 rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_0_or_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> WfResult<Option<Row>> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.query_opt(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries 1 row from the database and returns an error if there is less or more rows.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_1(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> WfResult<Row> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        self.query_one(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }

    /// Queries rows from the database.
    ///
    /// # Arguments
    /// * `query` - The SQL select query to execute.
    /// * `params` - The parameters to pass to the query.
    async fn query_n(
        &self,
        query: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
    ) -> WfResult<Vec<Row>> {
        let stmt = self
            .prepare(query)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;

        // execute the query
        self.query(&stmt, params)
            .await
            .map_err(|e| Error::DBError(Box::new(e)))
    }
}
