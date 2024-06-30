use std::future::Future;

use chrono::{DateTime, Local};
use deadpool_postgres::{Config, Pool};
use log::{error, info, trace};
use serde::Deserialize;
use tokio_postgres::{error::SqlState, NoTls, Row};
use uuid::Uuid;

use crate::{
    Error, Id, JobDetails, JobList, JobListEntry, JobState, JobTasks, ParameterSet,
    Result as WfResult, Secret, StatesBackend, Status, TaskListEntry, TimeStamp,
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
        let job_state = i32::from(Status::NotStarted);
        let job_stage = 0i32;

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

        client.execute_statement(
            "INSERT INTO jobs (job_id, job_type, job_state, job_stage, job_parameters, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
            &[&job_id.into_inner(), &job_type, &job_state, &job_stage, &job_parameters, &timestamp],
        )
        .await?;

        client.execute_statement(
            "INSERT INTO jobs_updates (job_id, job_state, job_stage, updated_at) VALUES ($1, $2, $3, $4)",
            &[&job_id.into_inner(), &job_state, &job_stage, &timestamp],
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
        let rows = client.query_n("SELECT j.job_id, j.job_state, j.job_stage, j.job_type, j.created_at, MAX(u.updated_at)
                        FROM jobs j, jobs_updates u
                        WHERE j.job_id = u.job_id
                        GROUP BY j.job_id
                        ORDER BY j.created_at
                        OFFSET $1 LIMIT $2;", &[&offset, &limit]).await?;

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
        todo!()
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

        let num_changes = client
            .execute_statement(
                "UPDATE jobs SET job_state = $1, job_stage = $2 WHERE job_id = $3",
                &[&new_status, &new_stage, &job_id.into_inner()],
            )
            .await?;

        // if no rows were changed, the job was not found
        if num_changes == 0 {
            return Err(Error::JobNotFound(*job_id));
        }

        // update the jobs_updates table
        client.execute_statement(
            "INSERT INTO jobs_updates (job_id, job_state, job_stage, updated_at) VALUES ($1, $2, $3, $4)",
            &[&job_id.into_inner(), &new_status, &new_stage, &update_timestamp],
        ).await?;

        Ok(())
    }

    async fn register_new_tasks_with_timestamp(
        &self,
        job_id: &Id,
        task_type: &str,
        timestamp: TimeStamp,
        task_parameter_sets: &[&ParameterSet],
    ) -> WfResult<Vec<Id>> {
        // check that the associated job exists and is in the running state
        let job_state = match self.job_state(job_id).await? {
            Some(state) => state,
            None => return Err(Error::JobNotFound(*job_id)),
        };

        if job_state.status != Status::Running {
            return Err(Error::JobNotRunning(*job_id));
        }

        let job_stage = job_state.stage as i32;

        // prepare the insert statements
        let client = self.get_client().await?;
        let insert_task_statement = client
            .prepare("INSERT INTO tasks (job_id, task_id, task_type, job_stage, task_state, task_parameters, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)")
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;
        let insert_task_update_statement = client
            .prepare(
                "INSERT INTO tasks_updates (task_id, task_state, updated_at) VALUES ($1, $2, $3)",
            )
            .await
            .map_err(|e| Error::DBError(Box::new(e)))?;

        // insert the tasks into the database
        let mut task_ids = Vec::with_capacity(task_parameter_sets.len());
        let task_state = i32::from(Status::NotStarted);
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
            if let Err(err) = client
                .execute(
                    &insert_task_statement,
                    &[
                        &job_id.into_inner(),
                        &task_id.into_inner(),
                        &task_type,
                        &job_stage,
                        &task_state,
                        &task_parameters,
                        &timestamp,
                    ],
                )
                .await
            {
                match err.code().cloned() {
                    Some(SqlState::UNIQUE_VIOLATION) => {
                        return Err(Error::InternalError(format!(
                            "Task with id {} already exists, error in id generation",
                            task_id
                        )));
                    }
                    _ => return Err(Error::DBError(Box::new(err))),
                }
            }

            // insert task update into the DB
            client
                .execute(
                    &insert_task_update_statement,
                    &[&task_id.into_inner(), &task_state, &timestamp],
                )
                .await
                .map_err(|e| Error::DBError(Box::new(e)))?;
        }

        Ok(task_ids)
    }

    async fn update_task_state(&self, task_id: &Id, state: Status) -> WfResult<bool> {
        todo!()
    }

    async fn num_active_tasks(&self, job_id: &Id) -> WfResult<usize> {
        todo!()
    }

    async fn task_state(&self, task_id: &Id) -> WfResult<Option<Status>> {
        todo!()
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
