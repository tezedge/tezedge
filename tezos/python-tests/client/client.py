import datetime
import json
import os
import shutil
import subprocess
import tempfile
import time
import sys
from typing import Any, List, Optional, Tuple

from . import client_output


def format_command(cmd: List[str]) -> str:
    # TODO the displayed command may not be 'shell' ready, for instance
    # Michelson string parameters may requires additional quotes
    color_code = '\033[34m'
    endc = '\033[0m'
    cmd_str = " ".join(cmd)
    return f'{color_code}# {cmd_str}{endc}'


class Client:
    """Client to a Tezos node.

    Manage the persistent client state and provides methods to call
    tezos-client/tezos-admin-client commands, and return structured
    representation of the client output.

    The most generic method to call the client is `run`. It calls the client
    with an arbitrary sequence of parameters and returns the stdout of the
    command. `CalledProcessError` is raised if command fails.

    Other commands such as `run_script`, `transfer`... are wrapper over `run`,
    they set up the commands parameters and return a structured representation
    of the client output.

    TODO: - the set of methods isn't complete. To be added when needed... some
            methods return client stdout instead of structured representation
          - deal correctly with additional parameters in command wrappers
          - this works for the current tests but should be more generic
    """

    def __init__(self,
                 client_path: str,
                 admin_client_path: str,
                 host: Optional[str] = None,
                 base_dir: Optional[str] = None,
                 rpc_port: Optional[int] = None,
                 use_tls: Optional[bool] = None,
                 endpoint: Optional[str] = 'http://127.0.0.1:8732',
                 disable_disclaimer: bool = True,
                 mode: str = None):
        """
        Args:
            client (str): path to the client executable file
            admin_client (str): path to the admin-client executable file
            host (str): IP of the host; deprecated: use endpoint instead
            base_dir (str): path to the client dir. If None, a temp file is
                            created.
            rpc_port (int): port of the server; deprecated: use endpoint
                            instead
            use_tls (bool): use TLS
            endpoint (str): the RPC endpoint
            disable_disclaimer (bool): disable disclaimer
            mode (str): the mode to use, one of "client" or "mockup"
        Returns:
            A Client instance.
        """
        assert os.path.isfile(client_path), f"{client_path} is not a file"
        assert os.path.isfile(admin_client_path), (f"{admin_client_path} is "
                                                   f"not a file")
        assert base_dir is None or os.path.isdir(base_dir), (f'{base_dir} not '
                                                             f'a dir')

        self.host = host
        self._disable_disclaimer = disable_disclaimer
        self._is_tmp_dir = base_dir is None

        if base_dir is None:
            base_dir = tempfile.mkdtemp(prefix='tezos-client.')
            assert base_dir
        self.base_dir = base_dir

        connectivity_options = []
        if host is not None:
            connectivity_options += ['-addr', host]
        if rpc_port is not None:
            connectivity_options += ['-port', str(rpc_port)]
        if use_tls is True:
            connectivity_options += ['-S']
        if endpoint is not None:
            connectivity_options += ['-endpoint', endpoint]

        client = [client_path, '-base-dir', base_dir]
        if mode is None or mode == "client":
            client.extend(connectivity_options)
        elif mode == "mockup":
            client.extend(['-mode', mode])
        else:
            msg = f"Unexpected mode: {mode}." + \
                  "Expected one of 'client' or 'mockup'."
            assert False, msg
        admin_client = [admin_client_path, '-base-dir', base_dir]

        admin_client.extend(connectivity_options)

        self._client = client
        self._admin_client = admin_client
        self.rpc_port = rpc_port

    def run_generic(self,
                    params: List[str],
                    admin: bool = False,
                    check: bool = True,
                    trace: bool = False) -> Tuple[str, str, int]:
        """Run an arbitrary command

        Args:
            params (list): list of parameters given to the tezos-client,
            admin (bool): False to call tezos-client, True to call
                          tezos-admin-client
            check (bool): raises an exception if client call fails
            trace (bool): use '-l' option to trace RPCs
        Returns:
            (stdout of command, stderr of command, return code)

        The actual command will be displayed according to 'format_command'.
        Client output (stdout, stderr) will be displayed unprocessed.
        Fails with `CalledProcessError` if command fails
        """
        client = self._admin_client if admin else self._client
        trace_opt = ['-l'] if trace else []
        cmd = client + trace_opt + params

        print(format_command(cmd))

        new_env = os.environ.copy()
        if self._disable_disclaimer:
            new_env["TEZOS_CLIENT_UNSAFE_DISABLE_DISCLAIMER"] = "Y"
        completed_process = subprocess.run(cmd,
                                           capture_output=True,
                                           text=True,
                                           check=False,
                                           env=new_env)
        stdout = completed_process.stdout
        stderr = completed_process.stderr
        if stdout:
            print(stdout)
        if stderr:
            print(stderr, file=sys.stderr)
        if check:
            completed_process.check_returncode()
        # `+ ""` makes pylint happy. It can't infer stdout/stderr can't
        # be `None` thanks to the `capture_output=True` option.
        return (stdout + "", stderr + "", completed_process.returncode)

    def run(self,
            params: List[str],
            admin: bool = False,
            check: bool = True,
            trace: bool = False) -> str:
        """Like 'run_generic' but returns just stdout."""
        (stdout, _, _) = self.run_generic(params, admin, check, trace)
        return stdout

    def rpc(self,
            verb: str,
            path: str,
            data: Any = None,
            params: List[str] = None) -> Any:
        """Run an arbitrary RPC command

        Args:
            verb (str): either `get`, `post` or `put`
            path (str): rpc path
            data (dict): json data for post
            params (list): any additional parameters to pass to the client
        Returns:
            dict representing the json output, raise exception
            if output isn't json.

        See `run` for more details.
        """
        assert verb in {'put', 'get', 'post'}
        params = [] if params is None else params
        params = params + ['rpc', verb, path]
        if data is not None:
            params = params + ['with', json.dumps(data)]
        compl_pr = self.run(params)
        return client_output.extract_rpc_answer(compl_pr)

    def remember_contract(self, alias: str, contract_address: str,
                          force: bool = False):
        params = ["remember", "contract", alias, contract_address]
        if force:
            params.append("--force")
        return self.run(params)

    def remember(self, alias: str, contract: str) -> str:
        assert os.path.isfile(contract), f'{contract} is not a file'
        return self.run(['remember', 'script', alias, f'file:{contract}'])

    def typecheck(self, contract: str, file: bool = True) -> str:
        if file:
            assert os.path.isfile(contract), f'{contract} is not a file'
        return self.run(['typecheck', 'script', contract])

    def typecheck_data(self, data: str, typ: str) -> str:
        return self.run(['typecheck', 'data', data, 'against', 'type', typ])

    def run_script(self,
                   contract: str,
                   storage: str,
                   inp: str,
                   amount: float = None,
                   trace_stack: bool = False,
                   file: bool = True) -> client_output.RunScriptResult:
        if file:
            assert os.path.isfile(contract), f'{contract} is not a file'
        cmd = ['run', 'script', contract, 'on', 'storage', storage, 'and',
               'input', inp]
        if amount is not None:
            cmd += ['-z', str(amount)]
        if trace_stack:
            cmd += ['--trace-stack']
        return client_output.RunScriptResult(self.run(cmd))

    def gen_key(self, alias: str, args: List[str] = None) -> str:
        cmd = ['gen', 'keys', alias]
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)

    def import_secret_key(self, name: str, secret: str) -> str:
        return self.run(['import', 'secret', 'key', name, secret])

    def add_address(self, name: str, address: str, force: bool = False):
        cmd = ['add', 'address', name, address]
        if force:
            cmd += ['--force']
        output = self.run(cmd)
        assert not output

    def show_address(self,
                     name: str,
                     show_secret: bool = False
                     ) -> client_output.ShowAddressResult:
        cmd = ['show', 'address', name]
        if show_secret:
            cmd += ['--show-secret']
        return client_output.ShowAddressResult(self.run(cmd))

    def activate_protocol(self,
                          protocol: str,
                          parameter_file: str,
                          fitness: str = '1',
                          key: str = 'activator',
                          timestamp: str = None
                          ) -> client_output.ActivationResult:
        assert os.path.isfile(parameter_file), f'{parameter_file} not a file'
        if timestamp is None:
            utc_now = datetime.datetime.utcnow()
            timestamp = utc_now.strftime("%Y-%m-%dT%H:%M:%SZ")
        cmd = ['-block', 'genesis', 'activate', 'protocol', protocol, 'with',
               'fitness', str(fitness), 'and', 'key', key, 'and', 'parameters',
               parameter_file, '--timestamp', timestamp]
        return client_output.ActivationResult(self.run(cmd))

    def activate_protocol_json(self,
                               protocol: str,
                               parameters: dict,
                               fitness: str = '1',
                               key: str = 'activator',
                               timestamp: str = None
                               ) -> client_output.ActivationResult:
        with tempfile.NamedTemporaryFile(mode='w+', delete=False) as params:
            param_json = json.dumps(parameters)
            params.write(param_json)
            params.close()
            return self.activate_protocol(protocol, params.name, fitness,
                                          key, timestamp)

    def show_voting_period(self) -> str:
        return self.run(['show', 'voting', 'period'])

    def ban_peer(self, port: int) -> dict:
        return self.rpc('get', f'/network/points/127.0.0.1:{port}/ban')

    def unban_peer(self, port: int) -> dict:
        return self.rpc('get', f'/network/points/127.0.0.1:{port}/unban')

    def trust_peer(self, port: int) -> dict:
        return self.rpc('get', f'/network/points/127.0.0.1:{port}/trust')

    def untrust_peer(self, port: int) -> dict:
        return self.rpc('get', f'/network/points/127.0.0.1:{port}/untrust')

    def endorse(self, account: str) -> client_output.EndorseResult:
        res = self.run(['endorse', 'for', account])
        return client_output.EndorseResult(res)

    def bake(self,
             account: str,
             args: List[str] = None) -> client_output.BakeForResult:
        cmd = ['bake', 'for', account]
        if args is None:
            args = []
        cmd += args
        return client_output.BakeForResult(self.run(cmd))

    def originate(self,
                  contract_name: str,
                  amount: float,
                  sender: str,
                  contract: str,
                  args: List[str] = None) -> client_output.OriginationResult:
        cmd = ['originate', 'contract', contract_name, 'transferring',
               str(amount), 'from', sender, 'running', contract]
        if args is None:
            args = []
        cmd += args
        return client_output.OriginationResult(self.run(cmd))

    def hash(self, data: str, typ: str) -> client_output.HashResult:
        cmd = ['hash', 'data', data, 'of', 'type', typ]
        return client_output.HashResult(self.run(cmd))

    def pack(self, data: str, typ: str) -> str:
        return self.hash(data, typ).packed

    def sign(self, data: str, identity: str) -> str:
        cmd = ['sign', 'bytes', data, 'for', identity]
        return client_output.SignatureResult(self.run(cmd)).sig

    def activate_account(self,
                         manager: str,
                         contract: str):
        cmd = ['activate', 'account', manager, 'with', contract]
        return self.run(cmd)

    def cmd_batch(self,
                  source: str,
                  json_ops: str) -> List[str]:
        return ['multiple', 'transfers', 'from', source, 'using', json_ops]

    def transfer(self,
                 amount: float,
                 giver: str,
                 receiver: str,
                 args: List[str] = None,
                 chain: str = None
                 ) -> client_output.TransferResult:
        cmd = ['transfer', str(amount), 'from', giver, 'to', receiver]
        if chain is not None:
            cmd = ['--chain', chain] + cmd

        if args is None:
            args = []
        cmd += args
        res = self.run(cmd)
        return client_output.TransferResult(res)

    def transfer_json(self,
                      amount: int,
                      giver: str,
                      receiver: str,
                      args: List[str] = None) -> client_output.TransferResult:
        json_obj = [{"destination": receiver, "amount": str(amount)}]
        json_ops = json.dumps(json_obj, separators=(',', ':'))
        if args is None:
            args = []
        cmd = self.cmd_batch(giver, json_ops) + args
        res = self.run(cmd)
        return client_output.TransferResult(res)

    def call(self,
             source: str,
             destination: str,
             args: List[str] = None) -> client_output.TransferResult:
        cmd = ['call', destination, 'from', source]
        if args is None:
            args = []
        cmd += args
        res = self.run(cmd)
        return client_output.TransferResult(res)

    def set_delegate(self,
                     account1: str,
                     account2: str,
                     args: List[str] = None
                     ) -> client_output.SetDelegateResult:
        cmd = ['set', 'delegate', 'for', account1, 'to', account2]
        if args is None:
            args = []
        cmd += args
        res = self.run(cmd)
        return client_output.SetDelegateResult(res)

    def get_delegate(self,
                     account1: str,
                     args: List[str] = None
                     ) -> client_output.GetDelegateResult:
        cmd = ['get', 'delegate', 'for', account1]
        if args is None:
            args = []
        cmd += args
        res = self.run(cmd)
        return client_output.GetDelegateResult(res)

    def get_contract_entrypoint_type(
            self,
            entrypoint: str,
            contract_name: str) \
            -> client_output .GetContractEntrypointTypeResult:
        cmd = ['get', 'contract', 'entrypoint', 'type',
               'of', entrypoint,
               'for', contract_name]
        return client_output.GetContractEntrypointTypeResult(self.run(cmd))

    def withdraw_delegate(
            self,
            account1: str,
            args: List[str] = None) -> str:
        cmd = ['withdraw', 'delegate', 'from', account1]
        if args is None:
            args = []
        cmd += args
        res = self.run(cmd)
        return res

    def register_delegate(self, delegate: str) -> str:
        return self.run(['register', 'key', delegate, 'as', 'delegate'])

    def p2p_stat(self) -> client_output.P2pStatResult:
        res = self.run(['p2p', 'stat'], admin=True)
        return client_output.P2pStatResult(res)

    def get_balance(self, account) -> float:
        res = self.run(['get', 'balance', 'for', account])
        return client_output.extract_balance(res)

    def get_mutez_balance(self, account) -> float:
        res = self.run(['get', 'balance', 'for', account])
        return int(client_output.extract_balance(res)*1000000)

    def get_timestamp(self) -> str:
        res = self.run(['get', 'timestamp'])
        return res[:-1]

    def get_now(self) -> str:
        """Returns the timestamp of next-to-last block,
        offset by time_between_blocks"""
        rfc3399_format = "%Y-%m-%dT%H:%M:%SZ"
        timestamp = self.rpc(
            'get', '/chains/main/blocks/head~1/header'
            )['timestamp']
        timestamp_date = datetime.datetime.strptime(timestamp, rfc3399_format)
        timestamp_date = timestamp_date.replace(tzinfo=datetime.timezone.utc)

        constants = self.rpc(
            'get', '/chains/main/blocks/head/context/constants'
        )
        delta = datetime.timedelta(
            seconds=int(constants['time_between_blocks'][0])
        )

        now_date = timestamp_date + delta

        return now_date.strftime(rfc3399_format)

    def get_receipt(self,
                    operation: str,
                    args: List[str] = None) -> client_output.GetReceiptResult:
        cmd = ['get', 'receipt', 'for', operation]
        if args is None:
            args = []
        cmd += args
        return client_output.GetReceiptResult(self.run(cmd))

    def get_storage(self, contract: str) -> str:
        cmd = ['get', 'contract', 'storage', 'for', contract]
        res = self.run(cmd)
        return res.rstrip()

    def get_prevalidator(self) -> dict:
        return self.rpc('get', '/workers/prevalidators')

    def get_mempool(self) -> dict:
        return self.rpc('get', '/chains/main/mempool/pending_operations')

    def mempool_is_empty(self) -> bool:
        rpc_res = self.rpc('get', '/chains/main/mempool/pending_operations')
        return rpc_res['applied'] == [] and \
            rpc_res['refused'] == [] and \
            rpc_res['branch_refused'] == [] and \
            rpc_res['branch_delayed'] == [] and \
            rpc_res['unprocessed'] == []

    def get_head(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head')

    def get_block(self, block_hash) -> dict:
        return self.rpc('get', f'/chains/main/blocks/{block_hash}')

    def get_ballot_list(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/votes/ballot_list')

    def get_ballots(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/votes/ballots')

    def get_contract_address(self, contract) -> str:
        return self.run(['show', 'known', 'contract', contract]).strip()

    def get_known_addresses(self) -> client_output.GetAddressesResult:
        return client_output.GetAddressesResult(self.run(
            ['list', 'known', 'addresses']
        ))

    def get_current_period_kind(self) -> dict:
        return self.rpc('get',
                        'chains/main/blocks/head/votes/current_period_kind')

    def get_current_proposal(self) -> dict:
        return self.rpc('get',
                        '/chains/main/blocks/head/votes/current_proposal')

    def get_current_quorum(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/votes/current_quorum')

    def get_listings(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/votes/listings')

    def get_proposals(self) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/votes/proposals')

    def get_metadata(self, params: List[str] = None) -> dict:
        return self.rpc('get', '/chains/main/blocks/head/metadata',
                        params=params)

    def get_protocol(self, params: List[str] = None) -> str:
        metadata = self.get_metadata(params=params)
        return metadata['protocol']

    def get_next_protocol(self, params: List[str] = None) -> str:
        metadata = self.get_metadata(params=params)
        return metadata['next_protocol']

    def get_period_position(self) -> str:
        rpc_res = self.rpc(
            'get', '/chains/main/blocks/head/helpers/current_level?offset=1')
        return rpc_res['voting_period_position']

    def get_level(self, params: List[str] = None, chain: str = 'main') -> int:
        assert chain in {'main', 'test'}
        rpc_res = self.rpc('get', f'/chains/{chain}/blocks/head/header/shell',
                           params=params)
        return int(rpc_res['level'])

    def get_checkpoint(self) -> dict:
        rpc_res = self.rpc('get', '/chains/main/checkpoint')
        return rpc_res

    def get_savepoint(self) -> str:
        rpc_res = self.get_checkpoint()
        return rpc_res['savepoint']

    def get_caboose(self) -> str:
        rpc_res = self.get_checkpoint()
        return rpc_res['caboose']

    def wait_for_inclusion(self,
                           operation_hash: str,
                           branch: str = None,
                           check_previous: int = None,
                           args=None) -> client_output.WaitForResult:
        cmd = ['wait', 'for', operation_hash, 'to', 'be', 'included']
        if check_previous is not None:
            cmd += ['--check-previous', str(check_previous)]
        if branch is not None:
            cmd += ['--branch', branch]
        if args is None:
            args = []
        cmd += args
        return client_output.WaitForResult(self.run(cmd))

    def inject_protocol(self, proto) -> str:
        return self.run(['inject', 'protocol', proto], admin=True)

    def list_protocols(self) -> List[str]:
        cmd = ['list', 'protocols']
        return client_output.extract_protocols(self.run(cmd, admin=True))

    def list_understood_protocols(self) -> List[str]:
        cmd = ['list', 'understood', 'protocols']
        return client_output.extract_protocols(self.run(cmd, admin=True))

    def submit_proposals(self,
                         account: str,
                         protos: List[str]
                         ) -> client_output.SubmitProposalsResult:
        cmd = ['submit', 'proposals', 'for', account] + protos
        return client_output.SubmitProposalsResult(self.run(cmd))

    def submit_ballot(self,
                      account: str,
                      proto: str,
                      vote: str) -> str:
        return self.run(['submit', 'ballot', 'for', account, proto, vote])

    def bootstrapped(self) -> str:
        return self.run(['bootstrapped'])

    def sync_state(self) -> str:
        res = self.rpc('get', 'chains/main/is_bootstrapped')
        return res['sync_state']

    def is_bootstrapped(self, chain: str = 'main') -> bool:
        assert chain in {'main', 'test'}
        res = self.rpc('get', f'chains/{chain}/is_bootstrapped')
        return res['bootstrapped']

    def cleanup(self) -> None:
        """Remove base dir, only if not provided by user."""
        if self._is_tmp_dir:
            shutil.rmtree(self.base_dir)

    def deploy_msig(self, msig_name: str,
                    amount: float, src: str,
                    threshold: int, keys: List[str],
                    args: List[str] = None
                    ) -> client_output.OriginationResult:
        cmd = ['deploy', 'multisig', msig_name,
               'transferring', str(amount), 'from', src, 'with',
               'threshold', str(threshold), 'on', 'public', 'keys']
        cmd += keys
        if args is None:
            args = []
        cmd += args
        return client_output.OriginationResult(self.run(cmd))

    def msig_sign_transfer(self, msig_name: str,
                           amount: float, dest: str,
                           secret_key: str) -> str:
        cmd = ['sign', 'multisig', 'transaction', 'on', msig_name,
               'transferring', str(amount), 'to', dest,
               'using', 'secret', 'key', secret_key]
        res = self.run(cmd)
        return res[:-1]

    def msig_sign_withdraw(self, msig_name: str,
                           amount: float, dest: str,
                           secret_key: str) -> str:
        cmd = ['sign', 'multisig', 'transaction', 'on', msig_name,
               'transferring', str(amount), 'to', dest,
               'using', 'secret', 'key', secret_key]
        return self.run(cmd)

    def msig_sign_set_delegate(self, msig_name: str,
                               delegate: str,
                               secret_key: str) -> str:
        cmd = ['sign', 'multisig', 'transaction', 'on', msig_name,
               'setting', 'delegate', 'to', delegate,
               'using', 'secret', 'key', secret_key]
        res = self.run(cmd)
        return res[:-1]

    def msig_sign_withdrawing_delegate(self, msig_name: str,
                                       secret_key: str) -> str:
        cmd = ['sign', 'multisig', 'transaction', 'on', msig_name,
               'withdrawing', 'delegate',
               'using', 'secret', 'key', secret_key]
        res = self.run(cmd)
        return res[:-1]

    def msig_sign_setting_threshold(self, msig_name: str,
                                    secret_key: str, threshold: int,
                                    public_keys: List[str]) -> str:
        cmd = ['sign', 'multisig', 'transaction', 'on', msig_name,
               'using', 'secret', 'key', secret_key, 'setting', 'threshold',
               'to', str(threshold), 'and', 'public', 'keys', 'to']
        cmd += public_keys
        res = self.run(cmd)
        return res[:-1]

    def sign_bytes(self, to_sign: bytes,
                   key: str) -> client_output.SignBytesResult:
        cmd = ['sign', 'bytes', str(to_sign), 'for', key]
        return client_output.SignBytesResult(self.run(cmd))

    def msig_prepare_transfer(self, msig_name: str,
                              amount: float, dest: str,
                              args: List[str] = None):
        cmd = ['prepare', 'multisig', 'transaction', 'on',
               msig_name, 'transferring', str(amount), 'to', dest]
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)[:-1]

    def msig_prepare_set_delegate(self, msig_name: str,
                                  delegate: str,
                                  args: List[str] = None):
        cmd = ['prepare', 'multisig', 'transaction', 'on',
               msig_name, 'setting', 'delegate', 'to', delegate]
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)[:-1]

    def msig_prepare_withdrawing_delegate(self, msig_name: str,
                                          args: List[str] = None):
        cmd = ['prepare', 'multisig', 'transaction', 'on',
               msig_name, 'withdrawing', 'delegate']
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)[:-1]

    def msig_prepare_setting_threshold(self, msig_name: str,
                                       threshold: int,
                                       public_keys: List[str],
                                       args: List[str] = None):
        cmd = ['prepare', 'multisig', 'transaction', 'on',
               msig_name, 'setting', 'threshold',
               'to', str(threshold), 'and', 'public', 'keys', 'to']
        cmd += public_keys
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)[:-1]

    def msig_transfer(self, msig_name: str,
                      amount: float, dest: str,
                      src: str, signatures: List[str],
                      args: List[str] = None) -> str:
        cmd = ['from', 'multisig', 'contract', msig_name,
               'transfer', str(amount), 'to', dest,
               'on', 'behalf', 'of', src, 'with', 'signatures'] + signatures
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)

    def msig_set_delegate(self, msig_name: str,
                          delegate: str,
                          src: str, signatures: List[str],
                          args: List[str] = None) -> str:
        cmd = ['set', 'delegate', 'of', 'multisig', 'contract', msig_name,
               'to', delegate,
               'on', 'behalf', 'of', src, 'with', 'signatures'] + signatures
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)

    def msig_withdrawing_delegate(self, msig_name: str,
                                  src: str, signatures: List[str],
                                  args: List[str] = None) -> str:
        cmd = ['withdraw', 'delegate', 'of', 'multisig', 'contract', msig_name,
               'on', 'behalf', 'of', src, 'with', 'signatures'] + signatures
        if args is None:
            args = []
        cmd += args
        return self.run(cmd)

    def msig_run_transaction(self, msig_name: str,
                             transaction: bytes,
                             src: str,
                             signatures: List[str]) -> str:
        cmd = ['run', 'transaction', str(transaction), 'on',
               'multisig', 'contract', msig_name,
               'on', 'behalf', 'of', src, 'with',
               'signatures'] + signatures
        return self.run(cmd)

    def check_node_listening(self,
                             timeout: float = 1,
                             attempts: int = 20) -> bool:
        """ Checks whether the node is responsive, by polling it
        using the `version` rpc.

        Args:
            timeout (float): time (sec) to wait between retries
            attempts (int): maximal number of attempts
        Returns:
            True iff the node is running, and successfully answered the
            `version` rpc.
        """
        for _ in range(attempts):
            try:
                time.sleep(timeout)
                # any shell RPC will do, this one is light-weight
                self.rpc('get', '/network/version')
                return True
            except Exception:  # pylint: disable=broad-except
                pass
        return False

    def expand_macros(self, src: str) -> str:
        cmd = ['expand', 'macros', 'in', src]
        return self.run(cmd)

    def list_mockup_protocols(self) -> client_output.ListMockupProtocols:
        cmd = ['list', 'mockup', 'protocols']
        return client_output.ListMockupProtocols(self.run(cmd))

    def create_mockup(self,
                      protocol: str,
                      check: bool = True,
                      protocol_constants_file: str = None,
                      bootstrap_accounts_file: str = None)\
            -> client_output.CreateMockup:
        cmd = ['--protocol', protocol, 'create', 'mockup']
        if protocol_constants_file is not None:
            cmd += ["--protocol-constants", protocol_constants_file]
        if bootstrap_accounts_file is not None:
            cmd += ["--bootstrap-accounts", bootstrap_accounts_file]
        (stdout, stderr, exit_code) = self.run_generic(cmd,
                                                       check=check)
        return client_output.CreateMockup(stdout, stderr, exit_code)
