use std::ffi::CStr;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate lazy_static;

use crossbeam::queue::SegQueue;
use panda::plugins::osi::OSI;
use panda::prelude::*;

// Exports -------------------------------------------------------------------------------------------------------------

pub mod il;
pub use crate::il::*;

// Globals -------------------------------------------------------------------------------------------------------------

static BB_NUM: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    static ref ARGS: Args = Args::from_panda_args();
}

lazy_static! {
    static ref BBQ_IN: SegQueue<il::BasicBlock> = SegQueue::new();
}

lazy_static! {
    static ref BBQ_OUT: SegQueue<il::BasicBlock> = SegQueue::new();
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

// Helpers -------------------------------------------------------------------------------------------------------------

fn finalize_bbs() -> Vec<il::BasicBlock> {
    // No iter on the lock-free queue
    let mut bbs_final = Vec::with_capacity(BBQ_OUT.len());
    while let Some(bb) = BBQ_OUT.pop() {
        bbs_final.push(bb);
    }

    // Guest execution order sort
    bbs_final.sort_unstable_by_key(|bb| bb.seq_num());

    // Determine indirect call/jump destinations
    for bb_chunk in bbs_final.chunks_exact_mut(2) {
        let dst = bb_chunk[1].pc();
        let next_seq = bb_chunk[1].seq_num();
        if let Some(branch) = bb_chunk[0].branch_mut() {
            match branch {
                Branch::CallSentinel(site, seq_num, reg) => {
                    assert!(next_seq == (*seq_num + 1));
                    *branch = Branch::IndirectCall(*site, dst, reg.to_string());
                },
                Branch::JumpSentinel(site, seq_num, reg) => {
                    assert!(next_seq == (*seq_num + 1));
                    *branch = Branch::IndirectJump(*site, dst, reg.to_string());
                },
                _ => continue,
            }
        }

    }

    bbs_final
}

// PANDA Callbacks -----------------------------------------------------------------------------------------------------

#[panda::init]
fn init(_: &mut PluginHandle) -> bool {
    lazy_static::initialize(&ARGS);

    let mut cpu_cnt = num_cpus::get();
    let cpu_total = cpu_cnt;

    // Leave producer thread a CPU, it'll be active most of runtime
    if cpu_cnt > 1 {
        cpu_cnt -= 1;
    }

    for _ in 0..cpu_cnt {
        WORKER_POOL.spawn(|| {
            loop {
                if let Some(mut bb) = BBQ_IN.pop() {
                    bb.process();
                    BBQ_OUT.push(bb);
                }
            }
        });
    }

    println!(
        "il_trace plugin init, target process: {}, CPU count: {}",
        ARGS.proc_name, cpu_total
    );

    true
}

#[panda::uninit]
fn uninit(_: &mut PluginHandle) {
    // Finish any in-process lifts
    while BBQ_OUT.len() != BB_NUM.load(Ordering::SeqCst) {
        println!("Finishing lifting/analysis...");
        thread::sleep(Duration::from_secs(5));
    }

    let bbs_final = finalize_bbs();
    let err_cnt = bbs_final.iter().filter(|bb| !bb.is_lifted()).count();

    println!(
        "il_trace plugin uninit, lifted {} BBs, {} errors.",
        bbs_final.len(),
        err_cnt
    );

    process::exit(0);
}

#[panda::before_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock) {
    if panda::in_kernel(cpu) {
        return;
    }

    let curr_proc = OSI.get_current_process(cpu);
    let curr_proc_name_c_str = unsafe { CStr::from_ptr((*curr_proc).name) };

    if let Ok(curr_proc_name) = curr_proc_name_c_str.to_str() {
        if ARGS.proc_name == curr_proc_name {
            if let Ok(bytes) = panda::virtual_memory_read(cpu, tb.pc, tb.size.into()) {
                let bb = il::BasicBlock::new_zero_copy(
                    BB_NUM.fetch_add(1, Ordering::SeqCst),
                    tb.pc as u64,
                    bytes,
                );
                BBQ_IN.push(bb);
            }
        }
    }
}
