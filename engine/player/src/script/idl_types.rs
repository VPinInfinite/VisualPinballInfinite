//! The shapes of the generated interface tables (`idl.rs`).

/// How a member is reached: called, read, or assigned (`Let` / `Set`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Method,
    Get,
    Put,
    PutRef,
}

/// An argument's or result's type, as far as converting values goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ty {
    Str,
    /// `long`, `int`, colours and enums.
    Int,
    Float,
    Double,
    Bool,
    Variant,
    Object,
    Array,
}

#[derive(Debug)]
pub struct Param {
    pub name: &'static str,
    pub ty: Ty,
    pub optional: bool,
    pub default: Option<f64>,
    /// Passed by reference: the script's variable can be written back.
    pub by_ref: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Member {
    pub name: &'static str,
    pub dispid: i32,
    pub access: Access,
    pub params: &'static [Param],
    pub ret: Option<Ty>,
}

#[derive(Debug)]
pub struct Interface {
    pub name: &'static str,
    pub members: &'static [Member],
}

/// A kind of object (VPX's coclass): its interface and its events.
#[derive(Debug)]
pub struct Class {
    pub name: &'static str,
    pub interface: Option<&'static Interface>,
    pub events: Option<&'static Interface>,
}

impl Interface {
    /// The members with this name, any access (case-insensitive, as VBScript).
    pub fn find<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'static Member> + 'a {
        self.members.iter().filter(move |m| m.name.eq_ignore_ascii_case(name))
    }

    /// The member with this id and access.
    pub fn by_id(&self, dispid: i32, access: Access) -> Option<&'static Member> {
        self.members.iter().find(|m| m.dispid == dispid && m.access == access)
    }
}
