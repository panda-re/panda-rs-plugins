use std::ffi::CStr;
use std::fs;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_with;

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

    #[arg(default = "il_trace.json", about = "File to log trace results")]
    out_trace_file: String,

    #[arg(about = "Verbose print")]
    debug: bool,

    #[arg(about = "If set, trace inside of DLL functions (faster!)")]
    trace_lib: bool,
}

// TODO: can this be macro-ed if every arg has a default?
impl Default for Args {
    fn default() -> Self {
        Self {
            proc_name: "init".to_string(),
            out_trace_file: "il_trace.json".to_string(),
            debug: false,
            trace_lib: true,
        }
    }
}

// Helpers -------------------------------------------------------------------------------------------------------------

fn finalize_bbs() -> BasicBlockList {
    // No iter on the lock-free queue
    let mut bbs_final = Vec::with_capacity(BBQ_OUT.len());
    while let Some(bb) = BBQ_OUT.pop() {
        bbs_final.push(bb);
    }

    // Guest execution order sort
    bbs_final.sort_unstable_by_key(|bb| bb.seq_num());

    // Determine indirect call/jump destinations
    for bb_chunk in bbs_final.chunks_exact_mut(2) {
        let dst_pc = bb_chunk[1].pc();
        let next_seq = bb_chunk[1].seq_num();
        let curr_seq = bb_chunk[0].seq_num();
        if let Some(branch) = bb_chunk[0].branch_mut() {
            match branch {
                Branch::CallSentinel(site_pc, seq_num, reg) => {
                    assert!(next_seq == (*seq_num + 1));

                    if reg == RET_MARKER {
                        *branch = Branch::Return(*site_pc, dst_pc);
                    } else {
                        *branch = Branch::IndirectCall(*site_pc, dst_pc, reg.to_string());
                    }
                }
                Branch::JumpSentinel(site_pc, seq_num, reg) => {
                    assert!(next_seq == (*seq_num + 1));
                    *branch = Branch::IndirectJump(*site_pc, dst_pc, reg.to_string());
                }
                _ => continue,
            };

            if ARGS.debug {
                println!("{}: {}", curr_seq, branch);
            }
        }
    }

    BasicBlockList::from(bbs_final)
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
        WORKER_POOL.spawn(|| loop {
            if let Some(mut bb) = BBQ_IN.pop() {
                bb.process();
                BBQ_OUT.push(bb);
            }
        });
    }

    println!(
        "il_trace plugin init, target process: {}, CPU count: {}, trace_lib: {}, debug: {}",
        ARGS.proc_name, cpu_total, ARGS.trace_lib, ARGS.debug
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

    let bbl = finalize_bbs();
    let err_cnt = bbl.trans_err_cnt();

    println!(
        "il_trace plugin uninit, lifted {} BBs, {} errors.",
        bbl.len(),
        err_cnt
    );

    println!("Writing trace to \'{}\'...", ARGS.out_trace_file,);
    fs::write(
        &ARGS.out_trace_file,
        serde_json::to_string(&bbl).expect("serialization of BB list failed!"),
    )
    .expect("trace file write failed!");

    process::exit(0);
}

#[panda::after_block_exec]
fn every_basic_block(cpu: &mut CPUState, tb: &mut TranslationBlock, exit_code: u8) {
    // Inside kernel code
    if panda::in_kernel(cpu) {
        return;
    }

    // This block will be re-translated/re-executed due to interrupts, etc
    if u32::from(exit_code) > panda_sys::TB_EXIT_IDX1 {
        return;
    }

    // Inside DLL and user didn't request
    let curr_proc = OSI.get_current_process(cpu);
    if (!ARGS.trace_lib) && OSI.in_shared_object(cpu, curr_proc.as_ptr()) {
        return;
    }

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
