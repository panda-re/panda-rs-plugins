use std::sync::Mutex;
use std::ffi::CStr;

#[macro_use]
extern crate lazy_static;

use panda::prelude::*;
use panda::plugins::osi::OSI;

// Args ----------------------------------------------------------------------------------------------------------------

#[derive(PandaArgs)]
#[name = "ir_call"]
struct Args {
    #[arg(required, about = "Process to trace")]
    proc_name: String,
}

// TODO: can this be macro-ed if every arg has a default?
impl Default for Args {
    fn default() -> Self {
        Self {
            proc_name: "init".to_string()
        }
    }
}

lazy_static! {
    static ref ARGS: Mutex<Args> = Mutex::new(Args::default());
}

// Init ----------------------------------------------------------------------------------------------------------------

#[panda::init]
fn init(_: &mut PluginHandle) {
    match ARGS.lock() {
        Ok(mut args) => {
            *args = Args::from_panda_args();
            println!("ir_call plugin init, target process: {}", args.proc_name);
        },
        _ => panic!("ir_call plugin init failed!")
    }
}

// Callbacks -----------------------------------------------------------------------------------------------------------

#[panda::before_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock) {
    let curr_proc = OSI.get_current_process(cpu);
    let curr_proc_name_c_str = unsafe { CStr::from_ptr((*curr_proc).name) };

    if let Ok(curr_proc_name) = curr_proc_name_c_str.to_str() {
        if let Ok(args) = ARGS.lock() {
            if args.proc_name == curr_proc_name {
                println!("bb hit, proc: {}", curr_proc_name);
            }
        }
    }
}