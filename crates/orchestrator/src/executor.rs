// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::net::SocketAddr;

use crate::{
    client::Instance,
    error::SshResult,
    local_executor::{LocalCommandExecutor, LocalConnection},
    ssh::{CommandContext, CommandStatus, SshConnection, SshConnectionManager},
};

/// An executor that can use either SSH or local execution.
#[derive(Clone)]
pub enum Executor {
    Ssh(SshConnectionManager),
    Local(LocalCommandExecutor),
}

impl Executor {
    /// Create an SSH executor.
    pub fn ssh(manager: SshConnectionManager) -> Self {
        Self::Ssh(manager)
    }

    /// Create a local executor.
    pub fn local(executor: LocalCommandExecutor) -> Self {
        Self::Local(executor)
    }

    /// Execute the specified command on all provided instances.
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
        match self {
            Self::Ssh(ssh) => ssh.execute(instances, command, context).await,
            Self::Local(local) => local.execute(instances, command, context).await,
        }
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
        match self {
            Self::Ssh(ssh) => ssh.execute_per_instance(instances, context).await,
            Self::Local(local) => local.execute_per_instance(instances, context).await,
        }
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
        match self {
            Self::Ssh(ssh) => ssh.wait_for_command(instances, command_id, status).await,
            Self::Local(local) => local.wait_for_command(instances, command_id, status).await,
        }
    }

    /// Wait until commands succeed.
    pub async fn wait_for_success<I, S>(&self, instances: I)
    where
        I: IntoIterator<Item = (Instance, S)> + Clone,
        S: Into<String> + Send + 'static + Clone,
    {
        match self {
            Self::Ssh(ssh) => ssh.wait_for_success(instances).await,
            Self::Local(local) => local.wait_for_success(instances).await,
        }
    }

    /// Kill a command running in the background.
    pub async fn kill<I>(&self, instances: I, command_id: &str) -> SshResult<()>
    where
        I: IntoIterator<Item = Instance>,
    {
        match self {
            Self::Ssh(ssh) => ssh.kill(instances, command_id).await,
            Self::Local(local) => local.kill(instances, command_id).await,
        }
    }

    /// Connect to an instance.
    pub async fn connect(&self, address: SocketAddr) -> SshResult<ExecutorConnection> {
        match self {
            Self::Ssh(ssh) => {
                let conn = ssh.connect(address).await?;
                Ok(ExecutorConnection::Ssh(conn))
            }
            Self::Local(local) => {
                let conn = local.connect(address).await?;
                Ok(ExecutorConnection::Local(conn))
            }
        }
    }
}

/// A connection that can download files, either via SSH or local.
pub enum ExecutorConnection {
    Ssh(SshConnection),
    Local(LocalConnection),
}

impl ExecutorConnection {
    /// Download a file from the remote/local machine.
    pub fn download<P: AsRef<std::path::Path>>(&self, path: P) -> SshResult<String> {
        match self {
            Self::Ssh(ssh) => ssh.download(path),
            Self::Local(local) => {
                let path_buf = path.as_ref().to_path_buf();
                local.download(&path_buf)
            }
        }
    }
}

