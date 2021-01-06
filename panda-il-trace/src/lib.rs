use std::ffi::CStr;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

#[macro_use]
extern crate lazy_static;

use crossbeam::queue::SegQueue;

use panda::plugins::osi::OSI;
use panda::prelude::*;

// Globals -------------------------------------------------------------------------------------------------------------

static BB_NUM: AtomicU64 = AtomicU64::new(0);

lazy_static! {
    static ref ARGS: Args = Args::from_panda_args();
}

lazy_static! {
    static ref BBQ: SegQueue<BasicBlock> = SegQueue::new();
}

lazy_static! {
    static ref WORKER_POOL: rayon::ThreadPool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_cpus::get())
        .build()
        .unwrap();
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
    seq_num: u64,
    pc: target_ulong,
    bytes: Vec<u8>,
}

impl BasicBlock {
    fn new(seq_num: u64, pc: target_ulong, bytes: Vec<u8>) -> Self {
        BasicBlock { seq_num, pc, bytes }
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SEQ: {}, PC: {:08x}, BB_BYTES: {:x?})",
            self.seq_num, self.pc, self.bytes
        )
    }
}

// Init ----------------------------------------------------------------------------------------------------------------

#[panda::init]
fn init(_: &mut PluginHandle) -> bool {
    lazy_static::initialize(&ARGS);

    let cpu_cnt = num_cpus::get();
    for i in 0..cpu_cnt {
        WORKER_POOL.spawn(|| {
            loop {
                if let Some(bb) = BBQ.pop() {
                    // TODO: remove this print
                    println!("queue pop: {}", bb);
                }
            }
        });
    }

    println!(
        "il_trace plugin init, target process: {}, CPU count: {}",
        ARGS.proc_name, cpu_cnt
    );

    true
}

// PANDA Callbacks -----------------------------------------------------------------------------------------------------

#[panda::before_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock) {
    if panda::in_kernel(cpu) {
        return;
    }

    let curr_proc = OSI.get_current_process(cpu);
    let curr_proc_name_c_str = unsafe { CStr::from_ptr((*curr_proc).name) };

    if let Ok(curr_proc_name) = curr_proc_name_c_str.to_str() {
        // BB belongs to target process
        if ARGS.proc_name == curr_proc_name {
            if let Ok(bytes) = panda::virtual_memory_read(cpu, tb.pc, tb.size.into()) {
                let bb = BasicBlock::new(BB_NUM.fetch_add(1, Ordering::SeqCst), tb.pc, bytes);

                // TODO: remove this print
                println!("proc: {}, queue push: {}", curr_proc_name, bb);

                BBQ.push(bb);
            }
        }
    }
}