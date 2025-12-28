// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::SocketAddr,
    path::PathBuf,
    process::Stdio,
    time::Duration,
};

use futures::future::try_join_all;
use tokio::{
    process::Command,
    time::sleep,
};

use crate::{
    client::Instance,
    error::{SshError, SshResult},
    ssh::{CommandContext, CommandStatus},
};

/// A local command executor that runs commands directly on the local machine
/// without using SSH. This is used when running benchmarks locally.
#[derive(Clone)]
pub struct LocalCommandExecutor {
    /// Working directory for local execution
    working_dir: PathBuf,
}

impl LocalCommandExecutor {
    /// Create a new local command executor.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Delay before re-attempting command execution.
    const RETRY_DELAY: Duration = Duration::from_secs(1);

    /// Execute a command locally using shell.
    async fn execute_command(
        &self,
        command: String,
        context: CommandContext,
    ) -> SshResult<(String, String)> {
        let full_command = context.apply(command);

        // Ensure working directory exists
        if let Err(e) = std::fs::create_dir_all(&self.working_dir) {
            return Err(SshError::ConnectionError {
                address: SocketAddr::from(([127, 0, 0, 1], 22)),
                error: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to create working directory {}: {}", self.working_dir.display(), e),
                ),
            });
        }

        // Run the command in a shell
        let output = Command::new("sh")
            .arg("-c")
            .arg(&full_command)
            .current_dir(&self.working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| SshError::ConnectionError {
                address: SocketAddr::from(([127, 0, 0, 1], 22)),
                error: std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to execute command: {}", e),
                ),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(SshError::NonZeroExitCode {
                address: SocketAddr::from(([127, 0, 0, 1], 22)),
                code: output.status.code().unwrap_or(1),
                message: stderr.clone(),
            });
        }

        Ok((stdout, stderr))
    }

    /// Execute the specified command on all provided instances.
    /// For local execution, all instances are the same (localhost), so we execute once.
    pub async fn execute<I, S>(
        &self,
        instances: I,
        command: S,
        context: CommandContext,
    ) -> SshResult<Vec<(String, String)>>
    where
        I: IntoIterator<Item = Instance>,
        S: Into<String> + Clone + Send + 'static,
    {
        // For local execution, we execute the command once per instance
        // but they all run on the same machine
        let instances: Vec<_> = instances.into_iter().collect();
        let command_str: String = command.into();
        let mut results = Vec::new();

        for _instance in &instances {
            let result = self.execute_command(command_str.clone(), context.clone()).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute the command associated with each instance.
    pub async fn execute_per_instance<I, S>(
        &self,
        instances: I,
        context: CommandContext,
    ) -> SshResult<Vec<(String, String)>>
    where
        I: IntoIterator<Item = (Instance, S)>,
        S: Into<String> + Send + 'static,
    {
        let instances: Vec<_> = instances.into_iter().collect();
        let handles: Vec<_> = instances
            .into_iter()
            .map(|(_instance, command)| {
                let executor = self.clone();
                let command: String = command.into();
                let context = context.clone();

                tokio::spawn(async move {
                    executor.execute_command(command, context).await
                })
            })
            .collect();

        let results = try_join_all(handles).await.unwrap();
        results.into_iter().collect()
    }

    /// Wait until a command running in the background returns or started.
    pub async fn wait_for_command<I>(
        &self,
        instances: I,
        command_id: &str,
        status: CommandStatus,
    ) -> SshResult<()>
    where
        I: IntoIterator<Item = Instance> + Clone,
    {
        loop {
            sleep(Self::RETRY_DELAY).await;

            let result = self
                .execute(
                    instances.clone(),
                    "(tmux ls || true)",
                    CommandContext::default(),
                )
                .await?;
            if result
                .iter()
                .all(|(stdout, _)| CommandStatus::status(command_id, stdout) == status)
            {
                break;
            }
        }
        Ok(())
    }

    /// Wait until commands succeed.
    pub async fn wait_for_success<I, S>(&self, instances: I)
    where
        I: IntoIterator<Item = (Instance, S)> + Clone,
        S: Into<String> + Send + 'static + Clone,
    {
        loop {
            sleep(Self::RETRY_DELAY).await;

            if self
                .execute_per_instance(instances.clone(), CommandContext::default())
                .await
                .is_ok()
            {
                break;
            }
        }
    }

    /// Kill a command running in the background.
    pub async fn kill<I>(&self, instances: I, command_id: &str) -> SshResult<()>
    where
        I: IntoIterator<Item = Instance>,
    {
        let command = format!("(tmux kill-session -t {command_id} || true)");
        let targets: Vec<_> = instances.into_iter().map(|x| (x, command.clone())).collect();
        self.execute_per_instance(targets, CommandContext::default())
            .await?;
        Ok(())
    }

    /// Connect to an instance (for local execution, this is a no-op wrapper).
    pub async fn connect(&self, _address: SocketAddr) -> SshResult<LocalConnection> {
        Ok(LocalConnection {
            working_dir: self.working_dir.clone(),
        })
    }
}

/// A local connection for downloading files.
pub struct LocalConnection {
    working_dir: PathBuf,
}

impl LocalConnection {
    /// Download a file from the local machine.
    pub fn download<P: AsRef<std::path::Path>>(&self, path: P) -> SshResult<String> {
        let path = path.as_ref();
        // Expand ~ to home directory
        let path_str = path.to_string_lossy();
        let expanded_path = if path_str.starts_with("~") {
            let home = std::env::var("HOME").map_err(|_| SshError::ConnectionError {
                address: SocketAddr::from(([127, 0, 0, 1], 22)),
                error: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "HOME environment variable not set",
                ),
            })?;
            PathBuf::from(path_str.replace("~", &home))
        } else {
            path.to_path_buf()
        };

        // If path is relative, make it relative to working dir
        let full_path = if expanded_path.is_absolute() {
            expanded_path
        } else {
            self.working_dir.join(&expanded_path)
        };

        std::fs::read_to_string(&full_path).map_err(|e| SshError::ConnectionError {
            address: SocketAddr::from(([127, 0, 0, 1], 22)),
            error: e,
        })
    }
}

