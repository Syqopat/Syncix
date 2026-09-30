#[allow(dead_code)]
mod api;
mod catalog;
mod cli;
mod file_sync;
mod health;
mod layout;
mod localization;
#[allow(dead_code)]
mod model;
mod project;
mod rbxmx;
mod rbxmx_import;
mod sourcemap;
mod tree_import;
mod upload;
mod scheduler;
#[allow(dead_code)]
mod serializers;
mod runtime;
mod server;
mod suggest;
#[allow(dead_code)]
mod transport;
#[allow(dead_code)]
#[cfg(test)]
mod tests;
mod values;


#[tokio::main]
async fn main() {
    // One binary is both the server and the CLI, so there is no PowerShell or Node
    // dependency. With arguments it acts as a client and never starts a server.
    let cli_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(exit_code) = cli::execute_run(&cli_args) {
        std::process::exit(exit_code);
    }

    runtime::run(cli_args).await;
}

