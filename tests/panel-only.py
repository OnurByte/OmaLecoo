import json,os,subprocess,tempfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix='lecoo-transport-test-') as tmp:
    lib=Path(tmp)/'capture.so'
    subprocess.run(['cc','-shared','-fPIC',str(root/'tests/ioctl_capture.c'),'-o',str(lib)],check=True)
    trace=Path(tmp)/'trace'
    env=dict(os.environ,LD_PRELOAD=str(lib),LECOO_TEST_TRACE=str(trace),LECOO_DEVICE='/dev/null',XDG_STATE_HOME=tmp)
    def run(*args):
        trace.write_text('')
        subprocess.run([str(root/'target/release/lecoo-gk301'),*args],env=env,check=True,capture_output=True)
        return trace.read_text().splitlines()
    run('profile-save','test')
    for args in [('profile-load','test'),('preset','night'),('restore',)]:
        for flags in [(), ('--panel-only',)]:
            packets=run(*args,*flags)
            assert len(packets)==1 and packets[0].startswith('07 FF FF ') and packets[0].split()[3] != '20',(args,packets)
    result=subprocess.run([str(root/'target/release/lecoo-gk301'),'response','--json'],env=env,capture_output=True,text=True)
    assert result.returncode == 0, result.stderr
    response=json.loads(result.stdout)
    assert response['firmware']=='1.16' and response['device']=='/dev/null',response
    assert response['kind']=='firmware' and response['lighting_readback'] is False,response
    assert response['request']=='07 F1 00 00 00 00 00 00',response
    assert response['raw']=='07 01 16 00 00 00 00 00' and response['roundtrip_ms'] >= 0,response
    for fixture in ['short','bad-id','empty','bad-version']:
        result=subprocess.run([str(root/'target/release/lecoo-gk301'),'response','--json'],env=dict(env,LECOO_TEST_RESPONSE=fixture),capture_output=True,text=True)
        assert result.returncode != 0,(fixture,result.stdout)
    result=subprocess.run([str(root/'target/release/lecoo-gk301'),'response','--json'],env=dict(env,LECOO_TEST_SHORT_SET='1'),capture_output=True,text=True)
    assert result.returncode != 0,result.stdout
print('Main-only operations and validated firmware responses passed; no USB hardware used.')
