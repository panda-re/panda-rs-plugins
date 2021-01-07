use std::fmt;

use panda::prelude::*;
use falcon::translator;
use falcon::translator::Translator;

pub struct BasicBlock {
    seq_num: usize,
    pc: target_ulong,
    bytes: Vec<u8>,
    pub trans: Option<translator::BlockTranslationResult>,
}

impl BasicBlock {
    pub fn new(seq_num: usize, pc: target_ulong, bytes: Vec<u8>) -> Self {
        BasicBlock { seq_num, pc, bytes, trans: None }
    }

    pub fn lift(&mut self) {

        // TODO: support other archs
        let translator = translator::x86::X86::new();

        if let Ok(trans) = translator.translate_block(&self.bytes[..], self.pc as u64) {
            self.trans = Some(trans);
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
                match &self.trans {
                    Some(trans) => {
                        let mut instrs = String::from("\n");

                        for (_, cfg) in trans.instructions() {
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
        assert(call_bb.trans.is_some());
        ret_bb.lift();
        assert(ret_bb.trans.is_some());

        // TODO: test call and ret detection here
    }
}
*/