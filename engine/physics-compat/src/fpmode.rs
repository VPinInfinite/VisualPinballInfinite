//! The processor mode VPX's physics runs in.
//!
//! VPX's player switches on flush-to-zero and denormals-are-zero (MXCSR bits
//! `0x8040`) on the thread that runs physics (`Player::Player`): numbers too
//! small for a float's normal range act as zero. The port must run in the same
//! mode, or rare tiny values play out differently.

/// MXCSR flush-to-zero (`0x8000`) and denormals-are-zero (`0x0040`).
pub const FTZ_DAZ: u32 = 0x8040;

/// Switches flush-to-zero and denormals-are-zero on for the current thread
/// while it lives, and restores the previous mode when dropped.
#[cfg(target_arch = "x86_64")]
#[must_use = "the mode is restored when this is dropped"]
pub struct VpxFloatMode {
    old: u32,
}

#[cfg(target_arch = "x86_64")]
impl VpxFloatMode {
    pub fn enter() -> Self {
        let old = read_mxcsr();
        write_mxcsr(old | FTZ_DAZ);
        Self { old }
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for VpxFloatMode {
    fn drop(&mut self) {
        write_mxcsr(self.old);
    }
}

#[cfg(target_arch = "x86_64")]
fn read_mxcsr() -> u32 {
    let mut v: u32 = 0;
    // SAFETY: stores the current thread's SSE control register into a local.
    unsafe { std::arch::asm!("stmxcsr [{}]", in(reg) &mut v, options(nostack, preserves_flags)) };
    v
}

#[cfg(target_arch = "x86_64")]
fn write_mxcsr(v: u32) {
    // SAFETY: loads the current thread's SSE control register; only the rounding/denormal
    // mode bits this module manages are changed from their previous value.
    unsafe { std::arch::asm!("ldmxcsr [{}]", in(reg) &v, options(nostack, preserves_flags)) };
}

#[cfg(all(test, target_arch = "x86_64"))]
mod tests {
    use super::*;

    #[test]
    fn denormals_act_as_zero_inside_and_not_after() {
        let tiny = std::hint::black_box(f32::from_bits(1)); // the smallest denormal
        {
            let _mode = VpxFloatMode::enter();
            assert_eq!(std::hint::black_box(tiny * 1.0), 0.0);
        }
        assert_eq!(std::hint::black_box(tiny * 1.0).to_bits(), 1);
    }
}
