-- Description: This file contains the schema for the states database
-- Create table to store all jobs that are part of a workflow
CREATE TABLE IF NOT EXISTS jobs (
    job_id UUID PRIMARY KEY,
    job_type VARCHAR(255) NOT NULL,
    job_state INTEGER NOT NULL,
    -- Is a redundant column, but is used for faster querying
    job_stage INTEGER NOT NULL,
    -- Is a redundant column, but is used for faster querying
    job_parameters JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Create table to store all tasks that are part of a job
CREATE TABLE IF NOT EXISTS tasks (
    task_id UUID PRIMARY KEY,
    job_id UUID REFERENCES jobs(job_id),
    task_type VARCHAR(255) NOT NULL,
    job_stage INTEGER NOT NULL,
    -- Is a redundant column, but is used for faster querying
    task_state INTEGER NOT NULL,
    task_parameters JSONB NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Create table to store all updates that are happening on the states of a job
CREATE TABLE IF NOT EXISTS jobs_updates (
    job_id UUID REFERENCES jobs(job_id),
    job_state INTEGER NOT NULL,
    job_stage INTEGER NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Create table to store all updates that are happening on the states of a task
CREATE TABLE IF NOT EXISTS tasks_updates (
    task_id UUID REFERENCES tasks(task_id),
    task_state INTEGER NOT NULL,
    updated_at TIMESTAMP WITH TIME ZONE NOT NULL
);

-- Create index on the task and job state columns
CREATE INDEX IF NOT EXISTS job_state_index ON jobs(job_state);

CREATE INDEX IF NOT EXISTS task_state_index ON tasks(task_state);

-- Function to create a job
CREATE
OR REPLACE FUNCTION create_job(
    job_id UUID,
    job_type VARCHAR(255),
    job_parameters JSONB,
    created_at TIMESTAMP WITH TIME ZONE
) RETURNS VOID AS $$ BEGIN
    INSERT INTO
        jobs (
            job_id,
            job_type,
            job_state,
            job_stage,
            job_parameters,
            created_at
        )
    VALUES
        (
            job_id,
            job_type,
            0,
            0,
            job_parameters,
            created_at
        );

INSERT INTO
    jobs_updates (
        job_id,
        job_state,
        job_stage,
        updated_at
    )
VALUES
    (
        job_id,
        0,
        0,
        created_at
    );

END;

$$ LANGUAGE plpgsql;

-- Function to returns a list of the jobs.
CREATE
OR REPLACE FUNCTION get_jobs (start_at BIGINT, max_num BIGINT) RETURNS TABLE(
    job_id UUID,
    job_state INTEGER,
    job_stage INTEGER,
    job_type VARCHAR(255),
    created_at TIMESTAMP WITH TIME ZONE,
    updated_at TIMESTAMP WITH TIME ZONE
) AS $$
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
LIMIT
    get_jobs.max_num;

$$ LANGUAGE SQL;

-- Function to update the job state. This function will also insert a new row in the jobs_updates
-- table if a job with the give id exists.
-- Returns FALSE if the job_id does not exist and TRUE if the job_id exists.
CREATE
OR REPLACE FUNCTION change_job_state(
    job_id UUID,
    job_state INTEGER,
    job_stage INTEGER,
    updated_at TIMESTAMP WITH TIME ZONE
) RETURNS BOOLEAN AS $$ BEGIN
    UPDATE
        jobs AS j
    SET
        job_state = change_job_state.job_state
    WHERE
        j.job_id = change_job_state.job_id;

IF NOT FOUND THEN RETURN FALSE;

END IF;

INSERT INTO
    jobs_updates (job_id, job_state, job_stage, updated_at)
VALUES
    (job_id, job_state, job_stage, updated_at);

RETURN TRUE;

END;

$$ LANGUAGE plpgsql;

-- Function to create a task
CREATE
OR REPLACE FUNCTION create_task(
    job_id UUID,
    task_id UUID,
    task_type VARCHAR(255),
    task_parameters JSONB,
    created_at TIMESTAMP WITH TIME ZONE
) RETURNS VOID AS $$ BEGIN
    INSERT INTO
        tasks (
            task_id,
            job_id,
            task_type,
            job_stage,
            task_state,
            task_parameters,
            created_at
        )
    VALUES
        (
            task_id,
            job_id,
            task_type,
            0,
            0,
            task_parameters,
            created_at
        );

INSERT INTO
    tasks_updates (task_id, task_state, updated_at)
VALUES
    (task_id, 0, created_at);

END;

$$ LANGUAGE plpgsql;