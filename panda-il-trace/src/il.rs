use std::fmt;

use falcon::il::ControlFlowGraph;
use falcon::translator;
use falcon::translator::Translator;

/// Guest basic block, can map to TCG/LLVM execution delimiters (e.g. subset of static BB in ELF/PE)
pub struct BasicBlock {
    pub seq_num: usize,
    pub pc: u64,
    pub bytes: Vec<u8>,
    pub translation: Option<ControlFlowGraph>,

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

    /// Lift BB to IR, populates `self.translation`.
    /// This is not done by the constructor so that the work can be deferred for multi-threading.
    pub fn lift(&mut self) {
        if let Ok(translation) = self
            .translator
            .translate_block(&self.bytes[..], self.pc as u64)
        {
            if let Ok(cfg) = translation.blockify() {
                self.translation = Some(cfg);
            }
        }
    }

    /*
    /// Get (call_site_pc, call_dst_pc) for first call present.
    /// `lift()` must be called before this function.
    pub fn find_call(&self) -> Option<(u64, u64)> {
        match &self.translation {
            Some(cfg) => {
                for block in cfg.blocks() {
                    for instr in block.instructions() {
                    }
                }
            },
            None => None,
        }
    }

    /// Get pc for first ret present.
    /// `lift()` must be called before this function.
    pub fn find_ret(&self) -> Option<u64> {
        unimplemented!();
    }
    */
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

// cargo test --features bin
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mips_simple() {
        let call_encoding: [u8; 4] = [0x40, 0x00, 0x00, 0x0c]; // jal 0x100
        let ret_encoding: [u8; 4] = [0x08, 0x00, 0xe0, 0x03]; // jr $ra

        let mut call_bb = BasicBlock::new(0, 0, &call_encoding);
        call_bb.lift();
        assert!(call_bb.translation.is_some());

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation.is_some());

        // TODO: test call and ret detection here
    }

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

        let mut call_ind_bb = BasicBlock::new(0, 0, &call_ind_encoding);
        call_ind_bb.lift();
        assert!(call_ind_bb.translation.is_some());

        let mut ret_bb = BasicBlock::new(0, 0, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation.is_some());

        // TODO: test call and ret detection here
    }
}
