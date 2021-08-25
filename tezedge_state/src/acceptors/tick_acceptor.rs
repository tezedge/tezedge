// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::proposals::TickProposal;
use crate::{Effects, TezedgeState};
use tla_sm::Acceptor;

impl<'a, Efs> Acceptor<TickProposal<'a, Efs>> for TezedgeState
where
    Efs: Effects,
{
    fn accept(&mut self, proposal: TickProposal<'a, Efs>) {
        self.time += proposal.time_passed;

        let interval_passed = self
            .time
            .duration_since(self.last_periodic_react)
            .map(|passed| passed > self.config.periodic_react_interval)
            .unwrap_or(false);

        if interval_passed {
            self.last_periodic_react = self.time;
            self.check_timeouts(proposal.effects);
            self.check_blacklisted_peers();
            self.initiate_handshakes(proposal.effects);
        }

        self.connected_peers.periodic_react(self.time);
    }
}
