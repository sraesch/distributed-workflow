# Simple Distributed Workflow Engine
This project focuses on a very simple workflow engine to execute jobs consisting of multiple tasks over a distributed microservice architecture.
The goals for this project are:
- **Simplicity**: The engine should be simple to use and simple to understand and also simple to integrate and deploy.
- **Scalability**: The engine should provide mechanisms to scale the execution of tasks horizontally.
- **Agnostic**: The engine should be agnostic to the underlying layer, e.g., it can run on Docker, Docker Compose or Kubernetes, but can also run bare-metal.
- **Tiny**: The engine should be as small as possible, with minimal dependencies. That is not plenty of different services that have to be deployed to run the engine.

## Concepts

### Job, Stages and Tasks
A single job consumes input provided as parameters and provides output as resulting artefact. A job consists of multiple stages that are sequentially processed.
Each stage can have an arbitrary number of tasks that are independent and being executed in parallel. Horizontal scaling happens within a stage by distributing the tasks to multiple workers.
A stage is considered completed when all tasks are finished.
When a stage is completed, the next stage is started.
The job is considered completed when all stages are finished.
The following diagram illustrates the composition of a job in stages and tasks.

![Job, Stages and Tasks](./img/job_stages_tasks.png)

### Software Architecture
The engine is composed of the following components:
- **Controller**: The controller is responsible for providing the external REST interfaces for managing the jobs. It is responsible for creating a job, starting a job, monitoring the progress of a job and finally completing a job.
- **Worker**: The worker is responsible for executing the job and corresponding tasks.
- **Message Queue**: The message queue is used to communicate between the worker and the control instances.
- **Shared Volume**: The shared volume is used to store the input and output of tasks and jobs. Worker consume input for their tasks from the shared volume and write the output of their tasks to the shared volume.

Following the architecture diagram illustrates the components and their interactions:

![Software Architecture](./img/software_architecture.png)

### Job & Task Description

#### Tasks
A task is a simple building block that can be executed by a worker. A task consumes input and produces output. They are oblivious to the job in which they are executed.

The individual tasks are described in a JSON file. The following is an example of a task description:
```json
{
  "tasks": {
    "export-data": {
      "name": "Export Data",
      "description": "Exports and serializes the data described by the given id",
      "exec_command": "serialize_data",
      "input": {
        "data_id": {
          "description": "The id of the data to be exported"
        }
      },
      "exec_args": [
        "--data_id",
        "{data_id}",
        "--endpoint",
        "{REST_ENDPOINT}",
        "--log_level",
        "{LOG_LEVEL}"
      ]
    },
    "download": {
      "name": "Download file",
      "description": "Downloads a single file from a given url",
      "exec_command": "wget",
      "input": {
        "url": {
          "description": "The url of the file to be downloaded"
        },
        "output_path": {
          "description": "The path where the file should be stored"
        }
      },
      "exec_args": ["{url}", "-P", "{output_path}"]
    },
    "env_whitelist": ["REST_ENDPOINT", "LOG_LEVEL"]
  }
  ...
}
```
The task description consists of two tasks: `export-data` and `download`. Each task has a name, a description, an executable command and a list of arguments.
For example, the export data command serializes some data with the command `serialize_data` and the arguments `--data_id`, `--endpoint` and `--log_level`.
The argument `{data_id}` is assumed to be an input to be given to the task. The other arguments `{REST_ENDPOINT}` and `{LOG_LEVEL}` are environment variables that are passed to the worker executing the task.
Thus the list `env_whitelist` contains a list of environment variables that are whitelisted and thus may be passed to the worker by replacing the corresponding placeholders in the arguments.

#### Jobs
A job is defining a number of stages to be executed in a consecutive order. Each stage consists of a number of tasks that are executed in parallel. The job description is a JSON file that describes the job and its stages and tasks. Following an example of a job description:
```json
{
  ...
  "jobs": {
    "request-data": {
      "name": "Requests Data",
      "description": "Requests data from the server",
      "input": {
        "data_id": {
          "description": "The id of the data to be exported"
        }
      },
      "stages": [
        {
            "name": "Request Data",
            "task_class": "export-data"
        },
        {
            "name": "Download Data",
            "task_class": "download"
        }
      ]
    }
  }
}
```
The job description has a clear defined set of input values. In this case, there is only one input which is the `data_id`. The parameters are being used to trigger the first task of the first stage.