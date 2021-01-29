# il_trace

## Summary

Multi-threaded, per-process tracing using an intermediate language (currently [Falcon IL](https://docs.rs/falcon/0.4.12/falcon/il/index.html)) for dynamic Control Flow Graph (CFG) recovery from binaries. Does not require any input information (e.g. debug symbols, locations of call/jump sites). Supports x86, x86-64, MIPS, MIPSEL, and PPC.

### How does this work?

* After each basic block executes, a callback reads the raw bytes encoding the block and pushes them to a lock-free queue, stamping them with an atomic counter value representing guest-order execution.
* While the guest continues executing (minimal blocking), a pool of worker threads (one per host core) lifts the bytes to an intermediate language, searches for jumps/calls/returns, and pushes results to another lock-free queue. In testing, this design gets near-full utilization of a 32-core machine during replay.
* Plugin un-initialization recovers execution order of all lifted blocks and resolves the targets of all direct/indirect jumps/calls and returns.

### What is the output?

A JSON of dynamic control flow for a given binary's execution - direct/indirect calls, direct/indirect jumps, and returns.
Supports indirect control flow both when register is dereferenced (e.g. `call rax`) and when a memory location is deference (e.g. `call [r12+0x60]`).

Example excerpt of call/return tracking. Notice recovery of register used (`r12` in this case) for an indirect call:

```json
{
    "seq_num": 1408509,
    "pc": 140737341446224,
    "branch": {
        "DirectCall": {
            "site_pc": 140737341446228,
            "dst_pc": 140737341462624
        }
    }
},
{
    "seq_num": 1408511,
    "pc": 140737341462682,
    "branch": {
        "IndirectCall": {
            "site_pc": 140737341462685,
            "dst_pc": 140737341445840,
            "reg_used": "r12"
        }
    }
},
{
    "seq_num": 1408514,
    "pc": 140737341445989,
    "branch": {
        "Return": {
            "site_pc": 140737341446008,
            "dst_pc": 140737341462690
        }
    }
},
```

Example excerpt of conditional jump tracking. Notice determination of whether or not jump was taken (`false` in this case):

```json
{
    "seq_num": 1309261,
    "pc": 140737347700480,
    "branch": {
        "DirectJump": {
            "site_pc": 140737347700480,
            "dst_pc": 140737345173440,
            "taken": false
        }
    }
},
```

## Arguments

* `proc_name`: String, required. Name of process to trace.
* `out_trace_file`: String, defaults to `il_trace.json`. File to output trace results to.
* `pretty_json`: boolean, defaults to false. Whether to include whitespace in the JSON that makes the file longer but more human-readable.
* `trace_lib`: boolean, defaults to true. Whether to trace inside of DLL functions for the target process. Turning this off (e.g. non-DLL functions only) slows down tracing significantly.
* `debug`: boolean, defaults to false. Whether to output trace to console at end of run.

## Dependencies

* `osi`

## APIs and Callbacks

TODO - `panda-rs` doesn't yet support PPP callbacks.

## Example

```
$PANDA_PATH/x86_64-softmmu/panda-system-x86_64 \
    -m 1G \
    -os "linux-64-ubuntu:4.15.0-72-generic-noaslr-nokaslr" \
    -replay /path/to/replay
    -panda il_trace:proc_name=some_process,debug=1,trace_lib=1,pretty_json=1
```

## Tests

```
cargo test --features bin -- --show-output
```

## TODO

* Unit tests for non-x86 arches
* Integration test for accuracy verification against DWARF symbols
* Translation caching support
* PANDALOG support
* PPP callbacks:
    * `on_ret`
    * `on_call`
    * `on_jmp_indirect`
* Ghidra importer that automates finding/labeling more code in the binary