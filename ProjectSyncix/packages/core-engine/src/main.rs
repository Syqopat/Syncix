#[allow(dead_code)]
mod api;
mod assets;
#[allow(dead_code)]
mod auth;
#[allow(dead_code)]
mod bus;
mod catalog;
mod cli;
#[allow(dead_code)]
mod command;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod contract;
mod file_sync;
mod health;
mod layout;
mod localization;
#[allow(dead_code)]
mod logging;
#[allow(dead_code)]
mod model;
#[allow(dead_code)]
mod pipeline;
mod project;
mod rbxmx;
mod rbxmx_import;
mod sourcemap;
mod tree_import;
mod upload;
#[allow(dead_code)]
mod scheduler;
#[allow(dead_code)]
mod schema;
#[allow(dead_code)]
mod serializers;
mod runtime;
mod server;
#[allow(dead_code)]
mod snapshot;
mod suggest;
#[allow(dead_code)]
mod transport;
#[allow(dead_code)]
mod tree_builder;
#[allow(dead_code)]
#[cfg(test)]
mod tests;
mod values;
mod vfs;


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

