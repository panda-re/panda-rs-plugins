use std::sync::Mutex;
use std::ffi::CStr;
use std::fmt;

#[macro_use]
extern crate lazy_static;

use panda::prelude::*;
use panda::plugins::osi::OSI;

lazy_static! {
    static ref ARGS: Mutex<Args> = Mutex::new(Args::default());
}

lazy_static! {
    static ref BBQ: Mutex<Vec<BasicBlock>> = Mutex::new(vec![]);
}

// Structs -------------------------------------------------------------------------------------------------------------

#[derive(PandaArgs)]
#[name = "il_trace"]
struct Args {
    #[arg(required, about = "Process to trace")]
    proc_name: String,

    #[arg(false, about = "Verbose print")]
    debug: bool,
}

// TODO: can this be macro-ed if every arg has a default?
impl Default for Args {
    fn default() -> Self {
        Self {
            proc_name: "init".to_string(),
            debug: false,
        }
    }
}

struct BasicBlock {
    pc: target_ulong,
    bytes: Vec<u8>,
}

impl BasicBlock {
    fn new(pc: target_ulong, bytes: Vec<u8>) -> Self {
        BasicBlock { pc, bytes }
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PC: {:08x}, BB_BYTES: {:x?})", self.pc, self.bytes)
    }
}

// Init ----------------------------------------------------------------------------------------------------------------

#[panda::init]
fn init(_: &mut PluginHandle) {
    match ARGS.lock() {
        Ok(mut args) => {
            *args = Args::from_panda_args();
            println!("il_trace plugin init, target process: {}", args.proc_name);
        },
        _ => panic!("il_trace plugin init failed!")
    }
}

// Callbacks -----------------------------------------------------------------------------------------------------------

#[panda::before_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock) {

    if panda::in_kernel(cpu) {
        return;
    }

    let curr_proc = OSI.get_current_process(cpu);
    let curr_proc_name_c_str = unsafe { CStr::from_ptr((*curr_proc).name) };

    if let Ok(curr_proc_name) = curr_proc_name_c_str.to_str() {
        if let Ok(args) = ARGS.lock() {

            // BB belongs to target process
            if args.proc_name == curr_proc_name {

                let bb_bytes = panda::virtual_memory_read(cpu, tb.pc, tb.size.into());

                if let Ok(bytes) = bb_bytes {
                    if let Ok(mut bbq) = BBQ.lock() {
                        let bb = BasicBlock::new(tb.pc, bytes);

                        // TODO: remove this print
                        println!("proc: {}, queue push: {}", curr_proc_name, bb);

                        bbq.push(bb);
                    }
                }
            }
        }
    }
}