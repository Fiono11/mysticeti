// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Orchestrator entry point.

use std::path::PathBuf;

use benchmark::BenchmarkParameters;
use clap::Parser;
use client::{aws::AwsClient, local::LocalClient, vultr::VultrClient, ServerProviderClient};
use eyre::Context;
use executor::Executor;
use local_executor::LocalCommandExecutor;
use measurements::MeasurementsCollection;
use orchestrator::Orchestrator;
use protocol::ProtocolParameters;
use settings::{CloudProvider, Settings};
use ssh::SshConnectionManager;
use testbed::Testbed;

mod benchmark;
mod client;
mod display;
mod error;
mod executor;
mod faults;
mod local_executor;
mod logs;
mod measurements;
mod monitor;
mod orchestrator;
mod protocol;
mod settings;
mod ssh;
mod testbed;

/// NOTE: Link these types to the correct protocol.
type Protocol = protocol::mysticeti::MysticetiProtocol;
type NodeParameters = protocol::mysticeti::MysticetiNodeParameters;
type ClientParameters = protocol::mysticeti::MysticetiClientParameters;

/// The orchestrator command line options.
#[derive(Parser, Debug)]
#[command(author, version, about = "Testbed orchestrator", long_about = None)]
#[clap(rename_all = "kebab-case")]
pub struct Opts {
    /// The path to the settings file. This file contains basic information to deploy testbeds
    /// and run benchmarks such as the url of the git repo, the commit to deploy, etc.
    #[clap(
        long,
        value_name = "FILE",
        default_value = "crates/orchestrator/assets/settings.yml",
        global = true
    )]
    settings_path: String,

    /// The type of operation to run.
    #[clap(subcommand)]
    operation: Operation,
}

/// The type of operation to run.
#[derive(Parser, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum Operation {
    /// Read or modify the status of the testbed.
    Testbed {
        /// The action to perform on the testbed.
        #[clap(subcommand)]
        action: TestbedAction,
    },
    /// Deploy nodes and run a benchmark on the specified testbed.
    Benchmark {
        /// The committee size to deploy.
        #[clap(long, value_name = "INT", default_value_t = 4, global = true)]
        committee: usize,

        /// The set of loads to submit to the system (tx/s). Each load triggers a separate
        /// benchmark run. Setting a load to zero will not deploy any benchmark clients
        /// (useful to boot testbeds designed to run with external clients and load generators).
        #[clap(long, value_name = "[INT]", default_value = "200", global = true)]
        loads: Vec<usize>,

        /// Whether to skip testbed updates before running benchmarks. This is a dangerous
        /// operation as it may lead to running benchmarks on outdated nodes. It is however
        /// useful when debugging in some specific scenarios.
        #[clap(long, action, default_value_t = false, global = true)]
        skip_testbed_update: bool,

        /// Whether to skip testbed configuration before running benchmarks. This is a dangerous
        /// operation as it may lead to running benchmarks on misconfigured nodes. It is however
        /// useful when debugging in some specific scenarios.
        #[clap(long, action, default_value_t = false, global = true)]
        skip_testbed_configuration: bool,
    },
    /// Print a summary of the specified measurements collection.
    Summarize {
        /// The path to the settings file.
        #[clap(long, value_name = "FILE")]
        path: PathBuf,
    },
}

/// The action to perform on the testbed.
#[derive(Parser, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum TestbedAction {
    /// Display the testbed status.
    Status,

    /// Deploy the specified number of instances in all regions specified by in the setting file.
    Deploy {
        /// Number of instances to deploy.
        #[clap(long)]
        instances: usize,

        /// The region where to deploy the instances. If this parameter is not specified, the
        /// command deploys the specified number of instances in all regions listed in the
        /// setting file.
        #[clap(long)]
        region: Option<String>,
    },

    /// Start at most the specified number of instances per region on an existing testbed.
    Start {
        /// Number of instances to deploy.
        #[clap(long, default_value_t = 10)]
        instances: usize,
    },

    /// Stop an existing testbed (without destroying the instances).
    Stop,

    /// Destroy the testbed and terminate all instances.
    Destroy,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let opts: Opts = Opts::parse();

    // Load the settings files.
    let settings = Settings::load(&opts.settings_path).wrap_err("Failed to load settings")?;

    match &settings.cloud_provider {
        CloudProvider::Aws => {
            // Create the client for the cloud provider.
            let client = AwsClient::new(settings.clone()).await;

            // Execute the command.
            run(settings, client, opts).await
        }
        CloudProvider::Vultr => {
            // Create the client for the cloud provider.
            let token = settings
                .load_token()
                .wrap_err("Failed to load cloud provider's token")?;
            let client = VultrClient::new(token, settings.clone());

            // Execute the command.
            run(settings, client, opts).await
        }
        CloudProvider::Local => {
            // Create the local client for running benchmarks locally.
            let client = LocalClient::new();

            // Execute the command.
            run(settings, client, opts).await
        }
    }
}

async fn run<C: ServerProviderClient>(
    settings: Settings,
    client: C,
    opts: Opts,
) -> eyre::Result<()> {
    // Create a new testbed.
    let mut testbed = Testbed::new(settings.clone(), client)
        .await
        .wrap_err("Failed to crate testbed")?;

    match opts.operation {
        Operation::Testbed { action } => match action {
            // Display the current status of the testbed.
            TestbedAction::Status => testbed.status(),

            // Deploy the specified number of instances on the testbed.
            TestbedAction::Deploy { instances, region } => testbed
                .deploy(instances, region)
                .await
                .wrap_err("Failed to deploy testbed")?,

            // Start the specified number of instances on an existing testbed.
            TestbedAction::Start { instances } => testbed
                .start(instances)
                .await
                .wrap_err("Failed to start testbed")?,

            // Stop an existing testbed.
            TestbedAction::Stop => testbed.stop().await.wrap_err("Failed to stop testbed")?,

            // Destroy the testbed and terminal all instances.
            TestbedAction::Destroy => testbed
                .destroy()
                .await
                .wrap_err("Failed to destroy testbed")?,
        },

        // Run benchmarks.
        Operation::Benchmark {
            committee,
            loads,
            skip_testbed_update,
            skip_testbed_configuration,
        } => {
            // Create the appropriate executor based on cloud provider.
            let executor = match &settings.cloud_provider {
                CloudProvider::Local => {
                    // For local execution, use direct command execution
                    let working_dir = settings.working_dir.clone();
                    Executor::local(LocalCommandExecutor::new(working_dir))
                }
                _ => {
                    // For cloud providers, use SSH
                    let username = testbed.username();
                    let private_key_file = settings.ssh_private_key_file.clone();
                    let ssh_manager = SshConnectionManager::new(username.into(), private_key_file)
                        .with_timeout(settings.ssh_timeout)
                        .with_retries(settings.ssh_retries);
                    Executor::ssh(ssh_manager)
                }
            };

            let mut instances = testbed.instances();

            // For local execution, auto-create instances if none exist
            if instances.is_empty() && matches!(settings.cloud_provider, CloudProvider::Local) {
                display::action("No instances found, creating local instances automatically");
                let needed_instances = committee
                    + settings.dedicated_clients
                    + if settings.monitoring { 1 } else { 0 };
                // Create a few extra instances to have some buffer
                let instances_to_create = (needed_instances + 2).max(4);
                testbed
                    .deploy(instances_to_create, None)
                    .await
                    .wrap_err("Failed to auto-create local instances")?;
                instances = testbed.instances();
                display::done();
            }

            let setup_commands = testbed
                .setup_commands()
                .await
                .wrap_err("Failed to load testbed setup commands")?;

            let protocol_commands = Protocol::new(&settings);
            let node_parameters = match &settings.node_parameters_path {
                Some(path) => {
                    NodeParameters::load(path).wrap_err("Failed to load node's parameters")?
                }
                None => NodeParameters::default(),
            };
            let client_parameters = match &settings.client_parameters_path {
                Some(path) => {
                    ClientParameters::load(path).wrap_err("Failed to load client's parameters")?
                }
                None => ClientParameters::default(),
            };

            let set_of_benchmark_parameters = BenchmarkParameters::new_from_loads(
                settings.clone(),
                node_parameters,
                client_parameters,
                committee,
                loads,
            );

            Orchestrator::new(
                settings,
                instances,
                setup_commands,
                protocol_commands,
                executor,
            )
            .skip_testbed_update(skip_testbed_update)
            .skip_testbed_configuration(skip_testbed_configuration)
            .run_benchmarks(set_of_benchmark_parameters)
            .await
            .wrap_err("Failed to run benchmarks")?;
        }

        // Print a summary of the specified measurements collection.
        Operation::Summarize { path } => MeasurementsCollection::load(path)?.display_summary(),
    }
    Ok(())
}
