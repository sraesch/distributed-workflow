use anyhow::Result;
use cli_common::parse_args_and_init_logging;
use log::{error, info};

/// Runs the program.
async fn run_program() -> Result<()> {
    // read the application name, version and description from the Cargo.toml file
    let (app_name, version, about) = (
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION"),
    );

    let options = parse_args_and_init_logging(app_name, version, about)?;

    info!("Workflow Controller Version: {}", env!("CARGO_PKG_VERSION"));
    info!("Jobs Config: {:#?}", options.task_job_desc);

    // TODO: Implement the controller

    Ok(())
}

#[actix_web::main]
async fn main() {
    match run_program().await {
        Ok(()) => {
            info!("SUCCESS");
        }
        Err(err) => {
            error!("Error: {}", err);
            eprintln!("{}", err);
            error!("FAILED");

            std::process::exit(-1);
        }
    }
}
