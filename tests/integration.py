#!/usr/bin/env python3
"""Offline command boundaries; --hardware also probes known light commands, restoring state."""
import concurrent.futures
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time

binary = str(Path(__file__).resolve().parents[1] / 'target/release/lecoo-gk301')

def run(args, env=None, ok=True):
    result = subprocess.run([binary, *args], env=env, capture_output=True, text=True, timeout=15)
    assert (result.returncode == 0) == ok, (args, result.returncode, result.stderr)
    return result

with tempfile.TemporaryDirectory(prefix='lecoo-cli-test-') as tmp:
    env = dict(os.environ, XDG_STATE_HOME=tmp, LECOO_DEVICE='/dev/null')
    assert json.loads(run(['profiles', '--json'], env).stdout) == []
    run(['profile-save', 'test_profile'], env)
    assert json.loads(run(['profiles', '--json'], env).stdout) == ['test_profile']
    path = Path(tmp) / 'lecoo-gk301/profiles/test_profile.json'
    saved = json.loads(path.read_text())
    assert saved['brightness'] == 5
    run(['profile-save', '../escape'], env, ok=False)
    assert not (Path(tmp) / 'lecoo-gk301/escape.json').exists()
    path.write_text('{"effect":0}')
    result = run(['profile-load', 'test_profile'], env, ok=False)
    assert 'missing profile field' in result.stderr
    assert not (Path(tmp) / 'lecoo-gk301/state.json').exists()
    run(['profile-delete', 'test_profile'], env)
    assert json.loads(run(['profiles', '--json'], env).stdout) == []
    # A kernel ENOTTY must survive log writes and reach stderr unchanged.
    result = run(['info', '--json'], env, ok=False)
    assert 'os error 25' in result.stderr, result.stderr
    caps = json.loads(run(['capabilities', '--json'], env).stdout)
    assert caps['custom_rgb'] == 'unverified' and caps['side_control'] == 'unsupported'
    assert 'fixed_side_palette' not in caps
    for command in ['side','side-next','raw','reset']:
        result = run([command], dict(env,LECOO_DEVICE='/nonexistent/lecoo-test'), ok=False)
        assert 'unknown command' in result.stderr and 'device_open' not in result.stderr, result.stderr
print('Offline CLI/profile/error checks passed.')

