"""Execute panel setup functions with real Quickshell Process spawn failures.
Run: python3 tests/setup_contract.py (requires qs).
"""
from pathlib import Path
import os, subprocess, tempfile
source = Path(__file__).resolve().parents[1].joinpath('Panel.qml').read_text()
def function(name):
    start = source.index('  function ' + name + '(')
    return source[start:source.index('\n  }', start) + 4]
probe_start = source.index('  Process {\n    id: autoProbe')
probe = source[probe_start:source.index('\n  Process {\n    id: autoStepProc', probe_start)]
common = '''import QtQuick
import Quickshell
import Quickshell.Io
Scope {
 id: root
 property bool autoSetupAttempted: false
 property string binary: "/nonexistent/omalecoo-setup-test"
 property string localBinary: binary
 property string pluginRootPath: "/nonexistent/source"
 property string homePath: "/nonexistent/home"
 property string cachePath: "/nonexistent/cache"
 property string autoSetupStage: ""
 property var autoSetupQueue: []
 property string autoSetupLastStep: ""
 property string setupMessage: ""
 function fail(message) { console.error(message); Qt.exit(1) }
 Timer { interval: 2000; running: true; onTriggered: root.fail("setup never advanced after spawn failure") }
'''
# Only the process boundary is replaced for the second phase; no installer runs.
checks = '''
 function autoSetupNext() {
   var labels = autoSetupQueue.map(function(s) { return s.label })
   if (labels.indexOf("Installing controller from source") < 0) { fail("build missing"); return }
   if (labels.indexOf("Checking device access") < 0) { fail("post-build device probe missing"); return }
   console.log("SETUP_OK"); Qt.quit()
 }
 Component.onCompleted: scheduleAutoSetup()
}'''
with tempfile.TemporaryDirectory() as d:
    p = Path(d)/'shell.qml'
    p.write_text(common + probe + function('scheduleAutoSetup') + function('autoSetupProbeFinished') + checks)
    result = subprocess.run(['qs','-p',str(p),'--no-color'], env=dict(os.environ, QT_QPA_PLATFORM='offscreen'), capture_output=True, text=True, timeout=8)
    log = result.stdout + result.stderr
    assert result.returncode == 0 and 'SETUP_OK' in log, log
print('Missing-controller spawn failure advances to build and device recheck.')

# Feed the real completion handler with device probe results; intercept only
# the next external process launch so tests never install packages or udev rules.
for presence, needs_auth in [
    ('{"connected":true,"writable":false}', True),
    ('{"connected":true,"writable":true}', False),
    ('{"connected":false,"writable":false}', False),
]:
    import json
    completion = common + function('autoSetupStepFinished') + """
 property var autoStepProc: ({output: PRESENCE, error: ""})
 function autoSetupNext() {
   var auth = autoSetupQueue.filter(function(s) { return s.argv[0] === "pkexec" })
   if ((auth.length === 1) !== NEEDS_AUTH) { fail("wrong authorization decision"); return }
   if (NEEDS_AUTH && (auth[0].argv.join(" ") !== "pkexec " + binary + " udev install" ||
       autoSetupQueue[1].argv.join(" ") !== binary + " dev --json")) { fail("wrong authorization or verification command"); return }
   console.log("SETUP_OK"); Qt.quit()
 }
 Component.onCompleted: {
   autoSetupLastStep = "Checking device access"
   Qt.callLater(function() { autoSetupStepFinished(0) })
 }
} """.replace('PRESENCE', json.dumps(presence)).replace('NEEDS_AUTH', str(needs_auth).lower())
    with tempfile.TemporaryDirectory() as d:
        p = Path(d)/'shell.qml'
        p.write_text(completion)
        result = subprocess.run(['qs','-p',str(p),'--no-color'], env=dict(os.environ, QT_QPA_PLATFORM='offscreen'), capture_output=True, text=True, timeout=8)
        assert result.returncode == 0 and 'SETUP_OK' in result.stdout + result.stderr, result.stdout + result.stderr
print('Post-build authorization is requested only for connected, unwritable devices.')
