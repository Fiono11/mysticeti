// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use super::{base_committer::BaseCommitter, LeaderStatus, DEFAULT_WAVE_LENGTH};
use crate::{
    block_store::BlockStore,
    committee::Committee,
    consensus::base_committer::BaseCommitterOptions,
    data::Data,
    metrics::Metrics,
    types::{BlockReference, RoundNumber, StatementBlock},
};

/// A universal committer uses a collection of committers to commit blocks.
/// It can be configured to use different commit strategies, including pipelines.
pub struct UniversalCommitter {
    block_store: BlockStore,
    committers: Vec<BaseCommitter>,
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
    fn update_metrics(&self, leader: &LeaderStatus, direct_decide: bool) {
        let authority = leader.authority().to_string();
        let direct_or_indirect = if direct_decide { "direct" } else { "indirect" };
        let status = match leader {
            LeaderStatus::Commit(..) => format!("{direct_or_indirect}-commit"),
            LeaderStatus::Skip(..) => format!("{direct_or_indirect}-skip"),
            LeaderStatus::Undecided(..) => return,
        };
        self.metrics
            .committed_leaders_total
            .with_label_values(&[&authority, &status])
            .inc();
    }
}

/// A builder for a universal committer. By default, the builder creates a single base committer
/// with no pipeline.
pub struct UniversalCommitterBuilder {
    committee: Arc<Committee>,
    block_store: BlockStore,
    metrics: Arc<Metrics>,
    wave_length: RoundNumber,
    pipeline: bool,
}

impl UniversalCommitterBuilder {
    pub fn new(committee: Arc<Committee>, block_store: BlockStore, metrics: Arc<Metrics>) -> Self {
        Self {
            committee,
            block_store,
            metrics,
            wave_length: DEFAULT_WAVE_LENGTH,
            pipeline: false,
        }
    }

    pub fn with_wave_length(mut self, wave_length: RoundNumber) -> Self {
        self.wave_length = wave_length;
        self
    }

    pub fn with_pipeline(mut self, pipeline: bool) -> Self {
        self.pipeline = pipeline;
        self
    }

    pub fn build(self) -> UniversalCommitter {
        let mut committers = Vec::new();
        let pipeline_stages = if self.pipeline { self.wave_length } else { 1 };
        for round_offset in 0..pipeline_stages {
            let options = BaseCommitterOptions {
                wave_length: self.wave_length,
                round_offset,
            };
            let committer = BaseCommitter::new(self.committee.clone(), self.block_store.clone())
                .with_options(options);
            committers.push(committer);
        }

        UniversalCommitter {
            block_store: self.block_store,
            committers,
            metrics: self.metrics,
        }
    }
}
