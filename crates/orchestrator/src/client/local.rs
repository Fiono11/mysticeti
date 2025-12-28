// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fmt::Display,
    net::Ipv4Addr,
    sync::Mutex,
};

use serde::Serialize;

use super::{Instance, InstanceStatus, ServerProviderClient};
use crate::error::CloudProviderResult;

/// A local client that creates localhost instances for running benchmarks locally.
/// This client creates virtual instances that all point to localhost (127.0.0.1),
/// allowing you to run benchmarks on your local machine without cloud instances.
///
/// Commands are executed directly on the local machine without SSH.
pub struct LocalClient {
    instances: Mutex<Vec<Instance>>,
}

impl LocalClient {
    /// Create a new local client.
    pub fn new() -> Self {
        Self {
            instances: Mutex::new(Vec::new()),
        }
    }
}

impl Display for LocalClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "LocalClient")
    }
}

impl ServerProviderClient for LocalClient {
    // For local execution, this username is not actually used since commands
    // are executed directly without SSH.
    const USERNAME: &'static str = "localhost";

    async fn list_instances(&self) -> CloudProviderResult<Vec<Instance>> {
        let guard = self.instances.lock().unwrap();
        Ok(guard.clone())
    }

    async fn start_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send,
    {
        let instance_ids: Vec<_> = instances.map(|x| x.id.clone()).collect();
        let mut guard = self.instances.lock().unwrap();
        for instance in guard.iter_mut().filter(|x| instance_ids.contains(&x.id)) {
            instance.status = InstanceStatus::Active;
        }
        Ok(())
    }

    async fn stop_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send,
    {
        let instance_ids: Vec<_> = instances.map(|x| x.id.clone()).collect();
        let mut guard = self.instances.lock().unwrap();
        for instance in guard.iter_mut().filter(|x| instance_ids.contains(&x.id)) {
            instance.status = InstanceStatus::Inactive;
        }
        Ok(())
    }

    async fn create_instance<S>(&self, _region: S) -> CloudProviderResult<Instance>
    where
        S: Into<String> + Serialize + Send,
    {
        let mut guard = self.instances.lock().unwrap();
        let id = guard.len();
        let instance = Instance {
            id: id.to_string(),
            region: "local".to_string(),
            main_ip: Ipv4Addr::LOCALHOST,
            tags: Vec::new(),
            specs: "local".to_string(),
            status: InstanceStatus::Active,
        };
        guard.push(instance.clone());
        Ok(instance)
    }

    async fn delete_instance(&self, instance: Instance) -> CloudProviderResult<()> {
        let mut guard = self.instances.lock().unwrap();
        guard.retain(|x| x.id != instance.id);
        Ok(())
    }

    async fn register_ssh_public_key(&self, _public_key: String) -> CloudProviderResult<()> {
        // No-op for local execution
        Ok(())
    }

    async fn instance_setup_commands(&self) -> CloudProviderResult<Vec<String>> {
        // Return empty setup commands for local execution
        // The user should have their environment already set up
        Ok(Vec::new())
    }
}

