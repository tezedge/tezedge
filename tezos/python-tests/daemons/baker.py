from typing import List, Dict
import os
import subprocess
from process import process_utils


# Timeout before killing a baker which doesn't react to SIGTERM
TERM_TIMEOUT = 10


class Baker(subprocess.Popen):
    """Fork a baker process linked to a node and a client"""

    def __init__(
        self,
        baker: str,
        rpc_port: int,
        base_dir: str,
        node_dir: str,
        accounts: List[str],
        params: List[str] = None,
        log_levels: Dict[str, str] = None,
        log_file: str = None,
        run_params: List[str] = None,
    ):
        """Create a new Popen instance for the baker process.

        Args:
            baker (str): path to the baker executable file
            rpc_port (int): rpc port of the node
            base_dir (str): client directory
            node_dir (str): node directory
            accounts (List[str]): delegates accounts
            params (list): additional parameters to be added to the command
            log_levels (dict): log levels. e.g. {"p2p.connection-pool":"debug"}
            log_file (str): log file name (optional)
        Returns:
            A Popen instance
        """
        self.log_file = log_file
        assert os.path.isfile(baker), f'{baker} not a file'
        assert os.path.isdir(node_dir), f'{node_dir} not a dir'
        assert os.path.isdir(base_dir), f'{base_dir} not a dir'
        if params is None:
            params = []
        if run_params is None:
            run_params = []
        endpoint = f'http://127.0.0.1:{rpc_port}'
        cmd = [baker, '-m', 'json']
        cmd.extend(params)
        if os.environ.get("RUN_TEZEDGE_BAKER") == "external":
            cmd.extend(['--base-dir', base_dir, '--endpoint', endpoint, '--archive'])
            cmd.extend(['--baker'] + accounts)
        else:
            cmd.extend(['-base-dir', base_dir, '-endpoint', endpoint])
            cmd.extend(['run', 'with', 'local', 'node', node_dir] + accounts)
        cmd.extend(run_params)
        env = os.environ.copy()
        if log_levels is not None:
            lwt_log = ";".join(
                f'{key} -> {values}' for key, values in log_levels.items()
            )
            env['TEZOS_LOG'] = lwt_log
        cmd_string = process_utils.format_command(cmd)
        print(cmd_string)
        stdout, stderr = process_utils.prepare_log(cmd, log_file)
        subprocess.Popen.__init__(
            self, cmd, stdout=stdout, env=env, stderr=stderr
        )  # type: ignore

    def terminate_or_kill(self):
        self.terminate()
        try:
            return self.wait(timeout=TERM_TIMEOUT)
        except subprocess.TimeoutExpired:
            return self.kill()
