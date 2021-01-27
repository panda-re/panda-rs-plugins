use std::fs;
use std::path::Path;

use crate::il::{BasicBlockList, Branch};

pub fn to_callstack_file<P: AsRef<Path>>(
    bb_list: &BasicBlockList,
    file_path: P,
) -> std::io::Result<()> {
    fs::write(
        file_path,
        fmt_callstack(bb_list).expect("Failed to format callstack!"),
    )
}

pub fn fmt_callstack(bb_list: &BasicBlockList) -> Result<String, ruut::Error> {
    println!("{}", to_lisp(bb_list));
    ruut::prettify(
        to_lisp(bb_list),
        ruut::InputFormat::LispLike,
        "unused".to_string(),
        "unused".to_string(),
        None,
    )
}

// List of basic blocks -> callstack as a Lisp-like string
fn to_lisp(bb_list: &BasicBlockList) -> String {
    let mut lisp_str = String::new();
    let mut ret_stack = String::new();

    match bb_list.is_empty() {
        true => return String::from("()"),
        false => {
            for (idx, bb) in bb_list.blocks().enumerate() {
                if let Some(branch) = bb.branch() {
                    match branch {
                        Branch::DirectCall {
                            site_pc: _,
                            dst_pc: _,
                        }
                        | Branch::IndirectCall {
                            site_pc: _,
                            dst_pc: _,
                            reg_used: _,
                        } => {
                            if idx == 0 {
                                lisp_str.push_str(&format!("{}", branch));
                            } else {
                                lisp_str.push_str(&format!("({}", branch));
                                ret_stack.push(')');
                            }
                        }
                        Branch::Return {
                            site_pc: _,
                            dst_pc: _,
                        } => {
                            if let Some(brace) = ret_stack.pop() {
                                lisp_str.push(brace);
                            }
                        },
                        _ => continue,
                    };
                }
            }
        }
    }

    // Call(s) without return
    while let Some(brace) = ret_stack.pop() {
        lisp_str.push(brace);
    }

    lisp_str
}

// cargo test --features bin -- --show-output
#[cfg(test)]
mod tests {
    use super::*;
    use crate::il::BasicBlock;

    #[cfg(feature = "x86_64")]
    #[test]
    fn test_callstack_print() {
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

        #[rustfmt::skip]
        let last_encoding: [u8; 6] = [
            0x48, 0x89, 0xd8,               // mov rax, rbx
            0x48, 0xff, 0xc0,               // inc rax
        ];

        let mut call_ind_bb = BasicBlock::new(0, 0, &call_ind_encoding);
        call_ind_bb.process();
        assert!(call_ind_bb.translation().is_some());
        assert!(call_ind_bb.branch().is_some());

        let mut ret_bb = BasicBlock::new(1, 0x1337, &ret_encoding);
        ret_bb.process();
        assert!(ret_bb.translation().is_some());
        assert!(ret_bb.branch().is_some());

        let mut call_ind_bb_2 = BasicBlock::new(2, 0, &call_ind_encoding);
        call_ind_bb_2.process();
        assert!(call_ind_bb_2.translation().is_some());
        assert!(call_ind_bb_2.branch().is_some());

        let mut call_ind_bb_3 = BasicBlock::new(3, 0, &call_ind_encoding);
        call_ind_bb_3.process();
        assert!(call_ind_bb_3.translation().is_some());
        assert!(call_ind_bb_3.branch().is_some());

        let mut ret_bb_2 = BasicBlock::new(4, 0x1337, &ret_encoding);
        ret_bb_2.process();
        assert!(ret_bb_2.translation().is_some());
        assert!(ret_bb_2.branch().is_some());

        let mut ret_bb_3 = BasicBlock::new(5, 0x1337, &ret_encoding);
        ret_bb_3.process();
        assert!(ret_bb_3.translation().is_some());
        assert!(ret_bb_3.branch().is_some());

        let mut last_bb = BasicBlock::new(6, 0, &last_encoding);
        last_bb.process();
        assert!(last_bb.translation().is_some());
        assert!(last_bb.branch().is_none());

        let bb_vec = vec![call_ind_bb, ret_bb, call_ind_bb_2, call_ind_bb_3, ret_bb_2, ret_bb_3, last_bb];
        let bb_list = BasicBlockList::from(bb_vec);
        assert_eq!(bb_list.len(), 7);
        assert_eq!(bb_list.trans_err_cnt(), 0);

        let cs = fmt_callstack(&bb_list);
        assert!(cs.is_ok());
        println!("{}", cs.unwrap());
    }
}