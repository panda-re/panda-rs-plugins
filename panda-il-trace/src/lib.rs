use std::ffi::CStr;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use lazy_static::lazy_static;
use crossbeam::queue::SegQueue;

use panda::plugins::osi::OSI;
use panda::prelude::*;

// Exports -------------------------------------------------------------------------------------------------------------

pub mod fil;
pub use crate::fil::*;

// Globals -------------------------------------------------------------------------------------------------------------

static BB_NUM: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
    static ref ARGS: Args = Args::from_panda_args();
}

// Producer: after basic block callback
// Consumer: worker threads
lazy_static! {
    static ref BBQ_IN: SegQueue<fil::BasicBlock> = SegQueue::new();
}

// Producer: worker threads
// Consumer: uninit plugin
lazy_static! {
    static ref BBQ_OUT: SegQueue<fil::BasicBlock> = SegQueue::new();
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

    #[arg(default = "il_trace.json", about = "File to log trace results (JSON)")]
    out_trace_file: String,

    #[arg(about = "If set, add whitespace to JSON (larger file)")]
    pretty_json: bool,

    #[arg(about = "If set, trace inside of DLL functions (faster!)")]
    trace_lib: bool,

    #[arg(about = "Verbose print")]
    debug: bool,
}

// TODO: unused if no arg name provided on command line?
// TODO: can this be macro-ed if every arg has a default?
impl Default for Args {
    fn default() -> Self {
        Self {
            proc_name: "init".to_string(),
            out_trace_file: "il_trace.json".to_string(),
            debug: false,
            trace_lib: true,
            pretty_json: false,
        }
    }
}

// Helpers -------------------------------------------------------------------------------------------------------------

fn finalize_bbs() -> BasicBlockList {
    // No iter on the lock-free queue
    let mut bbs = Vec::with_capacity(BBQ_OUT.len());
    while let Some(bb) = BBQ_OUT.pop() {
        bbs.push(bb);
    }

    let final_trace = BasicBlockList::from(bbs);

    if ARGS.debug {
        for bb in final_trace.blocks() {
            if let Some(branch) = bb.branch() {
                println!("{}: {}", bb.seq_num(), branch);
            }
        }
    }

    final_trace
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

    let bb_list = finalize_bbs();
    let err_cnt = bb_list.trans_err_cnt();

    println!(
        "il_trace plugin uninit, lifted {} BBs, {} errors.",
        bb_list.len(),
        err_cnt
    );

    println!("Writing trace to \'{}\'...", ARGS.out_trace_file);
    bb_list
        .to_branch_json(&ARGS.out_trace_file, ARGS.pretty_json)
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
    if u32::from(exit_code) > panda::sys::TB_EXIT_IDX1 {
        return;
    }

    let curr_proc = OSI.get_current_process(cpu);
    let curr_proc_handle = OSI.get_current_process_handle(cpu);
    let curr_proc_name_c_str = unsafe { CStr::from_ptr((*curr_proc).name) };

    if let Ok(curr_proc_name) = curr_proc_name_c_str.to_str() {
        if ARGS.proc_name == curr_proc_name {
            // Inside DLL and user didn't request
            if (!ARGS.trace_lib) && OSI.in_shared_object(cpu, curr_proc.as_ptr()) {
                return;
            }

            if let Ok(bytes) = panda::virtual_memory_read(cpu, tb.pc, tb.size.into()) {
                let bb = fil::BasicBlock::new_zero_copy(
                    BB_NUM.fetch_add(1, Ordering::SeqCst),                          // seq_num
                    tb.pc as u64,                                                   // pc
                    panda::current_asid(cpu) as u64,                                // asid
                    OSI.get_process_pid(cpu, curr_proc_handle.as_ptr()) as i32,     // pid
                    OSI.get_process_ppid(cpu, curr_proc_handle.as_ptr()) as i32,    // ppid
                    unsafe { panda::sys::rr_get_guest_instr_count_external() },     // icount - TODO: handle case were running live and NOT on a replay!
                    bytes,                                                          // bytes
                );
                BBQ_IN.push(bb);
            }
        }
    }
}
