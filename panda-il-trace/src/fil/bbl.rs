use std::fs;
use std::path::Path;
use std::collections::BTreeMap;

use serde::Serialize;

use super::{BasicBlock, Branch};

/// Final trace representation, for serialization.
#[derive(Debug, Clone, Serialize)]
pub struct BasicBlockList {
    list: Vec<BasicBlock>,
}

impl BasicBlockList {
    /// Constructor, resolves:
    /// * Indirect call/jump and return destinations
    /// * Conditional jump taken or not taken
    pub fn from(list: Vec<BasicBlock>) -> BasicBlockList {

        let total_bbs = list.len();
        let mut asid_map: BTreeMap<u64, Vec<BasicBlock>> = BTreeMap::new();

        // Bin by ASID
        list.into_iter().for_each(|bb| {
            let bbv = asid_map.entry(bb.asid()).or_insert(vec![]);
            bbv.push(bb);
        });

        // Resolution via looking at next BB executed
        for bbv in asid_map.values_mut() {

            // Guest execution order sort (via atomic sequence number)
            bbv.sort_unstable_by_key(|bb| bb.seq_num());

            // NOTE: cannot use slice::windows(2) b/c need to mutate prev BB
            let bbv_len = bbv.len();
            for idx in 0..bbv_len {
                let next_idx = idx + 1;
                if next_idx >= bbv_len {
                    break;
                }

                let actual_dst_pc = bbv[next_idx].pc();

                // TODO: add logic here to not fill in if next number is not correct sequence number (e.g. lift fail)

                if let Some(branch) = bbv[idx].branch_mut() {
                    match branch {
                        Branch::CallSentinel {
                            site_pc,
                            reg,
                            ..
                        } => {
                            *branch = Branch::IndirectCall {
                                site_pc: *site_pc,
                                dst_pc: actual_dst_pc,
                                reg_used: reg.to_string(),
                            };
                        }
                        Branch::ReturnSentinel { site_pc, .. } => {
                            *branch = Branch::Return {
                                site_pc: *site_pc,
                                dst_pc: actual_dst_pc,
                            };
                        }
                        Branch::IndirectJumpSentinel {
                            site_pc,
                            reg,
                            ..
                        } => {
                            *branch = Branch::IndirectJump {
                                site_pc: *site_pc,
                                dst_pc: actual_dst_pc,
                                reg_used: reg.to_string(),
                            };
                        }
                        Branch::DirectJumpSentinel { site_pc, .. } => {
                            *branch = Branch::DirectJump {
                                site_pc: *site_pc,
                                dst_pc: actual_dst_pc,
                                taken: true,
                            };
                        }
                        Branch::DirectJump {
                            site_pc,
                            dst_pc,
                            ..
                        } => {
                            let actual_taken = actual_dst_pc == *dst_pc;
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
        }

        let bbl = BasicBlockList {
            // TODO: into_values() once on stable
            list:  asid_map.values().flat_map(|bbv| bbv).cloned().collect()
        };

        assert_eq!(bbl.list.len(), total_bbs);

        bbl
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
