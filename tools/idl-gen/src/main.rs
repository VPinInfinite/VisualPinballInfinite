//! IDL generator: turns Visual Pinball X's script interface definition
//! (`src/core/vpinball.idl`, with the ids in `src/core/dispid.h`) into Rust
//! tables for the player's script objects: every object's
//! properties, methods and events, with VPX's names, ids and arguments, and
//! the enum constants VPX gives scripts.
//!
//! Usage, from the repository root:
//!   cargo run --release -p idl-gen [--idl PATH] [--dispid PATH] [--out PATH]
//!
//! Defaults read VPX build 5436 (`vpx-build/vpinball-5436`, the release our
//! physics follows) and write `engine/player/src/script/idl.rs`.
//! Re-run it when VPX's interface changes.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::process::ExitCode;

const DEFAULT_IDL: &str = "vpx-build/vpinball-5436/src/core/vpinball.idl";
const DEFAULT_DISPID: &str = "vpx-build/vpinball-5436/src/core/dispid.h";
const DEFAULT_OUT: &str = "engine/player/src/script/idl.rs";

#[derive(Debug, Clone)]
struct Param {
    name: String,
    ty: String,
    optional: bool,
    default: Option<String>,
    out: bool,
}

#[derive(Debug, Clone)]
struct Member {
    name: String,
    dispid: i32,
    access: &'static str,
    params: Vec<Param>,
    ret: Option<String>,
}

#[derive(Debug, Default)]
struct Idl {
    enums: Vec<(String, Vec<(String, i64)>)>,
    interfaces: Vec<(String, Vec<Member>)>,
    events: Vec<(String, Vec<Member>)>,
    coclasses: Vec<(String, Option<String>, Option<String>)>,
}

fn strip_comments(text: &str) -> String {
    text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n")
}

/// Splits at top-level commas: not inside (), [] or quotes.
fn split_top(s: &str, sep: char) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quoted) = (Vec::new(), String::new(), 0i32, false);
    for c in s.chars() {
        match c {
            '"' => quoted = !quoted,
            '(' | '[' if !quoted => depth += 1,
            ')' | ']' if !quoted => depth -= 1,
            _ => {}
        }
        if c == sep && depth == 0 && !quoted {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

/// The text between the brace that opens at or after `from` and its match.
fn braced(text: &str, from: usize) -> Option<(usize, usize)> {
    let open = from + text[from..].find('{')?;
    let mut depth = 0;
    for (i, c) in text[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open + 1, open + i));
                }
            }
            _ => {}
        }
    }
    None
}

fn number(s: &str) -> Option<i64> {
    let s = s.trim();
    let (neg, s) = s.strip_prefix('-').map_or((false, s), |r| (true, r));
    let v = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) { i64::from_str_radix(h, 16).ok()? } else { s.parse().ok()? };
    Some(if neg { -v } else { v })
}

fn dispid_value(s: &str, defines: &BTreeMap<String, String>) -> Option<i32> {
    let s = s.trim();
    if let Some(v) = number(s) {
        return Some(v as i32);
    }
    match s {
        "DISPID_NEWENUM" => return Some(-4),
        "DISPID_VALUE" => return Some(0),
        _ => {}
    }
    defines.get(s).and_then(|v| dispid_value(v, defines))
}

/// One declaration: `[attributes] HRESULT Name(params)` or `[id(n)] void Name(params)`.
fn member(decl: &str, defines: &BTreeMap<String, String>, auto_id: &mut i32) -> Option<Member> {
    let decl = decl.trim();
    let attrs_end = decl.find(']')?;
    let attrs = &decl[decl.find('[')? + 1..attrs_end];
    let rest = decl[attrs_end + 1..].trim();
    let rest = rest.strip_prefix("HRESULT").or_else(|| rest.strip_prefix("void"))?.trim();
    let name_end = rest.find('(')?;
    let name = rest[..name_end].trim().to_string();
    let args = &rest[name_end + 1..rest.rfind(')')?];
    let mut access = "Method";
    let mut dispid = None;
    for a in split_top(attrs, ',') {
        let a = a.trim();
        match a {
            "propget" => access = "Get",
            "propput" => access = "Put",
            "propputref" => access = "PutRef",
            _ if a.starts_with("id(") => dispid = dispid_value(&a[3..a.len() - 1], defines),
            _ => {}
        }
    }
    let dispid = dispid.unwrap_or_else(|| {
        *auto_id += 1;
        0x6002_0000 + *auto_id
    });
    let mut params = Vec::new();
    let mut ret = None;
    for p in split_top(args, ',') {
        let p = p.trim();
        if p.is_empty() || p == "void" {
            continue;
        }
        let (pattrs, decl) = match p.strip_prefix('[') {
            Some(r) => {
                let end = r.find(']')?;
                (r[..end].to_string(), r[end + 1..].trim().to_string())
            }
            None => (String::new(), p.to_string()),
        };
        let pattrs: Vec<String> = split_top(&pattrs, ',').into_iter().map(|s| s.trim().to_string()).collect();
        // the name is the last identifier; the type is what comes before it
        let decl = decl.replace('*', " * ");
        let words: Vec<&str> = decl.split_whitespace().collect();
        let pname = words.last()?.to_string();
        let ty = words[..words.len() - 1].join(" ").replace(" * ", "*").replace(" *", "*");
        let retval = pattrs.iter().any(|a| a == "retval");
        let out = pattrs.iter().any(|a| a == "out");
        let default = pattrs.iter().find_map(|a| a.strip_prefix("defaultvalue(").map(|v| v.trim_end_matches(')').to_string()));
        if retval {
            ret = Some(ty);
            continue;
        }
        params.push(Param { name: pname, ty, optional: pattrs.iter().any(|a| a == "optional") || default.is_some(), default, out });
    }
    Some(Member { name, dispid, access, params, ret })
}

fn parse(text: &str, defines: &BTreeMap<String, String>) -> Idl {
    let text = strip_comments(text);
    let mut idl = Idl::default();
    let mut at = 0;
    let mut auto_id = 0;
    loop {
        let next = ["enum ", "interface ", "dispinterface ", "coclass "]
            .iter()
            .filter_map(|k| {
                // a keyword at a word start
                let mut from = at;
                while let Some(i) = text[from..].find(k) {
                    let pos = from + i;
                    if pos == 0 || !text.as_bytes()[pos - 1].is_ascii_alphanumeric() {
                        return Some((pos, *k));
                    }
                    from = pos + k.len();
                }
                None
            })
            .min_by_key(|(p, _)| *p);
        let Some((pos, kind)) = next else { break };
        let after = &text[pos + kind.len()..];
        let name: String = after.trim_start().chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        // forward declarations (`interface IBall;`) have no body
        let semicolon = after.find(';').map(|i| pos + kind.len() + i);
        let brace = after.find('{').map(|i| pos + kind.len() + i);
        match (semicolon, brace) {
            (Some(s), Some(b)) if s < b => {
                at = s + 1;
                continue;
            }
            (_, None) => break,
            _ => {}
        }
        let Some((start, end)) = braced(&text, pos) else { break };
        let body = &text[start..end];
        match kind {
            "enum " => {
                let mut entries = Vec::new();
                for e in split_top(body, ',') {
                    if let Some((n, v)) = e.split_once('=')
                        && let Some(v) = number(v)
                    {
                        entries.push((n.trim().to_string(), v));
                    }
                }
                idl.enums.push((name, entries));
            }
            "interface " => {
                let members = body.split(';').filter_map(|d| member(d, defines, &mut auto_id)).collect();
                idl.interfaces.push((name, members));
            }
            "dispinterface " => {
                let methods = body.split("methods:").nth(1).unwrap_or("");
                let members = methods.split(';').filter_map(|d| member(d, defines, &mut auto_id)).collect();
                idl.events.push((name, members));
            }
            "coclass " => {
                let (mut iface, mut events) = (None, None);
                for line in body.split(';') {
                    let line = line.trim();
                    let last = line.split_whitespace().last().map(str::to_string);
                    if line.contains("dispinterface") && line.contains("source") {
                        events = last;
                    } else if line.contains("interface") && line.contains("default") {
                        iface = last;
                    }
                }
                idl.coclasses.push((name, iface, events));
            }
            _ => {}
        }
        at = end + 1;
    }
    idl
}

fn ty(t: &str, enums: &[(String, Vec<(String, i64)>)]) -> &'static str {
    let t = t.trim().trim_end_matches('*').trim();
    match t {
        "BSTR" => "Str",
        "long" | "int" | "short" | "OLE_COLOR" => "Int",
        "float" => "Float",
        "double" => "Double",
        "VARIANT_BOOL" => "Bool",
        "VARIANT" => "Variant",
        _ if t.starts_with("SAFEARRAY") => "Array",
        _ if enums.iter().any(|(n, _)| n == t) => "Int",
        _ if t.starts_with('I') => "Object",
        _ => "Variant",
    }
}

fn rust_ident(name: &str) -> String {
    let mut s = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            s.push('_');
        }
        s.push(c.to_ascii_uppercase());
    }
    s
}

fn write_members(out: &mut String, members: &[Member], enums: &[(String, Vec<(String, i64)>)]) {
    for m in members {
        let _ = write!(out, "        Member {{ name: {:?}, dispid: {}, access: Access::{}, params: &[", m.name, m.dispid, m.access);
        for p in &m.params {
            let default = match &p.default {
                Some(d) => match number(d) {
                    Some(v) => format!("Some({v}.0)"),
                    None => format!("Some({}.0)", d.parse::<f64>().unwrap_or(0.0)),
                },
                None => "None".into(),
            };
            let _ = write!(
                out,
                "Param {{ name: {:?}, ty: Ty::{}, optional: {}, default: {default}, by_ref: {} }}, ",
                p.name,
                ty(&p.ty, enums),
                p.optional,
                p.out || p.ty.ends_with('*') && ty(&p.ty, enums) != "Object"
            );
        }
        let ret = m.ret.as_deref().map_or("None".to_string(), |r| format!("Some(Ty::{})", ty(r, enums)));
        let _ = writeln!(out, "], ret: {ret} }},");
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut idl_path, mut dispid_path, mut out_path) = (DEFAULT_IDL.to_string(), DEFAULT_DISPID.to_string(), DEFAULT_OUT.to_string());
    while let Some(a) = args.next() {
        match a.as_str() {
            "--idl" => idl_path = args.next().unwrap_or_default(),
            "--dispid" => dispid_path = args.next().unwrap_or_default(),
            "--out" => out_path = args.next().unwrap_or_default(),
            _ => {
                eprintln!("usage: idl-gen [--idl PATH] [--dispid PATH] [--out PATH]");
                return ExitCode::from(2);
            }
        }
    }
    let (Ok(idl_text), Ok(dispid_text)) = (fs::read_to_string(&idl_path), fs::read_to_string(&dispid_path)) else {
        eprintln!("could not read {idl_path} or {dispid_path}");
        return ExitCode::FAILURE;
    };
    let defines: BTreeMap<String, String> = dispid_text
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            (w.next()? == "#define").then(|| Some((w.next()?.to_string(), w.next()?.to_string())))?
        })
        .collect();
    let idl = parse(&idl_text, &defines);

    let mut out = String::new();
    out.push_str("// Generated by tools/idl-gen from Visual Pinball X's src/core/vpinball.idl and dispid.h, build 5436\n");
    out.push_str("// (10.8.1 pre-release, commit af26b2d). dispid.h is license:GPLv3+; vpinball.idl carries no license\n");
    out.push_str("// marker, so only what a compatible implementation must answer to is taken from it: the names,\n");
    out.push_str("// argument lists and ids of the script interface, no code or descriptions. Do not edit: re-run the tool.\n");
    out.push_str("// Copyright (C) 2000-2026 Visual Pinball development team and contributors.\n");
    out.push_str("// This derived table is also GPL-3.0-or-later.\n\n");
    out.push_str("//! VPX's script interface: every object's properties, methods and events,\n");
    out.push_str("//! with VPX's names, ids and arguments, and the enum constants VPX\n");
    out.push_str("//! gives scripts.\n\n");
    out.push_str("use super::idl_types::*;\n\n");

    let mut constants: Vec<(String, i64)> = idl.enums.iter().flat_map(|(_, e)| e.iter().cloned()).collect();
    constants.sort_by_key(|(n, _)| n.to_lowercase());
    constants.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    out.push_str("/// The enum constants VPX gives scripts (`LightStateOn`, `TypeLight`...), by name.\n");
    out.push_str("pub static CONSTANTS: &[(&str, i32)] = &[\n");
    for (n, v) in &constants {
        let _ = writeln!(out, "    ({n:?}, {}),", *v as i32);
    }
    out.push_str("];\n\n");

    for (name, members) in &idl.interfaces {
        let _ = writeln!(out, "pub static {}: Interface = Interface {{\n    name: {name:?},\n    members: &[", rust_ident(name));
        write_members(&mut out, members, &idl.enums);
        out.push_str("    ],\n};\n\n");
    }
    for (name, members) in &idl.events {
        let _ = writeln!(out, "pub static {}: Interface = Interface {{\n    name: {name:?},\n    members: &[", rust_ident(name));
        write_members(&mut out, members, &idl.enums);
        out.push_str("    ],\n};\n\n");
    }
    out.push_str("/// Each kind of object: its interface and the events it fires.\n");
    out.push_str("pub static CLASSES: &[Class] = &[\n");
    for (name, iface, events) in &idl.coclasses {
        let iface = iface.as_ref().map_or("None".to_string(), |i| format!("Some(&{})", rust_ident(i)));
        let events = events.as_ref().map_or("None".to_string(), |e| format!("Some(&{})", rust_ident(e)));
        let _ = writeln!(out, "    Class {{ name: {name:?}, interface: {iface}, events: {events} }},");
    }
    out.push_str("];\n");

    if let Err(e) = fs::write(&out_path, out) {
        eprintln!("could not write {out_path}: {e}");
        return ExitCode::FAILURE;
    }
    println!(
        "{}: {} interfaces ({} members), {} event sets, {} classes, {} constants",
        out_path,
        idl.interfaces.len(),
        idl.interfaces.iter().map(|(_, m)| m.len()).sum::<usize>(),
        idl.events.len(),
        idl.coclasses.len(),
        constants.len()
    );
    ExitCode::SUCCESS
}
