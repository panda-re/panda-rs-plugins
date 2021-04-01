pub mod branch;
pub use branch::*;

pub mod bb;
pub use bb::*;

pub mod bbl;
pub use bbl::*;

// cargo test --features bin -- --show-output
#[cfg(test)]
mod tests {
    use super::*;

    static DUMMY_ASID: u64 = 0xDEADBEEF;
    static DUMMY_PID: i32 = 1;
    static DUMMY_PPID: i32 = 0;
    static DUMMY_ICOUNT: i32 = 100;

    // TODO: tests for MIPS and PPC

    // TODO: finish/expand/run this test
    #[cfg(feature = "mips")]
    #[test]
    fn test_mips_simple() {
        let call_imm_encoding: [u8; 4] = [0x40, 0x00, 0x00, 0x0c]; // jal 0x100
        let ret_encoding: [u8; 4] = [0x08, 0x00, 0xe0, 0x03]; // jr $ra

        let mut call_imm_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &call_imm_encoding);
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

        let mut ret_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation().is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_branch(), ret_bb);
        assert_eq!(
            ret_bb.find_branch(),
            Some(Branch::ReturnSentinel {
                site_pc: 0,
                seq_num: 0,
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

        let mut call_ind_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &call_ind_encoding);
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
                reg: "rax".to_string()
            })
        );
    }

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_call_indirect_2() {
        #[rustfmt::skip]
        let call_ind_encoding: [u8; 8] = [
            0x48, 0x89, 0xdf,               // mov rdi, rbx
            0x41, 0xff, 0x54, 0x24, 0x60    // call [r12+0x60]
        ];

        let mut call_ind_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &call_ind_encoding);
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
                site_pc: 3,
                seq_num: 0,
                reg: "r12".to_string()
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

        let mut call_imm_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &call_imm_encoding);
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

        let mut ret_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &ret_encoding);
        ret_bb.lift();
        assert!(ret_bb.translation().is_some());
        println!("RET -> {:x?}\n\n{}", ret_bb.find_branch(), ret_bb);
        assert_eq!(
            ret_bb.find_branch(),
            Some(Branch::ReturnSentinel {
                site_pc: 6,
                seq_num: 0,
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

        let mut jmp_imm_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, &jmp_imm_encoding);
        jmp_imm_bb.lift();
        assert!(jmp_imm_bb.translation().is_some());
        println!(
            "JMP_DIR -> {:x?}\n\n{}",
            jmp_imm_bb.find_branch(),
            jmp_imm_bb
        );
        assert_eq!(jmp_imm_bb.find_branch(), Some(Branch::DirectJump { site_pc: 6, dst_pc: 0x1337 }));

    }
    */

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_x64_jump_direct_2() {
        #[rustfmt::skip]
        let jmp_dir_encoding: [u8; 6] = [
            0xff, 0x25, 0x32, 0x1b, 0x3f, 0x00  // jmp [rip+0x3f1b32]
        ];

        let mut jmp_ind_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &jmp_dir_encoding);
        jmp_ind_bb.lift();
        assert!(jmp_ind_bb.translation().is_some());
        println!(
            "JMP_DIR -> {:x?}\n\n{}",
            jmp_ind_bb.find_branch(),
            jmp_ind_bb
        );
        assert_eq!(
            jmp_ind_bb.find_branch(),
            Some(Branch::DirectJumpSentinel {
                site_pc: 0,
                seq_num: 0,
            })
        );
    }

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

        let mut jmp_ind_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &jmp_ind_encoding);
        jmp_ind_bb.lift();
        assert!(jmp_ind_bb.translation().is_some());
        println!(
            "JMP_IND -> {:x?}\n\n{}",
            jmp_ind_bb.find_branch(),
            jmp_ind_bb
        );
        assert_eq!(
            jmp_ind_bb.find_branch(),
            Some(Branch::IndirectJumpSentinel {
                site_pc: 6,
                seq_num: 0,
                reg: "rax".to_string()
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

        let mut call_ind_bb = BasicBlock::new(0, 0, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &call_ind_encoding);
        call_ind_bb.process();
        assert!(call_ind_bb.translation().is_some());
        assert!(call_ind_bb.branch().is_some());

        let mut ret_bb = BasicBlock::new(1, 0x1337, DUMMY_ASID, DUMMY_PID, DUMMY_PPID, DUMMY_ICOUNT, &ret_encoding);
        ret_bb.process();
        assert!(ret_bb.translation().is_some());
        assert!(ret_bb.branch().is_some());

        let expected = "{\"seq_num\":1,\"pc\":4919,\"asid\":3735928559,\"pid\":1,\"ppid\":0,\"icount\":100,\"branch\":{\"ReturnSentinel\":{\"site_pc\":4925,\"seq_num\":1}}}";
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