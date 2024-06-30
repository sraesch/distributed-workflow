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