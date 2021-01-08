use std::fmt;

use falcon::il::ControlFlowGraph;
use falcon::il::Expression::Constant;
use falcon::il::Expression::Scalar;
use falcon::il::Operation;
use falcon::translator;
use falcon::translator::Translator;

static RET_MARKER: &'static str = "<RETURN>";

/// Direct calls: (call_site_pc, call_dst_pc).
/// Indirect calls: (call_site_pc, call_dst_pc, register_used).
/// Returns: (ret_site_pc, ret_dst_pc)
/// `Sentinel` is used internally to resolve indirect call and ret destinations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Direct(u64, u64),
    Indirect(u64, u64, String),
    Return(u64, u64),
    Sentinel(u64, usize, String),
}

/// Guest basic block, can map to TCG/LLVM execution delimiters (e.g. subset of static BB in ELF/PE)
#[derive(Debug, Clone)]
pub struct BasicBlock {
    seq_num: usize,
    pc: u64,
    bytes: Vec<u8>,
    translation: Option<ControlFlowGraph>,

    #[cfg(feature = "i386")]
    translator: translator::x86::X86,

    #[cfg(feature = "x86_64")]
    translator: translator::x86::Amd64,

    #[cfg(feature = "mips")]
    translator: translator::mips::Mips,

    #[cfg(feature = "mipsel")]
    translator: translator::mips::Mipsel,

    #[cfg(feature = "ppc")]
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

    /// Get lift status
    pub fn is_lifted(&self) -> bool {
        self.translation.is_some()
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

    /// Get first call or return present.
    /// `lift()` must be called before this function.
    pub fn find_call(&self) -> Option<Call> {
        match &self.translation {
            Some(cfg) => {
                for block in cfg.blocks() {
                    for instr in block.instructions() {
                        match (instr.operation(), instr.address()) {
                            (Operation::Branch { target }, Some(site)) => {
                                match target {
                                    // Direct call
                                    Constant(addr) => {
                                        if let Some(dst) = addr.value_u64() {
                                            return Some(Call::Direct(site, dst));
                                        }
                                    }

                                    // Indirect call or return
                                    // PC of the next BB executed is dest, fill sentinel with sequence number
                                    Scalar(reg) => {
                                        // TODO: is there a better way to differentiate returns from indirect calls?
                                        if reg.name().contains("temp") {
                                            return Some(Call::Sentinel(
                                                site,
                                                self.seq_num,
                                                String::from(RET_MARKER),
                                            ));
                                        } else {
                                            return Some(Call::Sentinel(
                                                site,
                                                self.seq_num,
                                                String::from(reg.name()),
                                            ));
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

// cargo test --features bin -- --show-output
#[cfg(test)]
mod tests {
    use super::*;

    // TODO: finish and run this test
    #[cfg(feature = "mips")]
    #[test]
    fn test_mips_simple() {
        let call_imm_encoding: [u8; 4] = [0x40, 0x00, 0x00, 0x0c]; // jal 0x100
        let ret_encoding: [u8; 4] = [0x08, 0x00, 0xe0, 0x03]; // jr $ra

        let mut call_imm_bb = BasicBlock::new(0, 0, &call_imm_encoding);
        call_imm_bb.lift();
        assert!(call_imm_bb.translation.is_some());
        println!(
            "CALL_IMM -> {:x?}\n\n{}",
            call_imm_bb.find_call(),
            call_imm_bb
        );
        assert_eq!(call_imm_bb.find_call(), Some(Call::Direct(0, 0x100)));

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation.is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_call(), ret_bb);
        assert_eq!(
            ret_bb.find_call(),
            Some(Call::Sentinel(0, 0, RET_MARKER.to_string()))
        );
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64() {
        #[rustfmt::skip]
        let call_imm_encoding: [u8; 14] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
            0xe8, 0x2c, 0x13, 0x00, 0x00,   // call 0x1331
            0x48, 0x31, 0xc0,               // xor rax, rax
        ];

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

        let mut call_imm_bb = BasicBlock::new(0, 0, &call_imm_encoding);
        call_imm_bb.lift();
        assert!(call_imm_bb.translation.is_some());
        println!(
            "CALL_IMM -> {:x?}\n\n{}",
            call_imm_bb.find_call(),
            call_imm_bb
        );
        assert_eq!(call_imm_bb.find_call(), Some(Call::Direct(6, 0x1337)));

        let mut call_ind_bb = BasicBlock::new(0, 0, &call_ind_encoding);
        call_ind_bb.lift();
        assert!(call_ind_bb.translation.is_some());
        println!(
            "CALL_IND -> {:x?}\n\n{}",
            call_ind_bb.find_call(),
            call_ind_bb
        );
        assert_eq!(
            call_ind_bb.find_call(),
            Some(Call::Sentinel(6, 0, "rax".to_string()))
        );

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation.is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_call(), ret_bb);
        assert_eq!(
            ret_bb.find_call(),
            Some(Call::Sentinel(6, 0, RET_MARKER.to_string()))
        );
    }
}
