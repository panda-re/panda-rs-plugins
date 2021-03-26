use std::fmt;

use falcon::il::Expression::{Constant, Scalar as ExpScalar};
use falcon::il::{Block, ControlFlowGraph, Operation, Scalar};
use falcon::translator;
use falcon::translator::Translator;
use serde::Serialize;

#[cfg(not(any(feature = "mips", feature = "mipsel")))]
use falcon::architecture::Architecture;

#[cfg(not(any(feature = "mips", feature = "mipsel")))]
use falcon::il::Instruction;

use super::Branch;

/// Guest basic block, can map to TCG/LLVM execution delimiters (e.g. subset of static BB in ELF/PE)
#[derive(Debug, Clone, Serialize)]
pub struct BasicBlock {
    seq_num: usize,
    pc: u64,
    asid: u64,
    pid: i32,
    ppid: i32,
    branch: Option<Branch>,

    #[serde(skip)]
    bytes: Vec<u8>,

    #[serde(skip)]
    translation: Option<ControlFlowGraph>,

    #[cfg(feature = "i386")]
    #[serde(skip)]
    translator: translator::x86::X86,

    #[cfg(feature = "x86_64")]
    #[serde(skip)]
    translator: translator::x86::Amd64,

    #[cfg(feature = "mips")]
    #[serde(skip)]
    translator: translator::mips::Mips,

    #[cfg(feature = "mipsel")]
    #[serde(skip)]
    translator: translator::mips::Mipsel,

    #[cfg(feature = "ppc")]
    #[serde(skip)]
    translator: translator::ppc::Ppc,

    // This is a dummy so the plugin compiles for ARM
    // Falcon IR doesn't support ARM, will panic at runtime.
    #[cfg(feature = "arm")]
    #[serde(skip)]
    translator: translator::ppc::Ppc,
}

impl BasicBlock {
    /// Constructor, copies bytes from parameter slice
    pub fn new(seq_num: usize, pc: u64, asid: u64, pid: i32, ppid: i32, bytes: &[u8]) -> Self {
        BasicBlock::new_zero_copy(seq_num, pc, asid, pid, ppid, bytes.to_vec())
    }

    /// Constructor, takes ownership of BB bytes to avoid copy
    pub fn new_zero_copy(seq_num: usize, pc: u64, asid: u64, pid: i32, ppid: i32, bytes: Vec<u8>) -> Self {

        // TODO: CI knowledge of arch features, so this plugin compiling is not a CI failure
        #[cfg(feature = "arm")]
        compile_error!("ARM is not supported by the Falcon IR! :(");

        #[cfg(feature = "i386")]
        let translator = translator::x86::X86::new();

        #[cfg(feature = "x86_64")]
        let translator = translator::x86::Amd64::new();

        #[cfg(feature = "mips")]
        let translator = translator::mips::Mips::new();

        #[cfg(feature = "mipsel")]
        let translator = translator::mips::Mipsel::new();

        #[cfg(feature = "ppc")]
        let translator = translator::ppc::Ppc::new();

        BasicBlock {
            seq_num,
            pc,
            asid,
            pid,
            ppid,
            bytes,
            translator,
            translation: None,
            branch: None,
        }
    }

    /// Get sequence number
    pub fn seq_num(&self) -> usize {
        self.seq_num
    }

    /// Get PC
    pub fn pc(&self) -> u64 {
        self.pc
    }

    /// Get bytes
    pub fn bytes(&self) -> &[u8] {
        &self.bytes[..]
    }

    /// Get translation
    pub fn translation(&self) -> &Option<ControlFlowGraph> {
        &self.translation
    }

    /// Get lift status
    pub fn is_lifted(&self) -> bool {
        self.translation().is_some()
    }

    /// Get branch
    pub fn branch(&self) -> &Option<Branch> {
        &self.branch
    }

    /// Get branch mut
    pub fn branch_mut(&mut self) -> &mut Option<Branch> {
        &mut self.branch
    }

    /// Lift BB to IR, caches successes so multiple calls won't re-lift.
    /// This is not done by the constructor so that the work can be deferred for multi-threading.
    pub fn lift(&mut self) -> &Option<ControlFlowGraph> {
        if self.translation.is_none() {
            if let Ok(translation) = self
                .translator
                .translate_block(&self.bytes[..], self.pc as u64)
            {
                if let Ok(cfg) = translation.blockify() {
                    self.translation = Some(cfg);
                }
            }
        }

        &self.translation
    }

    /// Lift BB, find first branch (if any).
    /// Save the results in `self.translation` and `self.branch`.
    /// This is not done by the constructor so that the work can be deferred for multi-threading.
    pub fn process(&mut self) {
        self.lift();
        self.branch = self.find_branch();
    }

    /// Get first call/jump/return present.
    /// `lift()` must be called before this function.
    pub fn find_branch(&self) -> Option<Branch> {
        match &self.translation {
            Some(cfg) => {
                for block in cfg.blocks() {
                    for (idx, instr) in block.instructions().iter().enumerate() {
                        match (instr.operation(), instr.address()) {
                            (Operation::Branch { target }, Some(site_pc)) => {
                                // Falcon doesn't differentiate calls/rets from jumps at the IL level
                                // This means we have to encode architecture-specific side effects here :(

                                // False to ensure init for later assert.
                                let mut maybe_call = false;
                                let mut maybe_ret = false;

                                if idx > 0 {
                                    let prev_instr = &block.instructions()[idx - 1];

                                    // call -> SP-offset write
                                    // ret -> SP-offset read
                                    #[cfg(any(feature = "i386", feature = "x86_64"))]
                                    {
                                        #[cfg(feature = "x86_64")]
                                        let sp = falcon::architecture::Amd64::new().stack_pointer();

                                        #[cfg(feature = "i386")]
                                        let sp = falcon::architecture::X86::new().stack_pointer();

                                        maybe_call = Self::stores_reg(&prev_instr, &sp);

                                        if idx > 1 && (!maybe_call) {
                                            let prev_prev_instr = &block.instructions()[idx - 2];
                                            if Self::loads_reg(&prev_prev_instr, &sp)
                                                || Self::loads_reg(&prev_instr, &sp)
                                            {
                                                maybe_ret = true;
                                            }
                                        }
                                    }

                                    // call -> LR write
                                    // ret -> LR read
                                    #[cfg(any(
                                        feature = "arm",
                                        feature = "ppc",
                                        feature = "mips",
                                        feature = "mipsel"
                                    ))]
                                    {
                                        if let Some(link_reg) = panda::reg_ret_addr() {
                                            let link_reg_scalar = Scalar::new(
                                                link_reg.to_string().to_lowercase(),
                                                32,
                                            );

                                            if let Some(regs) = prev_instr.scalars_written() {
                                                if regs.contains(&&link_reg_scalar) {
                                                    maybe_call = true;
                                                }
                                            }

                                            if let Some(regs) = prev_instr.scalars_read() {
                                                if regs.contains(&&link_reg_scalar) {
                                                    maybe_ret = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                assert!(!(maybe_ret && maybe_call));
                                match target {
                                    // Direct call or jump
                                    Constant(addr) => {
                                        if let Some(dst_pc) = addr.value_u64() {
                                            match maybe_call {
                                                true => {
                                                    return Some(Branch::DirectCall {
                                                        site_pc,
                                                        dst_pc,
                                                    })
                                                }
                                                false => {
                                                    return Some(Branch::DirectJump {
                                                        site_pc,
                                                        dst_pc,
                                                        taken: false,
                                                    })
                                                }
                                            }
                                        }
                                    }

                                    // Indirect call, jump, or return
                                    // PC of the next BB executed is dest, fill sentinel with sequence number
                                    ExpScalar(reg) => {
                                        if maybe_ret {
                                            return Some(Branch::ReturnSentinel {
                                                site_pc,
                                                seq_num: self.seq_num,
                                            });
                                        } else {
                                            let ind_reg = match reg.name().contains("temp") {
                                                // Mem computed call, e.g. call [r12+0x60]
                                                true => {
                                                    match Self::resolve_reg_indirect(&block, &reg) {
                                                        Some(val) => val,
                                                        // Mem far jump, e.g. jmp [rip+0x3f1b32]
                                                        None => {
                                                            return Some(
                                                                Branch::DirectJumpSentinel {
                                                                    site_pc,
                                                                    seq_num: self.seq_num,
                                                                },
                                                            )
                                                        }
                                                    }
                                                }
                                                false => reg,
                                            };

                                            match maybe_call {
                                                true => {
                                                    return Some(Branch::CallSentinel {
                                                        site_pc,
                                                        seq_num: self.seq_num,
                                                        reg: String::from(ind_reg.name()),
                                                    });
                                                }
                                                false => {
                                                    return Some(Branch::IndirectJumpSentinel {
                                                        site_pc,
                                                        seq_num: self.seq_num,
                                                        reg: String::from(ind_reg.name()),
                                                    });
                                                }
                                            }
                                        }
                                    }
                                    _ => continue,
                                }
                            }
                            _ => continue,
                        }
                    }
                }

                None
            }
            None => None,
        }
    }

    // Does instruction load a register?
    #[cfg(not(any(feature = "mips", feature = "mipsel")))]
    fn loads_reg(instr: &Instruction, reg: &Scalar) -> bool {
        if let Some(regs) = instr.scalars_read() {
            if regs.contains(&reg) && instr.is_load() {
                return true;
            }
        }

        false
    }

    // Does instruction store a register?
    #[cfg(not(any(feature = "mips", feature = "mipsel")))]
    fn stores_reg(instr: &Instruction, reg: &Scalar) -> bool {
        if let Some(regs) = instr.scalars_read() {
            if regs.contains(&reg) && instr.is_store() {
                return true;
            }
        }

        false
    }

    // What register is the scalar derived from?
    fn resolve_reg_indirect<'a>(block: &'a Block, scalar: &Scalar) -> Option<&'a Scalar> {
        for instr in block.instructions().iter().rev() {
            match instr.operation() {
                Operation::Assign { dst, src } => {
                    if dst == scalar {
                        if let Some(reg) = src.scalars().first() {
                            return Some(reg);
                        }
                    }
                }
                Operation::Load { dst, index } => {
                    if dst == scalar {
                        if let Some(reg) = index.scalars().first() {
                            return Some(reg);
                        }
                    }
                }
                _ => continue,
            }
        }

        None
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SEQ: {}, PC: {:08x}, BB_BYTES: {:x?}, LIFT: {}",
            self.seq_num,
            self.pc,
            self.bytes,
            {
                match &self.translation {
                    Some(cfg) => {
                        let mut instrs = String::from("\n");
                        for block in cfg.blocks() {
                            for instr in block.instructions() {
                                instrs.push_str(&format!("{}\n", instr));
                            }
                        }
                        instrs
                    }
                    None => "None".to_string(),
                }
            }
        )
    }
}
