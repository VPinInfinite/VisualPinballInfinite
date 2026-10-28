//! The table library's engine side: VPS's database, what a
//! dropped file or folder holds, a table matched to VPS as VPin Studio matches
//! one, where each file would go, and the tables installed with what each holds.
//! No windows of its own: the app shows it (`app/`).

pub mod altsound;
pub mod analysis;
pub mod archive;
pub mod backglass;
pub mod drop;
pub mod frontends;
pub mod inspect;
pub mod install;
pub mod installed;
pub mod layout_switch;
pub mod matcher;
pub mod patch;
pub mod physics;
pub mod plan;
pub mod popper;
pub mod processes;
pub mod pup_pack;
pub mod remove;
pub mod rar;
pub mod ratings;
pub mod rename;
pub mod rom_change;
pub mod rom_inventory;
pub mod roms;
pub mod scores;
pub mod script_edit;
pub mod table;
pub mod test;
pub mod vps;
