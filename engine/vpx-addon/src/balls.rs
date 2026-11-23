//! The balls in play, in VPX builds whose add-ons can run a line of a table's own script (VPX's
//! `RunScript`, its 1075904b4b; every build with the newer interface, from its 595d1fd5e3). VPX
//! tells add-ons nothing of a table's balls, so a routine of ours is put in the table's script once
//! — it lists each ball's place and speed (VPX's `GetBalls`) and hands the list to a script object
//! of ours, made through VPX's scriptable add-on interface the way VPX's PinMAME add-on gives
//! scripts its `Controller` — then run a few times a second. Every error inside it is passed over
//! (`On Error Resume Next`), and it's only run once it has said it's there, so a table never shows
//! an error of ours. Nothing of VPX or of the table's file changes.
//!
//! A ball counts as in play once it's travelled well away from where it last rested, until it's
//! sat still ten seconds — as our player counted them: a trough's balls shuffling along by a ball's
//! width never count, a ball served, plunged or ejected does, a ball parked in a lock stops.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};
use std::sync::Mutex;

/// `ScriptTypeNameDef`: a type's or member's name, and the number VPX gives it.
#[repr(C)]
#[derive(Clone, Copy)]
struct TypeName {
    name: *const c_char,
    id: c_uint,
}

impl TypeName {
    const fn of(name: &'static CStr) -> TypeName {
        TypeName { name: name.as_ptr(), id: 0 }
    }
    const NONE: TypeName = TypeName { name: std::ptr::null(), id: 0 };
}

/// `ScriptString`: a string handed in, the caller's to let go.
#[repr(C)]
#[derive(Clone, Copy)]
struct ScriptString {
    release: *const c_void,
    string: *mut c_char,
}

/// `ScriptVariant`: a value handed in or out, of the type its member says.
#[repr(C)]
#[derive(Clone, Copy)]
pub union Variant {
    uint32: u32,
    string: ScriptString,
    object: *mut c_void,
    int64: i64,
}

type Call = unsafe extern "C" fn(me: *mut c_void, member: c_int, args: *mut Variant, ret: *mut Variant);

/// `ScriptClassMemberDef`.
#[repr(C)]
#[derive(Clone, Copy)]
struct MemberDef {
    name: TypeName,
    ty: TypeName,
    n_args: c_uint,
    arg_types: [TypeName; 16],
    call: Call,
}

/// `ScriptClassDef` with our four members: AddRef and Release first, as VPX wants them.
#[repr(C)]
pub struct ClassDef {
    name: TypeName,
    create_object: unsafe extern "C" fn() -> *mut c_void,
    n_members: c_uint,
    members: [MemberDef; 4],
}

/// `ScriptablePluginAPI`, past its version number.
#[repr(C)]
pub struct ScriptableApi {
    register_script_class: unsafe extern "C" fn(def: *mut ClassDef),
    register_script_type_alias: *const c_void,
    register_script_array_type: *const c_void,
    submit_type_library: unsafe extern "C" fn(endpoint: c_uint),
    unregister_script_class: unsafe extern "C" fn(def: *mut ClassDef),
    unregister_script_type_alias: *const c_void,
    unregister_script_array_type: *const c_void,
    on_error: *const c_void,
    set_com_object_override: unsafe extern "C" fn(class_name: *const c_char, def: *const ClassDef),
    get_class_def: *const c_void,
}

/// The script object's name in VPX's type library, and the one scripts make it by.
const CLASS: &CStr = c"VPIAchievementsBalls";
const CREATED_AS: &CStr = c"VPIAchievements.Balls";

/// Our routine, put in the table's script once: the list maker, the step run each time (the list
/// handed to our object), and our object told it's there. Whole numbers only, read the same in
/// every language Windows speaks.
pub const ROUTINE: &CStr = c"Function VPIAchievementsBalls()\r\n\tOn Error Resume Next\r\n\tDim b, s\r\n\ts = \"\"\r\n\tFor Each b In GetBalls\r\n\t\ts = s & b.ID & \" \" & Int(b.X) & \" \" & Int(b.Y) & \" \" & Int(b.VelX * 100) & \" \" & Int(b.VelY * 100) & \";\"\r\n\tNext\r\n\tVPIAchievementsBalls = s\r\nEnd Function\r\nSub VPIAchievementsTick()\r\n\tOn Error Resume Next\r\n\tCreateObject(\"VPIAchievements.Balls\").Report VPIAchievementsBalls()\r\nEnd Sub\r\nOn Error Resume Next\r\nCreateObject(\"VPIAchievements.Balls\").Ready\r\n";

/// The step run each time.
pub const TICK: &CStr = c"VPIAchievementsTick\r\n";

/// What our object has been told: whether our routine is there, and the last list handed in.
struct Told {
    ready: bool,
    list: Option<String>,
}

static TOLD: Mutex<Told> = Mutex::new(Told { ready: false, list: None });

/// Our one object: it keeps nothing, so every script holding it holds the same one, never freed.
static OBJECT: u8 = 0;

unsafe extern "C" fn create_object() -> *mut c_void {
    (&raw const OBJECT).cast_mut().cast()
}

unsafe extern "C" fn add_ref(_: *mut c_void, _: c_int, _: *mut Variant, ret: *mut Variant) {
    if !ret.is_null() {
        unsafe { (*ret).uint32 = 1 };
    }
}

unsafe extern "C" fn release(_: *mut c_void, _: c_int, _: *mut Variant, ret: *mut Variant) {
    if !ret.is_null() {
        unsafe { (*ret).uint32 = 1 };
    }
}

unsafe extern "C" fn on_report(_: *mut c_void, _: c_int, args: *mut Variant, _: *mut Variant) {
    if args.is_null() {
        return;
    }
    // SAFETY: VPX hands the one string argument the member says it takes
    let s = unsafe { (*args).string.string };
    if s.is_null() {
        return;
    }
    let list = unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned();
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).list = Some(list);
}

unsafe extern "C" fn on_ready(_: *mut c_void, _: c_int, _: *mut Variant, _: *mut Variant) {
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).ready = true;
}

fn member(name: &'static CStr, ty: &'static CStr, arg: Option<&'static CStr>, call: Call) -> MemberDef {
    let mut arg_types = [TypeName::NONE; 16];
    if let Some(a) = arg {
        arg_types[0] = TypeName::of(a);
    }
    MemberDef { name: TypeName::of(name), ty: TypeName::of(ty), n_args: u32::from(arg.is_some()), arg_types, call }
}

/// Our script object made known to VPX's scripts: its class registered and made by the name
/// scripts ask for (VPX writes into the class as it reads it, so it lives on, ours).
///
/// # Safety
/// `api` is VPX's scriptable interface past its version, or null.
pub unsafe fn register(api: *const ScriptableApi, endpoint: u32) -> Option<*mut ClassDef> {
    if api.is_null() {
        return None;
    }
    let def = Box::into_raw(Box::new(ClassDef {
        name: TypeName::of(CLASS),
        create_object,
        n_members: 4,
        members: [
            member(c"AddRef", c"uint32", None, add_ref),
            member(c"Release", c"uint32", None, release),
            member(c"Report", c"void", Some(c"string"), on_report),
            member(c"Ready", c"void", None, on_ready),
        ],
    }));
    let api = unsafe { &*api };
    unsafe {
        (api.register_script_class)(def);
        (api.submit_type_library)(endpoint);
        (api.set_com_object_override)(CREATED_AS.as_ptr(), def);
    }
    Some(def)
}

/// Our script object taken back out, as the add-on unloads (its class kept: VPX may still read it).
///
/// # Safety
/// `api` and `def` as `register` gave and took them.
pub unsafe fn unregister(api: *const ScriptableApi, def: *mut ClassDef) {
    if api.is_null() || def.is_null() {
        return;
    }
    let api = unsafe { &*api };
    unsafe {
        (api.set_com_object_override)(CREATED_AS.as_ptr(), std::ptr::null());
        (api.unregister_script_class)(def);
    }
}

/// A game starting: nothing told yet.
pub fn reset() {
    let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
    told.ready = false;
    told.list = None;
}

/// Whether our routine has said it's there.
pub fn ready() -> bool {
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).ready
}

/// The list last handed in, taken.
pub fn take() -> Option<String> {
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).list.take()
}

/// A ball on the table as our routine lists it: its ID, its place, its speed (VPX's units).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ball {
    pub id: i64,
    pub x: f32,
    pub y: f32,
    pub speed: f32,
}

/// The balls in our routine's list (`id x y vx vy;` each, the speeds a hundred times over).
pub fn parse(list: &str) -> Vec<Ball> {
    list.split(';')
        .filter_map(|one| {
            let n: Vec<i64> = one.split_whitespace().map(|v| v.parse().ok()).collect::<Option<_>>()?;
            let [id, x, y, vx, vy] = n[..] else { return None };
            let (vx, vy) = (vx as f32 / 100.0, vy as f32 / 100.0);
            Some(Ball { id, x: x as f32, y: y as f32, speed: (vx * vx + vy * vy).sqrt() })
        })
        .collect()
}

/// How long a ball still counts as in play while it sits still: a ball cradled on a flipper, but
/// not one parked in a lock or a trough.
const STILL_IN_PLAY_MS: u64 = 10_000;

/// How far a ball travels from where it last rested before it counts as in play: a trough's balls
/// shuffling along by a ball's width never do, a ball plunged or ejected into play does.
const AWAY_FROM_REST: f32 = 150.0;

/// How far a ball moves between looks, or how fast it goes, for it to count as moving.
const MOVED: f32 = 1.0;
const MOVING_SPEED: f32 = 0.5;

/// A ball as `InPlay` follows it.
struct Followed {
    /// Where it last rested, and where it was at the last look.
    rest: (f32, f32),
    last: (f32, f32),
    /// When it last moved (the game's clock).
    moved: u64,
    /// In play: it's travelled away from where it rested since, and isn't parked.
    active: bool,
}

/// The balls in play, from look to look.
#[derive(Default)]
pub struct InPlay {
    followed: HashMap<i64, Followed>,
}

fn apart(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

impl InPlay {
    /// How many of `balls` are in play at `now` (milliseconds of the game's clock).
    pub fn count(&mut self, balls: &[Ball], now: u64) -> u32 {
        let mut count = 0;
        for b in balls {
            let at = (b.x, b.y);
            let f = self.followed.entry(b.id).or_insert(Followed { rest: at, last: at, moved: now, active: false });
            let moving = b.speed >= MOVING_SPEED || apart(f.last, at) >= MOVED;
            f.last = at;
            if moving {
                f.moved = now;
            }
            let parked = now.saturating_sub(f.moved) > STILL_IN_PLAY_MS;
            if parked || (!moving && !f.active) {
                // resting: where it rests now is where it's measured from
                f.rest = at;
                f.active = false;
            } else if !f.active && apart(f.rest, at) > AWAY_FROM_REST {
                f.active = true;
            }
            count += u32::from(f.active);
        }
        self.followed.retain(|id, _| balls.iter().any(|b| b.id == *id));
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_class_is_laid_out_as_vpx_reads_it() {
        assert_eq!(std::mem::size_of::<TypeName>(), 16);
        assert_eq!(std::mem::size_of::<Variant>(), 16);
        assert_eq!(std::mem::size_of::<MemberDef>(), 304);
        assert_eq!(std::mem::offset_of!(MemberDef, call), 296);
        assert_eq!(std::mem::offset_of!(ClassDef, members), 32);
        assert_eq!(std::mem::size_of::<ScriptableApi>(), 80);
    }

    #[test]
    fn our_routine_lists_whole_numbers_and_swallows_its_errors() {
        let text = ROUTINE.to_str().unwrap();
        assert_eq!(text.matches("On Error Resume Next").count(), 3);
        assert!(text.contains("CreateObject(\"VPIAchievements.Balls\").Ready"));
        assert_eq!(CREATED_AS.to_str().unwrap(), "VPIAchievements.Balls");
        assert!(text.contains("Int(b.VelX * 100)"));
        assert_eq!(TICK.to_str().unwrap().trim(), "VPIAchievementsTick");
    }

    #[test]
    fn the_list_is_read() {
        assert_eq!(parse(""), vec![]);
        let balls = parse("12 450 1800 300 -400;13 900 1950 0 0;bad;14 1 2;");
        assert_eq!(balls.len(), 2);
        assert_eq!(balls[0], Ball { id: 12, x: 450.0, y: 1800.0, speed: 5.0 });
        assert_eq!(balls[1].speed, 0.0);
    }

    fn ball(id: i64, x: f32, y: f32, speed: f32) -> Ball {
        Ball { id, x, y, speed }
    }

    #[test]
    fn a_ball_counts_once_it_leaves_where_it_rested_until_it_sits_still() {
        let mut p = InPlay::default();
        // four balls resting in a trough: none in play
        let trough: Vec<Ball> = (1..=4).map(|i| ball(i, 800.0 + 50.0 * i as f32, 2000.0, 0.0)).collect();
        assert_eq!(p.count(&trough, 0), 0);
        // the trough shuffling along by a ball's width: still none
        let shuffled: Vec<Ball> = (1..=4).map(|i| ball(i, 750.0 + 50.0 * i as f32, 2000.0, 2.0)).collect();
        assert_eq!(p.count(&shuffled, 250), 0);
        let rested: Vec<Ball> = (1..=4).map(|i| ball(i, 750.0 + 50.0 * i as f32, 2000.0, 0.0)).collect();
        assert_eq!(p.count(&rested, 500), 0);
        // one served to the shooter lane and plunged: in play
        let mut served = rested.clone();
        served[0] = ball(1, 900.0, 1700.0, 12.0);
        assert_eq!(p.count(&served, 750), 1);
        served[0] = ball(1, 900.0, 900.0, 30.0);
        assert_eq!(p.count(&served, 1000), 1);
        // a second one ejected: two
        served[1] = ball(2, 500.0, 1400.0, 8.0);
        assert_eq!(p.count(&served, 1250), 2);
        // the second one locked, sitting still: counted for ten seconds, then not
        served[1] = ball(2, 300.0, 600.0, 0.0);
        assert_eq!(p.count(&served, 1500), 2);
        served[0] = ball(1, 700.0, 1100.0, 20.0);
        assert_eq!(p.count(&served, 11_000), 2);
        served[0] = ball(1, 650.0, 1150.0, 20.0);
        assert_eq!(p.count(&served, 11_700), 1);
        // drained: gone from the table
        served.remove(0);
        assert_eq!(p.count(&served, 12_000), 0);
        // the locked one ejected again: in play once it's travelled
        served[0] = ball(2, 300.0, 800.0, 10.0);
        assert_eq!(p.count(&served, 12_250), 1);
    }

    #[test]
    fn a_ball_moving_slowly_counts_by_how_far_it_goes() {
        let mut p = InPlay::default();
        assert_eq!(p.count(&[ball(1, 100.0, 100.0, 0.0)], 0), 0);
        // its speed below the mark, but it's moved: away from where it rested, in play
        assert_eq!(p.count(&[ball(1, 100.0, 300.0, 0.1)], 250), 1);
        // cradled: still, counted until ten seconds have passed
        assert_eq!(p.count(&[ball(1, 100.0, 300.0, 0.0)], 5_000), 1);
        assert_eq!(p.count(&[ball(1, 100.0, 300.0, 0.0)], 10_300), 0);
    }
}
