//! The Virtual Pinball Spreadsheet's database (VPS, `vpsdb.json` from
//! VirtualPinballSpreadsheet/vps-db): every game, the table files made of it,
//! and their backglasses, ROMs, PinUP packs, AltSound packs, colourings, POV
//! files and the rest, each with its own ID, authors and links. The database
//! states no licence, so it's never shipped or committed: the app fetches it,
//! as VPin Studio does, and keeps it in the player's data folder. Ours.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where VPS publishes its database.
pub const URL: &str = "https://raw.githubusercontent.com/VirtualPinballSpreadsheet/vps-db/main/db/vpsdb.json";

/// A link to a game's page on VPS.
pub const GAME_LINK: &str = "https://virtualpinballspreadsheet.github.io/?game=";

/// One game: a real machine, or an original table, with everything made for it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Game {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub year: Option<i32>,
    /// SS, EM, PM, DG (digital).
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub players: Option<i32>,
    pub designers: Vec<String>,
    pub theme: Vec<String>,
    pub ipdb_url: Option<String>,
    #[serde(rename = "MPU")]
    pub mpu: Option<String>,
    pub img_url: Option<String>,
    pub table_files: Vec<TableFile>,
    pub b2s_files: Vec<Asset>,
    pub rom_files: Vec<Asset>,
    pub pup_pack_files: Vec<Asset>,
    pub alt_sound_files: Vec<Asset>,
    pub alt_color_files: Vec<Asset>,
    pub pov_files: Vec<Asset>,
    pub wheel_art_files: Vec<Asset>,
    pub topper_files: Vec<Asset>,
    pub media_pack_files: Vec<Asset>,
    pub sound_files: Vec<Asset>,
    pub rule_files: Vec<Asset>,
    pub tutorial_files: Vec<Asset>,
    pub updated_at: Option<i64>,
    pub last_created_at: Option<i64>,
    pub broken: Option<bool>,
}

/// One table file made of a game: its version, authors and format.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TableFile {
    pub id: String,
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub comment: Option<String>,
    pub features: Vec<String>,
    /// VPX, FP, FX3 and the like.
    pub table_format: Option<String>,
    /// Milliseconds since 1970.
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub urls: Vec<Link>,
    pub img_url: Option<String>,
}

/// Anything else made for a game: a backglass, a ROM, a pack, a POV file.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Asset {
    pub id: String,
    /// A ROM's set name, or the file's version.
    pub version: Option<String>,
    pub authors: Vec<String>,
    pub comment: Option<String>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
    pub urls: Vec<Link>,
    /// A colouring's kind (Serum, PIN2DMD), folder and file name.
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub folder: Option<String>,
    pub file_name: Option<String>,
    /// A tutorial's title and link.
    pub title: Option<String>,
    pub url: Option<String>,
    /// A backglass file's picture, on VPS.
    pub img_url: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct Link {
    pub url: String,
    pub broken: Option<bool>,
}

/// VPS's database, as read.
#[derive(Clone, Debug, Default)]
pub struct Database {
    pub games: Vec<Game>,
}

impl Database {
    pub fn load(path: &Path) -> Result<Database, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("VPS's database, {}: {e}", path.display()))?;
        Self::from_json(&text)
    }

    pub fn from_json(text: &str) -> Result<Database, String> {
        let games: Vec<Game> = serde_json::from_str(text).map_err(|e| format!("VPS's database: {e}"))?;
        Ok(Database { games })
    }

    pub fn game(&self, id: &str) -> Option<&Game> {
        self.games.iter().find(|g| g.id == id)
    }

    /// The game a table file belongs to, by the table file's ID.
    pub fn game_of_table_file(&self, id: &str) -> Option<(&Game, &TableFile)> {
        self.games.iter().find_map(|g| g.table_files.iter().find(|t| t.id == id).map(|t| (g, t)))
    }
}

/// Where the app keeps VPS's database: `VPS\vpsdb.json` in the player's data folder.
pub fn default_path() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Visual Pinball Infinite").join("VPS").join("vpsdb.json"))
}

/// Whether a saved copy is missing or more than a day old.
pub fn is_stale(path: &Path) -> bool {
    let age = std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok());
    age.is_none_or(|a| a.as_secs() > 24 * 3600)
}

/// Fetches VPS's database into `path` (a copy beside it first, then moved into place, so a failed
/// download leaves the old copy). Through Windows' own `curl.exe`.
pub fn fetch(path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let part = path.with_extension("json.part");
    let status = curl()
        .args(["--fail", "--silent", "--show-error", "--location", "--max-time", "120", "--output"])
        .arg(&part)
        .arg(URL)
        .status()
        .map_err(|e| format!("curl: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        return Err(format!("couldn't fetch VPS's database ({status})"));
    }
    // a database that doesn't read isn't kept
    let text = std::fs::read_to_string(&part).map_err(|e| e.to_string())?;
    Database::from_json(&text)?;
    std::fs::rename(&part, path).map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_game() {
        let db = Database::from_json(
            r#"[{"id":"9Paf7-CL","name":"Attack from Mars","manufacturer":"Bally","year":1995,"type":"SS",
                "tableFiles":[{"id":"1bCrozNZ","version":"6.0.0","authors":["JPSalas"],"tableFormat":"VPX","createdAt":1757376000000,"urls":[{"url":"https://example.org"}]}],
                "romFiles":[{"id":"uXfojkj3","version":"afm_113b","authors":["Destruk"]}],
                "altColorFiles":[{"id":"x","type":"Serum","folder":"afm_113b","fileName":"afm_113b.cRZ"}]}]"#,
        )
        .unwrap();
        let g = db.game("9Paf7-CL").unwrap();
        assert_eq!((g.year, g.kind.as_deref()), (Some(1995), Some("SS")));
        assert_eq!(g.table_files[0].table_format.as_deref(), Some("VPX"));
        assert_eq!(g.rom_files[0].version.as_deref(), Some("afm_113b"));
        assert_eq!(g.alt_color_files[0].kind.as_deref(), Some("Serum"));
        assert_eq!(db.game_of_table_file("1bCrozNZ").map(|(g, _)| g.name.as_str()), Some("Attack from Mars"));
    }
}

/// Windows' own `curl.exe`, started without a window of its own, for the app's screens.
pub fn curl() -> std::process::Command {
    let mut command = std::process::Command::new("curl.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    command
}
