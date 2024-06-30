-- Description: This file contains the schema for the states database
-- Create table to store all jobs that are part of a workflow
CREATE TABLE IF NOT EXISTS jobs(
    job_id uuid PRIMARY KEY,
    job_type varchar(255) NOT NULL,
    job_state integer NOT NULL,
    -- Is a redundant column, but is used for faster querying
    job_stage integer NOT NULL,
    -- Is a redundant column, but is used for faster querying
    job_parameters jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL
);

-- Create table to store all tasks that are part of a job
CREATE TABLE IF NOT EXISTS tasks(
    task_id uuid PRIMARY KEY,
    job_id uuid REFERENCES jobs(job_id),
    task_type varchar(255) NOT NULL,
    job_stage integer NOT NULL,
    -- Is a redundant column, but is used for faster querying
    task_state integer NOT NULL,
    task_parameters jsonb NOT NULL,
    created_at timestamp with time zone NOT NULL
);

-- Create table to store all updates that are happening on the states of a job
CREATE TABLE IF NOT EXISTS jobs_updates(
    job_id uuid REFERENCES jobs(job_id),
    job_state integer NOT NULL,
    job_stage integer NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

-- Create table to store all updates that are happening on the states of a task
CREATE TABLE IF NOT EXISTS tasks_updates(
    task_id uuid REFERENCES tasks(task_id),
    task_state integer NOT NULL,
    updated_at timestamp with time zone NOT NULL
);

-- Create index on the task and job state columns
CREATE INDEX IF NOT EXISTS job_state_index ON jobs(job_state);

CREATE INDEX IF NOT EXISTS task_state_index ON tasks(task_state);

-- Function to create a job
CREATE OR REPLACE FUNCTION create_job(job_id uuid, job_type varchar(255), job_parameters jsonb, created_at timestamp with time zone)
    RETURNS VOID
    AS $$
BEGIN
    INSERT INTO jobs(job_id, job_type, job_state, job_stage, job_parameters, created_at)
        VALUES(job_id, job_type, 0, 0, job_parameters, created_at);
    INSERT INTO jobs_updates(job_id, job_state, job_stage, updated_at)
        VALUES(job_id, 0, 0, created_at);
END;
$$
LANGUAGE plpgsql;

-- Function to return a list of the jobs.
CREATE OR REPLACE FUNCTION get_jobs(start_at bigint, max_num bigint)
    RETURNS TABLE(
        job_id uuid,
        job_state integer,
        job_stage integer,
        job_type varchar(255),
        created_at timestamp with time zone,
        updated_at timestamp with time zone
    )
    AS $$
    SELECT
        j.job_id,
        j.job_state,
        j.job_stage,
        j.job_type,
        j.created_at,
        MAX(u.updated_at)
    FROM
        jobs j,
        jobs_updates u
    WHERE
        j.job_id = u.job_id
    GROUP BY
        j.job_id
    ORDER BY
        j.created_at OFFSET get_jobs.start_at
    LIMIT get_jobs.max_num;
$$
LANGUAGE SQL;

-- Function to update the job state. This function will also insert a new row in the jobs_updates
-- table if a job with the give id exists.
-- Returns FALSE if the job_id does not exist and TRUE if the job_id exists.
CREATE OR REPLACE FUNCTION change_job_state(job_id uuid, job_state integer, job_stage integer, updated_at timestamp with time zone)
    RETURNS boolean
    AS $$
BEGIN
    UPDATE
        jobs AS j
    SET
        job_state = change_job_state.job_state
    WHERE
        j.job_id = change_job_state.job_id;
    IF NOT FOUND THEN
        RETURN FALSE;
    END IF;
    INSERT INTO jobs_updates(job_id, job_state, job_stage, updated_at)
        VALUES(job_id, job_state, job_stage, updated_at);
    RETURN TRUE;
END;
$$
LANGUAGE plpgsql;

-- Function to create a task.
-- Returns 0 on success and one of the following error codes on failure:
-- 1: The job_id does not exist
-- 2: The task_id already exists
-- 3: job is not in running state
CREATE OR REPLACE FUNCTION create_task(job_id uuid, task_id uuid, task_type varchar(255), task_parameters jsonb, created_at timestamp with time zone)
    RETURNS integer
    AS $$
DECLARE
    var_job_stage integer;
    var_job_state integer;
BEGIN
    -- get the current stage and state of the corresponding job
    SELECT
        j.job_stage,
        j.job_state INTO var_job_stage,
        var_job_state
    FROM
        jobs j
    WHERE
        j.job_id = create_task.job_id;
    -- check if we found something
    IF NOT FOUND THEN
        RETURN 1;
    END IF;
    -- check if the job is in running state
    IF var_job_state != 2 THEN
        RETURN 3;
    END IF;
    -- now insert into tasks and tasks updates
    INSERT INTO tasks(task_id, job_id, task_type, job_stage, task_state, task_parameters, created_at)
        VALUES (task_id, job_id, task_type, var_job_stage, 0, task_parameters, created_at);
    INSERT INTO tasks_updates(task_id, task_state, updated_at)
        VALUES (task_id, 0, created_at);
    RETURN 0;
EXCEPTION
    WHEN foreign_key_violation THEN
        RETURN 1;
    WHEN unique_violation THEN
        RETURN 2;
END;

$$
LANGUAGE plpgsql;

-- Returns the number of active tasks for the specified job_id
CREATE OR REPLACE FUNCTION num_active_tasks(job_id uuid)
    RETURNS bigint
    AS $$
    SELECT
        COUNT(*) AS num_active_tasks
    FROM
        tasks
    WHERE
        job_id = num_active_tasks.job_id
        AND task_state IN(0, 1, 2)
$$
LANGUAGE SQL;

-- Function for updating the state of a task. Returns one of the following return codes:
-- -2: The new task state is invalid
-- -1: The task_id does not exist
-- 0: Success and the corresponding job stage is finished
-- 1: Success and the corresponding job stage is not finished
CREATE OR REPLACE FUNCTION change_task_state(task_id uuid, task_state integer, updated_at timestamp with time zone)
    RETURNS integer
    AS $$
DECLARE
    var_job_id uuid;
    var_task_state integer;
BEGIN
    -- get the current state of the task and the job_id
    SELECT
        t.job_id,
        t.task_state INTO var_job_id,
        var_task_state
    FROM
        tasks t
    WHERE
        t.task_id = change_task_state.task_id;
    IF NOT FOUND THEN
        RETURN -1;
    END IF;
    -- check if the state switch is invalid. This is either the case when
    -- (a): the task is already done, i.e., state 3 or 4
    -- (b): the new state less or equal to the current state
    IF var_task_state IN (3, 4) OR task_state <= var_task_state THEN
        RETURN -2;
    END IF;
    -- now update the task and insert into the tasks_updates table
    UPDATE
        tasks AS t
    SET
        task_state = change_task_state.task_state
    WHERE
        t.task_id = change_task_state.task_id;
    -- insert into the tasks_updates table
    INSERT INTO tasks_updates(task_id, task_state, updated_at)
        VALUES (task_id, task_state, updated_at);
    -- returns 1 if num_active_tasks is greater than 0, 0 otherwise
    IF num_active_tasks(var_job_id) > 0 THEN
        RETURN 1;
    ELSE
        RETURN 0;
    END IF;
END;
$$
LANGUAGE plpgsql;

-- Get the details for the specified job.
CREATE OR REPLACE FUNCTION get_job_details(in_job_id uuid)
    RETURNS TABLE(
        job_id uuid,
        job_state integer,
        job_stage integer,
        job_type varchar(255),
        created_at timestamp with time zone,
        updated_at timestamp with time zone,
        input_parameters jsonb
    )
    AS $$
    SELECT
        j.job_id,
        j.job_state,
        j.job_stage,
        j.job_type,
        j.created_at,
        MAX(u.updated_at),
        j.job_parameters
    FROM
        jobs j,
        jobs_updates u
    WHERE
        j.job_id = get_job_details.in_job_id
        AND j.job_id = u.job_id
    GROUP BY
        j.job_id;
$$
LANGUAGE SQL;

-- Returns the tasks summary for the specified job in the specified stage.
CREATE OR REPLACE FUNCTION get_job_stage_tasks_summary(in_job_id uuid, in_job_stage integer)
    RETURNS TABLE(
        total_count bigint,
        not_started_count bigint,
        queued_count bigint,
        running_count bigint,
        finished_count bigint,
        failed_count bigint
    )
    AS $$
    SELECT
        COUNT(*) AS total_count,
        SUM(
            CASE WHEN t.task_state = 0 THEN
                1
            ELSE
                0
            END) AS not_started_count,
        SUM(
            CASE WHEN t.task_state = 1 THEN
                1
            ELSE
                0
            END) AS queued_count,
        SUM(
            CASE WHEN t.task_state = 2 THEN
                1
            ELSE
                0
            END) AS running_count,
        SUM(
            CASE WHEN t.task_state = 3 THEN
                1
            ELSE
                0
            END) AS finished_count,
        SUM(
            CASE WHEN t.task_state = 4 THEN
                1
            ELSE
                0
            END) AS failed_count
    FROM
        tasks t
    WHERE
        t.job_id = get_job_stage_tasks_summary.in_job_id
        AND t.job_stage = get_job_stage_tasks_summary.in_job_stage;
$$
LANGUAGE SQL;

