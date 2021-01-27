use std::fmt;
use std::fs;
use std::path::Path;

use falcon::architecture::Architecture;
use falcon::il::Expression::{Constant, Scalar};
use falcon::il::{ControlFlowGraph, Operation};
use falcon::translator;
use falcon::translator::Translator;
use serde::Serialize;

#[cfg(any(feature = "arm", feature = "ppc", feature = "mips", feature = "mipsel"))]
use falcon::il::Scalar as OtherScalar;

pub static RET_MARKER: &'static str = "<RETURN>";

/// Branch types.
/// Sentinels are used internally to resolve indirect call/jump and ret destinations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Branch {
    DirectCall {
        site_pc: u64,
        dst_pc: u64,
    },
    DirectJump {
        site_pc: u64,
        dst_pc: u64,
    },
    IndirectCall {
        site_pc: u64,
        dst_pc: u64,
        reg_used: String,
    },
    IndirectJump {
        site_pc: u64,
        dst_pc: u64,
        reg_used: String,
    },
    CallSentinel {
        site_pc: u64,
        seq_num: usize,
        reg_or_ret: String,
    },
    JumpSentinel {
        site_pc: u64,
        seq_num: usize,
        reg_or_ret: String,
    },
    Return {
        site_pc: u64,
        dst_pc: u64,
    },
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Branch::DirectCall { site_pc, dst_pc } => {
                write!(f, "DirectCall@0x{:016x} -> 0x{:016x}", site_pc, dst_pc)
            }
            Branch::DirectJump { site_pc, dst_pc } => {
                write!(f, "DirectJump@0x{:016x} -> 0x{:016x}", site_pc, dst_pc)
            }
            Branch::IndirectCall {
                site_pc,
                dst_pc,
                reg_used,
            } => write!(
                f,
                "IndirectCall@0x{:016x} -> 0x{:016x} [{}]",
                site_pc, dst_pc, reg_used
            ),
            Branch::IndirectJump {
                site_pc,
                dst_pc,
                reg_used,
            } => write!(
                f,
                "IndirectJump@0x{:016x} -> 0x{:016x} [{}]",
                site_pc, dst_pc, reg_used
            ),
            Branch::CallSentinel {
                site_pc,
                seq_num,
                reg_or_ret,
            } => write!(
                f,
                "CallSentinel@0x{:016x} [{}], seq_num: {}",
                site_pc, reg_or_ret, seq_num
            ),
            Branch::JumpSentinel {
                site_pc,
                seq_num,
                reg_or_ret,
            } => write!(
                f,
                "JumpSentinel@0x{:016x} [{}], seq_num: {}",
                site_pc, reg_or_ret, seq_num
            ),
            Branch::Return { site_pc, dst_pc } => {
                write!(f, "Return@0x{:016x} -> 0x{:016x}", site_pc, dst_pc)
            }
        }
    }
}

/// Guest basic block, can map to TCG/LLVM execution delimiters (e.g. subset of static BB in ELF/PE)
#[derive(Debug, Clone, Serialize)]
pub struct BasicBlock {
    seq_num: usize,
    pc: u64,
    bytes: Vec<u8>,
    branch: Option<Branch>,

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
}

impl BasicBlock {
    /// Constructor, copies bytes from parameter slice
    pub fn new(seq_num: usize, pc: u64, bytes: &[u8]) -> Self {
        BasicBlock::new_zero_copy(seq_num, pc, bytes.to_vec())
    }

    /// Constructor, takes ownership of BB bytes to avoid copy
    pub fn new_zero_copy(seq_num: usize, pc: u64, bytes: Vec<u8>) -> Self {
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

    /// Lift BB to IR, caches successes so multiple calls won't re-lift
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
                                // Falcon doesn't differentiate calls from jumps at the IL level
                                // This means we have to encode architecture-specific side effects here :(
                                let mut is_call = false;
                                if idx > 0 {
                                    let prev_instr = &block.instructions()[idx - 1];

                                    #[cfg(any(feature = "i386", feature = "x86_64"))]
                                    {
                                        #[cfg(feature = "x86_64")]
                                        let sp = falcon::architecture::Amd64::new().stack_pointer();

                                        #[cfg(feature = "i386")]
                                        let sp = falcon::architecture::X86::new().stack_pointer();

                                        if let Some(regs) = prev_instr.scalars_read() {
                                            if regs.contains(&&sp) && prev_instr.is_store() {
                                                is_call = true;
                                            }
                                        }
                                    }

                                    #[cfg(any(
                                        feature = "arm",
                                        feature = "ppc",
                                        feature = "mips",
                                        feature = "mipsel"
                                    ))]
                                    {
                                        if let Some(link_reg) = panda::reg_ret_addr() {
                                            let link_reg_scalar = OtherScalar::new(
                                                link_reg.to_string().to_lowercase(),
                                                32,
                                            );
                                            if let Some(regs) = prev_instr.scalars_written() {
                                                if regs.contains(&&link_reg_scalar) {
                                                    is_call = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                match target {
                                    // Direct call or jump
                                    Constant(addr) => {
                                        if let Some(dst_pc) = addr.value_u64() {
                                            match is_call {
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
                                                    })
                                                }
                                            }
                                        }
                                    }

                                    // Indirect call, jump, or return
                                    // PC of the next BB executed is dest, fill sentinel with sequence number
                                    Scalar(reg) => {
                                        // TODO: is there a better way to differentiate returns from indirect calls?
                                        if reg.name().contains("temp") {
                                            return Some(Branch::CallSentinel {
                                                site_pc,
                                                seq_num: self.seq_num,
                                                reg_or_ret: String::from(RET_MARKER),
                                            });
                                        } else {
                                            match is_call {
                                                true => {
                                                    return Some(Branch::CallSentinel {
                                                        site_pc,
                                                        seq_num: self.seq_num,
                                                        reg_or_ret: String::from(reg.name()),
                                                    });
                                                }
                                                false => {
                                                    return Some(Branch::JumpSentinel {
                                                        site_pc,
                                                        seq_num: self.seq_num,
                                                        reg_or_ret: String::from(reg.name()),
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

/// Final brace representation, for serialization
#[derive(Debug, Clone, Serialize)]
pub struct BasicBlockList {
    list: Vec<BasicBlock>,
}

impl BasicBlockList {
    /// Constructor
    pub fn from(mut list: Vec<BasicBlock>) -> BasicBlockList {
        // Guest execution order sort
        list.sort_unstable_by_key(|bb| bb.seq_num());

        // Determine indirect call/jump destinations
        //for bb_chunk in list.chunks_exact_mut(2) {
        let list_len = list.len();
        for idx in 0..list_len {
            //let dst_pc = bb_chunk[1].pc();
            //let next_seq = bb_chunk[1].seq_num();
            //let curr_seq = bb_chunk[0].seq_num();

            let next_idx = idx + 1;
            if next_idx >= list_len {
                break;
            }

            let dst_pc = list[next_idx].pc();
            let next_seq = list[next_idx].seq_num();
            //let curr_seq = list[idx].seq_num();

            //println!("Processing {} and {}", curr_seq, next_seq);

            //if let Some(branch) = bb_chunk[0].branch_mut() {
            if let Some(branch) = list[idx].branch_mut() {

                match branch {
                    Branch::CallSentinel {
                        site_pc,
                        seq_num,
                        reg_or_ret,
                    } => {
                        assert!(next_seq == (*seq_num + 1));
                        let site_pc = *site_pc;

                        if reg_or_ret == RET_MARKER {
                            *branch = Branch::Return { site_pc, dst_pc };
                        } else {
                            *branch = Branch::IndirectCall {
                                site_pc,
                                dst_pc,
                                reg_used: reg_or_ret.to_string(),
                            };
                        }
                    }
                    Branch::JumpSentinel {
                        site_pc,
                        seq_num,
                        reg_or_ret,
                    } => {
                        assert!(next_seq == (*seq_num + 1));
                        let site_pc = *site_pc;

                        *branch = Branch::IndirectJump {
                            site_pc,
                            dst_pc,
                            reg_used: reg_or_ret.to_string(),
                        };
                    }
                    _ => continue,
                };
            }
        }

        BasicBlockList { list }
    }

    /// Get count of translation errors
    pub fn trans_err_cnt(&self) -> usize {
        self.list.iter().filter(|bb| !bb.is_lifted()).count()
    }

    /// Get length
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    /// Return iterator to contained Basic Blocks
    pub fn blocks(&self) -> std::slice::Iter<'_, BasicBlock> {
        self.list.iter()
    }

    /// Serialize JSON
    pub fn to_branch_json<P: AsRef<Path>>(
        &self,
        file_path: P,
        white_space: bool,
    ) -> std::io::Result<()> {
        let bbs_with_branch: Vec<BasicBlock> = self
            .list
            .iter()
            .filter(|bb| bb.branch().is_some())
            .cloned()
            .collect();

        let branch_list = Self::from(bbs_with_branch);

        fs::write(
            file_path,
            match white_space {
                true => serde_json::to_string_pretty(&branch_list)
                    .expect("serialization of BB list failed!"),
                false => {
                    serde_json::to_string(&branch_list).expect("serialization of BB list failed!")
                }
            },
        )
    }
}

// cargo test --features bin -- --show-output
#[cfg(test)]
mod tests {
    use super::*;

    // TODO: tests for MIPS and PPC

    // TODO: finish/expand/run this test
    #[cfg(feature = "mips")]
    #[test]
    fn test_mips_simple() {
        let call_imm_encoding: [u8; 4] = [0x40, 0x00, 0x00, 0x0c]; // jal 0x100
        let ret_encoding: [u8; 4] = [0x08, 0x00, 0xe0, 0x03]; // jr $ra

        let mut call_imm_bb = BasicBlock::new(0, 0, &call_imm_encoding);
        call_imm_bb.lift();
        assert!(call_imm_bb.translation().is_some());
        println!(
            "CALL_IMM -> {:x?}\n\n{}",
            call_imm_bb.find_branch(),
            call_imm_bb
        );
        assert_eq!(
            call_imm_bb.find_branch(),
            Some(Branch::DirectCall {
                site_pc: 6,
                dst_pc: 0x100
            })
        );

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation().is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_branch(), ret_bb);
        assert_eq!(
            ret_bb.find_branch(),
            Some(Branch::CallSentinel {
                site_pc: 0,
                seq_nuum: 0,
                ret_or_reg: RET_MARKER.to_string()
            })
        );
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_call_indirect() {
        #[rustfmt::skip]
        let call_ind_encoding: [u8; 11] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xff, 0xd0,                     // call rax
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        let mut call_ind_bb = BasicBlock::new(0, 0, &call_ind_encoding);
        call_ind_bb.lift();
        assert!(call_ind_bb.translation().is_some());
        println!(
            "CALL_IND -> {:x?}\n\n{}",
            call_ind_bb.find_branch(),
            call_ind_bb
        );
        assert_eq!(
            call_ind_bb.find_branch(),
            Some(Branch::CallSentinel {
                site_pc: 6,
                seq_num: 0,
                reg_or_ret: "rax".to_string()
            })
        );
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_call_direct() {
        #[rustfmt::skip]
        let call_imm_encoding: [u8; 14] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xe8, 0x2c, 0x13, 0x00, 0x00,   // call 0x1337
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        let mut call_imm_bb = BasicBlock::new(0, 0, &call_imm_encoding);
        call_imm_bb.lift();
        assert!(call_imm_bb.translation().is_some());
        println!(
            "CALL_IMM -> {:x?}\n\n{}",
            call_imm_bb.find_branch(),
            call_imm_bb
        );
        assert_eq!(
            call_imm_bb.find_branch(),
            Some(Branch::DirectCall {
                site_pc: 6,
                dst_pc: 0x1337
            })
        );
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_ret() {
        #[rustfmt::skip]
        let ret_encoding: [u8; 10] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xc3,                           // ret
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation().is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_branch(), ret_bb);
        assert_eq!(
            ret_bb.find_branch(),
            Some(Branch::CallSentinel {
                site_pc: 6,
                seq_num: 0,
                reg_or_ret: RET_MARKER.to_string()
            })
        );
    }

    /*
    // TODO: This test fails. Falcon or Capstone bug?
    // We don't actually care about direct jumps, but there's no reason this should fail.
    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_jump_direct() {
        #[rustfmt::skip]
        let jmp_imm_encoding: [u8; 14] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xe9, 0x2c, 0x13, 0x00, 0x00,   // jmp 0x1337
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        let mut jmp_imm_bb = BasicBlock::new(0, 0, &jmp_imm_encoding);
        jmp_imm_bb.lift();
        assert!(jmp_imm_bb.translation().is_some());
        println!(
            "JMP_IMM -> {:x?}\n\n{}",
            jmp_imm_bb.find_branch(),
            jmp_imm_bb
        );
        assert_eq!(jmp_imm_bb.find_branch(), Some(Branch::DirectJump { site_pc: 6, dst_pc: 0x1337 }));

    }
    */

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_jump_indirect() {
        #[rustfmt::skip]
        let jmp_ind_encoding: [u8; 11] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xff, 0xe0,                     // jmp rax
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        let mut jmp_ind_bb = BasicBlock::new(0, 0, &jmp_ind_encoding);
        jmp_ind_bb.lift();
        assert!(jmp_ind_bb.translation().is_some());
        println!(
            "JMP_IND -> {:x?}\n\n{}",
            jmp_ind_bb.find_branch(),
            jmp_ind_bb
        );
        assert_eq!(
            jmp_ind_bb.find_branch(),
            Some(Branch::JumpSentinel {
                site_pc: 6,
                seq_num: 0,
                reg_or_ret: "rax".to_string()
            })
        );
    }

    #[test]
    fn test_branch_serialize() {
        let branch = Branch::IndirectCall {
            site_pc: 0x0,
            dst_pc: 0x1337,
            reg_used: "rax".to_string(),
        };
        let expected = "{\"IndirectCall\":{\"site_pc\":0,\"dst_pc\":4919,\"reg_used\":\"rax\"}}";
        let actual = serde_json::to_string(&branch).unwrap();
        println!("{}", actual);
        assert_eq!(expected, actual);

        let branch = Branch::Return {
            site_pc: 0x0,
            dst_pc: 0x1337,
        };
        let expected = "{\"Return\":{\"site_pc\":0,\"dst_pc\":4919}}";
        let actual = serde_json::to_string(&branch).unwrap();
        println!("{}", actual);
        assert_eq!(expected, actual);
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_block_serialize() {
        #[rustfmt::skip]
        let call_ind_encoding: [u8; 11] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xff, 0xd0,                     // call rax
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

        #[rustfmt::skip]
        let ret_encoding: [u8; 10] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xc3,                           // ret
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

         let mut call_ind_bb = BasicBlock::new(0, 0, &call_ind_encoding);
        call_ind_bb.process();
        assert!(call_ind_bb.translation().is_some());
        assert!(call_ind_bb.branch().is_some());

        let mut ret_bb = BasicBlock::new(1, 0x1337, &ret_encoding);
        ret_bb.process();
        assert!(ret_bb.translation().is_some());
        assert!(ret_bb.branch().is_some());

        let expected = "{\"seq_num\":1,\"pc\":4919,\"bytes\":[72,137,216,72,255,192,195,72,49,192],\"branch\":{\"CallSentinel\":{\"site_pc\":4925,\"seq_num\":1,\"reg_or_ret\":\"<RETURN>\"}}}";
        let actual = serde_json::to_string(&ret_bb).unwrap();
        println!("{}", actual);
        assert_eq!(expected, actual);

        let bb_vec = vec![call_ind_bb, ret_bb];
        let bb_list = BasicBlockList::from(bb_vec);
        assert_eq!(bb_list.len(), 2);
        assert_eq!(bb_list.trans_err_cnt(), 0);

        assert!(serde_json::to_string(&bb_list).is_ok());
    }
}
