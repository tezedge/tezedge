import os
import psutil
import signal
import time
import subprocess
from datetime import datetime

while True:
    for proc in psutil.process_iter():
        if 'light-node' == proc.name():
            date = datetime.now().strftime('%Y-%m-%d %H:%M:%S')
            print(f'[{date}] Sending signal to light-node ({proc.pid})...')
            os.kill(proc.pid, signal.SIGUSR2)
            time.sleep(1)
            commands = """rm -f *.gcov
            lcov --directory ../target/fuzz/deps/ --capture --output-file app.info --gcov-tool $(pwd)/gcov.sh
            genhtml app.info
            python3 coverage_history.py
            python3 generate_charts.py
            """.replace('\n', ';')
            subprocess.run(commands, shell=True)
            break

    time.sleep(60 * 5)
