// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use serial_test::serial;

use crypto::hash::{HashType, ProtocolHash};
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_client::client;

fn protocol(hash: &str) -> ProtocolHash {
    HashType::ProtocolHash.b58check_to_hash(hash).unwrap()
}

fn data(data_as_hex: &str) -> Vec<u8> {
    hex::decode(data_as_hex).unwrap()
}

fn key(key_as_string: &str) -> Vec<String> {
    let key: Vec<&str> = key_as_string.split(", ").collect();
    key.iter().map(|k| k.to_string()).collect()
}

#[test]
#[serial]
fn test_fn_decode_context_data() {
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: is_ocaml_log_enabled(),
    })
    .unwrap();

    assert_eq!(
        "\"Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H\"".to_string(),
        client::decode_context_data(
            protocol("Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H"),
            key("protocol"),
            data("32227de5351803223564d2f40dbda7fa0fd20682ddfe743d51af3d08f8114273"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"genesis\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, version"),
            data("67656e65736973"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "{\"status\":\"not_running\"}".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("test_chain"),
            data("00"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "1".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, v1, first_level"),
            data("00000001"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"42065708404\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, commitments, 6c, 00, 4d, 09, b8, 9efefb1abbc3555781ffa2ffa57e29"),
            data("f48abfda9c01"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "{\"preserved_cycles\":3,\"blocks_per_cycle\":2048,\"blocks_per_voting_period\":8192,\"time_between_blocks\":[\"30\",\"40\"]}".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, v1, constants"),
            data("ff03ff000008000000ff00002000ff00000010000000000000001e00000000000000280000000000000000000000000000"),
        ).unwrap().unwrap()
    );

    // should fail, som return None
    assert!(client::decode_context_data(
        protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
        key("data, v1, constants"),
        data("f48abfda9c01"),
    )
    .unwrap()
    .is_none());

    assert_eq!(
        "12".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, last_block_priority"),
            data("000c"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "0".to_string(),
        client::decode_context_data(
            protocol("PsBABY5HQTSkA4297zNHfsZNKtxULfL18y95qb3m53QJiXGmrbU"),
            key("data, block_priority"),
            data("0000"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, active_delegates_with_rolls, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, delegates, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, delegates_with_frozen_balance, 61, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_rolls_and_cycles() {
    assert_eq!(
        "0".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, next"),
            data("00000000"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "0".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, limbo"),
            data("00000000"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"p2pk66EmFoQS6b2mYLvCrwjXs7XT1A2znX26HcT9YMiGsyCHyDvsLaF\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, owner, current, 2, 0, 2"),
            data("0202db58471f14e5286a13a30b29c6c685649bfd312e8b80b100a7f1307cabd4ca86"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "797".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, index, 30, 3, 798, successor"),
            data("0000031d"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, random_seed"),
            data("0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "1".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, roll_snapshot"),
            data("0001"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "2700".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, last_roll, 0"),
            data("00000a8c"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "[\"nceVLU2CcmkFGAC4raDPbNMmutpnG3rzbfqPnK7aiCZAipFccp2TK\",\"tz1gT2uVzSqBq3ZdH6uG4uJq8bqLga9XKrrq\",\"0\",\"0\"]".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, cycle, 4, nonces, 8768"),
            data("00927e7e2e7df224676fc98f8f823fef5a49ceaeaae50249303c2a40dac1bee8c100e4463e7bea3a75cebb112159a7ec6c29225a12410000"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_ramp_up() {
    assert_eq!(
        "[\"16000000\",\"2000000\"]".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, ramp_up, rewards, 5"),
            data("80c8d00780897a"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "[\"512000000\",\"64000000\"]".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, ramp_up, deposits, 16"),
            data("808092f40180a0c21e"),
        )
        .unwrap()
        .unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_votes() {
    assert_eq!(
        "\"proposal\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, votes, current_period_kind"),
            data("00"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "8000".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, votes, current_quorum"),
            data("00001f40"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "59".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, listings, ed25519, 03, cb, 7d, 78, 42, 406496fc07288635562bfd17e176c4"),
            data("0000003b"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "1613".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, listings_size"),
            data("0000064d"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "5973".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, participation_ema"),
            data("00001755"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"yay\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key(
                "data, votes, ballots, ed25519, a3, 1e, 81, ac, 34, 25310e3274a4698a793b2839dc0afa"
            ),
            data("00"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, current_proposal"),
            data("3e5e3a606afab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "1".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, proposals_count, ed25519, 43, a8, 4d, 01, 3b, 61b4c2cafe3fb89463329d7295a377"),
            data("0001"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, votes, proposals, 3e, 5e, 3a, 60, 6a, fab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb, ed25519, 43, a8, 4d, 01, 3b, 61b4c2cafe3fb89463329d7295a377"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_contracts() {
    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, global_counter"),
            data("00"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"8000000000000\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, balance"),
            data("8080a2a9eae801"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, manager"),
            data("00026fde46af0356a0476dae4e4600172dc9309b3aa4"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, spendable"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, delegate"),
            data("026fde46af0356a0476dae4e4600172dc9309b3aa4"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"7990000000000\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, change"),
            data("80b8f288c5e801"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "31".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, roll_list"),
            data("0000001f"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "7".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, delegate_desactivation"),
            data("00000007"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, deposits"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, fees"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, rewards"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, paid_bytes"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"232\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, used_bytes"),
            data("a803"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "202".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, len, code"),
            data("000000ca"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "[{\"prim\":\"parameter\",\"args\":[{\"prim\":\"or\",\"args\":[{\"prim\":\"lambda\",\"args\":[{\"prim\":\"unit\"},{\"prim\":\"list\",\"args\":[{\"prim\":\"operation\"}]}],\"annots\":[\"%do\"]},{\"prim\":\"unit\",\"annots\":[\"%default\"]}]}]},{\"prim\":\"storage\",\"args\":[{\"prim\":\"key_hash\"}]},{\"prim\":\"code\",\"args\":[[[[{\"prim\":\"DUP\"},{\"prim\":\"CAR\"},{\"prim\":\"DIP\",\"args\":[[{\"prim\":\"CDR\"}]]}]],{\"prim\":\"IF_LEFT\",\"args\":[[{\"prim\":\"PUSH\",\"args\":[{\"prim\":\"mutez\"},{\"int\":\"0\"}]},{\"prim\":\"AMOUNT\"},[[{\"prim\":\"COMPARE\"},{\"prim\":\"EQ\"}],{\"prim\":\"IF\",\"args\":[[],[[{\"prim\":\"UNIT\"},{\"prim\":\"FAILWITH\"}]]]}],[{\"prim\":\"DIP\",\"args\":[[{\"prim\":\"DUP\"}]]},{\"prim\":\"SWAP\"}],{\"prim\":\"IMPLICIT_ACCOUNT\"},{\"prim\":\"ADDRESS\"},{\"prim\":\"SENDER\"},[[{\"prim\":\"COMPARE\"},{\"prim\":\"EQ\"}],{\"prim\":\"IF\",\"args\":[[],[[{\"prim\":\"UNIT\"},{\"prim\":\"FAILWITH\"}]]]}],{\"prim\":\"UNIT\"},{\"prim\":\"EXEC\"},{\"prim\":\"PAIR\"}],[{\"prim\":\"DROP\"},{\"prim\":\"NIL\",\"args\":[{\"prim\":\"operation\"}]},{\"prim\":\"PAIR\"}]]}]]}]".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, data, code"),
            data("000000c602000000c105000764085e036c055f036d0000000325646f046c000000082564656661756c740501035d050202000000950200000012020000000d03210316051f02000000020317072e020000006a0743036a00000313020000001e020000000403190325072c020000000002000000090200000004034f0327020000000b051f02000000020321034c031e03540348020000001e020000000403190325072c020000000002000000090200000004034f0327034f0326034202000000080320053d036d0342"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "30".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, len, storage"),
            data("0000001e"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "{\"bytes\":\"00b2e19a9e74440d86c59f13dab8a18ff873e889ea\"}".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, data, storage"),
            data("0000001a0a0000001500b2e19a9e74440d86c59f13dab8a18ff873e889ea"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, contracts, index, 55, 36, 40, 10, 0e, b8, 000043a84d013b61b4c2cafe3fb89463329d7295a377, delegated, 55, 36, 40, 10, 0e, b8, 000043a84d013b61b4c2cafe3fb89463329d7295a377"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, contracts, index, 55, 36, 40, 10, 0e, b8, 000043a84d013b61b4c2cafe3fb89463329d7295a377, inactive_delegate"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_big_maps() {
    assert_eq!(
        "\"14\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, next"),
            data("0e"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "\"228\"".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, index, f5, c8, 90, 54, 17, 93, 9, total_bytes"),
            data("a403"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "{\"prim\":\"address\"}".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, index, f5, c8, 90, 54, 17, 93, 9, key_type"),
            data("036e"),
        )
        .unwrap()
        .unwrap()
    );

    assert_eq!(
        "{\"prim\":\"pair\",\"args\":[{\"prim\":\"nat\"},{\"prim\":\"map\",\"args\":[{\"prim\":\"address\"},{\"prim\":\"nat\"}]}]}".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, index, f5, c8, 90, 54, 17, 93, 9, value_type"),
            data("076503620760036e0362"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "77".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, index, 64, 22, 06, 31, 4f, 53, 4, contents, 8f, 8e, 6a, 2e, 70, dd1df0b8ce0025ff9fd2ad956525abd0165a7d536534a60cf099d1, len"),
            data("0000004d"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "{\"prim\":\"Pair\",\"args\":[{\"prim\":\"Pair\",\"args\":[{\"bytes\":\"00001f67eb5af692ec0cab1c5743fcab82bfd20b2d61\"},{\"bytes\":\"0000b855ca20b48b2d22ff633f1e8f1be30b747ca722\"}]},{\"prim\":\"Pair\",\"args\":[{\"prim\":\"Pair\",\"args\":[{\"int\":\"7020000\"},{\"int\":\"1570129462\"}]},{\"int\":\"20000\"}]}]}".to_string(),
        client::decode_context_data(
            protocol("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"),
            key("data, big_maps, index, 64, 22, 06, 31, 4f, 53, 4, contents, 8f, 8e, 6a, 2e, 70, dd1df0b8ce0025ff9fd2ad956525abd0165a7d536534a60cf099d1, data"),
            data("070707070a0000001600001f67eb5af692ec0cab1c5743fcab82bfd20b2d610a000000160000b855ca20b48b2d22ff633f1e8f1be30b747ca7220707070700a0f7d80600b698b2d90b00a0b802"),
        ).unwrap().unwrap()
    );
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>()
        .unwrap()
}
