//! Future Pinball's tables, played in Future Pinball itself, through BAM where it's there, the app
//! their front end as it is VPX's: where Future Pinball is installed — the folder the settings name
//! (`[VPI]` `FuturePinballFolder`), else beside VPX's (`FuturePinball` beside `VisualPinball`, as
//! the Baller Installer and TerryRed's all-in-one lay them out), else its usual folders — its tables
//! (`Tables\*.fpt`, one folder down too), each matched to VPS among its Future Pinball table files,
//! and the command front ends play one with: BAM's loader from BAM's own folder,
//! `FPLoader.exe /open "<table>" /play /exit /arcaderender /STAYINRAM` (the loader starts Future
//! Pinball with BAM in it and, with `/STAYINRAM`, stays until it ends, so its end is the game's),
//! else Future Pinball's own program the same way, without the loader's flag. Ours.

use std::path::{Path, PathBuf};

use crate::matcher;
use crate::vps::{Database, Game, TableFile};

/// Future Pinball's program, in its folder.
pub const PROGRAM: &str = "Future Pinball.exe";

/// VPS's name for Future Pinball's table files (`tableFormat`).
pub const FORMAT: &str = "FP";

/// A Future Pinball install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    /// Its folder, holding `Future Pinball.exe`.
    pub folder: PathBuf,
    /// BAM's loader (`BAM\FPLoader.exe`), where BAM is there: it starts Future Pinball with BAM in
    /// it, from BAM's folder, whose settings it reads.
    pub loader: Option<PathBuf>,
}

impl Install {
    /// The install in this folder, where Future Pinball's program is there.
    pub fn at(folder: &Path) -> Option<Install> {
        if !folder.join(PROGRAM).is_file() {
            return None;
        }
        let loader = folder.join("BAM").join("FPLoader.exe");
        Some(Install { folder: folder.to_path_buf(), loader: loader.is_file().then_some(loader) })
    }

    pub fn program(&self) -> PathBuf {
        self.folder.join(PROGRAM)
    }

    /// Where its tables are kept.
    pub fn tables_folder(&self) -> PathBuf {
        self.folder.join("Tables")
    }
}

/// Where Future Pinball is: the folder the settings name (`named`), where it's there; else beside
/// VPX's own folder (`vpx`: its parent's `FuturePinball` or `Future Pinball`); else its usual
/// folders.
pub fn find(named: Option<&Path>, vpx: Option<&Path>) -> Option<Install> {
    if let Some(found) = named.and_then(Install::at) {
        return Some(found);
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(parent) = vpx.and_then(Path::parent) {
        candidates.extend(["FuturePinball", "Future Pinball"].iter().map(|n| parent.join(n)));
    }
    candidates.extend([r"C:\vPinball\FuturePinball", r"C:\Games\Future Pinball", r"C:\Future Pinball"].iter().map(PathBuf::from));
    if let Some(programs) = std::env::var_os("ProgramFiles(x86)") {
        candidates.push(PathBuf::from(programs).join("Future Pinball"));
    }
    candidates.iter().find_map(|c| Install::at(c))
}

/// Whether a file is a Future Pinball table (`.fpt`).
pub fn is_table(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("fpt"))
}

/// Its tables: the `.fpt` files in its tables folder and one folder down, by their paths.
pub fn tables(install: &Install) -> Vec<PathBuf> {
    let is_table = |p: &Path| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("fpt"));
    let mut out = Vec::new();
    for entry in std::fs::read_dir(install.tables_folder()).into_iter().flatten().flatten() {
        let path = entry.path();
        if is_table(&path) {
            out.push(path);
        } else if path.is_dir() {
            out.extend(std::fs::read_dir(&path).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| is_table(p)));
        }
    }
    out.sort();
    out
}

/// A Future Pinball table matched to VPS by its file's name, among the games VPS lists a Future
/// Pinball table file for (VPin Studio's matcher, its formats Future Pinball's): the game, and its
/// table file the name's version and authors fit best. A name with no maker and year in it, as
/// Future Pinball's editions are often named (`Retroflair - BAM Edition (PinEvent V2 - FizX -
/// 2.0)`), is matched by its own name too, what comes before its first ` - ` or `(`, the nearer
/// match kept.
pub fn matched<'a>(db: &'a Database, table: &Path) -> Option<(&'a Game, Option<&'a TableFile>)> {
    let name = table.file_stem()?.to_string_lossy().into_owned();
    let by = |file_name: &str| matcher::match_table_as(db, &matcher::Clues { file_name: file_name.to_string(), ..Default::default() }, &[FORMAT]);
    let mut found = by(&name);
    let own = own_name(&name);
    if found.distance > matcher::CONFIRMING && own != name && !own.is_empty() {
        let by_own = by(&own);
        if by_own.game.is_some() && by_own.distance < found.distance {
            found = by_own;
        }
    }
    found.game.filter(|_| found.distance <= matcher::NOT_FOUND).map(|g| (g, found.table_file))
}

/// A table file's own name where it gives no maker and year: what comes before its first ` - ` or
/// `(`; else the name as it is.
fn own_name(name: &str) -> String {
    let parts = matcher::parse_filename(name);
    if parts.year >= 0 || parts.manufacturer.is_some() {
        return name.to_string();
    }
    name.split(" - ").next().unwrap_or(name).split('(').next().unwrap_or(name).trim().to_string()
}

/// Where Future Pinball's player keeps its settings, in the user's part of Windows' registry.
pub const SETTINGS_KEY: &str = r"Software\Future Pinball\GamePlayer";

/// One of Future Pinball's player settings: a number, or a text (a screen's `\\.\DISPLAYn`).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Value {
    Number(u32),
    Text(String),
}

/// A window on a screen, as the app's Screens settings give VPX's: the screen by Windows' name for
/// it (`\\.\DISPLAY2`), the window's size, whether it fills the screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub screen: String,
    pub width: u32,
    pub height: u32,
    pub full: bool,
}

/// Future Pinball's player settings for a game, so it plays on the screens the app's Screens
/// settings give VPX: the playfield on its screen at its size; the backbox on the backglass's
/// screen at its size where a backglass window is set and the table has no PinUP pack — Future
/// Pinball shows its backbox only in its arcade mode (`ArcadeMode` and `SecondMonitorEnable` both
/// 1), and a table playing a PinUP pack needs both off — else neither (TerryRed's start files'
/// rules).
pub fn screen_settings(playfield: Option<&Window>, backbox: Option<&Window>, pup_pack: bool) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    if let Some(p) = playfield {
        out.push(("PlayfieldMonitorID", Value::Text(p.screen.clone())));
        out.push(("Width", Value::Number(p.width)));
        out.push(("Height", Value::Number(p.height)));
        out.push(("FullScreen", Value::Number(u32::from(p.full))));
    }
    match backbox.filter(|_| !pup_pack) {
        Some(b) => {
            out.push(("ArcadeMode", Value::Number(1)));
            out.push(("SecondMonitorEnable", Value::Number(1)));
            out.push(("BackboxMonitorID", Value::Text(b.screen.clone())));
            out.push(("SecondMonitorWidth", Value::Number(b.width)));
            out.push(("SecondMonitorHeight", Value::Number(b.height)));
            out.push(("SecondMonitorFullScreen", Value::Number(u32::from(b.full))));
        }
        None => {
            out.push(("ArcadeMode", Value::Number(0)));
            out.push(("SecondMonitorEnable", Value::Number(0)));
        }
    }
    out
}

/// Each of VPX's actions Future Pinball plays too, beside its player's setting for its key (a
/// DirectInput code) and the key it plays it with unset.
pub const KEYS: &[(&str, &str, u8)] = &[
    ("LeftFlipper", "LeftFlipperKey", 0x2A),
    ("RightFlipper", "RightFlipperKey", 0x36),
    ("LeftStagedFlipper", "Left2ndFlipper", 0x1E),
    ("RightStagedFlipper", "Right2ndFlipper", 0x28),
    ("LaunchBall", "PlungerKey", 0x1C),
    ("Start", "StartGameKey", 0x02),
    ("Credit1", "InsertCoinKey", 0x06),
    ("Credit2", "InsertCoin2Key", 0x07),
    ("Credit3", "InsertCoin3Key", 0x08),
    ("LeftNudge", "NudgeLeftKey", 0x2C),
    ("CenterNudge", "NudgeUpKey", 0x39),
    ("RightNudge", "NudgeRightKey", 0x35),
    ("LeftMagna", "Special1Key", 0x1E),
    ("RightMagna", "Special2Key", 0x28),
];

/// BAM's menu's key, a DirectInput code: Q, the second of `bam.cfg`'s `Menu` keys, as front ends'
/// guides give it.
pub const BAM_MENU_KEY: u8 = 0x10;

/// The key that ends a game in Future Pinball and, started with `/exit`, Future Pinball itself: Esc.
pub const EXIT_KEY: u8 = 0x01;

/// VPX's actions a game in Future Pinball plays on keys it keeps no setting for: VPX's in-game menu
/// on BAM's menu, its exit on Esc.
const FIXED_KEYS: &[(&str, u8)] = &[("InGameUI", BAM_MENU_KEY), ("ExitGame", EXIT_KEY)];

/// Future Pinball's key settings for a game as VPX's settings (`newer`) map VPX's actions: each
/// action's first key there, where its mapping names one (the others left as Future Pinball's own
/// settings have them).
pub fn key_settings(newer: &[(String, String, String)]) -> Vec<(&'static str, Value)> {
    KEYS.iter()
        .filter_map(|&(action, name, _)| {
            let key = vpi_settings::get(newer, "Input", &format!("Mapping.{action}")).and_then(crate::controller_1080::key_of)?;
            Some((name, Value::Number(u32::from(key))))
        })
        .collect()
}

/// The key Future Pinball plays one of VPX's actions with in a game: its setting as the game has it
/// (`setting`: a setting's value by its name), else the key it plays it with unset; BAM's menu and
/// Esc for those it keeps no setting for; none for an action it doesn't play.
pub fn key_for(action: &str, setting: impl Fn(&str) -> Option<u32>) -> Option<u8> {
    if let Some(&(_, key)) = FIXED_KEYS.iter().find(|(a, _)| *a == action) {
        return Some(key);
    }
    let &(_, name, unset) = KEYS.iter().find(|(a, _, _)| *a == action)?;
    Some(setting(name).and_then(|k| u8::try_from(k).ok()).filter(|&k| k != 0).unwrap_or(unset))
}

/// Future Pinball's player settings for a game in VR, as TerryRed's "Start FP - VR" sets them for
/// BAM's VR: no backbox or arcade mode, no vertical sync (the headset's program paces the frames),
/// the rest of its picture as TerryRed's guide has it.
pub const VR_SETTINGS: &[(&str, u32)] = &[
    ("ArcadeMode", 0),
    ("SecondMonitorEnable", 0),
    ("VerticalSync", 0),
    ("PlayfieldReflections", 0),
    ("RenderBallMirrors", 1),
    ("AntiAliasing", 0),
    ("TextureFilter", 1),
    ("FullScreen", 1),
    ("DefaultCamera", 1),
    ("CameraFollowsTheBall", 1),
    ("BallDirt", 1),
    ("CpuAffinity", 0),
    ("ModelRenderQuality", 2),
    ("AspectRatio", 169),
    ("BitsPerPixel", 32),
    ("RenderGameRoom", 0),
    ("RenderOrnaments", 1),
    ("GlassOverlay", 0),
    ("LightFacets", 64),
    ("GlassReflections", 0),
    ("SuperNiceCrystal", 1),
    ("HighQualityPinballs", 1),
    ("NonPowerOf2Textures", 1),
    ("DisableShaders", 0),
    ("RubberFacets", 64),
    ("RubberSides", 20),
    ("WireGuideSides", 20),
    ("HighQualityTextures", 1),
    ("TextureCompression", 0),
    ("ZBufferDepth", 24),
    ("AnisotropicFiltering", 1),
    ("HardwareLights", 7),
    ("FlasherFlares", 1),
    ("PlayfieldFlares", 1),
];

/// BAM's two VR folders: its SteamVR one, and its OpenComposite one (straight to the PC's OpenXR
/// program, Meta's Link among them).
const VR_FOLDERS: [&str; 2] = ["BAM-OpenVR", "BAM-OpenVR-OC"];

/// BAM's VR settings, as TerryRed's "BAM settings for VR" holds them: BAM's keys, the settings a
/// table starts with, its room's measures, and its plugins' settings.
const VR_SETTINGS_FILES: [&str; 5] = ["bam.cfg", "default.cfg", "Reality.dat", "plugins/iCom.cfg", "plugins/OpenVR.cfg"];

/// BAM's VR settings put in each of its VR folders that has none (no `default.cfg`), as TerryRed's
/// "BAM settings - VR - Reset and Install" puts them: the five files from the copy zipped in that
/// folder (`BAM settings for VR.zip`), else from TerryRed's guide's folder (`Setup and Guides\1 - Future
/// Pinball and BAM - Install Guide\2 - BAM settings\BAM settings for VR`), all five or none; BAM's
/// settings for tables already in its `cfg`, made without them, moved to the Recycle Bin, as TerryRed's
/// steps clear them for BAM to make again. A folder with VR settings of its own is left as it is.
/// The folders set up, by name.
pub fn put_vr_settings_in_place(install: &Install) -> Vec<String> {
    let guide = install.folder.join("Setup and Guides").join("1 - Future Pinball and BAM - Install Guide").join("2 - BAM settings").join("BAM settings for VR");
    let mut set_up = Vec::new();
    for name in VR_FOLDERS {
        let folder = install.folder.join(name);
        if !folder.join("FPLoader.exe").is_file() || folder.join("default.cfg").is_file() {
            continue;
        }
        let Some(files) = vr_settings_from_zip(&folder.join("BAM settings for VR.zip")).or_else(|| vr_settings_from_folder(&guide)) else { continue };
        let written = files.iter().all(|(file, bytes)| {
            let to = file.split('/').fold(folder.clone(), |path, part| path.join(part));
            to.parent().is_some_and(|p| std::fs::create_dir_all(p).is_ok()) && std::fs::write(&to, bytes).is_ok()
        });
        if !written {
            continue;
        }
        if let Ok(made) = std::fs::read_dir(folder.join("cfg")) {
            for f in made.flatten().filter(|f| f.path().is_file()) {
                let _ = crate::install::to_recycle_bin(&f.path());
            }
        }
        set_up.push(name.to_string());
    }
    set_up
}

/// BAM's VR settings from TerryRed's zip of them: all five, or none.
fn vr_settings_from_zip(zip: &Path) -> Option<Vec<(&'static str, Vec<u8>)>> {
    let mut archive = ::zip::ZipArchive::new(std::fs::File::open(zip).ok()?).ok()?;
    VR_SETTINGS_FILES
        .iter()
        .map(|&name| {
            let mut entry = archive.by_name(name).ok()?;
            let mut bytes = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut bytes).ok()?;
            Some((name, bytes))
        })
        .collect()
}

/// BAM's VR settings from the folder TerryRed's guide keeps them in: all five, or none.
fn vr_settings_from_folder(folder: &Path) -> Option<Vec<(&'static str, Vec<u8>)>> {
    VR_SETTINGS_FILES.iter().map(|&name| std::fs::read(name.split('/').fold(folder.to_path_buf(), |path, part| path.join(part))).ok().map(|b| (name, b))).collect()
}

/// BAM's VR loader for a game in VR: its OpenComposite one (`BAM-OpenVR-OC`, straight to the PC's
/// OpenXR program, which TerryRed's guide has for Meta's Link) where `openxr` — the VR program one
/// whose OpenXR takes OpenComposite's OpenGL pictures, Link's or Virtual Desktop's own, which
/// came after TerryRed's guide said Virtual Desktop goes through SteamVR — and it's set up, else its
/// SteamVR one (`BAM-OpenVR`); set up meaning BAM's VR settings are in its folder (its `default.cfg`,
/// as TerryRed's "BAM settings - VR - Reset and Install" puts them). None, said why, where neither
/// is.
pub fn vr_loader(install: &Install, openxr: bool) -> Result<PathBuf, String> {
    let set_up = |name: &str| {
        let folder = install.folder.join(name);
        (folder.join("FPLoader.exe").is_file() && folder.join("default.cfg").is_file()).then(|| folder.join("FPLoader.exe"))
    };
    let order: &[&str] = if openxr { &["BAM-OpenVR-OC", "BAM-OpenVR"] } else { &["BAM-OpenVR", "BAM-OpenVR-OC"] };
    order.iter().find_map(|n| set_up(n)).ok_or_else(|| {
        "BAM's VR isn't set up in Future Pinball's folder: its BAM-OpenVR folders need BAM's VR settings, and TerryRed's set of them (\"BAM settings for VR\") isn't there to put in place.".to_string()
    })
}

/// Whether BAM's VR `loader` is its OpenComposite one (`BAM-OpenVR-OC`), which reaches the VR program
/// through OpenXR, so through the one OpenXR's loader is handed (`XR_RUNTIME_JSON`) where one is.
pub fn through_open_composite(loader: &Path) -> bool {
    loader.parent().and_then(Path::file_name).is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("BAM-OpenVR-OC"))
}

/// The command that plays a table in VR through BAM's VR `loader`, from its folder: `play_command`'s,
/// but for the arcade's drawing, which VR leaves off.
pub fn play_command_vr(loader: &Path, table: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(loader);
    command.current_dir(loader.parent().unwrap_or(loader)).arg("/open").arg(table).arg("/play").arg("/exit").arg("/STAYINRAM");
    command
}

/// Where a screen VPX's settings name sits on the desktop, from its name (`Main Screen [0, 0]`, as
/// VPX names a display with its place).
pub fn screen_place(vpx_display: &str) -> Option<(i32, i32)> {
    let inside = vpx_display.rsplit_once('[')?.1.strip_suffix(']')?;
    let (x, y) = inside.split_once(',')?;
    Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
}

/// The command that plays a table and quits once it's over (`/exit`), drawn for a cabinet's screens
/// as Future Pinball's settings set them (`/arcaderender`): BAM's loader from BAM's folder where BAM
/// is there, kept running until Future Pinball ends (`/STAYINRAM`), else Future Pinball's own
/// program from its folder.
pub fn play_command(install: &Install, table: &Path) -> std::process::Command {
    let (program, folder) = match &install.loader {
        Some(loader) => (loader.clone(), loader.parent().map_or_else(|| install.folder.clone(), Path::to_path_buf)),
        None => (install.program(), install.folder.clone()),
    };
    let mut command = std::process::Command::new(program);
    command.current_dir(folder).arg("/open").arg(table).arg("/play").arg("/exit").arg("/arcaderender");
    if install.loader.is_some() {
        command.arg("/STAYINRAM");
    }
    command
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bams_open_composite_loader_told_from_its_steamvr_one() {
        assert!(through_open_composite(Path::new(r"C:\vPinball\FuturePinball\BAM-OpenVR-OC\FPLoader.exe")));
        assert!(!through_open_composite(Path::new(r"C:\vPinball\FuturePinball\BAM-OpenVR\FPLoader.exe")));
        assert!(!through_open_composite(Path::new(r"C:\vPinball\FuturePinball\BAM\FPLoader.exe")));
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vpi-fp-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// BAM's VR settings put where a VR folder has none, from the copy zipped in it, its tables'
    /// settings made before them cleared; a folder with its own left as it is.
    #[test]
    fn bams_vr_settings_put_in_a_vr_folder_without_them() {
        let fp = scratch("vr-settings");
        let (steam, oc) = (fp.join("BAM-OpenVR"), fp.join("BAM-OpenVR-OC"));
        for f in [&steam, &oc] {
            std::fs::create_dir_all(f.join("cfg")).unwrap();
            std::fs::write(f.join("FPLoader.exe"), b"").unwrap();
        }
        // the SteamVR folder has settings of its own
        std::fs::write(steam.join("default.cfg"), b"its own").unwrap();
        // the OpenComposite one has none, its set zipped in it
        {
            let mut zip = ::zip::ZipWriter::new(std::fs::File::create(oc.join("BAM settings for VR.zip")).unwrap());
            let stored = ::zip::write::SimpleFileOptions::default().compression_method(::zip::CompressionMethod::Stored);
            for name in VR_SETTINGS_FILES {
                zip.start_file(name, stored).unwrap();
                std::io::Write::write_all(&mut zip, format!("VR's {name}").as_bytes()).unwrap();
            }
            zip.finish().unwrap();
        }
        let install = Install { folder: fp.clone(), loader: None };
        assert_eq!(put_vr_settings_in_place(&install), vec!["BAM-OpenVR-OC".to_string()]);
        assert_eq!(std::fs::read(oc.join("default.cfg")).unwrap(), b"VR's default.cfg");
        assert_eq!(std::fs::read(oc.join("plugins").join("OpenVR.cfg")).unwrap(), b"VR's plugins/OpenVR.cfg");
        assert_eq!(std::fs::read(steam.join("default.cfg")).unwrap(), b"its own");
        // set up now: the Link route taken for Meta's VR program, nothing put again
        assert_eq!(vr_loader(&install, true).unwrap(), oc.join("FPLoader.exe"));
        assert!(put_vr_settings_in_place(&install).is_empty());
        let _ = std::fs::remove_dir_all(&fp);
    }

    #[test]
    fn found_beside_vpx_with_bam_and_its_tables_listed() {
        let root = scratch("find");
        let vpx = root.join("VisualPinball");
        let fp = root.join("FuturePinball");
        std::fs::create_dir_all(&vpx).unwrap();
        std::fs::create_dir_all(fp.join("BAM")).unwrap();
        std::fs::create_dir_all(fp.join("Tables").join("Its own folder")).unwrap();
        std::fs::write(fp.join(PROGRAM), b"").unwrap();
        // a folder named but not holding it: looked for beside VPX instead
        assert_eq!(find(Some(&root.join("elsewhere")), Some(&vpx)).map(|i| i.loader), Some(None));
        std::fs::write(fp.join("BAM").join("FPLoader.exe"), b"").unwrap();
        let install = find(None, Some(&vpx)).unwrap();
        assert_eq!(install.folder, fp);
        assert_eq!(install.loader.as_deref(), Some(fp.join("BAM").join("FPLoader.exe").as_path()));
        std::fs::write(fp.join("Tables").join("Retroflair (Original 2020).fpt"), b"").unwrap();
        std::fs::write(fp.join("Tables").join("Its own folder").join("Space Lizard.FPT"), b"").unwrap();
        std::fs::write(fp.join("Tables").join("Retroflair (Original 2020).fpl"), b"").unwrap();
        let names: Vec<String> = tables(&install).iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(names, ["Space Lizard.FPT", "Retroflair (Original 2020).fpt"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_editions_own_name_where_it_gives_no_maker_and_year() {
        assert_eq!(own_name("Retroflair - BAM Edition (PinEvent V2 - FizX - 2.0)"), "Retroflair");
        assert_eq!(own_name("Space Lizard (PinEvent)"), "Space Lizard");
        assert_eq!(own_name("RetroFlair (Original 2012) 2.2"), "RetroFlair (Original 2012) 2.2");
        assert_eq!(own_name("Retroflair"), "Retroflair");
    }

    #[test]
    fn its_keys_set_as_vpxs_settings_map_them() {
        let pad = "SDLJoy_030000005e040000130b000009057801_1";
        let newer = vpi_settings::parse_ini(&format!(
            "[Input]\nDevices = Key;{pad}\n\
             Mapping.LeftFlipper = Key;225 | {pad};514;o;-0.300000\n\
             Mapping.RightFlipper = Key;229 | {pad};517;o;-0.300000\n\
             Mapping.LaunchBall = {pad};0\n\
             Mapping.Start = Key;30 | {pad};1\n\
             Mapping.InGameUI = Key;69 | {pad};4\n"
        ));
        // the keys VPX's settings name, and none for an action they press with the controller alone
        assert_eq!(key_settings(&newer), [("LeftFlipperKey", Value::Number(0x2A)), ("RightFlipperKey", Value::Number(0x36)), ("StartGameKey", Value::Number(0x02))]);
        // its own setting, else its key unset; BAM's menu and Esc for those it keeps none for
        let own = |name: &str| (name == "PlungerKey").then_some(0x39);
        assert_eq!(key_for("LaunchBall", own), Some(0x39));
        assert_eq!(key_for("Credit1", own), Some(0x06));
        assert_eq!(key_for("InGameUI", own), Some(0x10));
        assert_eq!(key_for("Tilt", own), None);
        // the controller pressing them: its trigger the left flipper's key, A the plunger's, B start,
        // the left shoulder BAM's menu
        let keys = crate::controller_1080::keys_with(&newer, |action| key_for(action, own)).unwrap();
        let key_of = |code: u8| keys.keys.iter().find(|(k, _)| *k == code).map(|(_, i)| i.clone());
        assert_eq!(key_of(0x2A), Some(vec![crate::controller_1080::Input::Axis { axis: 2, point: -0.3, reversed: false }]));
        assert_eq!(key_of(0x39), Some(vec![crate::controller_1080::Input::Button(0x1000)]));
        assert_eq!(key_of(0x02), Some(vec![crate::controller_1080::Input::Button(0x2000)]));
        assert_eq!(key_of(0x10), Some(vec![crate::controller_1080::Input::Button(0x0100)]));
    }

    #[test]
    fn its_screens_set_as_the_apps_for_vpx() {
        let playfield = Window { screen: r"\\.\DISPLAY2".into(), width: 3840, height: 2160, full: true };
        let backbox = Window { screen: r"\\.\DISPLAY1".into(), width: 1920, height: 1440, full: false };
        let both = screen_settings(Some(&playfield), Some(&backbox), false);
        let get = |s: &[(&str, Value)], n: &str| s.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone());
        assert_eq!(get(&both, "PlayfieldMonitorID"), Some(Value::Text(r"\\.\DISPLAY2".into())));
        assert_eq!(get(&both, "Width"), Some(Value::Number(3840)));
        assert_eq!(get(&both, "ArcadeMode"), Some(Value::Number(1)));
        assert_eq!(get(&both, "SecondMonitorWidth"), Some(Value::Number(1920)));
        assert_eq!(get(&both, "SecondMonitorFullScreen"), Some(Value::Number(0)));
        // a PinUP pack, or no backglass window: no backbox, no arcade mode
        for none in [screen_settings(Some(&playfield), Some(&backbox), true), screen_settings(Some(&playfield), None, false)] {
            assert_eq!(get(&none, "ArcadeMode"), Some(Value::Number(0)));
            assert_eq!(get(&none, "SecondMonitorEnable"), Some(Value::Number(0)));
            assert_eq!(get(&none, "BackboxMonitorID"), None);
        }
        assert_eq!(screen_place("Main Screen [0, 0]"), Some((0, 0)));
        assert_eq!(screen_place("Wide Screen [-3440, 0]"), Some((-3440, 0)));
        assert_eq!(screen_place("Main Screen"), None);
    }

    #[test]
    fn played_through_bams_loader_else_future_pinball_itself() {
        let with_bam = Install { folder: PathBuf::from(r"C:\FP"), loader: Some(PathBuf::from(r"C:\FP\BAM\FPLoader.exe")) };
        let table = PathBuf::from(r"C:\FP\Tables\Retroflair.fpt");
        let c = play_command(&with_bam, &table);
        assert_eq!(c.get_program(), r"C:\FP\BAM\FPLoader.exe");
        assert_eq!(c.get_current_dir(), Some(Path::new(r"C:\FP\BAM")));
        let args: Vec<_> = c.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["/open", r"C:\FP\Tables\Retroflair.fpt", "/play", "/exit", "/arcaderender", "/STAYINRAM"]);
        let without = Install { folder: PathBuf::from(r"C:\FP"), loader: None };
        let c = play_command(&without, &table);
        assert_eq!(c.get_program(), r"C:\FP\Future Pinball.exe");
        assert_eq!(c.get_current_dir(), Some(Path::new(r"C:\FP")));
    }

    /// This PC's Future Pinball, its tables and their VPS matches: `cargo test -p library --lib
    /// future_pinball_on_this_pc -- --ignored --nocapture`. Nothing started.
    #[test]
    #[ignore]
    fn future_pinball_on_this_pc() {
        let vpx = PathBuf::from(r"C:\vPinball\VisualPinball");
        let install = find(None, Some(&vpx)).expect("Future Pinball beside VPX");
        println!("{install:?}");
        let db = crate::vps::default_path().and_then(|p| crate::vps::Database::load(&p).ok());
        for t in tables(&install) {
            let m = db.as_ref().and_then(|db| matched(db, &t));
            println!("{} -> {:?}", t.display(), m.map(|(g, f)| (g.name.clone(), g.id.clone(), f.map(|f| f.id.clone()))));
            println!("  {:?}", play_command(&install, &t));
        }
    }
}
