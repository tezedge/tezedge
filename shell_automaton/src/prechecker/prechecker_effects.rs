// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::{
    binary_message::{BinaryRead, MessageHash},
    encoding::operation::Operation,
};

use crate::{
    mempool::MempoolOperationRecvDoneAction, prechecker::PrevalidatorEndorsementOperation, Action, Service,
    State,
};

use super::{Key, OperationDecodedContents, PrecheckerDecodeOperationAction, PrecheckerEndorsementValidationReadyAction, PrecheckerEndorsingRightsReadyAction, PrecheckerError, PrecheckerErrorAction, PrecheckerGetEndorsingRightsAction, PrecheckerOperationDecodedAction, PrecheckerOperationState, PrecheckerPrecheckOperationAction, PrecheckerValidateEndorsementAction, PrecheckerValidationError};

pub fn preckecker_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>)
where
    S: Service,
{
    let prechecker_state_operations = &store.state.get().prechecker.operations;
    match &action.action {
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { address, operation }) => {
            // TODO get the hash along with the operation
            let hash = match operation.message_hash() {
                Ok(hash) => hash,
                Err(err) => {
                    eprintln!("Error getting hash of the message: {}", err);
                    return;
                }
            };
            let key = match hash.try_into() {
                Ok(hash) => Key { operation: hash },
                Err(err) => {
                    eprintln!("Error constructing typed hash of the message: {}", err);
                    return;
                }
            };
            store.dispatch(
                PrecheckerPrecheckOperationAction {
                    key,
                    operation: operation.clone(),
                }
                .into(),
            );
        }
        Action::PrecheckerPrecheckOperation(PrecheckerPrecheckOperationAction { key, .. }) => {
            if let Some(PrecheckerOperationState::Init { .. }) =
                prechecker_state_operations.get(key)
            {
                store.dispatch(PrecheckerDecodeOperationAction { key: key.clone() }.into());
            }
        }
        Action::PrecheckerDecodeOperation(PrecheckerDecodeOperationAction { key }) => {
            if let Some(PrecheckerOperationState::PendingContentDecoding { operation }) =
                prechecker_state_operations.get(key)
            {
                // TODO use proper protocol to parse operation
                match tezos_messages::protocol::proto_010::operation::OperationContents::from_bytes(
                    operation.data(),
                ) {
                    Ok(contents) => {
                        store.dispatch(
                            PrecheckerOperationDecodedAction {
                                key: key.clone(),
                                contents,
                            }
                            .into(),
                        );
                    }
                    // TODO: report error or checking verdict?
                    Err(err) => store.dispatch(
                        PrecheckerErrorAction {
                            key: key.clone(),
                            error: err.into(),
                        }
                        .into(),
                    ),
                }
            }
        }
        Action::PrecheckerOperationDecoded(PrecheckerOperationDecodedAction { key, contents }) => {}
        Action::PrecheckerGetEndorsingRights(PrecheckerGetEndorsingRightsAction { key }) => {}
        Action::PrecheckerEndorsingRightsReady(PrecheckerEndorsingRightsReadyAction { key }) => {}
        Action::PrecheckerValidateEndorsement(PrecheckerValidateEndorsementAction { key }) => {}
        Action::PrecheckerEndorsementValidationReady(
            PrecheckerEndorsementValidationReadyAction { key, result },
        ) => {}

        _ => (),
    }
}

fn get_operation_contents(
    operation: Operation,
) -> Result<OperationDecodedContents, PrecheckerValidationError> {
    use tezos_messages::protocol::proto_010::operation::*;
    let OperationContents {
        contents,
        signature,
    } = match OperationContents::from_bytes(operation.data()) {
        Ok(contents) => contents,
        Err(err) => return Err(err.into()),
    };
    let only_contents = if contents.len() == 1 {
        contents[0]
    } else {
        return Ok(OperationDecodedContents::Other);
    };
    let decoded_contents = match only_contents {
        Contents::Endorsement(EndorsementOperation { level }) => {
            OperationDecodedContents::Endorsement(super::PrevalidatorEndorsementOperation {
                branch: operation.branch().clone(),
                signature,
                level,
                slot: None,
            })
        }
        Contents::EndorsementWithSlot(
            EndorsementWithSlotOperation {
                slot,
                endorsement:
                    InlinedEndorsement {
                        branch: inl_branch,
                        operations: InlinedEndorsementContents::Endorsement(InlinedEndorsementVariant { level }),
                        signature: inl_signature,
                    },
            },
        ) => {
            OperationDecodedContents::Endorsement(super::PrevalidatorEndorsementOperation {
                branch: operation.branch().clone(),
                signature,
                level,
                slot: None,
            })
        }
        _ => return Ok(OperationDecodedContents::Other),
    };
    Ok(decoded_contents)
}
