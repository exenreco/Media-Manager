"""Check an installed GUI stays alive through startup on a disposable CI host."""
import subprocess
import sys
import time

process = subprocess.Popen([sys.argv[1]])
try:
    time.sleep(12)
    if process.poll() is not None:
        raise SystemExit(f'Installed app exited during startup: {process.returncode}')
    print('Installed app remained running through startup.')
finally:
    if process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
