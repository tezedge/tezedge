# replacing original file `test_baker_endorser.py`
# added check for operations inclusion

import itertools
import random
import time
import subprocess
import pytest
from tools import utils, constants
from launchers.sandbox import Sandbox
from . import protocol

random.seed(42)
KEYS = [f'bootstrap{i}' for i in range(1, 6)]
NEXT_KEY = itertools.cycle(KEYS)
NUM_NODES = 5
NEW_NODES = 5
REPLACE = False
NUM_CYCLES = 60
TIME_BETWEEN_CYCLE = 2
assert NEW_NODES <= NUM_CYCLES


def random_op(client):
    sender = next(NEXT_KEY)
    dest = random.choice([key for key in KEYS if key != sender])
    amount = random.randrange(10000)
    return client.transfer(amount, sender, dest)


@pytest.mark.baker
@pytest.mark.endorser
@pytest.mark.multinode
@pytest.mark.slow
@pytest.mark.incremental
class TestAllDaemonsWithOperations:
    """Runs two baker and two endorsers, generates random op, and
    add (or replace) new nodes dynamically. After a little while,
    we kill the bakers and check everyone synchronize to the same head."""

    def test_setup_network(self, sandbox: Sandbox):
        parameters = protocol.get_parameters()
        # each priority has a delay of 1 sec
        # parameters["time_between_blocks"] = ["1"]
        for i in range(NUM_NODES):
            sandbox.add_node(i, params=constants.NODE_PARAMS)
        protocol.activate(sandbox.client(0), parameters=parameters)
        time.sleep(3)
        for i in range(NUM_NODES - 1):
            sandbox.add_baker(
                i,
                [f'bootstrap{5 - i}'],
                protocol.DAEMON,
                run_params=['--liquidity-baking-toggle-vote', 'pass'],
            )

    def test_wait_for_protocol(self, sandbox: Sandbox):
        clients = sandbox.all_clients()
        for client in clients:
            proto = protocol.HASH
            assert utils.check_protocol(client, proto)

    def test_network_gen_operations_and_add_nodes(
        self, sandbox: Sandbox, session
    ):
        node_add_period = NUM_CYCLES // NEW_NODES
        for cycle in range(NUM_CYCLES):
            i = random.randrange(NUM_NODES)
            client = sandbox.client(i)
            try:
                transfer = random_op(client)
                session[f'op{cycle}'] = transfer.operation_hash
            except subprocess.CalledProcessError:
                # some operations may be invalid, e.g. the client sends
                # several operation with the same counter
                print('# IGNORED INVALID OPERATION')

            if cycle % node_add_period == 0:
                # add node
                running_nodes = list(sandbox.nodes.keys())
                new_node = max(running_nodes) + 1
                if REPLACE:
                    running_nodes.remove(0)
                    running_nodes.remove(1)
                    sandbox.rm_node(random.choice(running_nodes))
                sandbox.add_node(new_node, params=constants.NODE_PARAMS)
                proto = protocol.HASH
                assert utils.check_protocol(sandbox.client(new_node), proto)
            time.sleep(TIME_BETWEEN_CYCLE)

    def test_kill_baker(self, sandbox: Sandbox):
        for i in range(NUM_NODES - 1):
            sandbox.rm_baker(i, proto=protocol.DAEMON)

    def test_synchronize(self, sandbox: Sandbox):
        utils.synchronize(sandbox.all_clients())

    @pytest.mark.xfail(reason="Not enough time to reach level?")
    def test_progress(self, sandbox: Sandbox):
        level = sandbox.client(0).get_level()
        assert level >= 5

    def test_check_operations(self, sandbox: Sandbox):
        min_level = min(
            [client.get_level() for client in sandbox.all_clients()]
        )
        heads_hash = set()
        # check there is exactly one head
        for client in sandbox.all_clients():
            block_hash = utils.get_block_hash(client, min_level)
            heads_hash.add(block_hash)
        assert len(heads_hash) == 1
        # check for operations inclusion
        ops_len = [0, 0, 0, 0]
        client = sandbox.all_clients()[0]
        for level in range(min_level):
            op_ll = utils.get_block_at_level(client, level)['operations']
            if len(op_ll) == 4:
                for i in range(4):
                    ops_len[i] += len(op_ll[i])
        print(ops_len)
        # more then half is included
        assert ops_len[3] * 2 > NUM_CYCLES

    def test_check_logs(self, sandbox: Sandbox):
        if not sandbox.log_dir:
            pytest.skip()
        assert sandbox.logs
        # TODO check more things in the log! endorsement, baking...
        error_pattern = r"Uncaught|registered"
        assert utils.check_logs(sandbox.logs, error_pattern)
