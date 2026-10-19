#!/usr/bin/env python3
"""Reproduce the recovered Windows startup sequence; --send changes the main effect."""
import argparse,fcntl,os,subprocess,time
from pathlib import Path
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--send',action='store_true',help='Send the exact recovered startup record to GK301')
args=parser.parse_args()
packet=bytearray.fromhex('07 ff ff 00 0d 00 00 00')
print('Recovered startup packet:',packet.hex(' '))
if not args.send:
    print('No device write. Use --send to reproduce the physically observed startup experiment.')
    raise SystemExit(0)
root=Path(__file__).resolve().parents[1]
node=subprocess.check_output([str(root/'target/release/lecoo-gk301'),'dev'],text=True).splitlines()[0]
assert node.startswith('/dev/hidraw'), 'GK301 HID node was not found'
state=Path(os.environ.get('XDG_STATE_HOME',str(Path.home()/'.local/state')))/'lecoo-gk301'
state.mkdir(parents=True,exist_ok=True)
with (state/'command.lock').open('a') as lock:
    fcntl.flock(lock,fcntl.LOCK_EX)
    fd=os.open(node,os.O_RDWR)
    try:
        fcntl.ioctl(fd,0xc0084806,bytearray.fromhex('07 f1 00 00 00 00 00 00'))
        time.sleep(.15)
        response=bytearray([7]+[0]*7)
        fcntl.ioctl(fd,0xc0084807,response)
        print('Firmware report:',response.hex(' '))
        time.sleep(2)
        print('Transfer result:',fcntl.ioctl(fd,0xc0084806,packet))
        time.sleep(2)
    finally:os.close(fd)
print('Experiment sent; no state readback is claimed and cached lighting state is unchanged.')
