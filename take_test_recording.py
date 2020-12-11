#!/usr/bin/env python3

from sys import argv
from pandare import blocking, Panda

# No arguments, i386. Otherwise argument should be guest arch
generic_type = argv[1] if len(argv) > 1 else "x86_64"
command = argv[2] if len(argv) > 2 else "sudo dhclient -v -4"
panda = Panda(generic=generic_type)

@blocking
def run_cmd():
    # First revert to root snapshot, then type a command via serial
    panda.revert_sync("root")
    print(f"Running \'{command}\' in guest")
    panda.record_cmd(command, recording_name="test")
    panda.end_analysis()

panda.queue_async(run_cmd)

panda.run()
