from typing import List
import os
import subprocess
from process import process_utils


# Timeout before killing an accuser which doesn't react to SIGTERM
TERM_TIMEOUT = 10


class Accuser(subprocess.Popen):
    """Fork an accuser linked to a client"""

    def __init__(
        self,
        accuser: str,
        rpc_port: int,
        base_dir: str,
        params: List[str] = None,
        log_file: str = None,
    ):
        """Create a new Popen instance for the accuser process.

        Args:
            accuser (str): path to the accuser executable file
            rpc_port (int): rpc port of the node
            base_dir (str): client directory
            params (list): additional parameters to be added to the command
            log_file (str): log file name (optional)

        Returns:
            A Popen instance
        """
        assert os.path.isfile(accuser), f'{accuser} not a file'
        assert os.path.isdir(base_dir), f'{base_dir} not a dir'

        if params is None:
            params = []
        endpoint = f'http://127.0.0.1:{rpc_port}'
        cmd = [accuser, '-base-dir', base_dir, '-endpoint', endpoint, '-m', 'json', '-l']
        cmd.extend(params)
        cmd_string = process_utils.format_command(cmd)
        print(cmd_string)
        stdout, stderr = process_utils.prepare_log(cmd, log_file)
        subprocess.Popen.__init__(
            self, cmd, stdout=stdout, stderr=stderr
        )  # type: ignore

    def terminate_or_kill(self):
        self.terminate()
        try:
            return self.wait(timeout=TERM_TIMEOUT)
        except subprocess.TimeoutExpired:
            return self.kill()
