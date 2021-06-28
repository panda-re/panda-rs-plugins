use panda::prelude::*;
use panda::regs::Reg;

use peg::{str::LineCol, error::ParseError};

pub(crate) enum Command {
    Taint(TaintTarget, u32),
    CheckTaint(TaintTarget),
    GetTaint(TaintTarget),
    Help,
}

impl Command {
    pub(crate) fn parse(cmd: &str) -> Result<Self, ParseError<LineCol>> {
        monitor_commands::command(cmd)
    }
}

pub(crate) enum TaintTarget {
    Address(target_ptr_t),
    Register(Reg),
}

peg::parser!{
    grammar monitor_commands() for str {
        pub(crate) rule command() -> Command
            = taint()
            / check_taint()
            / get_taint()
            / help()
            / expected!("command")

        rule help() -> Command
            = "help" _ { Command::Help }

        rule taint() -> Command
            = "taint" _ target:taint_target() _ label:number() {
                Command::Taint(target, label as u32)
            }

        rule taint_target() -> TaintTarget
            = "*" addr:number() { TaintTarget::Address(addr) }
            / reg:register() { TaintTarget::Register(reg) }

        rule check_taint() -> Command
            = "check_taint" _ target:taint_target() { Command::CheckTaint(target) }

        rule get_taint() -> Command
            = "get_taint" _ target:taint_target() { Command::GetTaint(target) }

        rule register() -> Reg
            = reg:$(['a'..='z' | 'A'..='Z'] ['a'..='z' | 'A'..='Z' | '0'..='9']*) {?
                reg.parse()
                    .map_err(|_| "invalid register name")
            }

        rule number() -> u64
            = "0x" hex:$(['0'..='9' | 'a'..='f' | 'A'..='F']+) {?
                u64::from_str_radix(hex, 16)
                    .map_err(|_| "invalid hex number")
            }
            / decimal:$(['0'..='9']+) {?
                decimal.parse()
                    .map_err(|_| "invalid decimal number")
            }
            / expected!("number")

        rule _() = [' ' | '\n' | '\t']+
    }
}
