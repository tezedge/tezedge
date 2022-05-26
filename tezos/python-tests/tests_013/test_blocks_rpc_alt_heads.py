import time
import pytest
from tools import constants, utils
from launchers.sandbox import Sandbox
from . import protocol
from client.client import Client
from client.client_output import ProposeForResult
from typing import List


@pytest.mark.incremental
@pytest.mark.slow
@pytest.mark.baker
class TestNonceSeedRevelationMod:
    """Test /chains/:chain/blocks RPC.

    Runs three nodes. Nodes 0 and 1 are connected via node 2.
    While connected, node 0 is used to propose a block.
    Then node 2 terminates, breaking communications between 0 and 1.
    Node 1 is used to propose another block on the same level.
    Node 0 is used to endorse round 0 block and bake the next level.
    Then node 2 is started, and node 0's head is accepted (higher fitness)
    making node 1 proposal orphaned.

    After that the test checks that the RPC returns identical results for both nodes,
    thus including blocks that are in the chain, not applied locally only."""

    def test_init(self, sandbox: Sandbox):
        """Run nodes 0 and 1, and noce 2 that connected to 0 and 1."""

        sandbox.add_node(0, params=constants.NODE_PARAMS, peers=[])
        sandbox.add_node(1, params=constants.NODE_PARAMS, peers=[])
        sandbox.add_node(2, params=constants.NODE_PARAMS, peers=[0, 1])

        # client setup
        parameters=protocol.get_parameters()
        parameters['consensus_threshold'] = int(2*parameters['consensus_committee_size']/3)
        protocol.activate(sandbox.client(0), parameters=parameters)

    def test_propose_round0(self, sandbox: Sandbox, session: dict):
        """Propose round 0 block"""
        client=sandbox.client(0)
        level=client.get_header('head')['level']
        client.propose(delegates=['bootstrap3'], args=['--minimal-timestamp', '--force'])
        assert client.get_header('head')['level'] == level + 1

    def test_disconnect(self, sandbox: Sandbox):
        """Disconnects nodes 0 and 1"""
        sandbox.node(2).terminate()

    def test_propose_round1(self, sandbox: Sandbox, session: dict):
        """Propose round 1 block, only visible to node 1"""
        client=sandbox.client(1)
        header1=client.get_header('head')
        while True:
            try:
                client.run(['propose', 'for', 'bootstrap5', '--minimal-timestamp'])
                break
            except:
                time.sleep(1)
        header2=client.get_header('head')
        assert header2['hash'] != header1['hash']
        assert header2['level'] == header1['level']

    def test_endorse_round0(self, sandbox: Sandbox):
        """Endorse round 0 block, only visible to node 0"""
        client=sandbox.client(0)
        for i in range(1, 6):
            client.run(["endorse", "for", f"bootstrap{i}", "--force"])

    def test_bake_successor(self, sandbox: Sandbox):
        """Bake round 0's successor, only visible to node 0"""
        client=sandbox.client(0)
        level=client.get_header('head')['level']
        utils.bake(client)
        assert client.get_header('head')['level'] == level + 1

    def test_sync_nodes(self, sandbox: Sandbox):
        """Restore communication between nodes 0 and 1, making round 1 block orphaned"""
        sandbox.node(2).run()
        assert sandbox.client(2).check_node_listening()
        time.sleep(2)

    def test_verify_blocks_rpc(self, sandbox: Sandbox, session: dict):
        """Test that blocks RPC is identical for all nodes"""
        blocks=[sandbox.client(i).rpc('get', f'/chains/main/blocks?length=2') for i in range(3)]
        for i in range(2):
            assert blocks[i][0] == blocks[i+1][0]
