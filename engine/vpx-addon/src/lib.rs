//! Visual Pinball Infinite's add-on for VPX 10.8.1: a table's achievements followed while VPX plays
//! it, and screenshots taken on a key (`shots.rs`). VPX loads it from its `plugins` folder (`plugins\vpi-achievements`: this library as
//! `plugin-vpi-achievements64.dll` beside its `plugin.cfg`, id `VPIAchievements`) where its settings
//! say `[Plugin.VPIAchievements]` `Enable = 1`, and it does nothing unless the app hands it a
//! table's packs (`VPI_ACHIEVEMENTS`, the file `follow::Asked` reads).
//!
//! VPX's add-on interface as its headers declare it at build 5436 (`plugins/plugins/MsgPlugin.h`,
//! `VPXPlugin.h`, `ControllerPlugin.h`, GPLv3+), and the newer one VPX's later builds declare — its
//! structures opening with their version number, its messages' names ending in `:1`, a machine's
//! states handed over as groups of their own, each state with its own reader — told apart as VPX
//! loads the add-on (`interface_of`). The parts used here: the game starting and ending;
//! each physics update, when the machine's state is read; the controllers' state sources — PinMAME's
//! add-on's: its memory map's game state (the map in its `memmaps` folder), its lamps, switches and
//! solenoids — and the backglass add-ons' changes of state (`B2S`/`OnStateChange`, a script's
//! B2S calls); each frame VPX prepares, the game's clock standing still while they come telling a
//! pause — where the achievements stand is shown then (VPX's action events number its actions its
//! own way, not as its header's `VPXAction` declares, so they aren't used); and VPX's own notices,
//! one for each achievement earned, with a sound Windows plays
//! (its pack's own WAV, else ours, at the level the app hands over). The following itself, which
//! knows nothing of VPX: `follow.rs`. Each call from VPX is caught whole, so nothing here can take
//! VPX down with it. Ours.

use std::ffi::{CString, c_char, c_int, c_uint, c_void};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub mod balls;
pub mod follow;
pub mod hardcore;
pub mod menu;
pub mod popup;
pub mod shots;
pub mod standalone;

use follow::{Asked, Chime, Following, PageLine, Value, game_state_key};

/// VPX's `VPXInfo`: its own folder and its preferences folder, native narrow paths VPX owns.
#[repr(C)]
struct VpxInfo {
    path: *const c_char,
    pref_path: *const c_char,
}

/// VPX's `VPXTableInfo`: the table file played (a native narrow path), or none.
#[repr(C)]
struct VpxTableInfo {
    path: *const c_char,
    table_width: f32,
    table_height: f32,
}

/// The controllers running, as VPX's controllers list asks (`GetControllers`): each one's add-on
/// and its game, `namespace::game`.
#[repr(C)]
#[derive(Clone, Copy)]
struct ControllerDef {
    endpoint_id: u32,
    game_id: *const c_char,
}

#[repr(C)]
struct GetControllersMsg {
    max_entry_count: c_uint,
    count: c_uint,
    entries: *mut ControllerDef,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn MultiByteToWideChar(code_page: u32, flags: u32, src: *const u8, src_len: i32, dst: *mut u16, dst_len: i32) -> i32;
}

/// A path as VPX hands it, native narrow: in the process's code page (ANSI, or UTF-8 where Windows
/// makes that the code page), turned to Windows' own.
fn native_path(p: *const c_char) -> Option<std::path::PathBuf> {
    if p.is_null() {
        return None;
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(p) }.to_bytes();
    if bytes.is_empty() {
        return None;
    }
    let n = unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, std::ptr::null_mut(), 0) };
    if n <= 0 {
        return None;
    }
    let mut wide = vec![0u16; n as usize];
    let n = unsafe { MultiByteToWideChar(0, 0, bytes.as_ptr(), bytes.len() as i32, wide.as_mut_ptr(), n) };
    wide.truncate(n.max(0) as usize);
    Some(<std::ffi::OsString as std::os::windows::ffi::OsStringExt>::from_wide(&wide).into())
}

/// The add-on's own settings, where it plays on its own with no app handing it a game's packs:
/// VPX keeps them under `[Plugin.VPIAchievements]` and shows them in its menu (the newer interface
/// alone). The achievement sound, on or off; the card's place — none, the backglass, the score
/// display; its size — small, medium, large.
static OWN_SOUND: AtomicBool = AtomicBool::new(true);
static OWN_CARD: AtomicUsize = AtomicUsize::new(1);
static OWN_CARD_SIZE: AtomicUsize = AtomicUsize::new(1);

/// VPX's `MsgSettingDef`, for the kinds the add-on registers: a whole number with its values'
/// names (a choice), or a switch.
#[repr(C)]
#[derive(Clone, Copy)]
struct IntDef {
    min: c_int,
    max: c_int,
    def: c_int,
    values: *const *const c_char,
    get: unsafe extern "C" fn() -> c_int,
    set: unsafe extern "C" fn(c_int),
}

#[repr(C)]
#[derive(Clone, Copy)]
struct BoolDef {
    def: c_int,
    get: unsafe extern "C" fn() -> c_int,
    set: unsafe extern "C" fn(c_int),
}

/// The setting's kind: as large as VPX's own union of them, its largest a whole number's.
#[repr(C)]
union SettingKind {
    int: IntDef,
    boolean: BoolDef,
    _room: [u64; 5],
}

#[repr(C)]
struct SettingDef {
    prop_id: *const c_char,
    name: *const c_char,
    description: *const c_char,
    is_user_editable: c_int,
    kind: c_int,
    def: SettingKind,
}

const SETTING_INT: c_int = 1;
const SETTING_BOOL: c_int = 2;

unsafe extern "C" fn own_sound_get() -> c_int {
    c_int::from(OWN_SOUND.load(Ordering::Relaxed))
}
unsafe extern "C" fn own_sound_set(v: c_int) {
    OWN_SOUND.store(v != 0, Ordering::Relaxed);
}
unsafe extern "C" fn own_card_get() -> c_int {
    OWN_CARD.load(Ordering::Relaxed) as c_int
}
unsafe extern "C" fn own_card_set(v: c_int) {
    OWN_CARD.store(v.clamp(0, 2) as usize, Ordering::Relaxed);
}
unsafe extern "C" fn own_size_get() -> c_int {
    OWN_CARD_SIZE.load(Ordering::Relaxed) as c_int
}
unsafe extern "C" fn own_size_set(v: c_int) {
    OWN_CARD_SIZE.store(v.clamp(0, 2) as usize, Ordering::Relaxed);
}

struct Names([*const c_char; 3]);
unsafe impl Sync for Names {}
static CARD_PLACES: Names = Names([c"None".as_ptr(), c"Backglass".as_ptr(), c"Score display".as_ptr()]);
static CARD_SIZES: Names = Names([c"Small".as_ptr(), c"Medium".as_ptr(), c"Large".as_ptr()]);

struct Own([SettingDef; 3]);
unsafe impl Sync for Own {}
static OWN_SETTINGS: Own = Own([
    SettingDef {
        prop_id: c"Sound".as_ptr(),
        name: c"Achievement sound".as_ptr(),
        description: c"Play a sound when an achievement is earned (its level is in VPX's sound settings)".as_ptr(),
        is_user_editable: 1,
        kind: SETTING_BOOL,
        def: SettingKind { boolean: BoolDef { def: 1, get: own_sound_get, set: own_sound_set } },
    },
    SettingDef {
        prop_id: c"Card".as_ptr(),
        name: c"Achievement card".as_ptr(),
        description: c"Where a card shows when an achievement is earned".as_ptr(),
        is_user_editable: 1,
        kind: SETTING_INT,
        def: SettingKind { int: IntDef { min: 0, max: 2, def: 1, values: CARD_PLACES.0.as_ptr(), get: own_card_get, set: own_card_set } },
    },
    SettingDef {
        prop_id: c"CardSize".as_ptr(),
        name: c"Achievement card size".as_ptr(),
        description: c"How big the card is, against its window".as_ptr(),
        is_user_editable: 1,
        kind: SETTING_INT,
        def: SettingKind { int: IntDef { min: 0, max: 2, def: 1, values: CARD_SIZES.0.as_ptr(), get: own_size_get, set: own_size_set } },
    },
]);

type MsgCallback = unsafe extern "C" fn(msg_id: c_uint, user_data: *mut c_void, msg_data: *mut c_void);

/// `MsgPluginAPI`: messages between VPX and its add-ons.
#[repr(C)]
pub struct MsgPluginApi {
    get_plugin_endpoint: unsafe extern "C" fn(id: *const c_char) -> c_uint,
    get_endpoint_info: *const c_void,
    get_msg_id: unsafe extern "C" fn(name_space: *const c_char, name: *const c_char) -> c_uint,
    subscribe_msg: unsafe extern "C" fn(endpoint: u32, msg_id: c_uint, callback: MsgCallback, user_data: *mut c_void),
    unsubscribe_msg: unsafe extern "C" fn(msg_id: c_uint, callback: MsgCallback, user_data: *mut c_void),
    broadcast_msg: unsafe extern "C" fn(endpoint: u32, msg_id: c_uint, data: *mut c_void),
    send_msg: *const c_void,
    release_msg_id: unsafe extern "C" fn(msg_id: c_uint),
    register_setting: *const c_void,
    save_setting: *const c_void,
    run_on_main_thread: *const c_void,
    flush_pending_callbacks: unsafe extern "C" fn(endpoint: u32),
}

/// `VPXPluginAPI`, up to its textures: VPX's own services to its add-ons.
#[repr(C)]
pub struct VpxPluginApi {
    get_vpx_info: *const c_void,
    get_table_info: *const c_void,
    push_notification: unsafe extern "C" fn(msg: *const c_char, length_ms: c_int) -> c_uint,
    update_notification: *const c_void,
    disable_static_prerendering: *const c_void,
    get_active_view_setup: *const c_void,
    set_active_view_setup: *const c_void,
    get_input_state: *const c_void,
    set_input_state: *const c_void,
    get_game_time: unsafe extern "C" fn() -> f64,
    create_texture: *const c_void,
    /// A texture made, or made again, from a picture in one of VPX's formats (`TEXTURE_RGBA`).
    update_texture: unsafe extern "C" fn(texture: *mut *mut c_void, width: c_int, height: c_int, format: c_int, image: *const c_void),
    get_texture_info: *const c_void,
    delete_texture: unsafe extern "C" fn(texture: *mut c_void),
}

/// The newer interface's own services, past those both interfaces have: `RunScript`, a line run in
/// the table's own script (VPX's 1075904b4b, in every build with the newer interface).
#[repr(C)]
struct VpxPluginApiV1 {
    both: VpxPluginApi,
    run_script: Option<unsafe extern "C" fn(script: *const c_char)>,
}

/// For checks, where `VPI_ACHIEVEMENTS_CHECK_SCRIPT` asks, in the newer interface: lines run in the
/// table's own script as the game's clock reaches each moment (`ms=line`, `|` between them) — a
/// table's own key routine called, say, as VPX's capture mode takes no input.
fn check_lines(text: &str) -> Vec<(u64, CString)> {
    let mut out: Vec<(u64, CString)> = text
        .split('|')
        .filter_map(|one| {
            let (at, line) = one.split_once('=')?;
            Some((at.trim().parse().ok()?, CString::new(line.trim()).ok()?))
        })
        .collect();
    out.sort_by_key(|l| l.0);
    out
}

/// `VPXTEXFMT_sRGBA8`: rows of RGBA, each a byte.
const TEXTURE_RGBA: c_int = 2;

/// `VPXRenderContext2D`: what an add-on's renderer draws with, for one of VPX's windows (or a flasher
/// showing one: `is_2d` 0), the same in both interfaces. `draw_image` lays a texture's part over the
/// window's, in the window's own units (`src_width` by `src_height`, the window's pixels to begin).
#[repr(C)]
struct RenderContext {
    window: c_int,
    src_width: f32,
    src_height: f32,
    is_2d: c_int,
    out_width: f32,
    out_height: f32,
    #[allow(clippy::type_complexity)]
    draw_image: unsafe extern "C" fn(ctx: *mut RenderContext, texture: *mut c_void, tint_r: f32, tint_g: f32, tint_b: f32, alpha: f32, tex_x: f32, tex_y: f32, tex_w: f32, tex_h: f32, pivot_x: f32, pivot_y: f32, rotation: f32, src_x: f32, src_y: f32, src_w: f32, src_h: f32),
    draw_display: *const c_void,
    draw_seg_display: *const c_void,
    renderer_data: *mut c_void,
}

type RenderFn = unsafe extern "C" fn(ctx: *mut RenderContext, context: *mut c_void) -> c_int;

/// `AncillaryRendererDef`: a renderer an add-on offers for one of VPX's windows. VPX asks a window's
/// renderers to draw in the order of their priorities (`Priority.<id>` in the window's settings),
/// until one says it drew.
#[repr(C)]
#[derive(Clone, Copy)]
struct RendererDef {
    id: *const c_char,
    name: *const c_char,
    description: *const c_char,
    context: *mut c_void,
    render: Option<RenderFn>,
}

impl Default for RendererDef {
    fn default() -> Self {
        RendererDef { id: std::ptr::null(), name: std::ptr::null(), description: std::ptr::null(), context: std::ptr::null_mut(), render: None }
    }
}

/// `GetAncillaryRendererMsg`: a window's renderers asked for, Vulkan's way (the count grows past the
/// room given).
#[repr(C)]
struct GetRendererMsg {
    window: c_int,
    max_entry_count: c_uint,
    count: c_uint,
    entries: *mut RendererDef,
}

/// Our renderer's id, which VPX keeps its priority under.
const RENDERER_ID: &std::ffi::CStr = c"VPIAchievements";

/// `CtlResId`: a resource of an add-on, by the add-on's endpoint.
#[repr(C, align(8))]
#[derive(Clone, Copy, Default)]
struct CtlResId {
    endpoint_id: u32,
    res_id: u32,
}

/// `CtlStateId`: a state's group and its number in it.
#[repr(C, align(8))]
#[derive(Clone, Copy, Default)]
struct CtlStateId {
    group_id: u32,
    state_id: u32,
}

/// `StateDef`: one of a controller's states, its name and description owned by the controller.
#[repr(C)]
#[derive(Clone, Copy)]
struct StateDef {
    name: *const c_char,
    desc: *const c_char,
    id: CtlStateId,
    type_mask: c_int,
    writable: c_int,
}

type StateFn = unsafe extern "C" fn(index: c_uint, kind: c_int, result: *mut c_void) -> c_int;

/// `StateSrcId`: a controller's states and how to read them, valid until its source changes.
#[repr(C)]
#[derive(Clone, Copy)]
struct StateSrcId {
    id: CtlResId,
    n_groups: c_uint,
    group_defs: *mut c_void,
    n_states: c_uint,
    state_defs: *mut StateDef,
    get_state: Option<StateFn>,
    set_state: Option<StateFn>,
}

impl Default for StateSrcId {
    fn default() -> Self {
        StateSrcId { id: CtlResId::default(), n_groups: 0, group_defs: std::ptr::null_mut(), n_states: 0, state_defs: std::ptr::null_mut(), get_state: None, set_state: None }
    }
}

/// `GetStateSrcMsg`: the sources asked for, Vulkan's way (the count grows past the room given).
#[repr(C)]
struct GetStateSrcMsg {
    max_entry_count: c_uint,
    count: c_uint,
    entries: *mut StateSrcId,
}

/// The newer interface's state reader: each state its own, with its own context.
type StateFnV1 = unsafe extern "C" fn(context: *mut c_void, result: *mut c_void);

/// The newer interface's `StateDef`: a state's names, its number in its group (VPinMAME's for
/// lamps, switches and solenoids), its data format, and its own reader.
#[repr(C)]
#[derive(Clone, Copy)]
struct StateDefV1 {
    name: *const c_char,
    desc: *const c_char,
    mapping_id: u32,
    data_format: c_int,
    semantic_type: c_int,
    call_context: *mut c_void,
    get_state: Option<StateFnV1>,
    set_state: *const c_void,
}

/// The newer interface's `StateSrcId`: one group of a controller's states (PinMAME's "Game States",
/// "VPinMAME Lamps", "Switches", "VPinMAME Solenoids" and so on), valid until its source changes.
#[repr(C)]
#[derive(Clone, Copy)]
struct StateSrcIdV1 {
    id: CtlResId,
    name: *const c_char,
    desc: *const c_char,
    n_states: c_uint,
    state_defs: *mut StateDefV1,
}

impl Default for StateSrcIdV1 {
    fn default() -> Self {
        StateSrcIdV1 { id: CtlResId::default(), name: std::ptr::null(), desc: std::ptr::null(), n_states: 0, state_defs: std::ptr::null_mut() }
    }
}

/// `GetStateSrcMsg` of the newer interface, the same shape around its own entries.
#[repr(C)]
struct GetStateSrcMsgV1 {
    max_entry_count: c_uint,
    count: c_uint,
    entries: *mut StateSrcIdV1,
}

/// The newer interface's data formats (`CTLPI_STATE_FORMAT_*`).
const FORMAT_UINT8: c_int = 1;
const FORMAT_UINT16: c_int = 2;
const FORMAT_UINT32: c_int = 3;
const FORMAT_UINT64: c_int = 4;
const FORMAT_INT8: c_int = 5;
const FORMAT_INT16: c_int = 6;
const FORMAT_INT32: c_int = 7;
const FORMAT_INT64: c_int = 8;
const FORMAT_FLOAT: c_int = 9;
const FORMAT_DOUBLE: c_int = 10;

/// The newer interface's menu pages (`PagedUIPlugin.h`): a page's id, title and foot line.
#[repr(C)]
struct PageInfo {
    id: *const c_char,
    title: *const c_char,
    info: *const c_char,
}

/// One item of a page — here a heading or a paragraph of markdown — the settings' kinds (a union
/// as wide as its widest, the float setting's) left empty.
#[repr(C)]
struct ItemDef {
    kind: c_int,
    id: *const c_char,
    label: *const c_char,
    tooltip: *const c_char,
    flags: c_uint,
    storage: c_int,
    context: *mut c_void,
    settings: [u64; 6],
}

const ITEM_HEADER: c_int = 0;
const ITEM_MARKDOWN: c_int = 2;

/// A sheet of pages an add-on puts in VPX's menu, VPX asking for its pages and items as it shows
/// them. Its edit session is for values an add-on keeps itself, which this one has none of.
#[repr(C)]
struct SheetDef {
    version: c_int,
    id: *const c_char,
    name: *const c_char,
    description: *const c_char,
    context: *mut c_void,
    get_page_count: unsafe extern "C" fn(context: *mut c_void) -> c_uint,
    get_page_info: unsafe extern "C" fn(context: *mut c_void, index: c_uint, info: *mut PageInfo),
    get_item_count: unsafe extern "C" fn(context: *mut c_void, page: *const c_char) -> c_uint,
    get_items: unsafe extern "C" fn(context: *mut c_void, page: *const c_char, items: *mut ItemDef),
    begin_edit: Option<unsafe extern "C" fn(context: *mut c_void)>,
    commit_edit: Option<unsafe extern "C" fn(context: *mut c_void) -> c_int>,
    end_edit: Option<unsafe extern "C" fn(context: *mut c_void, committed: c_int)>,
    is_dirty: Option<unsafe extern "C" fn(context: *mut c_void) -> c_int>,
    is_root: c_int,
}

/// The menu pages' services, past their version number.
#[repr(C)]
struct PagedUiApi {
    register_sheet: unsafe extern "C" fn(endpoint: u32, sheet: *const SheetDef),
    unregister_sheet: unsafe extern "C" fn(endpoint: u32, id: *const c_char),
    request_sheet_refresh: unsafe extern "C" fn(endpoint: u32, id: *const c_char),
}

/// The page's own id, and its sheet's.
const PAGE_ID: &std::ffi::CStr = c"standing";
const SHEET_ID: &std::ffi::CStr = c"achievements";

/// The page as VPX last asked for it, its strings kept while VPX reads them: its title, its foot
/// line, each item's kind, id and text.
struct PageStrings {
    title: CString,
    info: CString,
    items: Vec<(c_int, CString, CString)>,
}

static PAGE: Mutex<Option<PageStrings>> = Mutex::new(None);

/// The page where the achievements stand now, its strings made for VPX.
fn page_strings() -> PageStrings {
    let mut page = None;
    with_addon(|a| page = a.following.as_ref().map(Following::page));
    let text = |s: String| CString::new(s.replace('\0', "")).unwrap_or_default();
    let Some(page) = page else {
        return PageStrings { title: text("Achievements".into()), info: CString::default(), items: vec![(ITEM_MARKDOWN, text("line0".into()), text("No achievements are followed in this game.".into()))] };
    };
    let items = page
        .lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| match line {
            PageLine::Heading(t) => (ITEM_HEADER, text(format!("line{i}")), text(t)),
            PageLine::Achievement(t) => (ITEM_MARKDOWN, text(format!("line{i}")), text(t)),
        })
        .collect();
    PageStrings { title: text("Achievements".into()), info: text(page.summary), items }
}

unsafe extern "C" fn sheet_page_count(_: *mut c_void) -> c_uint {
    1
}

unsafe extern "C" fn sheet_page_info(_: *mut c_void, _: c_uint, info: *mut PageInfo) {
    if info.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        let mut kept = PAGE.lock().unwrap_or_else(|e| e.into_inner());
        let page = kept.insert(page_strings());
        unsafe { *info = PageInfo { id: PAGE_ID.as_ptr(), title: page.title.as_ptr(), info: page.info.as_ptr() } };
    });
}

unsafe extern "C" fn sheet_item_count(_: *mut c_void, _: *const c_char) -> c_uint {
    std::panic::catch_unwind(|| {
        // the items read afresh, as VPX counts them before asking for them
        let mut kept = PAGE.lock().unwrap_or_else(|e| e.into_inner());
        kept.insert(page_strings()).items.len() as c_uint
    })
    .unwrap_or(0)
}

unsafe extern "C" fn sheet_items(_: *mut c_void, _: *const c_char, items: *mut ItemDef) {
    if items.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(|| {
        let kept = PAGE.lock().unwrap_or_else(|e| e.into_inner());
        let Some(page) = kept.as_ref() else { return };
        for (i, (kind, id, label)) in page.items.iter().enumerate() {
            let item = ItemDef { kind: *kind, id: id.as_ptr(), label: label.as_ptr(), tooltip: std::ptr::null(), flags: 0, storage: 0, context: std::ptr::null_mut(), settings: [0; 6] };
            unsafe { items.add(i).write(item) };
        }
    });
}

unsafe extern "C" fn sheet_commit(_: *mut c_void) -> c_int {
    0
}

/// Which of VPX's add-on interfaces an add-on is handed: build 5436's, or the newer one, whose
/// structures open with their version number (1) and whose messages end in `:1`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interface {
    Build5436,
    V1,
}

impl Interface {
    /// A message's name in this interface.
    fn name(self, name: &str) -> CString {
        CString::new(match self {
            Interface::Build5436 => name.to_string(),
            Interface::V1 => format!("{name}:1"),
        })
        .expect("no NUL in a message name")
    }
}

/// A backglass add-on's change of state: `E` a value the script set (`B2SSetData`), `B` a score
/// digit, `C` a player's score.
#[repr(C)]
struct B2sEvent {
    kind: u8,
    index: i32,
    value: i32,
}

const TYPE_UINT8: c_int = 0x0001;
const TYPE_INT64: c_int = 0x0080;
/// PinMAME's state groups (`PinMAMEPlugin.h`): the solenoids' several (1 to 0x10), lamps, switches,
/// the game state from its memory map.
const GROUP_MASK: u32 = 0xFF00;
const GROUP_SOLENOID: u32 = 0x0000;
const GROUP_LAMP: u32 = 0x0200;
const GROUP_SWITCH: u32 = 0x0400;
const GROUP_GAMESTATE: u32 = 0x0600;

/// How often the machine's game state is read, in milliseconds of the game's clock.
const GAME_EVERY_MS: u64 = 100;
/// How often a machine not yet found is looked for.
const LOOK_EVERY_MS: u64 = 1000;
/// How long a notice shows.
const NOTICE_MS: c_int = 5000;

/// How long the game's clock stands still, frames still coming, before it counts as paused.
const PAUSED_AFTER: std::time::Duration = std::time::Duration::from_millis(400);
/// How far into the game before a pause is looked for: its clock can stand still as it starts.
const PAUSES_FROM_MS: u64 = 2000;
/// How often the balls are looked at, and how far into a game they're first looked at (the
/// game's clock): the table's own script well started by then.
const BALLS_EVERY_MS: u64 = 250;
const BALLS_FROM_MS: u64 = 1000;
/// How long the standing shows.
const STANDING_MS: c_int = 6000;

/// Our chime, built in.
const OUR_CHIME: &[u8] = include_bytes!("../../player/assets/vpi/sounds/achievement.wav");

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "system" {
    fn PlaySoundW(sound: *const u16, module: *mut c_void, flags: u32) -> i32;
}

/// The sound playing, kept while Windows reads it.
static PLAYING: Mutex<Vec<u8>> = Mutex::new(Vec::new());

/// `AudioSrcId`: a source of sound an add-on offers VPX, mixed into one of VPX's outputs (here the
/// backglass's, `target` 0) with a level of its own in VPX's sound settings
/// (`[Player]` `AudioSource.<add-on>.Gain`).
#[repr(C)]
#[derive(Clone, Copy)]
struct AudioSrcId {
    id: CtlResId,
    override_id: CtlResId,
    name: *const c_char,
    desc: *const c_char,
    target: c_uint,
}

/// `GetAudioSrcMsg`: the sources asked for, Vulkan's way.
#[repr(C)]
struct GetAudioSrcMsg {
    max_entry_count: c_uint,
    count: c_uint,
    entries: *mut AudioSrcId,
}

/// `AudioUpdateMsg`: a stream of one of an add-on's sources begun with its first samples, more
/// samples for it, or (no samples) its end.
#[repr(C)]
struct AudioUpdateMsg {
    source_id: CtlResId,
    stream_id: CtlResId,
    channel_format: c_uint,
    sample_format: c_uint,
    sample_rate: f64,
    volume: f32,
    buffer_size: c_uint,
    buffer: *mut u8,
}

/// `MsgEndpointInfo`: what VPX knows of an add-on.
#[repr(C)]
struct EndpointInfo {
    id: *const c_char,
    name: *const c_char,
    description: *const c_char,
    author: *const c_char,
    version: *const c_char,
    link: *const c_char,
}

/// VPX's messages API, for the sound's levels, which VPX's sound callback reaches without the
/// add-on's lock.
struct LevelsApi(*const MsgPluginApi);

// VPX's own table of functions, for the life of the add-on
unsafe impl Send for LevelsApi {}

static LEVELS_API: Mutex<Option<LevelsApi>> = Mutex::new(None);

/// A C string VPX owns, as text.
fn text_of(p: *const c_char) -> Option<String> {
    (!p.is_null()).then(|| unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned()).filter(|t| !t.is_empty())
}

/// An add-on's own ID and name, by the number VPX gives it this session.
fn endpoint_facts(endpoint: u32) -> Option<(String, Option<String>)> {
    let guard = LEVELS_API.lock().unwrap_or_else(|e| e.into_inner());
    let msg = unsafe { &*guard.as_ref()?.0 };
    if msg.get_endpoint_info.is_null() {
        return None;
    }
    let get: unsafe extern "C" fn(u32, *mut EndpointInfo) = unsafe { std::mem::transmute(msg.get_endpoint_info) };
    let mut info = EndpointInfo { id: std::ptr::null(), name: std::ptr::null(), description: std::ptr::null(), author: std::ptr::null(), version: std::ptr::null(), link: std::ptr::null() };
    unsafe { get(endpoint, &mut info) };
    Some((text_of(info.id)?, text_of(info.name)))
}

/// An add-on's sound on its way to VPX's mixer: its level as the app's pause menu set it, this
/// add-on having been listening since before VPX was.
unsafe extern "C" fn on_audio_update(_: c_uint, _: *mut c_void, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let update = unsafe { &mut *data.cast::<AudioUpdateMsg>() };
        if let Some(level) = menu::level_of(update.source_id.endpoint_id) {
            update.volume *= level;
        }
    }));
}

/// The add-ons' sources of sound now, by add-on — its ID and its name for the source — leaving out
/// one another source takes the place of (a machine's own, under its AltSound pack).
fn sound_sources(msg: &MsgPluginApi, endpoint: u32, ask: c_uint) -> Vec<(String, String)> {
    const ROOM: usize = 32;
    let blank = AudioSrcId { id: CtlResId::default(), override_id: CtlResId::default(), name: std::ptr::null(), desc: std::ptr::null(), target: 0 };
    let mut room = [blank; ROOM];
    let mut asked = GetAudioSrcMsg { max_entry_count: ROOM as c_uint, count: 0, entries: room.as_mut_ptr() };
    unsafe { (msg.broadcast_msg)(endpoint, ask, (&raw mut asked).cast()) };
    let found = &room[..(asked.count as usize).min(ROOM)];
    let mut out: Vec<(String, String)> = Vec::new();
    for source in found {
        let replaced = found.iter().any(|other| other.override_id.endpoint_id == source.id.endpoint_id && other.override_id.res_id == source.id.res_id && other.override_id.endpoint_id != 0);
        if replaced {
            continue;
        }
        let Some((id, add_on_name)) = endpoint_facts(source.id.endpoint_id) else { continue };
        if out.iter().any(|(known, _)| *known == id) {
            continue;
        }
        let name = text_of(source.name).or(add_on_name).unwrap_or_else(|| id.clone());
        out.push((id, name));
    }
    out
}

/// Our source of sound in VPX: whether it's offered (the app's sound on, a game playing), and the
/// sound playing.
struct SoundState {
    msg: *const MsgPluginApi,
    endpoint: u32,
    src_changed: c_uint,
    update: c_uint,
    offered: bool,
    playing: Option<Playing>,
    next: u32,
}

/// A sound playing through VPX: its stream's number, its samples, and how many of its frames
/// VPX has been handed since it began.
struct Playing {
    stream: u32,
    samples: Vec<u8>,
    channels: u16,
    rate: u32,
    float: bool,
    fed: usize,
    since: std::time::Instant,
}

impl Playing {
    fn frame_bytes(&self) -> usize {
        usize::from(self.channels) * if self.float { 4 } else { 2 }
    }

    fn frames(&self) -> usize {
        self.samples.len() / self.frame_bytes()
    }
}

/// How far ahead of what's heard a sound is handed to VPX, whose streams from add-ons hold a
/// second at most (the newest code drops what's queued past that).
const SOUND_AHEAD: std::time::Duration = std::time::Duration::from_millis(500);

// Its pointers are VPX's, used on the thread VPX calls add-ons on; the mutex is for Rust's sake.
unsafe impl Send for SoundState {}

static SOUND: Mutex<Option<SoundState>> = Mutex::new(None);

/// Our source's number among this add-on's resources.
const SOUND_SOURCE: u32 = 1;

/// VPX asking the add-ons for their sources of sound: ours, while it's offered.
unsafe extern "C" fn on_get_audio_src(_: c_uint, _: *mut c_void, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(endpoint) = SOUND.lock().unwrap_or_else(|e| e.into_inner()).as_ref().filter(|s| s.offered).map(|s| s.endpoint) else { return };
        let ask = unsafe { &mut *(data as *mut GetAudioSrcMsg) };
        if !ask.entries.is_null() && ask.count < ask.max_entry_count {
            let ours = AudioSrcId {
                id: CtlResId { endpoint_id: endpoint, res_id: SOUND_SOURCE },
                override_id: CtlResId::default(),
                name: c"Achievements".as_ptr(),
                desc: c"The sound of each achievement earned".as_ptr(),
                target: 0,
            };
            unsafe { ask.entries.add(ask.count as usize).write(ours) };
        }
        ask.count += 1;
    }));
}

/// A game begun: our source offered to VPX where the app has sounds on, VPX told so (it asks the
/// add-ons for their sources again); none otherwise.
fn sound_start(on: bool) {
    let told = {
        let mut guard = SOUND.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        let was = s.offered;
        s.offered = on;
        s.playing = None;
        (on || was).then_some((s.msg, s.endpoint, s.src_changed))
    };
    if let Some((msg, endpoint, changed)) = told
        && !msg.is_null()
    {
        unsafe { ((*msg).broadcast_msg)(endpoint, changed, std::ptr::null_mut()) };
    }
}

/// A stream of ours told to VPX: `samples` (in `pcm`'s kind) for it — its first begin it — or its
/// end (none).
#[allow(clippy::too_many_arguments)]
fn send_stream(msg: *const MsgPluginApi, endpoint: u32, update: c_uint, stream: u32, channels: u16, rate: u32, float: bool, samples: Option<&[u8]>) {
    let mut out = AudioUpdateMsg {
        source_id: CtlResId { endpoint_id: endpoint, res_id: SOUND_SOURCE },
        stream_id: CtlResId { endpoint_id: endpoint, res_id: stream },
        channel_format: u32::from(channels == 2),
        sample_format: u32::from(float),
        sample_rate: f64::from(rate),
        volume: 1.0,
        buffer_size: samples.map_or(0, |s| s.len() as c_uint),
        // VPX copies the samples as it takes them
        buffer: samples.map_or(std::ptr::null_mut(), |s| s.as_ptr().cast_mut()),
    };
    unsafe { ((*msg).broadcast_msg)(endpoint, update, (&raw mut out).cast()) };
}

/// `pcm` played through VPX's own sound, mixed into its backglass output at VPX's volume and the
/// level its sound settings give our source, the one before stopped first; its first half second
/// handed over at once, the rest as it plays (`sound_feed`). Whether VPX took it: our source is
/// offered there.
fn play_in_vpx(pcm: &follow::Pcm) -> bool {
    let (msg, endpoint, update, before) = {
        let mut guard = SOUND.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut().filter(|s| s.offered && !s.msg.is_null()) else { return false };
        s.next += 1;
        let playing = Playing { stream: s.next, samples: pcm.samples.to_vec(), channels: pcm.channels, rate: pcm.rate, float: pcm.float, fed: 0, since: std::time::Instant::now() };
        let before = s.playing.replace(playing).map(|p| (p.stream, p.channels, p.rate, p.float));
        (s.msg, s.endpoint, s.update, before)
    };
    if let Some((stream, channels, rate, float)) = before {
        send_stream(msg, endpoint, update, stream, channels, rate, float, None);
    }
    sound_feed();
    true
}

/// The sound playing handed to VPX up to half a second past what's been heard of it; called each
/// frame.
fn sound_feed() {
    let due = {
        let mut guard = SOUND.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        let (msg, endpoint, update) = (s.msg, s.endpoint, s.update);
        let Some(p) = s.playing.as_mut() else { return };
        let ahead = p.since.elapsed() + SOUND_AHEAD;
        let to = ((ahead.as_secs_f64() * f64::from(p.rate)) as usize).min(p.frames());
        if to <= p.fed || msg.is_null() {
            return;
        }
        let fb = p.frame_bytes();
        let chunk = p.samples[p.fed * fb..to * fb].to_vec();
        p.fed = to;
        (msg, endpoint, update, p.stream, p.channels, p.rate, p.float, chunk)
    };
    let (msg, endpoint, update, stream, channels, rate, float, chunk) = due;
    send_stream(msg, endpoint, update, stream, channels, rate, float, Some(&chunk));
}

/// A game ended: the stream playing ended, our source no longer offered.
fn sound_end() {
    let open = {
        let mut guard = SOUND.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = guard.as_mut() else { return };
        s.offered = false;
        s.playing.take().map(|p| (s.msg, s.endpoint, s.update, p.stream, p.channels, p.rate, p.float))
    };
    if let Some((msg, endpoint, update, stream, channels, rate, float)) = open
        && !msg.is_null()
    {
        send_stream(msg, endpoint, update, stream, channels, rate, float, None);
    }
}

/// An achievement's sound — its pack's own (a WAV, or an Ogg Vorbis, MP3 or FLAC decoded into one),
/// else ours — at `gain` (the WAV's own where it's a kind not scaled): through VPX's own sound where
/// it takes it, else played by Windows; one at a time, the one before stopped first. Which played
/// it.
#[cfg(windows)]
fn play(chime: &Chime, gain: f64) -> &'static str {
    const SND_ASYNC: u32 = 0x0001;
    const SND_NODEFAULT: u32 = 0x0002;
    const SND_MEMORY: u32 = 0x0004;
    let bytes = match chime {
        Chime::File(f) => std::fs::read(f).ok().and_then(|b| follow::as_wav(&b)).and_then(|w| follow::scaled_wav(&w, gain)),
        Chime::Ours => None,
    };
    let Some(bytes) = bytes.or_else(|| follow::scaled_wav(OUR_CHIME, gain)) else { return "none" };
    if let Some(pcm) = follow::pcm_of(&bytes)
        && play_in_vpx(&pcm)
    {
        return "VPX";
    }
    let mut playing = PLAYING.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: stops what plays before its bytes are let go; the new bytes stay in `PLAYING` while
    // Windows plays them, until the next sound or the add-on's unloading stops them
    unsafe { PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0) };
    *playing = bytes;
    unsafe { PlaySoundW(playing.as_ptr().cast(), std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT) };
    "Windows"
}

#[cfg(not(windows))]
fn play(_chime: &Chime, _gain: f64) -> &'static str {
    "none"
}

/// Whatever sound plays, stopped.
fn stop_sound() {
    let mut playing = PLAYING.lock().unwrap_or_else(|e| e.into_inner());
    #[cfg(windows)]
    // SAFETY: stops what plays before its bytes are let go
    unsafe {
        PlaySoundW(std::ptr::null(), std::ptr::null_mut(), 0)
    };
    playing.clear();
}

/// A string VPX or another add-on owns, copied.
fn text(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

/// A card drawn for a window of one size, and its texture, once VPX has it.
struct Drawn {
    seq: u64,
    out: (u32, u32),
    geometry: popup::Geometry,
    picture: Option<popup::Picture>,
    texture: *mut c_void,
}

/// The cards: as the app asked for them, those waiting and the one showing (from its first frame),
/// each drawn on a thread of its own for the window's size, then handed to VPX as a texture; the
/// window's other renderers, in the order VPX asks them, which draw under a card; and when VPX last
/// asked ours to draw (the window shows, and ours comes first there).
struct PopupState {
    msg: *const MsgPluginApi,
    vpx: *const VpxPluginApi,
    endpoint: u32,
    get_renderer: c_uint,
    renderer_changed: c_uint,
    settings: Option<popup::Settings>,
    waiting: std::collections::VecDeque<popup::Card>,
    showing: Option<(popup::Card, u64, Option<std::time::Instant>)>,
    seq: u64,
    drawn: Vec<Drawn>,
    others: Vec<RendererDef>,
    asked_last: Option<std::time::Instant>,
    log: Option<std::path::PathBuf>,
}

// Its pointers are VPX's and other add-ons', used on the threads VPX calls add-ons on; the
// mutex is for Rust's sake.
unsafe impl Send for PopupState {}

static POPUP: Mutex<Option<PopupState>> = Mutex::new(None);
/// While we ask for a window's renderers ourselves: ours left out.
static COLLECTING: AtomicBool = AtomicBool::new(false);
/// Cards being drawn, on their threads: waited for before VPX lets the add-on go.
static DRAWING: AtomicUsize = AtomicUsize::new(0);

/// How recently VPX must have asked our renderer to draw for a card to show there.
const ASKED_WITHIN: std::time::Duration = std::time::Duration::from_millis(1500);

fn with_popup<R>(f: impl FnOnce(&mut PopupState) -> Option<R>) -> Option<R> {
    let mut guard = POPUP.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_mut().and_then(f)
}

impl PopupState {
    fn note(&self, line: &str) {
        let Some(file) = &self.log else { return };
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(f, "{line}");
        }
    }

    /// The window the cards show in, where VPX lets add-ons draw.
    fn window(&self) -> Option<c_int> {
        self.settings.as_ref().and_then(|s| s.screen.window())
    }

    fn forget_drawn(&mut self) {
        for d in self.drawn.drain(..) {
            if !d.texture.is_null() && !self.vpx.is_null() {
                unsafe { ((*self.vpx).delete_texture)(d.texture) };
            }
        }
    }

    /// The card showing now and how opaque, the next taken up as one ends.
    fn current(&mut self, now: std::time::Instant) -> Option<f32> {
        loop {
            if let Some((_, _, since)) = &self.showing {
                let Some(since) = since else { return Some(0.0) };
                if let Some(a) = popup::alpha_at(now.duration_since(*since).as_millis() as u64) {
                    return Some(a);
                }
                self.showing = None;
                self.forget_drawn();
            }
            let card = self.waiting.pop_front()?;
            self.seq += 1;
            self.showing = Some((card, self.seq, None));
        }
    }

    /// The card showing as drawn for a window `out`, with its texture: the drawing begun on a
    /// thread of its own where there's none yet, and its texture made once it's done.
    fn drawn_for(&mut self, out: (u32, u32)) -> Option<(*mut c_void, popup::Geometry, u32, u32)> {
        let seq = self.showing.as_ref()?.1;
        let Some(i) = self.drawn.iter().position(|d| d.seq == seq && d.out == out) else {
            let card = self.showing.as_ref()?.0.clone();
            // one size drawn at a time a card, the oldest let go past two
            let size = self.settings.as_ref().map_or(popup::MEDIUM, |s| s.size);
            let geometry = popup::geometry(out.0 as f32, out.1 as f32, size);
            if self.drawn.len() >= 2 {
                let old = self.drawn.remove(0);
                if !old.texture.is_null() && !self.vpx.is_null() {
                    unsafe { ((*self.vpx).delete_texture)(old.texture) };
                }
            }
            self.drawn.push(Drawn { seq, out, geometry, picture: None, texture: std::ptr::null_mut() });
            DRAWING.fetch_add(1, Ordering::AcqRel);
            let spawned = std::thread::Builder::new().name("vpi-card".into()).spawn(move || {
                let picture = popup::draw(&card, &geometry);
                with_popup(|p| {
                    if let Some(d) = p.drawn.iter_mut().find(|d| d.seq == seq && d.out == out) {
                        d.picture = Some(picture);
                    }
                    Some(())
                });
                DRAWING.fetch_sub(1, Ordering::AcqRel);
            });
            if spawned.is_err() {
                DRAWING.fetch_sub(1, Ordering::AcqRel);
            }
            return None;
        };
        let vpx = self.vpx;
        let d = &mut self.drawn[i];
        if d.texture.is_null() {
            let picture = d.picture.as_ref()?;
            unsafe { ((*vpx).update_texture)(&mut d.texture, picture.width as c_int, picture.height as c_int, TEXTURE_RGBA, picture.rgba.as_ptr().cast()) };
            if d.texture.is_null() {
                return None;
            }
            let line = format!("card shown: {} ({}×{} on a window {}×{})", self.showing.as_ref().map_or("", |s| s.0.name.as_str()), picture.width, picture.height, out.0, out.1);
            d.picture = None;
            let d = &self.drawn[i];
            let found = (d.texture, d.geometry, d.geometry.w as u32, d.geometry.h as u32);
            self.note(&line);
            return Some(found);
        }
        Some((d.texture, d.geometry, d.geometry.w as u32, d.geometry.h as u32))
    }
}

/// Our renderer, for one of VPX's windows (`window`, in its context).
fn our_renderer(window: c_int) -> RendererDef {
    RendererDef {
        id: RENDERER_ID.as_ptr(),
        name: c"Achievements popup".as_ptr(),
        description: c"A card for each achievement earned, over what this window shows".as_ptr(),
        context: window as isize as *mut c_void,
        render: Some(render_card),
    }
}

/// `def` put first among the renderers handed back, the others moved up one: with no priorities
/// set, VPX keeps them in this order, so it asks ours first.
///
/// # Safety
/// `ask.entries` points at room for `ask.max_entry_count` entries, where it isn't null.
unsafe fn put_first(ask: &mut GetRendererMsg, def: RendererDef) {
    if !ask.entries.is_null() && ask.count < ask.max_entry_count {
        unsafe {
            std::ptr::copy(ask.entries, ask.entries.add(1), ask.count as usize);
            ask.entries.write(def);
        }
    }
    ask.count += 1;
}

/// VPX asking the add-ons for a window's renderers: ours first, for the window the cards show in.
unsafe extern "C" fn on_get_renderer(_: c_uint, _: *mut c_void, data: *mut c_void) {
    if data.is_null() || COLLECTING.load(Ordering::Acquire) {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ask = unsafe { &mut *(data as *mut GetRendererMsg) };
        if with_popup(|p| p.window()) == Some(ask.window) {
            unsafe { put_first(ask, our_renderer(ask.window)) };
        }
    }));
}

/// The window's other renderers, asked for as VPX asks, in the order VPX gives them: their
/// priorities as the app read them, the highest first, those below 0 left out.
fn collect_others() {
    let Some((msg, endpoint, id, window, settings)) = with_popup(|p| Some((p.msg, p.endpoint, p.get_renderer, p.window()?, p.settings.clone()?))) else { return };
    if msg.is_null() {
        return;
    }
    let msg = unsafe { &*msg };
    COLLECTING.store(true, Ordering::Release);
    let mut ask = GetRendererMsg { window, max_entry_count: 0, count: 0, entries: std::ptr::null_mut() };
    unsafe { (msg.broadcast_msg)(endpoint, id, (&raw mut ask).cast()) };
    let mut entries = vec![RendererDef::default(); ask.count as usize];
    if !entries.is_empty() {
        let mut ask = GetRendererMsg { window, max_entry_count: entries.len() as c_uint, count: 0, entries: entries.as_mut_ptr() };
        unsafe { (msg.broadcast_msg)(endpoint, id, (&raw mut ask).cast()) };
        entries.truncate((ask.count as usize).min(entries.len()));
    }
    COLLECTING.store(false, Ordering::Release);
    let mut others: Vec<(RendererDef, i32)> = entries.into_iter().filter(|e| e.render.is_some() && text(e.id) != RENDERER_ID.to_string_lossy()).map(|e| (e, settings.priority_of(&text(e.id)))).collect();
    others.sort_by_key(|(_, p)| std::cmp::Reverse(*p));
    others.retain(|(_, p)| *p >= 0);
    let names: Vec<String> = others.iter().map(|(e, _)| text(e.id)).collect();
    with_popup(|p| {
        p.others = others.into_iter().map(|(e, _)| e).collect();
        p.note(&format!("under the cards: {names:?}"));
        Some(())
    });
}

/// A window's renderers changed (an add-on's came or went): ours asked for again.
unsafe extern "C" fn on_renderer_changed(_: c_uint, _: *mut c_void, _: *mut c_void) {
    let _ = std::panic::catch_unwind(collect_others);
}

/// VPX asking ours to draw: nothing while there's no card, so the window's own renderers draw as
/// ever; with one, they draw first, in VPX's order, then the card over them.
unsafe extern "C" fn render_card(ctx: *mut RenderContext, context: *mut c_void) -> c_int {
    if ctx.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ctx = unsafe { &mut *ctx };
        let _window = context as isize as c_int;
        let now = std::time::Instant::now();
        let (out_w, out_h) = (ctx.out_width, ctx.out_height);
        let found = with_popup(|p| {
            if ctx.is_2d != 0 {
                p.asked_last = Some(now);
            }
            if p.vpx.is_null() || !(out_w >= 32.0 && out_h >= 16.0 && out_w <= 16384.0 && out_h <= 16384.0) {
                return None;
            }
            p.current(now)?;
            let (texture, geometry, w, h) = p.drawn_for((out_w.round() as u32, out_h.round() as u32))?;
            // from its first frame shown
            if let Some((_, _, since @ None)) = &mut p.showing {
                *since = Some(now);
            }
            let alpha = p.current(now)?;
            Some((p.others.clone(), texture, geometry, w, h, alpha))
        });
        let Some((others, texture, g, w, h, alpha)) = found else { return 0 };
        for r in &others {
            if let Some(render) = r.render
                && unsafe { render(ctx, r.context) } != 0
            {
                break;
            }
        }
        ctx.src_width = out_w;
        ctx.src_height = out_h;
        unsafe { (ctx.draw_image)(ctx, texture, 1.0, 1.0, 1.0, alpha, 0.0, 0.0, w as f32, h as f32, 0.0, 0.0, 0.0, g.x, g.y, w as f32, h as f32) };
        1
    }))
    .unwrap_or(0)
}

/// Whether a card would show now: the app asked for them on a window VPX lets us draw in, and
/// VPX has asked ours to draw there lately.
fn popup_live() -> bool {
    with_popup(|p| Some(p.window().is_some() && p.asked_last.is_some_and(|t| t.elapsed() < ASKED_WITHIN))).unwrap_or(false)
}

fn popup_show(cards: Vec<popup::Card>) {
    with_popup(|p| {
        p.waiting.extend(cards);
        Some(())
    });
}

/// A game begun: the cards as the app asked for them, and VPX told our renderer is there, which
/// has it ask the add-ons for their renderers again.
fn popup_start(settings: Option<popup::Settings>, log: Option<std::path::PathBuf>) {
    let Some((msg, endpoint, changed)) = with_popup(|p| {
        p.forget_drawn();
        p.waiting.clear();
        p.showing = None;
        p.others.clear();
        p.asked_last = None;
        p.settings = settings;
        p.log = log;
        p.window()?;
        Some((p.msg, p.endpoint, p.renderer_changed))
    }) else {
        return;
    };
    if !msg.is_null() {
        unsafe { ((*msg).broadcast_msg)(endpoint, changed, std::ptr::null_mut()) };
    }
    collect_others();
}

/// A game ended: what's waiting or showing let go, with its textures.
fn popup_end() {
    with_popup(|p| {
        p.forget_drawn();
        p.waiting.clear();
        p.showing = None;
        p.others.clear();
        p.settings = None;
        Some(())
    });
}

/// The message IDs this add-on uses.
#[derive(Default)]
struct Ids {
    get_api: c_uint,
    game_start: c_uint,
    game_end: c_uint,
    physics: c_uint,
    frame: c_uint,
    state_src_changed: c_uint,
    get_state_src: c_uint,
    b2s_change: c_uint,
    /// The menu pages' services, in the newer interface alone. (VPX numbers its messages from 0,
    /// its own `GetAPI` the first, so 0 is an id like any other: one not asked for is none.)
    paged_ui: Option<c_uint>,
    /// VPX asking for a window's renderers, and the add-ons telling theirs changed.
    get_renderer: c_uint,
    renderer_changed: c_uint,
    /// VPX asking the add-ons for their sources of sound, their telling them changed, a stream's
    /// samples.
    get_audio_src: c_uint,
    audio_src_changed: c_uint,
    audio_update: c_uint,
    /// VPX's scriptable interface, in the newer interface alone.
    get_scriptable: Option<c_uint>,
    /// The controllers running (`GetControllers`), the newer interface's alone: the machine's set,
    /// where the add-on plays on its own.
    get_controllers: Option<c_uint>,
    /// An action pressed or let go (`OnActionChanged`), the newer interface's alone: VPX's own
    /// menu's, kept from VPX for the app's pause menu (`menu`).
    action_changed: Option<c_uint>,
}

impl Ids {
    /// Every id asked for, each once, to give back as the add-on unloads: 0 among them where VPX
    /// gave it (its own `GetAPI`'s).
    fn all(&self) -> impl Iterator<Item = c_uint> {
        let asked = [
            self.get_api,
            self.game_start,
            self.game_end,
            self.physics,
            self.frame,
            self.state_src_changed,
            self.get_state_src,
            self.b2s_change,
            self.get_renderer,
            self.renderer_changed,
            self.get_audio_src,
            self.audio_src_changed,
            self.audio_update,
        ];
        asked.into_iter().chain(self.paged_ui).chain(self.get_scriptable).chain(self.get_controllers).chain(self.action_changed)
    }
}

/// How one of the machine's states is read: build 5436's one reader for the whole source, by the
/// state's index; or the newer interface's reader of the state's own, in its data format.
#[derive(Clone, Copy)]
enum Reader {
    Indexed { get: StateFn, index: c_uint },
    Own { get: StateFnV1, context: *mut c_void, format: c_int },
}

impl Reader {
    /// The state as a number: None where it can't be read, or isn't one.
    fn number(&self) -> Option<i64> {
        match *self {
            Reader::Indexed { get, index } => {
                let mut n: i64 = 0;
                (unsafe { get(index, TYPE_INT64, (&raw mut n).cast()) } == 0).then_some(n)
            }
            Reader::Own { get, context, format } => {
                // room for the widest number, its low bytes first as on x86
                let mut raw: u64 = 0;
                unsafe { get(context, (&raw mut raw).cast()) };
                Some(match format {
                    FORMAT_UINT8 => i64::from(raw as u8),
                    FORMAT_UINT16 => i64::from(raw as u16),
                    FORMAT_UINT32 => i64::from(raw as u32),
                    FORMAT_UINT64 | FORMAT_INT64 => raw as i64,
                    FORMAT_INT8 => i64::from(raw as u8 as i8),
                    FORMAT_INT16 => i64::from(raw as u16 as i16),
                    FORMAT_INT32 => i64::from(raw as u32 as i32),
                    FORMAT_FLOAT => f32::from_bits(raw as u32) as i64,
                    FORMAT_DOUBLE => f64::from_bits(raw) as i64,
                    _ => return None,
                })
            }
        }
    }

    /// A lamp, switch or solenoid: on or off.
    fn on(&self) -> Option<bool> {
        match *self {
            Reader::Indexed { get, index } => {
                let mut v: u8 = 0;
                (unsafe { get(index, TYPE_UINT8, (&raw mut v).cast()) } == 0).then_some(v != 0)
            }
            Reader::Own { get, context, format } if format == FORMAT_FLOAT || format == FORMAT_DOUBLE => {
                let mut raw: u64 = 0;
                unsafe { get(context, (&raw mut raw).cast()) };
                let level = if format == FORMAT_FLOAT { f64::from(f32::from_bits(raw as u32)) } else { f64::from_bits(raw) };
                Some(level > 0.0)
            }
            Reader::Own { .. } => self.number().map(|n| n != 0),
        }
    }

    /// The state as a number with its fractions: a table light's, from 0, off, to 1, on, and 2
    /// while it blinks.
    fn level(&self) -> Option<f64> {
        match *self {
            Reader::Own { get, context, format } if format == FORMAT_FLOAT || format == FORMAT_DOUBLE => {
                let mut raw: u64 = 0;
                unsafe { get(context, (&raw mut raw).cast()) };
                Some(if format == FORMAT_FLOAT { f64::from(f32::from_bits(raw as u32)) } else { f64::from_bits(raw) })
            }
            _ => self.number().map(|n| n as f64),
        }
    }
}

/// The game's states read here: each game state value of the machine, its key and its place among
/// its key's; each output wanted; the scores of its high score table; each of the table's lights
/// wanted, by its name in the table.
struct Source {
    game: Vec<(Reader, String, usize)>,
    outputs: Vec<(Reader, char, u32)>,
    high: Vec<Reader>,
    lights: Vec<(Reader, String)>,
}

struct Addon {
    /// VPX's interface, past its version number where it has one.
    msg: *const MsgPluginApi,
    vpx: *const VpxPluginApi,
    iface: Interface,
    endpoint: u32,
    ids: Ids,
    /// VPX's menu pages, where it has them, and whether ours is in its menu now.
    paged_ui: *const PagedUiApi,
    page_in_menu: bool,
    following: Option<Following>,
    source: Option<Source>,
    next_look: u64,
    next_game: u64,
    log: Option<std::path::PathBuf>,
    /// The game's clock at the last frame, since when it's stood still, and whether the standing
    /// was shown for this pause.
    clock: u64,
    still_since: Option<std::time::Instant>,
    standing_shown: bool,
    /// Screenshots on a key, where the app asks for them.
    shots: Option<shots::Shots>,
    /// The machine's high scores as last read, and when they were first read.
    high_seen: Option<Vec<i64>>,
    high_since: u64,
    /// For checks, where `VPI_ACHIEVEMENTS_CARD_AT_MS` asks: a card shown once the game's clock
    /// reaches it, as if its pack's first achievement were earned.
    card_check_at: Option<u64>,
    /// A script table's game as its backglass tells it, for a screenshot at a new high score:
    /// whether a game's on, and the best score of it.
    b2s_playing: Option<bool>,
    b2s_best: u64,
    /// The newer interface's own: a line run in the table's script (`VpxPluginApiV1`), VPX's
    /// scriptable interface, and our script object's class in it (`balls`).
    run_script: Option<unsafe extern "C" fn(*const c_char)>,
    scriptable: *const balls::ScriptableApi,
    ball_class: Option<*mut balls::ClassDef>,
    /// The balls in play, as counted; whether our routine is in the table's script, when they're
    /// next looked at, and the count last told.
    balls: balls::InPlay,
    balls_put_in: bool,
    next_balls: u64,
    balls_told: Option<u32>,
    /// For checks: lines still to run in the table's script (`check_lines`).
    check_lines: Vec<(u64, CString)>,
    /// On its own: since when (the game's clock) the machine's set is waited for, the next look,
    /// and a game begun late, its card's settings and sound for VPX outside the lock.
    own_wait: Option<u64>,
    own_next: u64,
    late: Option<(Option<popup::Settings>, bool)>,
    /// A hardcore game's settings files, watched for what makes a game softcore being changed in
    /// VPX's menu during it (`hardcore::Watch`).
    hardcore: Option<hardcore::Watch>,
}

// Only ever used on the thread VPX calls its add-ons on; the mutex is for Rust's sake.
unsafe impl Send for Addon {}

static ADDON: Mutex<Option<Addon>> = Mutex::new(None);

/// The add-on handed to `f`, every panic caught. A call that comes while another is under way (VPX
/// telling something while we ask it something) is let go, rather than waiting on itself.
fn with_addon(f: impl FnOnce(&mut Addon)) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = match ADDON.try_lock() {
            Ok(g) => g,
            Err(std::sync::TryLockError::Poisoned(p)) => p.into_inner(),
            Err(std::sync::TryLockError::WouldBlock) => return,
        };
        if let Some(a) = guard.as_mut() {
            f(a);
        }
    }));
}

impl Addon {
    fn note(&self, line: &str) {
        let Some(file) = &self.log else { return };
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(file) {
            let _ = writeln!(f, "{line}");
        }
    }

    fn now(&self) -> u64 {
        if self.vpx.is_null() {
            return 0;
        }
        let secs = unsafe { ((*self.vpx).get_game_time)() };
        (secs.max(0.0) * 1000.0) as u64
    }

    fn notify(&self, text: &str) {
        self.notify_for(text, NOTICE_MS);
    }

    fn notify_for(&self, text: &str, ms: c_int) {
        self.note(&format!("notice: {text}"));
        if self.vpx.is_null() {
            return;
        }
        if let Ok(c) = CString::new(text) {
            unsafe { ((*self.vpx).push_notification)(c.as_ptr(), ms) };
        }
    }

    /// The notices due shown — as cards of our own where they show, else VPX's notices — and the
    /// sound due played.
    /// For checks: the lines due by `now` run in the table's script (`VPI_ACHIEVEMENTS_CHECK_SCRIPT`).
    fn run_check_lines(&mut self, now: u64) {
        let Some(run) = self.run_script else { return };
        while self.check_lines.first().is_some_and(|l| l.0 <= now) {
            let (_, line) = self.check_lines.remove(0);
            unsafe { run(line.as_ptr()) };
            self.note(&format!("{now} ms: ran {} (a check)", line.to_string_lossy()));
        }
    }

    /// The balls in play looked at, for achievements counting them, in a VPX whose add-ons can run
    /// a line of the table's script: our routine put in it once the game's begun, then run every
    /// quarter second once it's said it's there, a change in the count told.
    fn look_at_balls(&mut self, now: u64) {
        let (Some(run), Some(_)) = (self.run_script, self.ball_class) else { return };
        if now < BALLS_FROM_MS || !self.following.as_ref().is_some_and(Following::wants_balls) {
            return;
        }
        if !self.balls_put_in {
            self.balls_put_in = true;
            unsafe { run(balls::ROUTINE.as_ptr()) };
            let there = balls::ready();
            self.note(&format!("balls: our routine put in the table's script: {}", if there { "it's there" } else { "it didn't say it's there, so the balls aren't followed" }));
            return;
        }
        if now < self.next_balls || !balls::ready() {
            return;
        }
        self.next_balls = now + BALLS_EVERY_MS;
        unsafe { run(balls::TICK.as_ptr()) };
        let Some(list) = balls::take() else { return };
        let on_table = balls::parse(&list);
        let count = self.balls.count(&on_table, now);
        if self.balls_told == Some(count) {
            return;
        }
        if self.balls_told.is_none() {
            self.note(&format!("balls: first look: {list}"));
        }
        self.balls_told = Some(count);
        self.note(&format!("{now} ms: balls: {} on the table, {count} in play", on_table.len()));
        let notices = self.following.as_mut().map(|f| f.balls_in_play(count, now)).unwrap_or_default();
        self.tell(notices);
    }

    fn tell(&mut self, notices: Vec<String>) {
        let Some(f) = &mut self.following else { return };
        let told = f.take_told();
        let chime = f.chime_due.take().map(|c| (c, f.gain));
        let cards = f.take_cards();
        for t in told {
            self.note(&t);
        }
        if !cards.is_empty() && popup_live() {
            for c in &cards {
                self.note(&format!("card: {} · {}", c.kicker, c.name));
            }
            popup_show(cards);
        } else {
            for n in notices {
                self.notify(&n);
            }
        }
        if let Some((chime, gain)) = chime {
            let by = play(&chime, gain);
            self.note(&format!("sound: {chime:?}, played by {by}"));
        }
    }

    /// PinMAME's state source, looked for among the controllers': its game state values and the
    /// outputs the achievements want.
    fn look(&mut self) {
        if self.iface == Interface::V1 {
            self.look_v1();
            return;
        }
        let wanted = self.following.as_ref().map(Following::outputs_wanted).unwrap_or_default();
        let msg = unsafe { &*self.msg };
        let mut ask = GetStateSrcMsg { max_entry_count: 0, count: 0, entries: std::ptr::null_mut() };
        unsafe { (msg.broadcast_msg)(self.endpoint, self.ids.get_state_src, (&raw mut ask).cast()) };
        if ask.count == 0 {
            return;
        }
        let mut entries = vec![StateSrcId::default(); ask.count as usize];
        let mut ask = GetStateSrcMsg { max_entry_count: entries.len() as c_uint, count: 0, entries: entries.as_mut_ptr() };
        unsafe { (msg.broadcast_msg)(self.endpoint, self.ids.get_state_src, (&raw mut ask).cast()) };
        let pinmame = unsafe { (msg.get_plugin_endpoint)(c"PinMAME".as_ptr()) };
        let filled = (ask.count as usize).min(entries.len());
        for e in &entries[..filled] {
            if pinmame != 0 && e.id.endpoint_id != pinmame {
                continue;
            }
            let Some(get_state) = e.get_state else { continue };
            if e.state_defs.is_null() || e.n_states == 0 {
                continue;
            }
            let defs = unsafe { std::slice::from_raw_parts(e.state_defs, e.n_states as usize) };
            let mut game: Vec<(Reader, String, usize)> = Vec::new();
            let mut outputs = Vec::new();
            let mut high = Vec::new();
            for (i, d) in defs.iter().enumerate() {
                let reader = Reader::Indexed { get: get_state, index: i as c_uint };
                let group = d.id.group_id & GROUP_MASK;
                if group == GROUP_GAMESTATE && d.type_mask & TYPE_INT64 != 0 {
                    let path = text(d.desc);
                    if let Some(key) = game_state_key(&path) {
                        let slot = game.iter().filter(|(_, k, _)| k == key).count();
                        game.push((reader, key.to_string(), slot));
                    } else if path.starts_with("high_scores\\score") {
                        high.push(reader);
                    }
                    continue;
                }
                let kind = match group {
                    GROUP_LAMP => 'L',
                    GROUP_SWITCH => 'W',
                    GROUP_SOLENOID if d.id.group_id != 0 => 'S',
                    _ => continue,
                };
                let number = d.id.state_id;
                if wanted.contains(&(kind, number)) && d.type_mask & TYPE_UINT8 != 0 {
                    outputs.push((reader, kind, number));
                }
            }
            if game.is_empty() && outputs.is_empty() && high.is_empty() {
                continue;
            }
            self.found(Source { game, outputs, high, lights: Vec::new() }, wanted.len());
            return;
        }
    }

    /// The newer interface's way: PinMAME's state groups, each a source of its own — its memory
    /// map's game state, VPinMAME's lamps and solenoids as VPinMAME numbers them, the switches —
    /// and VPX's own group of the table's lights (VPX from build 6148), each by its name in the
    /// table, those the achievements read.
    fn look_v1(&mut self) {
        let wanted = self.following.as_ref().map(Following::outputs_wanted).unwrap_or_default();
        let lights_wanted = self.following.as_ref().map(Following::lights_wanted).unwrap_or_default();
        let msg = unsafe { &*self.msg };
        let mut ask = GetStateSrcMsgV1 { max_entry_count: 0, count: 0, entries: std::ptr::null_mut() };
        unsafe { (msg.broadcast_msg)(self.endpoint, self.ids.get_state_src, (&raw mut ask).cast()) };
        if ask.count == 0 {
            return;
        }
        let mut entries = vec![StateSrcIdV1::default(); ask.count as usize];
        let mut ask = GetStateSrcMsgV1 { max_entry_count: entries.len() as c_uint, count: 0, entries: entries.as_mut_ptr() };
        unsafe { (msg.broadcast_msg)(self.endpoint, self.ids.get_state_src, (&raw mut ask).cast()) };
        let pinmame = unsafe { (msg.get_plugin_endpoint)(c"PinMAME".as_ptr()) };
        let vpx = unsafe { (msg.get_plugin_endpoint)(c"vpx".as_ptr()) };
        let filled = (ask.count as usize).min(entries.len());
        let mut game: Vec<(Reader, String, usize)> = Vec::new();
        let mut outputs = Vec::new();
        let mut high = Vec::new();
        let mut lights = Vec::new();
        for e in &entries[..filled] {
            // VPX's own: the table's lights, each light's state (0 off to 1 on, 2 blinking)
            if vpx != 0 && e.id.endpoint_id == vpx {
                if text(e.name) == "Lights" && !e.state_defs.is_null() && !lights_wanted.is_empty() {
                    let defs = unsafe { std::slice::from_raw_parts(e.state_defs, e.n_states as usize) };
                    for d in defs {
                        let Some(get) = d.get_state else { continue };
                        let name = text(d.name);
                        if lights_wanted.iter().any(|w| w.eq_ignore_ascii_case(&name)) {
                            lights.push((Reader::Own { get, context: d.call_context, format: d.data_format }, name));
                        }
                    }
                }
                continue;
            }
            if pinmame != 0 && e.id.endpoint_id != pinmame || e.state_defs.is_null() || e.n_states == 0 {
                continue;
            }
            let group = text(e.name);
            let kind = match group.as_str() {
                "Game States" => None,
                "VPinMAME Lamps" => Some('L'),
                "Switches" => Some('W'),
                "VPinMAME Solenoids" => Some('S'),
                _ => continue,
            };
            let defs = unsafe { std::slice::from_raw_parts(e.state_defs, e.n_states as usize) };
            for d in defs {
                let Some(get) = d.get_state else { continue };
                let reader = Reader::Own { get, context: d.call_context, format: d.data_format };
                match kind {
                    None => {
                        if reader.number().is_none() {
                            continue;
                        }
                        let path = text(d.desc);
                        if let Some(key) = game_state_key(&path) {
                            let slot = game.iter().filter(|(_, k, _)| k == key).count();
                            game.push((reader, key.to_string(), slot));
                        } else if path.starts_with("high_scores\\score") {
                            high.push(reader);
                        }
                    }
                    Some(kind) => {
                        if wanted.contains(&(kind, d.mapping_id)) {
                            outputs.push((reader, kind, d.mapping_id));
                        }
                    }
                }
            }
        }
        if game.is_empty() && outputs.is_empty() && high.is_empty() && lights.is_empty() {
            return;
        }
        self.found(Source { game, outputs, high, lights }, wanted.len());
    }

    /// Our page put in VPX's menu (its add-on settings' page for us), where VPX takes add-ons'
    /// pages; VPX asks for its lines each time it shows it.
    fn show_page(&mut self) {
        if self.paged_ui.is_null() || self.page_in_menu {
            return;
        }
        let sheet = SheetDef {
            version: 1,
            id: SHEET_ID.as_ptr(),
            name: c"Achievements".as_ptr(),
            description: c"Where this table's achievements stand".as_ptr(),
            context: std::ptr::null_mut(),
            get_page_count: sheet_page_count,
            get_page_info: sheet_page_info,
            get_item_count: sheet_item_count,
            get_items: sheet_items,
            begin_edit: None,
            commit_edit: Some(sheet_commit),
            end_edit: None,
            is_dirty: None,
            is_root: 1,
        };
        unsafe { ((*self.paged_ui).register_sheet)(self.endpoint, &sheet) };
        self.page_in_menu = true;
        self.note("menu page: in VPX's menu");
    }

    /// Our page taken out of VPX's menu.
    fn hide_page(&mut self) {
        if self.paged_ui.is_null() || !self.page_in_menu {
            return;
        }
        unsafe { ((*self.paged_ui).unregister_sheet)(self.endpoint, SHEET_ID.as_ptr()) };
        self.page_in_menu = false;
    }

    /// A game's instructions where no app hands them over (`standalone`): the table VPX plays, the
    /// packs found for it, VPX's preferences folder for what's earned, the add-on's own settings
    /// for the sound and the card. The newer interface alone; none where no pack fits the table.
    fn on_its_own(&mut self, rom: Option<String>) -> Option<Asked> {
        if self.iface != Interface::V1 || self.vpx.is_null() {
            return None;
        }
        let vpx = unsafe { &*self.vpx };
        if vpx.get_table_info.is_null() || vpx.get_vpx_info.is_null() {
            return None;
        }
        let table_info: unsafe extern "C" fn(*mut VpxTableInfo) = unsafe { std::mem::transmute(vpx.get_table_info) };
        let vpx_info: unsafe extern "C" fn(*mut VpxInfo) = unsafe { std::mem::transmute(vpx.get_vpx_info) };
        let mut t = VpxTableInfo { path: std::ptr::null(), table_width: 0.0, table_height: 0.0 };
        unsafe { table_info(&raw mut t) };
        let table = native_path(t.path)?;
        let mut v = VpxInfo { path: std::ptr::null(), pref_path: std::ptr::null() };
        unsafe { vpx_info(&raw mut v) };
        let prefs = native_path(v.pref_path)?;
        let card = standalone::Card {
            on: match OWN_CARD.load(Ordering::Relaxed) {
                0 => None,
                2 => Some("scoreview"),
                _ => Some("backglass"),
            },
            size: match OWN_CARD_SIZE.load(Ordering::Relaxed) {
                0 => "small",
                2 => "large",
                _ => "medium",
            },
        };
        let asked = standalone::asked(&table, &prefs, rom.as_deref(), OWN_SOUND.load(Ordering::Relaxed), card);
        let machine = rom.as_deref().unwrap_or("none");
        match &asked {
            Some(a) => self.note(&format!("on its own: {} pack(s) for {}, {}{}; its machine {machine}", a.packs.len(), table.display(), a.mode, a.why.as_deref().map(|w| format!(" ({w})")).unwrap_or_default())),
            None => self.note(&format!("on its own: no pack for {}; its machine {machine}", table.display())),
        }
        asked
    }

    /// A game followed from its instructions: what's earned watched (a hardcore game's settings
    /// files too), the menu's page put in, the card said; the card's settings and whether sounds
    /// play returned, for VPX to be offered our renderer and our source of sound outside the lock.
    fn begin(&mut self, asked: Asked) -> (Option<popup::Settings>, bool) {
        let cards = asked.popup.clone();
        let sounds = asked.sound_on && asked.gain > 0.0;
        // a hardcore game's settings files, as they stand as play begins
        if asked.mode == "hardcore" && !asked.trying && !asked.hardcore_files.is_empty() {
            self.hardcore = Some(hardcore::Watch::new(&asked.hardcore_files));
        }
        let (mut f, card) = Following::start(asked);
        f.telling = self.log.is_some();
        self.note(&format!("started: {} lamps, switches and solenoids wanted, {} lights", f.outputs_wanted().len(), f.lights_wanted().len()));
        self.following = Some(f);
        self.show_page();
        if let Some(card) = card {
            self.notify(&card);
        }
        (cards, sounds)
    }

    /// The ROM set of the machine running, where one is: PinMAME's controller's game, as VPX's
    /// controllers list names it (`namespace::game`).
    fn controller_rom(&self) -> Option<String> {
        let id = self.ids.get_controllers?;
        let msg = unsafe { &*self.msg };
        let pinmame = unsafe { (msg.get_plugin_endpoint)(c"PinMAME".as_ptr()) };
        let mut ask = GetControllersMsg { max_entry_count: 0, count: 0, entries: std::ptr::null_mut() };
        unsafe { (msg.broadcast_msg)(self.endpoint, id, (&raw mut ask).cast()) };
        if ask.count == 0 {
            return None;
        }
        let mut entries = vec![ControllerDef { endpoint_id: 0, game_id: std::ptr::null() }; ask.count as usize];
        let mut ask = GetControllersMsg { max_entry_count: entries.len() as c_uint, count: 0, entries: entries.as_mut_ptr() };
        unsafe { (msg.broadcast_msg)(self.endpoint, id, (&raw mut ask).cast()) };
        let filled = (ask.count as usize).min(entries.len());
        entries[..filled]
            .iter()
            .filter(|e| pinmame == 0 || e.endpoint_id == pinmame)
            .map(|e| text(e.game_id))
            .map(|g| g.rsplit("::").next().unwrap_or_default().trim().to_string())
            .find(|g| !g.is_empty())
    }

    fn found(&mut self, source: Source, wanted: usize) {
        if !(source.game.is_empty() && source.outputs.is_empty() && source.high.is_empty()) {
            self.note(&format!("machine: {} game state values, {} of {wanted} outputs wanted, {} high scores", source.game.len(), source.outputs.len(), source.high.len()));
        }
        if !source.lights.is_empty() {
            self.note(&format!("lights: {} of the table's read: {}", source.lights.len(), source.lights.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>().join(", ")));
        }
        self.source = Some(source);
    }

    /// Each physics update: the outputs wanted read, the game state every so often.
    fn update(&mut self) {
        if let Some(since) = self.own_wait {
            let now = self.now();
            if now >= self.own_next {
                self.own_next = now + OWN_LOOK_EVERY_MS;
                let rom = self.controller_rom();
                if rom.is_some() || now.saturating_sub(since) >= OWN_WAIT_MS {
                    self.own_wait = None;
                    if let Some(asked) = self.on_its_own(rom) {
                        self.late = Some(self.begin(asked));
                    }
                }
            }
        }
        let high_wanted = self.shots.as_ref().is_some_and(|s| s.at_high_score);
        if self.following.is_none() && !high_wanted {
            return;
        }
        let now = self.now();
        if self.source.is_none() {
            if now >= self.next_look {
                self.next_look = now + LOOK_EVERY_MS;
                self.look();
            }
            if self.source.is_none() {
                return;
            }
        }
        let Some(src) = &self.source else { return };
        let mut notices = Vec::new();
        if let Some(f) = &mut self.following {
            for &(reader, kind, number) in &src.outputs {
                if let Some(on) = reader.on() {
                    notices.extend(f.output(kind, number, on, now));
                }
            }
            for (reader, name) in &src.lights {
                if let Some(level) = reader.level() {
                    notices.extend(f.light(name, level, now));
                }
            }
        }
        let mut new_high = false;
        if now >= self.next_game {
            self.next_game = now + GAME_EVERY_MS;
            if let Some(f) = &mut self.following
                && !src.game.is_empty()
            {
                let values: Vec<Value> = src.game.iter().filter_map(|(reader, key, slot)| reader.number().map(|n| Value { key: key.clone(), index: *slot, number: n })).collect();
                if !values.is_empty() {
                    notices.extend(f.game_values(&values, now));
                }
            }
            // the machine's high scores changed after its start-up: a game made them
            if high_wanted && !src.high.is_empty() {
                let scores: Vec<i64> = src.high.iter().map(|r| r.number().unwrap_or(0)).collect();
                match &self.high_seen {
                    None => {
                        self.high_seen = Some(scores);
                        self.high_since = now;
                    }
                    Some(before) if *before != scores => {
                        self.high_seen = Some(scores);
                        new_high = now >= self.high_since + follow::SETTLING_MS;
                    }
                    _ => {}
                }
            }
        }
        if new_high && let Some(shots) = &self.shots {
            shots.take_now();
            self.notify_for("Screenshot saved: a new high score", 3000);
        }
        if self.following.is_some() {
            self.tell(notices);
        }
    }
}

unsafe extern "C" fn on_game_start(_: c_uint, _: *mut c_void, _: *mut c_void) {
    let mut cards = None;
    let mut log = None;
    let mut sounds = false;
    with_addon(|a| {
        a.following = None;
        a.source = None;
        a.next_look = 0;
        a.next_game = 0;
        a.clock = 0;
        a.still_since = None;
        a.standing_shown = false;
        a.shots = shots::Shots::from_env();
        a.high_seen = None;
        a.high_since = 0;
        a.log = std::env::var_os("VPI_ACHIEVEMENTS_LOG").filter(|v| !v.is_empty()).map(Into::into);
        a.card_check_at = std::env::var("VPI_ACHIEVEMENTS_CARD_AT_MS").ok().and_then(|v| v.trim().parse().ok());
        a.b2s_playing = None;
        a.b2s_best = 0;
        a.balls = balls::InPlay::default();
        a.balls_put_in = false;
        a.next_balls = 0;
        a.balls_told = None;
        a.hardcore = None;
        a.own_wait = None;
        a.own_next = 0;
        a.late = None;
        a.check_lines = if a.run_script.is_some() { std::env::var("VPI_ACHIEVEMENTS_CHECK_SCRIPT").map(|v| check_lines(&v)).unwrap_or_default() } else { Vec::new() };
        balls::reset();
        log = a.log.clone();
        // the app's pause menu, where it asks for it: VPX's menu key kept for it (`menu`)
        if let Ok(mut m) = menu::MENU.lock() {
            *m = None;
            let port = std::env::var("VPI_MENU_PORT").ok().and_then(|p| p.trim().parse::<u16>().ok());
            if let (Some(port), Interface::V1, false) = (port, a.iface, a.vpx.is_null()) {
                let (get, set) = unsafe { ((*a.vpx).get_input_state, (*a.vpx).set_input_state) };
                if !get.is_null() && !set.is_null() {
                    let (get, set): (menu::InputFn, menu::InputFn) = unsafe { (std::mem::transmute(get), std::mem::transmute(set)) };
                    *m = menu::Menu::connect(port, get, set, a.run_script, |endpoint| endpoint_facts(endpoint).map(|f| f.0));
                    a.note(if m.is_some() { "the app's pause menu on VPX's menu key" } else { "the app's pause menu couldn't be reached" });
                }
            }
        }
        let Some(file) = std::env::var_os("VPI_ACHIEVEMENTS").filter(|f| !f.is_empty()) else {
            // on its own: the machine's set waited for a moment (VPX lists a machine once it runs),
            // then the table's packs found here (`update`)
            if a.iface == Interface::V1 {
                a.own_wait = Some(a.now());
            }
            return;
        };
        match std::fs::read(&file).map_err(|e| e.to_string()).and_then(|b| Asked::read(&b)) {
            Ok(asked) => (cards, sounds) = a.begin(asked),
            Err(e) => a.note(&format!("the packs couldn't be read: {e}")),
        }
    });
    // outside the add-on's own lock: VPX asks for our renderer and our source of sound at once
    let _ = std::panic::catch_unwind(|| popup_start(cards, log));
    let _ = std::panic::catch_unwind(|| sound_start(sounds));
}

unsafe extern "C" fn on_game_end(_: c_uint, _: *mut c_void, _: *mut c_void) {
    if let Ok(mut m) = menu::MENU.lock() {
        *m = None;
    }
    let _ = std::panic::catch_unwind(popup_end);
    let _ = std::panic::catch_unwind(sound_end);
    with_addon(|a| {
        a.hide_page();
        if let Some(mut f) = a.following.take() {
            f.finish();
            a.note(&format!("over: heard {:?}, earned {:?}", f.heard(), f.earned_names));
        }
        a.source = None;
    });
}

unsafe extern "C" fn on_physics(_: c_uint, _: *mut c_void, _: *mut c_void) {
    let mut late = None;
    let mut log = None;
    with_addon(|a| {
        a.update();
        late = a.late.take();
        log = a.log.clone();
    });
    // a game followed once its machine was heard, on its own: our renderer and our source of sound
    // offered outside the add-on's own lock, as at a game's start
    if let Some((cards, sounds)) = late {
        let _ = std::panic::catch_unwind(|| popup_start(cards, log));
        let _ = std::panic::catch_unwind(|| sound_start(sounds));
    }
}

/// How long the add-on on its own waits for a machine to be listed before choosing a table's packs
/// without one, and how often it looks, by the game's clock.
const OWN_WAIT_MS: u64 = 3000;
const OWN_LOOK_EVERY_MS: u64 = 100;

unsafe extern "C" fn on_frame(_: c_uint, _: *mut c_void, _: *mut c_void) {
    // the sound playing handed to VPX as it's heard
    let _ = std::panic::catch_unwind(sound_feed);
    let notes = menu::level_notes();
    if !notes.is_empty() {
        with_addon(|a| notes.iter().for_each(|n| a.note(n)));
    }
    // what the app's pause menu asks of VPX, a press a frame, outside the add-on's own lock (VPX
    // tells of the add-on's own presses as it makes them)
    let mut clock = None;
    with_addon(|a| clock = Some(a.now()));
    if let Some(clock) = clock {
        let _ = std::panic::catch_unwind(|| {
            let mut check = None;
            if let Ok(mut m) = menu::MENU.try_lock()
                && let Some(menu) = m.as_mut()
            {
                menu.frame(clock);
                check = menu.check_press(clock);
            }
            // a check's press of VPX's menu action, made as a player's: outside the menu's lock
            if let Some((set, down)) = check {
                menu::press_menu(set, down);
            }
        });
    }
    with_addon(|a| {
        let now = a.now();
        if let Some(shots) = &mut a.shots
            && shots.poll(now)
        {
            a.notify_for("Screenshot saved", 2000);
        }
        a.run_check_lines(now);
        if a.following.is_none() {
            return;
        }
        // what makes a game softcore changed in VPX's menu during a hardcore game: softcore from now
        if let Some(why) = a.hardcore.as_mut().and_then(|w| w.look(std::time::Instant::now())) {
            a.hardcore = None;
            if let Some(notice) = a.following.as_mut().and_then(|f| f.turn_softcore(why)) {
                a.note(&format!("softcore from now: {why}"));
                a.notify(&notice);
            }
        }
        let now = a.now();
        a.look_at_balls(now);
        if a.card_check_at.is_some_and(|at| now >= at) {
            a.card_check_at = None;
            if let Some(card) = a.following.as_ref().and_then(Following::sample_card) {
                a.note(&format!("card: {} · {} (a check)", card.kicker, card.name));
                popup_show(vec![card]);
            }
            // and its sound, as one earned plays it
            if let Some(gain) = a.following.as_ref().filter(|f| f.sound_on).map(|f| f.gain) {
                let by = play(&Chime::Ours, gain);
                a.note(&format!("sound: Ours, played by {by} (a check)"));
            }
        }
        if now != a.clock || now < PAUSES_FROM_MS {
            a.clock = now;
            a.still_since = None;
            a.standing_shown = false;
            return;
        }
        // the clock standing still while frames come: paused, or VPX's menu open
        let since = *a.still_since.get_or_insert_with(std::time::Instant::now);
        if !a.standing_shown && since.elapsed() >= PAUSED_AFTER {
            a.standing_shown = true;
            if let Some(f) = &a.following {
                let standing = f.standing(4);
                a.notify_for(&standing, STANDING_MS);
            }
        }
    });
}

/// An action pressed or let go, as VPX tells its add-ons: VPX's own menu's kept from VPX while the
/// app's pause menu is asked for (`menu`), the app asked to open it with where the achievements
/// stand. One the add-on presses itself comes while it holds the menu: VPX's to handle.
unsafe extern "C" fn on_action_changed(_: c_uint, _: *mut c_void, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let ev = unsafe { &mut *data.cast::<menu::ActionEvent>() };
        let Ok(mut guard) = menu::MENU.try_lock() else { return };
        let Some(m) = guard.as_mut() else { return };
        m.on_action(ev, || {
            let (mut page, mut sources) = (None, Vec::new());
            with_addon(|a| {
                page = a.following.as_ref().map(Following::page);
                sources = sound_sources(unsafe { &*a.msg }, a.endpoint, a.ids.get_audio_src);
            });
            menu::open_line(page.as_ref(), &sources)
        });
    }));
}

unsafe extern "C" fn on_state_src_changed(_: c_uint, _: *mut c_void, _: *mut c_void) {
    with_addon(|a| {
        // what the source gave is good only until it changes: looked for again at once
        a.source = None;
        a.next_look = 0;
    });
}

unsafe extern "C" fn on_b2s_change(_: c_uint, _: *mut c_void, data: *mut c_void) {
    if data.is_null() {
        return;
    }
    let e = unsafe { &*(data as *const B2sEvent) };
    let (kind, index, value) = (char::from(e.kind), e.index, e.value);
    with_addon(|a| {
        let now = a.now();
        // a script keeping its own high scores: a game its backglass tells over past the lowest
        // on its list made a new one
        if let Some(from) = a.shots.as_ref().and_then(|s| s.high_score_from)
            && high_score_made(&mut a.b2s_playing, &mut a.b2s_best, from, kind, index, value)
            && let Some(shots) = &a.shots
        {
            shots.take_now();
            a.notify_for("Screenshot saved: a new high score", 3000);
        }
        let Some(f) = &mut a.following else { return };
        let notices = f.b2s(kind, index, value, now);
        a.tell(notices);
    });
}

/// A backglass's change of state (`kind`, `index`, `value`, as `B2sEvent`) followed for a new high
/// score: each player's score kept as the best of the game, a game on or over as B2S's 35 tells it
/// (0 on, else over). Whether this one ended a game past `from`.
fn high_score_made(playing: &mut Option<bool>, best: &mut u64, from: u64, kind: char, index: i32, value: i32) -> bool {
    match (kind, index) {
        ('C', p) if p > 0 => {
            if playing.is_some_and(|on| on) {
                *best = (*best).max(u64::try_from(value).unwrap_or(0));
            }
            false
        }
        ('E', 35) => {
            let on = value == 0;
            let was = playing.replace(on);
            if on && was != Some(true) {
                // a game begun: its scores from naught
                *best = 0;
                return false;
            }
            let ended = was == Some(true) && !on;
            let made = ended && *best > from;
            if ended {
                *best = 0;
            }
            made
        }
        _ => false,
    }
}

/// Which interface VPX hands over, from how its structure opens: build 5436's with a function,
/// whose address is never within an image's first page; the newer one with its version number.
/// None for a version this add-on doesn't know, which leaves it loaded but idle rather than called
/// through a structure laid out otherwise.
///
/// # Safety
/// `api` points at VPX's structure, at least a pointer long.
unsafe fn interface_of(api: *const c_void) -> Option<Interface> {
    match unsafe { std::ptr::read(api.cast::<u32>()) } {
        1 => Some(Interface::V1),
        n if n >= 0x1000 => Some(Interface::Build5436),
        _ => None,
    }
}

/// A structure of the newer interface, past its version number (an `int`, padded to a pointer's
/// width): from there on laid out as build 5436's.
fn past_version<T>(p: *const c_void) -> *const T {
    p.cast::<u8>().wrapping_add(std::mem::size_of::<usize>()).cast()
}

/// This library kept loaded until VPX ends, whenever VPX lets the add-on go: a thread of ours
/// still running then — a screenshot Windows hasn't handed back yet, a card being drawn — never
/// runs code that's gone.
#[cfg(windows)]
fn stay_loaded() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleExW(flags: u32, name: *const u16, module: *mut *mut c_void) -> i32;
    }
    let mut module = std::ptr::null_mut();
    // GET_MODULE_HANDLE_EX_FLAG_PIN | GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
    // SAFETY: an address inside this library, a handle to fill
    unsafe { GetModuleHandleExW(0x1 | 0x4, stay_loaded as *const () as *const u16, &mut module) };
}

#[cfg(not(windows))]
fn stay_loaded() {}

/// VPX loading the add-on: its messages asked for and listened to.
///
/// # Safety
/// Called by VPX alone, with its own interface.
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn VPIAchievementsPluginLoad(session_id: u32, api: *const c_void) {
    if api.is_null() {
        return;
    }
    let Some(iface) = (unsafe { interface_of(api) }) else { return };
    stay_loaded();
    let api: *const MsgPluginApi = match iface {
        Interface::Build5436 => api.cast(),
        Interface::V1 => past_version(api),
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let msg = unsafe { &*api };
        let id = |space: &std::ffi::CStr, name: &str| unsafe { (msg.get_msg_id)(space.as_ptr(), iface.name(name).as_ptr()) };
        let ids = Ids {
            get_api: id(c"VPX", "GetAPI"),
            game_start: id(c"VPX", "OnGameStart"),
            game_end: id(c"VPX", "OnGameEnd"),
            physics: id(c"VPX", "OnUpdatePhysics"),
            frame: id(c"VPX", "OnPrepareFrame"),
            state_src_changed: id(c"Controller", "OnStateSrcChanged"),
            get_state_src: id(c"Controller", "GetStateSrc"),
            b2s_change: id(c"B2S", "OnStateChange"),
            paged_ui: (iface == Interface::V1).then(|| id(c"PagedUI", "GetAPI")),
            get_renderer: id(c"VPX", "GetAuxRenderer"),
            renderer_changed: id(c"VPX", "AuxRendererChanged"),
            get_audio_src: id(c"Controller", "GetAudioSrc"),
            audio_src_changed: id(c"Controller", "OnAudioSrcChanged"),
            audio_update: id(c"Controller", "AudioUpdate"),
            get_scriptable: (iface == Interface::V1).then(|| id(c"Scriptable", "GetAPI")),
            get_controllers: (iface == Interface::V1).then(|| id(c"Controller", "GetControllers")),
            action_changed: (iface == Interface::V1).then(|| id(c"VPX", "OnActionChanged")),
        };
        // on its own, with no app handing it a game's packs: its own settings, which VPX keeps and
        // shows in its menu, the newer interface alone
        if iface == Interface::V1 && std::env::var_os("VPI_ACHIEVEMENTS").is_none_or(|v| v.is_empty()) && !msg.register_setting.is_null() {
            let register: unsafe extern "C" fn(u32, *mut SettingDef) = unsafe { std::mem::transmute(msg.register_setting) };
            for def in &OWN_SETTINGS.0 {
                unsafe { register(session_id, std::ptr::from_ref(def).cast_mut()) };
            }
        }
        // VPX's menu pages, the newer interface's alone: their services past their version
        let mut handed_ui: *const c_void = std::ptr::null();
        if let Some(ui) = ids.paged_ui {
            unsafe { (msg.broadcast_msg)(session_id, ui, (&raw mut handed_ui).cast()) };
        }
        let paged_ui: *const PagedUiApi = if !handed_ui.is_null() && unsafe { std::ptr::read(handed_ui.cast::<c_int>()) } == 1 { past_version(handed_ui) } else { std::ptr::null() };
        let mut handed: *const c_void = std::ptr::null();
        unsafe { (msg.broadcast_msg)(session_id, ids.get_api, (&raw mut handed).cast()) };
        let vpx: *const VpxPluginApi = match iface {
            _ if handed.is_null() => std::ptr::null(),
            Interface::Build5436 => handed.cast(),
            // VPX's own services past their version number, where it's the one known here
            Interface::V1 if unsafe { std::ptr::read(handed.cast::<c_int>()) } == 1 => past_version(handed),
            Interface::V1 => std::ptr::null(),
        };
        // the newer interface's own: a line run in the table's script, and VPX's scriptable
        // interface (past its version number) for our script object, which hears the balls
        let run_script = match iface {
            Interface::V1 if !vpx.is_null() => unsafe { (*vpx.cast::<VpxPluginApiV1>()).run_script },
            _ => None,
        };
        let mut handed_scriptable: *const c_void = std::ptr::null();
        if let Some(scriptable) = ids.get_scriptable {
            unsafe { (msg.broadcast_msg)(session_id, scriptable, (&raw mut handed_scriptable).cast()) };
        }
        let scriptable: *const balls::ScriptableApi = if !handed_scriptable.is_null() && unsafe { std::ptr::read(handed_scriptable.cast::<c_int>()) } == 1 { past_version(handed_scriptable) } else { std::ptr::null() };
        let ball_class = if run_script.is_some() { unsafe { balls::register(scriptable, session_id) } } else { None };
        let null = std::ptr::null_mut();
        unsafe {
            (msg.subscribe_msg)(session_id, ids.game_start, on_game_start, null);
            (msg.subscribe_msg)(session_id, ids.game_end, on_game_end, null);
            (msg.subscribe_msg)(session_id, ids.physics, on_physics, null);
            (msg.subscribe_msg)(session_id, ids.frame, on_frame, null);
            if let Some(action_changed) = ids.action_changed {
                (msg.subscribe_msg)(session_id, action_changed, on_action_changed, null);
            }
            (msg.subscribe_msg)(session_id, ids.state_src_changed, on_state_src_changed, null);
            (msg.subscribe_msg)(session_id, ids.b2s_change, on_b2s_change, null);
            (msg.subscribe_msg)(session_id, ids.get_renderer, on_get_renderer, null);
            (msg.subscribe_msg)(session_id, ids.renderer_changed, on_renderer_changed, null);
            (msg.subscribe_msg)(session_id, ids.get_audio_src, on_get_audio_src, null);
            // where the app's pause menu sets the sound's levels: heard before VPX, which starts
            // listening after the add-ons have loaded
            if iface == Interface::V1 && std::env::var_os("VPI_MENU_PORT").is_some() {
                *LEVELS_API.lock().unwrap_or_else(|e| e.into_inner()) = Some(LevelsApi(api));
                (msg.subscribe_msg)(session_id, ids.audio_update, on_audio_update, null);
            }
        }
        *SOUND.lock().unwrap_or_else(|e| e.into_inner()) = Some(SoundState { msg: api, endpoint: session_id, src_changed: ids.audio_src_changed, update: ids.audio_update, offered: false, playing: None, next: 0 });
        *POPUP.lock().unwrap_or_else(|e| e.into_inner()) = Some(PopupState {
            msg: api,
            vpx,
            endpoint: session_id,
            get_renderer: ids.get_renderer,
            renderer_changed: ids.renderer_changed,
            settings: None,
            waiting: std::collections::VecDeque::new(),
            showing: None,
            seq: 0,
            drawn: Vec::new(),
            others: Vec::new(),
            asked_last: None,
            log: None,
        });
        *ADDON.lock().unwrap_or_else(|e| e.into_inner()) = Some(Addon { msg: api, vpx, iface, endpoint: session_id, ids, paged_ui, page_in_menu: false, following: None, source: None, next_look: 0, next_game: 0, log: None, clock: 0, still_since: None, standing_shown: false, shots: None, high_seen: None, high_since: 0, card_check_at: None, b2s_playing: None, b2s_best: 0, run_script, scriptable, ball_class, balls: balls::InPlay::default(), balls_put_in: false, next_balls: 0, balls_told: None, check_lines: Vec::new(), hardcore: None, own_wait: None, own_next: 0, late: None });
    }));
}

/// VPX unloading the add-on: what's followed finished, every message let go.
///
/// # Safety
/// Called by VPX alone, after loading it.
#[unsafe(no_mangle)]
#[allow(non_snake_case)]
pub unsafe extern "C" fn VPIAchievementsPluginUnload() {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(mut a) = ADDON.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        a.hide_page();
        if let Some(mut f) = a.following.take() {
            f.finish();
        }
        let msg = unsafe { &*a.msg };
        let null = std::ptr::null_mut();
        unsafe {
            (msg.unsubscribe_msg)(a.ids.game_start, on_game_start, null);
            (msg.unsubscribe_msg)(a.ids.game_end, on_game_end, null);
            (msg.unsubscribe_msg)(a.ids.physics, on_physics, null);
            (msg.unsubscribe_msg)(a.ids.frame, on_frame, null);
            (msg.unsubscribe_msg)(a.ids.state_src_changed, on_state_src_changed, null);
            (msg.unsubscribe_msg)(a.ids.b2s_change, on_b2s_change, null);
            (msg.unsubscribe_msg)(a.ids.get_renderer, on_get_renderer, null);
            (msg.unsubscribe_msg)(a.ids.renderer_changed, on_renderer_changed, null);
            (msg.unsubscribe_msg)(a.ids.get_audio_src, on_get_audio_src, null);
            if LEVELS_API.lock().unwrap_or_else(|e| e.into_inner()).take().is_some() {
                (msg.unsubscribe_msg)(a.ids.audio_update, on_audio_update, null);
            }
            // our script object taken out of VPX's scripts
            if let Some(class) = a.ball_class {
                balls::unregister(a.scriptable, class);
            }
            for id in a.ids.all() {
                (msg.release_msg_id)(id);
            }
            (msg.flush_pending_callbacks)(a.endpoint);
        }
        a.source = None;
        stop_sound();
        // cards still being drawn and screenshots still being written finished first: their code
        // is this library's
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while DRAWING.load(Ordering::Acquire) > 0 && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // the cards' textures went with the game; nothing of VPX's is asked for any more
        POPUP.lock().unwrap_or_else(|e| e.into_inner()).take();
        SOUND.lock().unwrap_or_else(|e| e.into_inner()).take();
        shots::wait_for_all(std::time::Duration::from_secs(3));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checks_lines_are_read_in_order() {
        let lines = check_lines("33000=table1_KeyDown StartGameKey| 31000 = table1_KeyDown AddCreditKey |nonsense|32000=table1_KeyUp AddCreditKey");
        let read: Vec<(u64, String)> = lines.into_iter().map(|(at, l)| (at, l.into_string().unwrap())).collect();
        assert_eq!(read, vec![(31000, "table1_KeyDown AddCreditKey".into()), (32000, "table1_KeyUp AddCreditKey".into()), (33000, "table1_KeyDown StartGameKey".into())]);
        assert!(check_lines("").is_empty());
    }

    #[test]
    fn every_id_asked_for_is_given_back_0_among_them() {
        // VPX's own GetAPI is the first message VPX names, so its id is 0
        let ids = |newer: bool| Ids {
            get_api: 0,
            game_start: 1,
            game_end: 2,
            physics: 3,
            frame: 4,
            state_src_changed: 5,
            get_state_src: 6,
            b2s_change: 7,
            paged_ui: newer.then_some(8),
            get_renderer: 9,
            renderer_changed: 10,
            get_audio_src: 11,
            audio_src_changed: 12,
            audio_update: 13,
            get_scriptable: newer.then_some(14),
            get_controllers: newer.then_some(15),
            action_changed: newer.then_some(16),
        };
        let mut newer: Vec<c_uint> = ids(true).all().collect();
        newer.sort_unstable();
        assert_eq!(newer, (0..=16).collect::<Vec<c_uint>>());
        // build 5436's: no menu pages or scriptable interface asked for, nothing else left out
        let older: Vec<c_uint> = ids(false).all().collect();
        assert_eq!(older.len(), 13);
        assert!(older.contains(&0) && !older.contains(&8) && !older.contains(&14) && !older.contains(&15) && !older.contains(&16));
    }

    #[test]
    fn the_interfaces_are_laid_out_as_vpxs_headers_declare_them() {
        // x64: pointers 8 bytes; each union of two 32-bit numbers with a 64-bit one 8 aligned
        assert_eq!(std::mem::size_of::<MsgPluginApi>(), 12 * 8);
        assert_eq!(std::mem::size_of::<VpxPluginApi>(), 14 * 8);
        assert_eq!(std::mem::offset_of!(VpxPluginApi, update_texture), 11 * 8);
        assert_eq!(std::mem::offset_of!(VpxPluginApi, delete_texture), 13 * 8);
        // MsgSettingDef: three names, two whole numbers, then the union of its kinds, 40 bytes at
        // its largest (a choice: three numbers, its values' names, a getter and a setter)
        assert_eq!(std::mem::offset_of!(SettingDef, def), 32);
        assert_eq!(std::mem::size_of::<SettingDef>(), 72);
        assert_eq!(std::mem::offset_of!(IntDef, values), 16);
        assert_eq!(std::mem::offset_of!(BoolDef, get), 8);
        // VPXInfo, VPXTableInfo and a controller's definition, as VPX's headers declare them
        assert_eq!(std::mem::size_of::<VpxInfo>(), 16);
        assert_eq!(std::mem::size_of::<VpxTableInfo>(), 16);
        assert_eq!(std::mem::offset_of!(ControllerDef, game_id), 8);
        assert_eq!(std::mem::size_of::<GetControllersMsg>(), 16);
        // the newer interface's RunScript after them
        assert_eq!(std::mem::offset_of!(VpxPluginApiV1, run_script), 14 * 8);
        // the renderers' (`VPXRenderContext2D`, `AncillaryRendererDef`, `GetAncillaryRendererMsg`)
        assert_eq!(std::mem::offset_of!(RenderContext, out_height), 20);
        assert_eq!(std::mem::offset_of!(RenderContext, draw_image), 24);
        assert_eq!(std::mem::size_of::<RenderContext>(), 56);
        assert_eq!(std::mem::size_of::<RendererDef>(), 40);
        assert_eq!(std::mem::offset_of!(RendererDef, render), 32);
        assert_eq!(std::mem::size_of::<GetRendererMsg>(), 24);
        assert_eq!(std::mem::offset_of!(GetRendererMsg, entries), 16);
        // the sound's (`AudioSrcId`, `GetAudioSrcMsg`, `AudioUpdateMsg`)
        assert_eq!(std::mem::size_of::<AudioSrcId>(), 40);
        assert_eq!(std::mem::offset_of!(AudioSrcId, target), 32);
        assert_eq!(std::mem::size_of::<GetAudioSrcMsg>(), 16);
        assert_eq!(std::mem::offset_of!(AudioUpdateMsg, sample_rate), 24);
        assert_eq!(std::mem::offset_of!(AudioUpdateMsg, volume), 32);
        assert_eq!(std::mem::offset_of!(AudioUpdateMsg, buffer_size), 36);
        assert_eq!(std::mem::offset_of!(AudioUpdateMsg, buffer), 40);
        assert_eq!(std::mem::size_of::<AudioUpdateMsg>(), 48);
        assert_eq!(std::mem::size_of::<StateDef>(), 32);
        assert_eq!(std::mem::offset_of!(StateDef, type_mask), 24);
        assert_eq!(std::mem::size_of::<StateSrcId>(), 56);
        assert_eq!(std::mem::offset_of!(StateSrcId, group_defs), 16);
        assert_eq!(std::mem::offset_of!(StateSrcId, state_defs), 32);
        assert_eq!(std::mem::offset_of!(StateSrcId, get_state), 40);
        assert_eq!(std::mem::size_of::<GetStateSrcMsg>(), 16);
        assert_eq!(std::mem::size_of::<B2sEvent>(), 12);
        assert_eq!(std::mem::offset_of!(B2sEvent, index), 4);
        // the newer interface's
        assert_eq!(std::mem::size_of::<StateDefV1>(), 56);
        assert_eq!(std::mem::offset_of!(StateDefV1, mapping_id), 16);
        assert_eq!(std::mem::offset_of!(StateDefV1, call_context), 32);
        assert_eq!(std::mem::offset_of!(StateDefV1, get_state), 40);
        assert_eq!(std::mem::size_of::<StateSrcIdV1>(), 40);
        assert_eq!(std::mem::offset_of!(StateSrcIdV1, n_states), 24);
        assert_eq!(std::mem::offset_of!(StateSrcIdV1, state_defs), 32);
        assert_eq!(std::mem::size_of::<GetStateSrcMsgV1>(), 16);
        // the menu pages'
        assert_eq!(std::mem::size_of::<PageInfo>(), 24);
        assert_eq!(std::mem::size_of::<ItemDef>(), 96);
        assert_eq!(std::mem::offset_of!(ItemDef, flags), 32);
        assert_eq!(std::mem::offset_of!(ItemDef, context), 40);
        assert_eq!(std::mem::offset_of!(ItemDef, settings), 48);
        assert_eq!(std::mem::size_of::<SheetDef>(), 112);
        assert_eq!(std::mem::offset_of!(SheetDef, get_page_count), 40);
        assert_eq!(std::mem::offset_of!(SheetDef, is_root), 104);
        assert_eq!(std::mem::size_of::<PagedUiApi>(), 24);
    }

    #[test]
    fn the_menu_page_is_handed_over_as_vpx_asks() {
        // nothing followed: one line saying so, its strings kept for VPX to read
        let mut info = PageInfo { id: std::ptr::null(), title: std::ptr::null(), info: std::ptr::null() };
        unsafe { sheet_page_info(std::ptr::null_mut(), 0, &mut info) };
        assert_eq!(unsafe { std::ffi::CStr::from_ptr(info.title) }.to_str().unwrap(), "Achievements");
        let n = unsafe { sheet_item_count(std::ptr::null_mut(), PAGE_ID.as_ptr()) } as usize;
        assert_eq!(n, 1);
        let mut items: Vec<ItemDef> = (0..n).map(|_| ItemDef { kind: -1, id: std::ptr::null(), label: std::ptr::null(), tooltip: std::ptr::null(), flags: 9, storage: 9, context: std::ptr::null_mut(), settings: [7; 6] }).collect();
        unsafe { sheet_items(std::ptr::null_mut(), PAGE_ID.as_ptr(), items.as_mut_ptr()) };
        assert_eq!(items[0].kind, ITEM_MARKDOWN);
        assert_eq!(unsafe { std::ffi::CStr::from_ptr(items[0].id) }.to_str().unwrap(), "line0");
        assert!(unsafe { std::ffi::CStr::from_ptr(items[0].label) }.to_str().unwrap().starts_with("No achievements"));
        assert_eq!((items[0].flags, items[0].settings), (0, [0; 6]));
    }

    #[test]
    fn the_interface_vpx_hands_over_is_told_apart() {
        // build 5436's opens with a function; the newer one with its version, 1
        let old = [on_game_start as *const () as usize as u64; 12];
        let newer: [u64; 13] = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(unsafe { interface_of(old.as_ptr().cast()) }, Some(Interface::Build5436));
        assert_eq!(unsafe { interface_of(newer.as_ptr().cast()) }, Some(Interface::V1));
        // past the version, the newer one's functions sit where build 5436's do
        let past: *const u64 = past_version(newer.as_ptr().cast());
        assert_eq!(past, newer[1..].as_ptr());
        assert_eq!(Interface::V1.name("GetAPI").as_bytes(), b"GetAPI:1");
        assert_eq!(Interface::Build5436.name("GetAPI").as_bytes(), b"GetAPI");
        // a version not known here: left alone, nothing called
        let later: [u64; 13] = [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(unsafe { interface_of(later.as_ptr().cast()) }, None);
        unsafe { VPIAchievementsPluginLoad(7, later.as_ptr().cast()) };
        assert!(ADDON.lock().unwrap_or_else(|e| e.into_inner()).is_none());
        unsafe { VPIAchievementsPluginUnload() };
    }

    #[test]
    fn a_state_of_the_newer_interface_is_read_in_its_own_format() {
        unsafe extern "C" fn minus_two(_: *mut c_void, out: *mut c_void) {
            unsafe { *out.cast::<i16>() = -2 };
        }
        unsafe extern "C" fn half(_: *mut c_void, out: *mut c_void) {
            unsafe { *out.cast::<f32>() = 0.5 };
        }
        let short = Reader::Own { get: minus_two, context: std::ptr::null_mut(), format: FORMAT_INT16 };
        let level = Reader::Own { get: half, context: std::ptr::null_mut(), format: FORMAT_FLOAT };
        assert_eq!((short.number(), short.on()), (Some(-2), Some(true)));
        assert_eq!(level.on(), Some(true));
        let text = Reader::Own { get: minus_two, context: std::ptr::null_mut(), format: 11 };
        assert_eq!(text.number(), None);
    }

    #[test]
    fn our_renderer_is_handed_back_first() {
        unsafe extern "C" fn theirs(_: *mut RenderContext, _: *mut c_void) -> c_int {
            1
        }
        // B2S's handed back before ours: ours first after, theirs moved up one
        let b2s = RendererDef { id: c"B2S".as_ptr(), render: Some(theirs), ..RendererDef::default() };
        let mut room = [b2s, RendererDef::default()];
        let mut ask = GetRendererMsg { window: 1, max_entry_count: 2, count: 1, entries: room.as_mut_ptr() };
        unsafe { put_first(&mut ask, our_renderer(1)) };
        assert_eq!(ask.count, 2);
        assert_eq!((text(room[0].id), text(room[1].id)), ("VPIAchievements".to_string(), "B2S".to_string()));
        assert_eq!(room[0].context as isize, 1);
        // VPX counting them first, with no room: one more
        let mut count = GetRendererMsg { window: 1, max_entry_count: 0, count: 3, entries: std::ptr::null_mut() };
        unsafe { put_first(&mut count, our_renderer(1)) };
        assert_eq!(count.count, 4);
    }

    #[test]
    fn a_card_is_drawn_over_the_windows_own_picture_once_its_ready() {
        static TEXTURES: AtomicUsize = AtomicUsize::new(0);
        static THEIRS_DREW: AtomicUsize = AtomicUsize::new(0);
        static DRAWN: Mutex<Vec<[f32; 5]>> = Mutex::new(Vec::new());
        unsafe extern "C" fn push(_: *const c_char, _: c_int) -> c_uint {
            0
        }
        unsafe extern "C" fn clock() -> f64 {
            0.0
        }
        unsafe extern "C" fn update(texture: *mut *mut c_void, w: c_int, h: c_int, format: c_int, image: *const c_void) {
            assert!(format == TEXTURE_RGBA && w > 0 && h > 0 && !image.is_null());
            unsafe { *texture = 0x1000 as *mut c_void };
            TEXTURES.fetch_add(1, Ordering::SeqCst);
        }
        unsafe extern "C" fn delete(_: *mut c_void) {
            TEXTURES.fetch_sub(1, Ordering::SeqCst);
        }
        #[allow(clippy::too_many_arguments)]
        unsafe extern "C" fn draw(_: *mut RenderContext, _: *mut c_void, _: f32, _: f32, _: f32, alpha: f32, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32, _: f32, x: f32, y: f32, w: f32, h: f32) {
            DRAWN.lock().unwrap().push([alpha, x, y, w, h]);
        }
        unsafe extern "C" fn theirs(_: *mut RenderContext, _: *mut c_void) -> c_int {
            THEIRS_DREW.fetch_add(1, Ordering::SeqCst);
            1
        }
        let null = std::ptr::null();
        let vpx: &'static VpxPluginApi = Box::leak(Box::new(VpxPluginApi {
            get_vpx_info: null,
            get_table_info: null,
            push_notification: push,
            update_notification: null,
            disable_static_prerendering: null,
            get_active_view_setup: null,
            set_active_view_setup: null,
            get_input_state: null,
            set_input_state: null,
            get_game_time: clock,
            create_texture: null,
            update_texture: update,
            get_texture_info: null,
            delete_texture: delete,
        }));
        *POPUP.lock().unwrap() = Some(PopupState {
            msg: null.cast(),
            vpx,
            endpoint: 0,
            get_renderer: 0,
            renderer_changed: 0,
            settings: Some(popup::Settings { screen: popup::Screen::Backglass, size: popup::MEDIUM, priorities: Vec::new() }),
            waiting: std::collections::VecDeque::new(),
            showing: None,
            seq: 0,
            drawn: Vec::new(),
            others: vec![RendererDef { id: c"B2S".as_ptr(), render: Some(theirs), ..RendererDef::default() }],
            asked_last: None,
            log: None,
        });
        let mut ctx = RenderContext { window: 1, src_width: 1920.0, src_height: 1080.0, is_2d: 1, out_width: 1920.0, out_height: 1080.0, draw_image: draw, draw_display: null, draw_seg_display: null, renderer_data: std::ptr::null_mut() };
        let ours = 1isize as *mut c_void;
        // no card: ours draws nothing, and VPX asks the window's own renderers as ever
        assert_eq!(unsafe { render_card(&mut ctx, ours) }, 0);
        assert!(popup_live());
        popup_show(vec![popup::Card { kicker: "Achievement earned · 5 points".into(), name: "Test".into(), picture: None }]);
        // while it's drawn on its thread, nothing over the window's own; then the window's own
        // picture, the card over it, fading in from its first frame
        let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while unsafe { render_card(&mut ctx, ours) } == 0 {
            assert!(std::time::Instant::now() < until, "the card was never drawn");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(THEIRS_DREW.load(Ordering::SeqCst), 1);
        assert_eq!(TEXTURES.load(Ordering::SeqCst), 1);
        let [alpha, x, _, w, _] = DRAWN.lock().unwrap()[0];
        assert_eq!((x, w), (384.0, 1152.0));
        assert!(alpha < 0.2, "{alpha}");
        // the game ended: its texture let go
        popup_end();
        assert_eq!(TEXTURES.load(Ordering::SeqCst), 0);
        *POPUP.lock().unwrap() = None;
    }

    #[test]
    fn the_sound_is_offered_to_vpx_and_played_as_its_streams() {
        static SENT: Mutex<Vec<(c_uint, u32, u32, u32, f64)>> = Mutex::new(Vec::new());
        unsafe extern "C" fn endpoint(_: *const c_char) -> c_uint {
            0
        }
        unsafe extern "C" fn msg_id(_: *const c_char, _: *const c_char) -> c_uint {
            0
        }
        unsafe extern "C" fn subscribe(_: u32, _: c_uint, _: MsgCallback, _: *mut c_void) {}
        unsafe extern "C" fn unsubscribe(_: c_uint, _: MsgCallback, _: *mut c_void) {}
        unsafe extern "C" fn release(_: c_uint) {}
        unsafe extern "C" fn flush(_: u32) {}
        // VPX taking each message: a stream's number, its samples' size, its sample rate
        unsafe extern "C" fn broadcast(_: u32, id: c_uint, data: *mut c_void) {
            if id == 33 {
                let m = unsafe { &*(data as *const AudioUpdateMsg) };
                SENT.lock().unwrap().push((id, m.stream_id.res_id, m.buffer_size, m.channel_format, m.sample_rate));
            } else {
                SENT.lock().unwrap().push((id, 0, 0, 0, 0.0));
            }
        }
        let null = std::ptr::null();
        let msg: &'static MsgPluginApi = Box::leak(Box::new(MsgPluginApi {
            get_plugin_endpoint: endpoint,
            get_endpoint_info: null,
            get_msg_id: msg_id,
            subscribe_msg: subscribe,
            unsubscribe_msg: unsubscribe,
            broadcast_msg: broadcast,
            send_msg: null,
            release_msg_id: release,
            register_setting: null,
            save_setting: null,
            run_on_main_thread: null,
            flush_pending_callbacks: flush,
        }));
        *SOUND.lock().unwrap() = Some(SoundState { msg, endpoint: 9, src_changed: 32, update: 33, offered: false, playing: None, next: 0 });
        // not offered: VPX told nothing, our source not handed back, nothing played through it
        sound_start(false);
        let mut room = [AudioSrcId { id: CtlResId::default(), override_id: CtlResId::default(), name: null.cast(), desc: null.cast(), target: 7 }];
        let mut ask = GetAudioSrcMsg { max_entry_count: 1, count: 0, entries: room.as_mut_ptr() };
        unsafe { on_get_audio_src(0, std::ptr::null_mut(), (&raw mut ask).cast()) };
        assert_eq!(ask.count, 0);
        let ours = follow::pcm_of(OUR_CHIME).unwrap();
        assert!(!play_in_vpx(&ours));
        // offered: VPX told, our source handed back, for the backglass's output
        sound_start(true);
        unsafe { on_get_audio_src(0, std::ptr::null_mut(), (&raw mut ask).cast()) };
        assert_eq!((ask.count, room[0].id.endpoint_id, room[0].id.res_id, room[0].target), (1, 9, SOUND_SOURCE, 0));
        assert_eq!(text(room[0].name), "Achievements");
        // a sound: a stream begun with its first half second; the next one ends it first
        let (rate, stereo) = (f64::from(ours.rate), u32::from(ours.channels == 2));
        let frame = usize::from(ours.channels) * 2;
        let half = (ours.rate as usize / 2 * frame) as u32;
        assert!(play_in_vpx(&ours));
        assert!(play_in_vpx(&ours));
        {
            let sent = SENT.lock().unwrap();
            assert_eq!(sent.len(), 4);
            assert_eq!(sent[0], (32, 0, 0, 0, 0.0));
            assert_eq!((sent[1].0, sent[1].1, sent[1].3, sent[1].4), (33, 1, stereo, rate));
            assert!(sent[1].2 >= half && sent[1].2 < half + 4 * frame as u32 * 100, "{}", sent[1].2);
            assert_eq!(sent[2], (33, 1, 0, stereo, rate));
            assert_eq!((sent[3].0, sent[3].1), (33, 2));
        }
        // the rest handed over as it's heard, never more than all of it
        std::thread::sleep(std::time::Duration::from_millis(150));
        sound_feed();
        std::thread::sleep(std::time::Duration::from_millis(400));
        sound_feed();
        let fed: u32 = SENT.lock().unwrap()[3..].iter().map(|s| s.2).sum();
        assert_eq!(fed as usize, ours.samples.len());
        // the game ended: the stream playing ended
        sound_end();
        assert_eq!(SENT.lock().unwrap().last(), Some(&(33, 2, 0, stereo, rate)));
        *SOUND.lock().unwrap() = None;
    }

    #[test]
    fn a_script_tables_game_past_its_lowest_high_score_is_told_as_it_ends() {
        let (mut playing, mut best) = (None, 0u64);
        let mut tell = |kind: char, index: i32, value: i32| high_score_made(&mut playing, &mut best, 1_000_000, kind, index, value);
        // attract mode, then a game: scores told as they come (again each frame), the game over
        assert!(!tell('E', 35, 1));
        assert!(!tell('C', 1, 5_000_000));
        assert!(!tell('E', 35, 0));
        assert!(!tell('C', 1, 400_000));
        assert!(!tell('C', 2, 1_250_000));
        assert!(!tell('C', 2, 1_250_000));
        assert!(tell('E', 35, 1));
        // told again: nothing more; a game under the lowest: nothing
        assert!(!tell('E', 35, 1));
        assert!(!tell('E', 35, 0));
        assert!(!tell('C', 1, 900_000));
        assert!(!tell('E', 35, 1));
    }

    #[test]
    fn our_chime_is_a_wav_it_can_scale() {
        assert!(follow::scaled_wav(OUR_CHIME, 0.5).is_some_and(|w| w.len() == OUR_CHIME.len()));
    }
}
