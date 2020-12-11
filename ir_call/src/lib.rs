use std::sync::Mutex;

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
    *(ARGS.lock().unwrap()) = Args::from_panda_args();
    println!("ir_call plugin init, target process: {}", ARGS.lock().unwrap().proc_name);
}

// Callbacks -----------------------------------------------------------------------------------------------------------

#[panda::before_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock) {
    // TOOD: something here
    println!("bb hit!")
}