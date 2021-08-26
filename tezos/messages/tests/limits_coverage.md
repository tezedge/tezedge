# Encoding Limits Coverage

| Field                                                        | Covered |
|:-------------------------------------------------------------|:--------|
| AdvertiseMessage::id                                         | true |
| BlockHeader::fitness                                         | true |
| BlockHeader::protocol_data                                   | true |
| BlockHeaderMessage::block_header                             | false |
| Component::implementation                                    | true |
| Component::interface                                         | false |
| Component::name                                              | true |
| ConnectionMessage::message_nonce                             | true |
| ConnectionMessage::proof_of_work_stamp                       | true |
| ConnectionMessage::public_key                                | true |
| ConnectionMessage::version                                   | false |
| CurrentBranch::current_head                                  | false |
| CurrentBranch::history                                       | true |
| CurrentBranchMessage::current_branch                         | false |
| CurrentHeadMessage::current_block_header                     | false |
| CurrentHeadMessage::current_mempool                          | true |
| GetBlockHeadersMessage::get_block_headers                    | true |
| GetOperationsForBlocksMessage::get_operations_for_blocks     | true |
| GetOperationsMessage::get_operations                         | true |
| GetProtocolsMessage::get_protocols                           | true |
| Mempool::known_valid                                         | true |
| Mempool::pending                                             | true |
| NackInfo::potential_peers_to_connect                         | true |
| NetworkVersion::chain_name                                   | true |
| Operation::data                                              | true |
| OperationMessage::operation                                  | false |
| OperationsForBlocksMessage::operation_hashes_path            | true |
| OperationsForBlocksMessage::operations                       | true |
| PeerMessageResponse::message                                 | false |
| Protocol::components                                         | false |
| ProtocolMessage::protocol                                    | false |
| SwapMessage::point                                           | true |
