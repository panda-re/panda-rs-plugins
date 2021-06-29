use panda::prelude::*;
use panda::taint;

use gdbstub::outputln;

mod parser;
use parser::{Command, TaintTarget};

pub(crate) fn handle_command(cmd: &str, cpu: &mut CPUState, mut out: impl std::fmt::Write) {
    let cmd = cmd.trim();
    // this parsing is totally fine™
    match Command::parse(cmd) {
        Ok(Command::Taint(target, label)) => {
            match target {
                TaintTarget::Address(addr) => {
                    let addr = panda::mem::virt_to_phys(cpu, addr);
                    taint::label_ram(addr, label);
                    outputln!(out, "Memory location {:#x?} tainted.", addr);
                }
                TaintTarget::Register(reg) => {
                    taint::label_reg(reg, label);
                    outputln!(out, "Register {} tainted.", reg.to_string());
                }
            }
        },
        Ok(Command::CheckTaint(target)) => {
            match target {
                TaintTarget::Address(addr) => {
                    let addr = panda::mem::virt_to_phys(cpu, addr);
                    outputln!(out, "{:?}", taint::check_ram(addr));
                }
                TaintTarget::Register(reg) => {
                    outputln!(out, "{:?}", taint::check_reg(reg));
                }
            }
        },
        Ok(Command::GetTaint(target)) => {
            match target {
                TaintTarget::Address(addr) => {
                    let addr = panda::mem::virt_to_phys(cpu, addr);
                    // TODO: fix segfault
                    if taint::check_ram(addr) {
                        outputln!(out, "{:?}", taint::get_ram(addr));
                    } else {
                        outputln!(out, "[]")
                    }
                }
                TaintTarget::Register(reg) => {
                    // TODO: fix segfault
                    if taint::check_reg(reg) {
                        outputln!(out, "{:?}", taint::get_reg(reg));
                    } else {
                        outputln!(out, "[]")
                    }
                }
            }
        },
        Ok(Command::MemInfo) => crate::memory_map::print_to_gdb(cpu, out),
        Ok(Command::Help) => print_help_text(out),
        Err(peg::error::ParseError { location, expected }) => {
            outputln!(out);
            outputln!(out, "Error:");
            outputln!(out, "    {}", cmd);
            let expected: Vec<&str> = expected.tokens().collect();
            if let &[expected] = &expected[..] {
                outputln!(
                    out,
                    "   {}^------ Invalid syntax, expected {}",
                    " ".repeat(location.column),
                    expected
                );
            } else {
                outputln!(
                    out,
                    "   {}^------ Invalid syntax, expected one of the following: {}",
                    " ".repeat(location.column),
                    expected.join(", ")
                );
            }
            outputln!(out);
        }
    }
}

fn print_help_text(mut out: impl std::fmt::Write) {
    outputln!(out);
    outputln!(out, "Commands:");
    outputln!(out, "  meminfo - print out the current memory map");
    outputln!(out, "  taint - apply taint to a given register/memory location");
    outputln!(out, "  check_taint - check if a given register/memory location is tainted");
    outputln!(out, "  get_taint - get the taint labels for a given register/memory location");
}
