import random
import time
import pytest
from tools import utils, constants
from launchers.sandbox import Sandbox

NUM_NODES = 2
NEW_NODES = 2
REPLACE = False
ERROR_PATTERN = r"Uncaught|registered"


@pytest.mark.baker
@pytest.mark.multinode
@pytest.mark.slow
@pytest.mark.incremental
class TestManyNodesBootstrap:
    """Run many nodes, wait a while, run more nodes, check logs"""

    def test_init(self, sandbox: Sandbox):
        sandbox.add_node(0, params=constants.NODE_PARAMS)
        parameters = dict(constants.PARAMETERS)
        parameters["time_between_blocks"] = ["1", "0"]
        utils.activate_alpha(sandbox.client(0), parameters)
        time.sleep(2)
        sandbox.add_baker(0, 'bootstrap1', proto=constants.ALPHA_DAEMON)
        sandbox.add_node(1, params=constants.NODE_PARAMS)
        sandbox.add_baker(1, 'bootstrap2', proto=constants.ALPHA_DAEMON)

    def test_add_nodes(self, sandbox: Sandbox):
        for i in range(2, NUM_NODES):
            sandbox.add_node(i, params=constants.NODE_PARAMS)

    def test_sleep_10s(self):
        time.sleep(10)

    def test_add_more_nodes(self, sandbox: Sandbox):
        new_node = NUM_NODES
        for i in range(NEW_NODES):
            if REPLACE:
                running_nodes = list(sandbox.nodes.keys())
                running_nodes.remove(0)
                running_nodes.remove(1)
                sandbox.rm_node(random.choice(running_nodes))
            sandbox.add_node(new_node + i, params=constants.NODE_PARAMS)

    def test_kill_baker(self, sandbox: Sandbox):
        assert utils.check_logs(sandbox.logs, ERROR_PATTERN)
        sandbox.rm_baker(0, proto=constants.ALPHA_DAEMON)
        sandbox.rm_baker(1, proto=constants.ALPHA_DAEMON)

    def test_synchronize(self, sandbox: Sandbox):
        utils.synchronize(sandbox.all_clients())

    def test_progress(self, sandbox: Sandbox):
        level = sandbox.client(0).get_level()
        assert level >= 5

    def test_check_logs(self, sandbox: Sandbox):
        if not sandbox.log_dir:
            pytest.skip()
        assert sandbox.logs
        assert utils.check_logs(sandbox.logs, ERROR_PATTERN)
