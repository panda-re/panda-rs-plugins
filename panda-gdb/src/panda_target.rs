use crate::target_state::{STATE, BreakStatus};
use gdbstub::{SINGLE_THREAD_TID, Target, BreakOp, ResumeAction, StopReason, Tid, arch::Arch};

use std::convert::TryInto;

pub struct PandaTarget;

#[cfg(feature = "x86_64")]
use gdbstub::arch::x86::{X86_64, reg::{X86_64CoreRegs, F80}};

#[cfg(feature = "i386")]
use gdbstub::arch::x86::{X86, reg::{X86CoreRegs, F80}};

#[cfg(feature = "arm")]
use gdbstub::arch::arm::{Armv4t, reg::ArmCoreRegs};

#[cfg(feature = "ppc")]
use gdbstub::arch::ppc::{PowerPc, reg::PowerPcCoreRegs};

#[cfg(any(feature = "mips", feature = "mipsel"))]
use gdbstub::arch::mips::{Mips, reg::MipsCoreRegs};

impl Target for PandaTarget {
    #[cfg(feature = "x86_64")]
    type Arch = X86_64;
    
    #[cfg(feature = "i386")]
    type Arch = X86;
    
    #[cfg(feature = "arm")]
    type Arch = Armv4t;
    
    #[cfg(feature = "ppc")]
    type Arch = PowerPc;

    #[cfg(any(feature = "mips", feature = "mipsel"))]
    type Arch = Mips;

    type Error = ();
    
    fn resume(
            &mut self,
            mut actions: gdbstub::Actions,
            _check_gdb_interrupt: &mut dyn FnMut() -> bool,
        ) -> Result<(Tid, StopReason<<Self::Arch as Arch>::Usize>), Self::Error> {
        let (_, action) = actions.next().unwrap();

        match action {
            ResumeAction::Step => {
                STATE.start_single_stepping();
                STATE.cont.signal(());
                Ok((
                    SINGLE_THREAD_TID,
                    match STATE.brk.wait_for() {
                        BreakStatus::Break => StopReason::DoneStep,
                        BreakStatus::Exit => StopReason::Halted
                    }
                ))
            }
            ResumeAction::Continue => {
                STATE.cont.signal(());
                Ok((
                    SINGLE_THREAD_TID,
                    match STATE.brk.wait_for() {
                        BreakStatus::Break => StopReason::SwBreak,
                        BreakStatus::Exit => StopReason::Halted
                    }
                ))
            }
        }
    }

    fn read_registers(
            &mut self,
            regs: &mut <Self::Arch as Arch>::Registers,
        ) -> Result<(), Self::Error> {

        let cpu = STATE.wait_for_cpu();

        #[cfg(feature = "x86_64")] {
            let env = unsafe { &*(cpu.env_ptr as *const panda::sys::CPUX86State) };

            *regs = X86_64CoreRegs {
                eflags: env.eflags as _,
                regs: (*env).regs.clone(),
                rip: STATE.get_pc(),
                segments: (&env.segs.iter().map(|seg| seg.base as u32).collect::<Vec<_>>()[..6]).try_into().unwrap(),
                st: (&env.fpregs.iter().map(fpreg_to_bytes).collect::<Vec<_>>()[..8]).try_into().unwrap(),
                xmm: (&env.xmm_regs.iter().map(zmm_to_xmm).collect::<Vec<_>>()[..16]).try_into().unwrap(),
                mxcsr: env.mxcsr,
                ..Default::default()
            };
        }
        
        #[cfg(feature = "i386")] {
            let env = unsafe { &*(cpu.env_ptr as *const panda::sys::CPUX86State) };

            use panda::sys::{R_EAX, R_EBX, R_ECX, R_EDX, R_ESP, R_EBP, R_ESI, R_EDI};

            *regs = X86CoreRegs {
                eflags: env.eflags,
                eax: env.regs[R_EAX as usize],
                ebx: env.regs[R_EBX as usize],
                ecx: env.regs[R_ECX as usize],
                edx: env.regs[R_EDX as usize],
                esp: env.regs[R_ESP as usize],
                ebp: env.regs[R_EBP as usize],
                esi: env.regs[R_ESI as usize],
                edi: env.regs[R_EDI as usize],
                eip: STATE.get_pc(),
                segments: (&(*env).segs.iter().map(|seg| seg.base as u32).collect::<Vec<_>>()[..6]).try_into().unwrap(),
                st: (&(*env).fpregs.iter().map(fpreg_to_bytes).collect::<Vec<_>>()[..8]).try_into().unwrap(),
                xmm: (&(*env).xmm_regs.iter().map(zmm_to_xmm).collect::<Vec<_>>()[..8]).try_into().unwrap(),
                mxcsr: (*env).mxcsr,
                ..Default::default()
            };
        }
        
        #[cfg(feature = "arm")] {
            let env = unsafe { &*(cpu.env_ptr as *const panda::sys::CPUARMState) };

            *regs = ArmCoreRegs {
                r: env.regs[0..13].try_into().unwrap(),
                sp: env.regs[13],
                lr: env.regs[14],
                pc: STATE.get_pc(),
                cpsr: env.uncached_cpsr,
                ..Default::default()
            };
        }

        #[cfg(feature = "ppc")] {

        }
        
        #[cfg(any(feature = "mips", feature = "mipsel"))] {
            let env = unsafe { &*(cpu.env_ptr as *const panda::sys::CPUMIPSState) };

            regs.r = env.active_tc.gpr;
            regs.lo = env.active_tc.LO[0];
            regs.hi = env.active_tc.HI[0];
            regs.pc = STATE.get_pc();
            regs.cp0.status = env.CP0_Status as _;
            regs.cp0.badvaddr = env.CP0_BadVAddr as _;
            regs.cp0.cause = env.CP0_Cause as _;
            // TODO: fpu
        }

        Ok(())
    }

    fn write_registers(
            &mut self,
            regs: &<Self::Arch as Arch>::Registers,
        ) -> Result<(), Self::Error> {
        let cpu = STATE.wait_for_cpu();

        #[cfg(feature = "x86_64")] {
            let env = cpu.env_ptr as *mut panda::sys::CPUX86State;

            unsafe {
                (*env).regs = regs.regs.clone();
                (*env).eip = regs.rip;
                (*env).mxcsr = regs.mxcsr;
            }
        }
        #[cfg(feature = "arm")] {
            let env = unsafe { &mut *(cpu.env_ptr as *mut panda::sys::CPUARMState) };
            
            for i in 0..13 {
                env.regs[i] = regs.r[i];
            }
            env.regs[13] = regs.sp;
            env.regs[14] = regs.lr;
            env.regs[15] = regs.pc;
            env.uncached_cpsr = regs.cpsr;
        }
        #[cfg(feature = "i386")] {
            let env = unsafe { &mut *(cpu.env_ptr as *mut panda::sys::CPUX86State) };

            use panda::sys::{R_EAX, R_EBX, R_ECX, R_EDX, R_ESP, R_EBP, R_ESI, R_EDI};

            for &(i, val) in &[
                (R_EAX, regs.eax),
                (R_EBX, regs.ebx),
                (R_ECX, regs.ecx),
                (R_EDX, regs.edx),
                (R_ESP, regs.esp),
                (R_EBP, regs.ebp),
                (R_ESI, regs.esi),
                (R_EDI, regs.edi),
            ] {
                env.regs[i as usize] = val;
            }

            env.eflags = regs.eflags;
            STATE.set_pc(regs.eip);
        }

        Ok(())
    }

    fn read_addrs(
            &mut self,
            addr: <Self::Arch as Arch>::Usize,
            out: &mut [u8]
        ) -> Result<bool, Self::Error> {
        let cpu = STATE.wait_for_cpu();

        if let Some(mem) = cpu.try_mem_read(addr, out.len()) {
            out.clone_from_slice(&mem);
            Ok(true)
        } else {
            // NOTE: `Err(())` is unrecoverable so use this
            Ok(false)
        }
    }

    fn write_addrs(
            &mut self,
            addr: <Self::Arch as Arch>::Usize,
            data: &[u8],
        ) -> Result<bool, Self::Error> {
         let cpu = STATE.wait_for_cpu();

         cpu.mem_write(addr, data);

         Ok(true)
    }

    fn update_sw_breakpoint(
            &mut self,
            addr: <Self::Arch as Arch>::Usize,
            op: BreakOp,
        ) -> Result<bool, Self::Error> {
        match op {
            BreakOp::Add => {
                Ok(STATE.add_breakpoint(addr))
            }
            BreakOp::Remove => {
                Ok(STATE.remove_breakpoint(addr))
            }
        }
    }
}

#[cfg(any(feature = "x86_64", feature = "i386"))]
fn fpreg_to_bytes(x: &panda::sys::FPReg) -> F80 {
    unsafe {
        std::mem::transmute_copy(x)
    }
}

#[cfg(any(feature = "x86_64", feature = "i386"))]
fn zmm_to_xmm(x: &panda::sys::ZMMReg) -> u128 {
    unsafe {
        std::mem::transmute_copy(x)
    }
}
