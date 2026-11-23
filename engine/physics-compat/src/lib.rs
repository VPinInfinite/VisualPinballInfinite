//! Compatibility-mode physics: Visual Pinball X's physics, ported to Rust.
//!
//! Existing tables were tuned on VPX's physics, so for them we don't imitate it,
//! we copy it. Pinball is chaotic: one bounce that differs in the last
//! bit becomes a different shot a few bounces later. The port therefore keeps
//! VPX's algorithms, loop orders and 32-bit float arithmetic, operation by
//! operation, and is checked against VPX's own physics tester, millisecond by
//! millisecond.
//!
//! **Source.** VPX 10.8.1 pre-release build 5436 (commit `af26b2d`, 2026-08-21).
//! Every ported file names the VPX file it comes from and keeps VPX's
//! license notice: the originals are marked `license:GPLv3+`, Copyright (C)
//! 2000-2026 Visual Pinball development team and contributors.
//!
//! **Float rules for this crate.**
//! - Same operations in the same order as VPX. Rust never fuses a multiply and
//!   an add on its own, and VPX's x64 build doesn't either (no `/arch:AVX2`).
//! - Write VPX's helpers as VPX does: its `min`/`max` are plain comparisons, not
//!   `f32::min`/`f32::max`, which treat NaN differently; its divisions by a
//!   vector length multiply by a reciprocal.
//! - VPX is compiled with fast math, which rounds differently at each place a
//!   function is used, so even VPX's own builds disagree in the last bit. The
//!   port follows VPX's source exactly and is checked against VPX built with
//!   exact float rules.
//! - **Processor mode.** VPX's player runs physics with flush-to-zero and
//!   denormals-are-zero switched on (MXCSR `0x8040`): numbers too small for a
//!   float's normal range act as zero. The thread running this crate must set
//!   the same mode ([`fpmode::VpxFloatMode`]), or rare tiny values play out
//!   differently.
//! - VPX's `min`/`max` compile to the processor's min/max instructions, whose
//!   operand order varies by call site; in flush-to-zero mode that can only
//!   change a zero's sign or turn a denormal into zero, which acts the same.
//!
//! **Checked so far**, bit for bit against VPX's tester built with exact float
//! rules, and within float noise of VPX as shipped (`tests/vpx_*_reference.rs`):
//! every math function the physics uses; the basic colliders and the ball; the
//! collision search; the step loop, on whole scenes of walls, ramps, meshes and
//! balls for 1,000 steps; flippers, the plunger and every other moving part.
//! The colliders `setup` builds for real tables are checked against VPX's in
//! the `table-physics` crate, which reads the tables.

pub mod collide;
pub mod collideex;
pub mod dump;
pub mod engine;
pub mod env;
pub mod flipper;
pub mod fpmode;
pub mod gain_calibrator;
pub mod hitball;
pub mod kdtree;
pub mod kicker;
pub mod math;
pub mod meshes;
pub mod motion_kalman;
pub mod movers;
pub mod nudge;
pub mod physconst;
pub mod plumb;
pub mod plunger;
pub mod quadtree;
pub mod setup;
pub mod vpx_source;
