use std::fs;
use std::path::Path;

use serde::Serialize;

use super::{BasicBlock, Branch, RET_MARKER};

/// Final trace representation, for serialization.
#[derive(Debug, Clone, Serialize)]
pub struct BasicBlockList {
    list: Vec<BasicBlock>,
}

impl BasicBlockList {
    /// Constructor, resolves:
    /// * Indirect call/jump and return destinations
    /// * Conditional jump taken or not taken
    pub fn from(mut list: Vec<BasicBlock>) -> BasicBlockList {
        // Guest execution order sort (via atomic sequence number)
        list.sort_unstable_by_key(|bb| bb.seq_num());

        // Resolution via looking at next BB executed
        let list_len = list.len();
        for idx in 0..list_len {

            let next_idx = idx + 1;
            if next_idx >= list_len {
                break;
            }

            let actual_dst_pc = list[next_idx].pc();
            let next_seq = list[next_idx].seq_num();

            if let Some(branch) = list[idx].branch_mut() {
                match branch {
                    Branch::CallSentinel {
                        site_pc,
                        seq_num,
                        reg_or_ret,
                    } => {
                        assert!(next_seq == (*seq_num + 1));

                        if reg_or_ret == RET_MARKER {
                            *branch = Branch::Return { site_pc: *site_pc, dst_pc: actual_dst_pc };
                        } else {
                            *branch = Branch::IndirectCall {
                                site_pc: *site_pc,
                                dst_pc: actual_dst_pc,
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

                        *branch = Branch::IndirectJump {
                            site_pc: *site_pc,
                            dst_pc: actual_dst_pc,
                            reg_used: reg_or_ret.to_string(),
                        };
                    }
                    Branch::DirectJump {
                        site_pc,
                        dst_pc,
                        taken
                    } => {
                        let actual_taken = (actual_dst_pc == *dst_pc);
                        *branch = Branch::DirectJump {
                            site_pc: *site_pc,
                            dst_pc: *dst_pc,
                            taken: actual_taken,
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