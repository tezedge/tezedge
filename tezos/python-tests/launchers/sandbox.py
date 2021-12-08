import os
import time
from typing import Callable, Dict, List, Tuple

from client.client import Client
from daemons.baker import Baker
from daemons.endorser import Endorser
from daemons.accuser import Accuser
from daemons.node import Node

NODE = 'light-node'
CLIENT = 'tezos-client'
CLIENT_ADMIN = 'tezos-admin-client'
BAKER = 'tezos-baker'
ENDORSER = 'tezos-endorser'
ACCUSER = 'tezos-accuser'


class Sandbox:
    """A Sandbox manages a set of clients, nodes and daemons running in
    sandbox mode.

    Wrapper objects (Client, Node, Baker...) allow interacting with the
    corresponding processes. In the sandbox, they are identified by an
    integer 0 <= node_id < num_peers, which corresponds to ports
    ``rpc + node_id``, ``p2p + node_id``.

    Clients, nodes and daemons can be dynamically added or removed. Daemons are
    protocol specific. There can be more than one daemon for a given node,
    as long as they correspond to different protocol.

    Whenever a node has been added with `add_node()`, we can access to a
    corresponding client object `client()` to interact with this node.

    Sandbox is a python context manager. In particular, the allocated resources
    are cleaned when `__exit__()` is called. It cleans all temporary
    directories, allocated for the sandbox, or the Tezos binaries. And
    it terminates all running processes managed by this sandbox.
    """

    def _wrap_path(self, binary: str, branch: str, proto="") -> str:
        """construct binary name from branch and proto

        - follows tezos naming convention for binaries based on protocol
        - use branch as a prefix dir.
        """
        if proto:
            binary = f'{binary}-{proto}'
        res = os.path.join(self.binaries_path, branch, binary)  # type: str
        assert os.path.isfile(res), f'{res} is not a file'
        return res

    def __init__(
        self,
        binaries_path: str,
        identities: Dict[str, Dict[str, str]],
        rpc: int = 28730,
        p2p: int = 29730,
        num_peers: int = 45,
        log_dir: str = None,
        singleprocess: bool = False,
    ):
        """
        Args:
            binaries_path (str): path to the binaries (client, node, baker,
                endorser). Typically, this parameter is TEZOS_HOME.
            identities (dict): identities known to all clients.
            rpc (int): base RPC port
            p2p (int): base P2P port
            num_peers (int): max number of peers
            log_dir (str): optional log directory for node/daemons logs

        Binaries contained in `binaries_path` are supposed to follow the
        naming conventions used in the Tezos codebase. For instance,
        `tezos-endorser-002-PsYLVpVv` is the endorser for protocol
        `-002-PsYLVpVv`.

        `identities` map aliases to dict of the form
        { 'identity': "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
            'public': "edpkuBknW28nW72KG6RoHtYW7p12T6GKc7nAbwYX5m8Wd9sDVC9yav",
            'secret': "unencrypted:edsk3gUfUPy...") }
        """
        assert 1 <= num_peers <= 100
        assert os.path.isdir(binaries_path), f'{binaries_path} is not a dir'
        self.binaries_path = binaries_path
        self.log_dir = log_dir
        self.identities = dict(identities)
        self.rpc = rpc
        self.p2p = p2p
        self.num_peers = num_peers
        self.clients = {}  # type: Dict[int, Client]
        self.nodes = {}  # type: Dict[int, Node]
        # bakers for each protocol
        self.bakers = {}  # type: Dict[str, Dict[int, Baker]]
        self.endorsers = {}  # type: Dict[str, Dict[int, Endorser]]
        self.accusers = {}  # type: Dict[str, Dict[int, Accuser]]
        self.counter = 0
        self.logs = []  # type: List[str]
        self.singleprocess = singleprocess

    def __enter__(self):
        return self

    def register_node(
        self,
        node_id: int,
        node_dir: str = None,
        peers: List[int] = None,
        params: List[str] = None,
        log_levels: Dict[str, str] = None,
        private: bool = True,
        use_tls: Tuple[str, str] = None,
        branch: str = "",
        node_config: dict = None,
    ):
        """Instantiate a Node object and add to sandbox manager

        See add_node for args documentation.

        This node isn't run yet, but is "prepared" with the following
        parameters.

        tezos-node run
           --data-dir TMP_DIR
           --no-bootstrap-peers
           --peer TRUSTED_PEER_1 ... --peer TRUSTED_PEER_n #
           --private-mode # if private is True

        for tezedge light-node
        light-node --tezos-data-dir TMP_DIR
            --disable-bootstrap-lookup
            --peers TRUSTED_PEER_1,...,TRUSTED_PEER_n
            --private-mode # if private is True
        """
        assert node_id not in self.nodes, f'Already a node for id={node_id}'
        rpc_node = self.rpc + node_id
        p2p_node = self.p2p + node_id
        assert 0 <= node_id < self.num_peers, f'{node_id} outside bounds'
        if peers is None:
            peers = list(range(self.num_peers))
        assert all(0 <= peer < self.num_peers for peer in peers)

        log_file = None
        if self.log_dir:
            log_file = f'{self.log_dir}/node{node_id}_{self.counter}.txt'
            self.logs.append(log_file)
            self.counter += 1

        params = [] if params is None else params
        if private:
            # TODO: FIX THIS IN TEZEDGE
            params = params + ['--private-node', 'true']
        params = params + ['--network=sandbox']
        if os.environ.get('CONTEXT_MUST_SURVIVE_RESTARTS'):
            # NOTE: The TezEdge in-memory context doesn't survive restarts,
            # and some tests require that, disable for now.
            params = params + ['--tezos-context-storage=irmin']
        peers_rpc = [self.p2p + p for p in peers]
        node_bin = self._wrap_path(NODE, branch)
        node = Node(
            node_bin,
            config=node_config,
            node_dir=node_dir,
            p2p_port=p2p_node,
            rpc_port=rpc_node,
            peers=peers_rpc,
            log_file=log_file,
            params=params,
            log_levels=log_levels,
            use_tls=use_tls,
            singleprocess=self.singleprocess,
        )

        self.nodes[node_id] = node
        return node

    def instanciate_client(
        self,
        rpc_port: int,
        use_tls: Tuple[str, str] = None,
        branch: str = "",
        mode: str = None,
        client_factory: Callable = Client,
    ):
        scheme = 'https' if use_tls else 'http'
        endpoint = f'{scheme}://localhost:{rpc_port}'
        return self.create_client(
            branch=branch,
            client_factory=client_factory,
            mode=mode,
            endpoint=endpoint,
        )

    def create_client(
        self,
        branch: str = "",
        mode: str = None,
        client_factory: Callable = Client,
        **kwargs,
    ) -> Client:
        """
        Creates a new client. Because this method doesn't require a Node,
        it is appropriate for tests that do not need a node, such as
        those of the mockup client.

        This client isn't registered in the Sandbox. It means the caller
        has to perform the cleanup itself by calling the client cleanup method.

        Args:
            branch (str): sub-dir where to lookup the node and client
                          binary, default = "". Allows execution of different
                          versions of nodes.
            mode (str): the client's mode to use. One of
                        ["client", "mockup", "proxy]. None defaults to
                        "client".
            client_factory (Callable): the constructor of clients. Defaults to
                                       Client. Allows e.g. regression testing.
            **kwargs: arguments passed to client_factory
        """
        local_admin_client = self._wrap_path(CLIENT_ADMIN, branch)
        local_client = self._wrap_path(CLIENT, branch)
        return client_factory(
            local_client, local_admin_client, mode=mode, **kwargs
        )

    def get_new_client(
        self,
        node: Node,
        use_tls: Tuple[str, str] = None,
        branch: str = "",
        client_factory: Callable = Client,
        config_client: bool = True,
    ):
        """
        Create a new client for a node.
            node (Node): the node the client should be initialised for
            use_tls (Tuple[str, str]): use TLS
            branch (str): sub-dir where to lookup the node and client
                            binary, default = "". Allows execution of different
                            versions of nodes.
            client_factory (Callable): the constructor of clients. Defaults to
                                        Client. Allows e.g. regression testing.
            config_client (bool): initialize client with sandbox identities,
                                    default=True

        This client isn't registered in the Sandbox. It means the caller
        has to perform the cleanup itself by calling the client cleanup method.

        TODO: we probably want to be able to register more than one client for
              a given node, so that the sandbox context manager can properly
              clean up all resources.
        """
        client = self.instanciate_client(
            rpc_port=node.rpc_port,
            use_tls=use_tls,
            branch=branch,
            client_factory=client_factory,
        )
        self.init_client(client, node, config_client)
        return client

    def register_client(
        self,
        node_id: int,
        rpc_port: int,
        use_tls: Tuple[str, str] = None,
        branch: str = "",
        mode: str = None,
        client_factory: Callable = Client,
    ):
        """Instantiate a Client and add it to the sandbox manager"""
        error_msg = f'Already a client for id={node_id}'
        assert node_id not in self.clients, error_msg
        client = self.instanciate_client(
            rpc_port=rpc_port,
            use_tls=use_tls,
            branch=branch,
            mode=mode,
            client_factory=client_factory,
        )
        self.clients[node_id] = client
        return client

    def init_node(self, node, snapshot, reconstruct):
        """Generate node id and import snapshot """
        # node.init_id()
        # node.init_config()
        if snapshot is not None:
            params = ['--reconstruct'] if reconstruct else []
            node.snapshot_import(snapshot, params)

    def init_client(
        self, client, node: Node = None, config_client: bool = True
    ):
        """Initialize client with bootstrap keys. If node object is provided,
        check whether the node is running and responsive"""

        if node is not None and not client.check_node_listening():
            node_id = node.rpc_port - self.rpc
            assert node.poll() is None, "# Node {node_id} isn't running"
            node.kill()
            assert False, f"# Node {node_id} isn't responding to RPC"

        client.run(['-w', 'none', 'config', 'update'])
        if config_client:
            for name, iden in self.identities.items():
                client.import_secret_key(name, iden['secret'])

    def add_node(
        self,
        node_id: int,
        node_dir: str = None,
        peers: List[int] = None,
        params: List[str] = None,
        log_levels: Dict[str, str] = None,
        private: bool = True,
        config_client: bool = True,
        use_tls: Tuple[str, str] = None,
        snapshot: str = None,
        reconstruct: bool = False,
        branch: str = "",
        node_config: dict = None,
        mode: str = None,
        client_factory: Callable = Client,
    ) -> None:
        """Launches new node with given node_id and initializes client

        Args:
            node_id (int): id of the node, defines its RPC/P2P port and serves
                           as an identifier for client and daemons
            node_dir (str): path to the node's data directory
            peer (list): id of peers initialized trusted by this nodes
            params (list): list of additional parameters to run the node
            log_levels (dict): log levels. e.g. {"p2p.connection-pool":"debug"}
            private (bool): enable private mode, default=True
            config_client (bool): initialize client with sandbox identities,
                                  default=True
            use_tls (Tuple[str, str]): couple of filenames containing
                           (tezos.crt, tezos.k), default=None
            snapshot (str): import snapshot  before initializing node
            branch (str): sub-dir where to lookup the node and client
                          binary, default = "". Allows execution of different
                          versions of nodes.
            mode (str): the mode to use, one of "client", "mockup", or
                        "proxy", default=None (equivalent to "client").
            client_factory (Callable): the constructor of clients. Defaults to
                                       Client. Allows e.g. regression testing.

        This registers a node and a client for the given id. It initializes
        both the client and the node, and run the node.

        tezos-node run
           --data-dir TMP_DIR
           --no-bootstrap-peers
           --peer TRUSTED_PEER_1 ... --peer TRUSTED_PEER_n #
           --private-mode # if private is True

        Whenever a node has been added with `add_node()`, we can access a
        corresponding client object `client()` to interact with this node.
        """
        node = self.register_node(
            node_id,
            node_dir,
            peers,
            params,
            log_levels,
            private,
            use_tls,
            branch,
            node_config,
        )

        self.init_node(node, snapshot, reconstruct)

        node.run()

        rpc_port = node.rpc_port
        client = self.register_client(
            node_id=node_id,
            rpc_port=rpc_port,
            use_tls=use_tls,
            branch=branch,
            mode=mode,
            client_factory=client_factory,
        )

        self.init_client(client, node, config_client)

    def add_baker(
        self,
        node_id: int,
        account: str,
        proto: str,
        params: List[str] = None,
        branch: str = "",
        run_params: List[str] = None,
    ) -> None:
        """
        Add a baker associated to a node.

        Args:
            node_id (int): id of corresponding node
            account (str): account to bake for
            proto (str): name of protocol, used to determine the binary to
                         use. E.g. 'alpha` for `tezos-baker-alpha`.
            params (list): additional parameters
            branch (str): see branch parameter for `add_node()`
        """
        assert node_id in self.nodes, f'No node running with id={node_id}'
        if proto not in self.bakers:
            self.bakers[proto] = {}
        assert_msg = f'Already a baker for proto={proto} and id={node_id}'
        assert node_id not in self.bakers[proto], assert_msg
        baker_path = self._wrap_path(BAKER, branch, proto)
        node = self.nodes[node_id]
        client = self.clients[node_id]
        rpc_node = node.rpc_port

        log_file = None
        if self.log_dir:
            log_file = (
                f'{self.log_dir}/baker-{proto}_{node_id}_#'
                f'{self.counter}.txt'
            )
            self.logs.append(log_file)
            self.counter += 1

        baker = Baker(
            baker_path,
            rpc_node,
            client.base_dir,
            node.node_dir,
            account,
            params=params,
            log_file=log_file,
            run_params=run_params,
        )
        time.sleep(0.1)
        assert baker.poll() is None, 'seems baker failed at startup'
        self.bakers[proto][node_id] = baker

    def add_endorser(
        self,
        node_id: int,
        account: str,
        proto: str,
        endorsement_delay: int = 0,
        branch: str = "",
    ) -> None:
        """
        Add an endorser associated to a node.

        Args:
            node_id (int): id of corresponding node
            account (str): account to endorse for
            proto (str): name of protocol, used to determine the binary to
                         use. E.g. 'alpha` for `tezos-endorser-alpha`.
            params (list): additional parameters
            branch (str): see branch parameter for `add_node()`
        """
        assert node_id in self.nodes, f'No node running with id={node_id}'
        if proto not in self.endorsers:
            self.endorsers[proto] = {}
        account_param = []  # type: List[str]
        if account is not None:
            account_param = [account]

        assert_msg = f'Already an endorser for proto={proto} and id={node_id}'
        assert node_id not in self.endorsers[proto], assert_msg
        endorser_path = self._wrap_path(ENDORSER, branch, proto)
        node = self.nodes[node_id]
        client = self.clients[node_id]
        rpc_node = node.rpc_port

        log_file = None
        if self.log_dir:
            log_file = (
                f'{self.log_dir}/endorser-{proto}_{node_id}_#'
                f'{self.counter}.txt'
            )
            self.logs.append(log_file)
            self.counter += 1
        params = (
            ['run']
            + account_param
            + ['--endorsement-delay', str(endorsement_delay)]
        )
        endorser = Endorser(
            endorser_path,
            rpc_node,
            client.base_dir,
            params=params,
            log_file=log_file,
        )
        time.sleep(0.1)
        assert endorser.poll() is None, 'seems endorser failed at startup'
        self.endorsers[proto][node_id] = endorser

    def add_accuser(
        self,
        node_id: int,
        proto: str,
        branch: str = "",
    ) -> None:
        """
        Add an accuser associated to a node.

        Args:
            node_id (int): id of corresponding node
            proto (str): name of protocol, used to determine the binary to
                         use. E.g. 'alpha` for `tezos-accuser-alpha`.
            branch (str): see branch parameter for `add_node()`
        """
        assert node_id in self.nodes, f'No node running with id={node_id}'
        if proto not in self.accusers:
            self.accusers[proto] = {}

        assert_msg = f'Already an accuser for proto={proto} and id={node_id}'
        assert node_id not in self.accusers[proto], assert_msg
        accuser_path = self._wrap_path(ACCUSER, branch, proto)
        node = self.nodes[node_id]
        client = self.clients[node_id]
        rpc_node = node.rpc_port

        log_file = None
        if self.log_dir:
            log_file = (
                f'{self.log_dir}/accuser-{proto}_{node_id}_#'
                f'{self.counter}.txt'
            )
            self.logs.append(log_file)
            self.counter += 1
        params = ['run']
        accuser = Accuser(
            accuser_path,
            rpc_node,
            client.base_dir,
            params=params,
            log_file=log_file,
        )
        time.sleep(0.1)
        assert accuser.poll() is None, 'seems accuser failed at startup'
        self.accusers[proto][node_id] = accuser

    def rm_baker(self, node_id: int, proto: str) -> None:
        """Kill baker for given node_id and proto"""
        baker = self.bakers[proto][node_id]
        del self.bakers[proto][node_id]
        baker.terminate_or_kill()

    def rm_endorser(self, node_id: int, proto: str) -> None:
        """Kill endorser for given node_id and proto"""
        endorser = self.bakers[proto][node_id]
        del self.endorsers[proto][node_id]
        endorser.terminate_or_kill()

    def rm_client(self, client_id: int) -> None:
        """Delete client for given client_id"""
        error_msg = f"Client {client_id} wasn't registered"
        assert client_id in self.clients, error_msg
        self.clients[client_id].cleanup()
        del self.clients[client_id]

    def rm_node(self, node_id: int) -> None:
        """Kill/cleanup node for given node_id. Also delete corresponding
        client if was created."""
        node = self.nodes[node_id]
        del self.nodes[node_id]
        if node_id in self.clients:
            self.rm_client(node_id)
        node.terminate_or_kill()
        node.cleanup()

    def client(self, node_id: int) -> Client:
        """ Returns client for node node_id """
        return self.clients[node_id]

    def node(self, node_id: int) -> Node:
        """ Returns node for node_id """
        return self.nodes[node_id]

    def baker(self, node_id: int, proto: str) -> Baker:
        """ Returns baker for node node_i and proto """
        return self.bakers[proto][node_id]

    def all_clients(self) -> List[Client]:
        """Returns the list of all clients to an active node
        (no particular order)."""
        return list(self.clients.values())

    def all_nodes(self) -> List[Node]:
        """ Returns the list of all active nodes (no particular order)."""
        return list(self.nodes.values())

    def __exit__(self, *exc):
        self.cleanup()

    def cleanup(self):
        """Kill all daemons and cleanup temp dirs."""
        for node in self.nodes.values():
            node.terminate_or_kill()
            node.cleanup()
        for proto in self.bakers:
            for baker in self.bakers[proto].values():
                baker.terminate_or_kill()
        for proto in self.endorsers:
            for endorser in self.endorsers[proto].values():
                endorser.terminate_or_kill()
        for client in self.clients.values():
            client.cleanup()

    def are_daemons_alive(self) -> bool:
        """Returns True iff all started daemons/nodes are still alive.

        This is useful to check that background processes didn't die
        accidentally. It assumes that all background processes are running.
        If this is known not to be the case (for instance a node has been
        added, but is not running), it must be removed with
        ``sandbox.rm_node()``.
        """
        daemons_alive = True
        for node_id, node in self.nodes.items():
            if node.poll() is not None:
                print(f'# node {node_id} has failed')
                daemons_alive = False
        for proto in self.bakers:
            for baker_id, baker in self.bakers[proto].items():
                if baker.poll() is not None:
                    print(f'# baker {baker_id} for proto {proto} has failed')
                    daemons_alive = False
        for proto in self.endorsers:
            for endo_id, endorser in self.endorsers[proto].items():
                if endorser.poll() is not None:
                    print(f'# endorser {endo_id} for proto {proto} has failed')
                    daemons_alive = False
        return daemons_alive


class SandboxMultiBranch(Sandbox):
    """Specialized version of `Sandbox` using binaries with different versions.

    Binaries are looked up according to a map from node_id to branch

    For instance,  if we define `branch_map` as:

    branch_map = {i: 'zeronet' if i % 2 == 0 else 'alphanet'
                  for i in range(20)}

    Nodes/client for even ids will be looked up in `binaries_path/zeronet`
    Nodes/client for odd ids will be looked up in `binaries_path/alphanet`

    One advantage of using `SandboxMultibranch` rather than `Sandbox` is that
    we can sometimes run the same tests with different binaries revision by
    simply changing the sandbox.
    """

    def __init__(
        self,
        binaries_path: str,
        identities: Dict[str, Dict[str, str]],
        branch_map: Dict[int, str],
        rpc: int = 18730,
        p2p: int = 19730,
        num_peers: int = 45,
        log_dir: str = None,
        singleprocess: bool = False,
    ):
        """Same semantics as Sandbox class, plus a `branch_map` parameter"""
        super().__init__(
            binaries_path,
            identities,
            rpc,
            p2p,
            num_peers,
            log_dir,
            singleprocess,
        )
        self._branch_map = branch_map
        for branch in list(branch_map.values()):
            error_msg = f'{binaries_path}/{branch} not a dir'
            assert os.path.isdir(f'{binaries_path}/{branch}'), error_msg

    def add_baker(
        self,
        node_id: int,
        account: str,
        proto: str,
        params: List[str] = None,
        branch: str = "",
    ) -> None:
        """branch is overridden by branch_map"""
        branch = self._branch_map[node_id]
        super().add_baker(node_id, account, proto, params, branch)

    def add_endorser(
        self,
        node_id: int,
        account: str,
        proto: str,
        endorsement_delay: int = 0,
        branch: str = "",
    ) -> None:
        """branchs is overridden by branch_map"""
        branch = self._branch_map[node_id]
        super().add_endorser(node_id, account, proto, endorsement_delay, branch)

    def add_node(
        self,
        node_id: int,
        node_dir: str = None,
        peers: List[int] = None,
        params: List[str] = None,
        log_levels: Dict[str, str] = None,
        private: bool = True,
        config_client: bool = True,
        use_tls: Tuple[str, str] = None,
        snapshot: str = None,
        reconstruct: bool = False,
        branch: str = "",
        node_config: dict = None,
        mode: str = None,
        client_factory: Callable = Client,
    ) -> None:
        assert not branch
        branch = self._branch_map[node_id]
        super().add_node(
            node_id,
            node_dir,
            peers,
            params,
            log_levels,
            private,
            config_client,
            use_tls,
            snapshot,
            reconstruct,
            branch,
            node_config,
            mode,
            client_factory,
        )
