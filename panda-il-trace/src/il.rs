use std::fmt;

use panda::prelude::*;
use falcon::translator;
use falcon::translator::Translator;

pub struct BasicBlock {
    pub seq_num: usize,
    pub pc: target_ulong,
    pub bytes: Vec<u8>,
    pub translation: Option<translator::BlockTranslationResult>,

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
    pub fn new(seq_num: usize, pc: target_ulong, bytes: Vec<u8>) -> Self {

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
            translation: None
        }
    }

    pub fn lift(&mut self) {
        if let Ok(translation) = self.translator.translate_block(&self.bytes[..], self.pc as u64) {
            self.translation = Some(translation);
        }
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SEQ: {}, PC: {:08x}, BB_BYTES: {:x?}, LIFT: {}",
            self.seq_num, self.pc, self.bytes,
            {
                match &self.translation {
                    Some(translation) => {
                        let mut instrs = String::from("\n");

                        for (_, cfg) in translation.instructions() {
                            for block in cfg.blocks() {
                                for instr in block.instructions() {
                                    instrs.push_str(&format!("{}\n", instr));
                                }
                            }
                        }

                        instrs
                    },
                    None => "None".to_string()
                }
            }
        )
    }
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp() {
        let call_encoding: [u8; 4] = [0x40, 0x00, 0x00, 0x0c]; // jal 0x100
        let ret_encoding: [u8; 4] = [0x08, 0x00, 0xe0, 0x03];  // jr $ra

        let mut call_bb = BasicBlock::new(0, 0, call_encoding.to_vec());
        let mut ret_bb = BasicBlock::new(0, 0, ret_encoding.to_vec());

        call_bb.lift();
        assert(call_bb.translation.is_some());
        ret_bb.lift();
        assert(ret_bb.translation.is_some());

        // TODO: test call and ret detection here
    }
}
*/