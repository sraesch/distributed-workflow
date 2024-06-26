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