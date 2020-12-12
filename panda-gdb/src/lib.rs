use panda::prelude::*;
use gdbstub::GdbStub;

use std::os::raw::{c_char, c_int};
use std::ffi::CStr;

mod panda_target;
use panda_target::PandaTarget;

mod target_state;
use target_state::{BreakStatus, STATE};

mod connection;

#[derive(PandaArgs)]
#[name = "gdb"]
struct Args {
    #[cfg(not(any(feature = "ppc", feature = "mips", feature = "mipsel", feature = "i386")))]
    #[arg(default = "invalid")]
    file: String,
    on_start: bool,
}

lazy_static::lazy_static!{
    static ref ARGS: Args = Args::from_panda_args();
}

#[panda::init]
fn init(_: &mut PluginHandle) -> bool {
    lazy_static::initialize(&ARGS);
    lazy_static::initialize(&STATE);
    if ARGS.on_start {
        let connection = connection::wait_for_gdb();
        STATE.start_single_stepping();

        // Debugger runs in a seperate thread
        std::thread::spawn(||{
            let mut debugger = GdbStub::new(connection);
            debugger.run(&mut PandaTarget)
        });    
    }

    true
}

#[panda::pre_shutdown]
fn on_shutdown() {
    STATE.brk.signal(BreakStatus::Exit);
}

#[panda::on_process_end]
#[cfg(not(any(feature = "ppc", feature = "mips", feature = "mipsel", feature = "i386")))]
fn on_process_end(_cpu: &mut CPUState, _name: *const c_char, _asid: target_ulong, pid: c_int) {
    if STATE.get_pid() == Some(pid as _) {
        STATE.unset_pid();
        STATE.brk.signal(BreakStatus::Exit);
    }
}

#[panda::on_process_start]
#[cfg(not(any(feature = "ppc", feature = "mips", feature = "mipsel", feature = "i386")))]
fn on_process_start(_cpu: &mut CPUState, name: *const c_char, _asid: target_ulong, pid: c_int) {
    // If a process is already being debugged, don't attach another debugger
    if !STATE.is_pid_set() {
        let name = unsafe { CStr::from_ptr(name) }.to_string_lossy();
        if name == ARGS.file {
            println!("*****************************");
            println!("{} started, pid: {}", name, pid);
            println!("*****************************");
            STATE.set_pid(pid as _);
            STATE.start_single_stepping();
            let connection = connection::wait_for_gdb();
            // Debugger runs in a seperate thread
            std::thread::spawn(||{
                let mut debugger = GdbStub::new(connection);
                debugger.run(&mut PandaTarget)
            });
        }
    }
 }

#[panda::insn_exec]
fn every_instruction(cpu: &mut CPUState, pc: target_ptr_t) {
    // Break if single stepping or if we hit a breakpoint
    if STATE.single_stepping() || STATE.breakpoints_contain(pc) {
        // Mark single step as completed
        STATE.stop_single_stepping();
        // Pass the CPU to the debugging thread
        STATE.set_cpu(cpu);
        STATE.set_pc(pc);
        // Signal the process has breaked
        STATE.brk.signal(BreakStatus::Break);
        // Wait for the signal to begin running again
        STATE.cont.wait_for();
        // Revoke the CPU from the debugging thread
        STATE.unset_cpu();
    }
}

#[panda::insn_translate]
fn translate_instr(_: &mut CPUState, pc: target_ptr_t) -> bool {
    // Only instrument the instruction if we might break on it
    STATE.single_stepping() || STATE.breakpoints_contain(pc)
}
