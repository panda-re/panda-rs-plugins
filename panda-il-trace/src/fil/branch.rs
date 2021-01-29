use std::fmt;

use serde::Serialize;

/// Branch types detected by the IL.
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
        taken: bool,
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
        reg: String,
    },
    JumpSentinel {
        site_pc: u64,
        seq_num: usize,
        reg: String,
    },
    ReturnSentinel {
        site_pc: u64,
        seq_num: usize,
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
            Branch::DirectJump {
                site_pc,
                dst_pc,
                taken,
            } => {
                write!(
                    f,
                    "DirectJump@0x{:016x} -> 0x{:016x} [taken: {}]",
                    site_pc, dst_pc, taken
                )
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
                reg,
            } => write!(
                f,
                "CallSentinel@0x{:016x} [{}], seq_num: {}",
                site_pc, reg, seq_num
            ),
            Branch::JumpSentinel {
                site_pc,
                seq_num,
                reg,
            } => write!(
                f,
                "JumpSentinel@0x{:016x} [{}], seq_num: {}",
                site_pc, reg, seq_num
            ),
            Branch::ReturnSentinel { site_pc, seq_num } => {
                write!(f, "ReturnSentinel@0x{:016x}, seq_num: {}", site_pc, seq_num)
            }
            Branch::Return { site_pc, dst_pc } => {
                write!(f, "Return@0x{:016x} -> 0x{:016x}", site_pc, dst_pc)
            }
        }
    }
}
