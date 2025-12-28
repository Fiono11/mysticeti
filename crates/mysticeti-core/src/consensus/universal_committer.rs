// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use super::LeaderStatus;
use crate::{
    block_store::BlockStore,
    committee::Committee,
    metrics::Metrics,
    types::{BlockReference, RoundNumber},
};

/// A universal committer commits blocks using leaderless consensus.
pub struct UniversalCommitter {
    block_store: BlockStore,
    metrics: Arc<Metrics>,
}

impl UniversalCommitter {
    /// Try to commit part of the dag. This function is idempotent and returns a list of
    /// ordered decided blocks.
    ///
    /// For leaderless consensus, we use a simple strategy: commit blocks in round order.
    /// This is a minimal implementation for testing. A production implementation should
    /// consider DAG structure, transaction commitment status, and safety properties.
    #[tracing::instrument(skip_all, fields(last_decided = %last_decided))]
    pub fn try_commit(&self, last_decided: BlockReference) -> Vec<LeaderStatus> {
        let highest_round = self.block_store.highest_round();
        let last_decided_round = last_decided.round();

        // Don't commit genesis round (round 0)
        if last_decided_round >= highest_round {
            return vec![];
        }

        let mut committed = vec![];

        // Simple strategy: commit one block per round in order
        // Start from the round after last_decided
        for round in (last_decided_round + 1)..=highest_round {
            let blocks = self.block_store.get_blocks_by_round(round);

            // Commit the first block from each round (deterministic choice)
            // In a more sophisticated implementation, we might commit all blocks
            // or choose based on DAG structure
            if let Some(block) = blocks.first() {
                // Only commit if we haven't seen this block before (idempotency check)
                // The linearizer will handle duplicates, but we can be more efficient here
                if block.reference().round > last_decided_round {
                    committed.push(LeaderStatus::Commit(block.clone()));
                    // Update metrics
                    self.update_metrics(&LeaderStatus::Commit(block.clone()), true);
                }
            }
        }

        committed
    }

    /// Update metrics.
    fn update_metrics(&self, leader: &LeaderStatus, _direct_decide: bool) {
        let authority = leader.authority().to_string();
        let status = match leader {
            LeaderStatus::Commit(..) => "commit",
            LeaderStatus::Skip(..) => "skip",
            LeaderStatus::Undecided(..) => return,
        };
        self.metrics
            .committed_blocks_total
            .with_label_values(&[&authority, status])
            .inc();
    }
}

/// A builder for a universal committer.
pub struct UniversalCommitterBuilder {
    block_store: BlockStore,
    metrics: Arc<Metrics>,
    pipeline: bool,
}

impl UniversalCommitterBuilder {
    pub fn new(_committee: Arc<Committee>, block_store: BlockStore, metrics: Arc<Metrics>) -> Self {
        Self {
            block_store,
            metrics,
            pipeline: false,
        }
    }

    pub fn with_wave_length(self, _wave_length: RoundNumber) -> Self {
        // Wave length is not used in leaderless consensus
        self
    }

    pub fn with_pipeline(mut self, pipeline: bool) -> Self {
        self.pipeline = pipeline;
        self
    }

    pub fn build(self) -> UniversalCommitter {
        // Pipeline is not used in leaderless consensus, but we keep the option for API compatibility
        let _ = self.pipeline;

        UniversalCommitter {
            block_store: self.block_store,
            metrics: self.metrics,
        }
    }
}
