import pytest
from tools import utils, constants
from launchers.sandbox import Sandbox
import time


BAKE_ARGS = ['--max-priority', '512', '--minimal-timestamp']
NUM_NODES = 3


@pytest.mark.multinode
@pytest.mark.incremental
class TestFork:
    """Constructs two independent branches on disconnected subsets of nodes,
       one head has higher fitness. At reconnection, check the the highest
       fitness head is the chosen one """

    def test_init(self, sandbox: Sandbox):
        for i in range(NUM_NODES):
            sandbox.add_node(i, params=constants.NODE_PARAMS)
        utils.activate_alpha(sandbox.client(0))

    def test_level(self, sandbox: Sandbox):
        time.sleep(3)
        level = 1
        for client in sandbox.all_clients():
            assert utils.check_level(client, level)

    def test_terminate_nodes_1_and_2(self, sandbox: Sandbox):
        time.sleep(3)
        sandbox.node(1).terminate()
        sandbox.node(2).terminate()

    def test_bake_node_0(self, sandbox: Sandbox):
        time.sleep(3)
        """Client 0 bakes block A at level 2, not communicated to 1 and 2"""
        sandbox.client(0).bake('bootstrap1', BAKE_ARGS)

    def test_endorse_node_0(self, sandbox: Sandbox, session: dict):
        time.sleep(3)
        """bootstrap1 builds an endorsement for block A"""
        client = sandbox.client(0)
        client.endorse('bootstrap1')
        mempool = client.get_mempool()
        endorsement = mempool['applied'][0]
        session['endorsement1'] = endorsement

    def test_bake_node_0_again(self, sandbox: Sandbox):
        time.sleep(3)
        """Client 0 bakes block A' at level 3, not communicated to 1 and 2"""
        sandbox.client(0).bake('bootstrap1', BAKE_ARGS)

    def test_first_branch(self, sandbox: Sandbox, session: dict):
        time.sleep(3)
        head = sandbox.client(0).get_head()
        assert head['header']['level'] == 3
        session['hash1'] = head['hash']
        assert len(head['operations'][0]) == 1

    def test_terminate_node_0(self, sandbox: Sandbox):
        time.sleep(3)
        sandbox.node(0).terminate()

    def test_restart_node_2(self, sandbox: Sandbox):
        time.sleep(3)
        sandbox.node(2).run()
        assert sandbox.client(2).check_node_listening()

    def test_bake_node_2(self, sandbox: Sandbox):
        time.sleep(3)
        """Client 2 bakes block B at level 2, not communicated to 0 and 1"""
        sandbox.client(2).bake('bootstrap1', BAKE_ARGS)

    def test_bake_node_2_again(self, sandbox: Sandbox):
        time.sleep(3)
        """Client 2 bakes block B' at level 3, not communicated to 0 and 1"""
        sandbox.client(2).bake('bootstrap1', BAKE_ARGS)

    def test_second_branch(self, sandbox: Sandbox, session: dict):
        time.sleep(3)
        head = sandbox.client(2).get_head()
        session['hash2'] = head['hash']
        assert head['header']['level'] == 3
        assert not head['operations'][0]

    def test_restart_all(self, sandbox: Sandbox):
        time.sleep(3)
        sandbox.node(0).run()
        sandbox.node(1).run()
        assert sandbox.client(0).check_node_listening()
        assert sandbox.client(1).check_node_listening()

#    def test_check_head(self, sandbox: Sandbox, session: dict):
#        """All nodes are at level 3, head should be hash1"""
#        for client in sandbox.all_clients():
#            head = client.get_head()
#            assert session['hash1'] == head['hash']
