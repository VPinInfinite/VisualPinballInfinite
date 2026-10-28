// Follows Visual Pinball X (license:GPLv3+), build 5436 (10.8.1 pre-release, commit af26b2d): its PinUp Player
// plugin's triggers (plugins/pup/PUPTrigger.cpp, PUPManager.h's PlayAction), with the newer fixes matching
// PinUP Player (to commit a4a1e75ca: f03f32689, 59536ebbb, 16c805a69; and to 401567c15: PlaySSF's
// sounds, 7b7f7ec0e, and custom functions, 7028ee18a).
// Copyright (C) 2000-2026 Visual Pinball development team and contributors.
// This port is also GPL-3.0-or-later.

//! A pack's trigger: what plays when an event comes (`triggers.pup`:
//! `ID,Active,Descript,Trigger,ScreenNum,PlayList,PlayFile,Volume,Priority,Length,Counter,RestSeconds,Loop,Defaults`).
//! A trigger names its events as a letter and a number, with or without a
//! value: `S10` (solenoid 10 at 1), `W5=1` (switch 5 closed); S solenoid, W
//! switch, L lamp, G GI, E an event the script sends, M mech, D the score
//! display or a B2S LED. A negative rest time delays the trigger instead
//! (seconds, or milliseconds from -99 down); a counter fires it on hits 1,
//! N+1, 2N+1…; `Defaults` plays its playlist's next file at the screen's
//! default volume. A `PlaySSF` trigger plays a sound of the pack's sound bank
//! at the `x,y,z` its counter column holds; a `CustomFunc` one runs the custom
//! function its playlist column holds on its screen, and sends its file column
//! as a message when there's one.

use super::playlist::Playlist;
use super::{parse_csv_line, string_to_float, string_to_int};

/// How a trigger, or a script's `playevent`, plays (VPX's `PlayAction`, in its order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayAction {
    /// Plays the file to its end.
    Normal,
    /// Plays it round and round.
    Loop,
    /// Over a looping file: plays, then the loop starts again from its beginning.
    SplashReset,
    /// Over a looping file: plays, then the loop goes on where it was.
    SplashReturn,
    /// Stops what's playing, when its priority is higher.
    StopPlayer,
    /// Stops only the file named, if it's playing.
    StopFile,
    /// Sets the background file, which loops when nothing else plays.
    SetBG,
    /// A sound of the pack's sound bank (`PUPSOUNDS`), placed left to right.
    PlaySSF,
    /// Not played if what's playing has the same priority.
    SkipSamePriority,
    /// A custom function (`SendMSG` 301's), on the trigger's screen.
    CustomFunction,
    /// Queued after what plays, when it passes the priority rules (11).
    QueueSameP,
    /// Queued after what plays, whatever its priority (12).
    QueueAlways,
}

impl PlayAction {
    /// The action of a number, as scripts pass it (`playevent`; 10 is none).
    pub fn from_number(n: i32) -> Option<PlayAction> {
        use PlayAction::*;
        match n {
            11 => Some(QueueSameP),
            12 => Some(QueueAlways),
            _ => [Normal, Loop, SplashReset, SplashReturn, StopPlayer, StopFile, SetBG, PlaySSF, SkipSamePriority, CustomFunction].get(usize::try_from(n).ok()?).copied(),
        }
    }
}

/// One of a trigger's events and the value it waits for (1 when none is written).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub kind: char,
    pub number: i32,
    pub expected: i32,
    /// The value last sent.
    pub value: i32,
}

impl Condition {
    pub fn is_triggered(&self) -> bool {
        self.value == self.expected
    }
}

#[derive(Clone, Debug)]
pub struct Trigger {
    pub description: String,
    /// Its events as written (`W5=1,S10`), which groups triggers that share them.
    pub trigger: String,
    /// The playlist it plays, by its index in its screen's playlists (none for a sound or a
    /// custom function).
    pub playlist: Option<usize>,
    pub play_file: String,
    pub volume: f32,
    pub priority: i32,
    pub length: i32,
    pub counter: i32,
    pub rest_seconds: i32,
    pub action: PlayAction,
    pub conditions: Vec<Condition>,
    /// When it last played (the game's milliseconds).
    pub last_triggered: Option<u64>,
    /// `Defaults`: its playlist's next file at the screen's default volume, whatever else it says.
    pub use_defaults: bool,
    /// Hits counted towards its counter.
    pub hit_count: i32,
    /// A delayed play (a negative rest time) is waiting.
    pub delay_pending: bool,
    /// A `PlaySSF`'s place (`x,y,z`), or a `CustomFunc`'s function.
    pub custom_function: String,
}

impl Trigger {
    /// A line of `triggers.pup` for the screen `screen`, its playlist found among `playlists`
    /// (VPX's `CreateFromCSV`): None for another screen's, an inactive one, a comment (no
    /// playlist), a sound without its `x,y,z`, or one whose playlist or file isn't there.
    pub fn from_csv(screen: i32, playlists: &[Playlist], line: &str) -> Option<Trigger> {
        let parts = parse_csv_line(line);
        if parts.len() != 14 {
            return None;
        }
        if string_to_int(&parts[4], -1) != screen || string_to_int(&parts[1], 0) != 1 {
            return None;
        }
        let action_name = parts[12].to_ascii_lowercase();
        let without_playlist = |action: PlayAction, volume: f32, counter: i32, custom_function: &str| Trigger {
            description: parts[2].clone(),
            trigger: parts[3].clone(),
            playlist: None,
            play_file: parts[6].clone(),
            volume,
            priority: 0,
            length: 0,
            counter,
            rest_seconds: string_to_int(&parts[11], 0),
            action,
            conditions: conditions(&parts[3]),
            last_triggered: None,
            use_defaults: false,
            hit_count: 0,
            delay_pending: false,
            custom_function: custom_function.to_string(),
        };
        if action_name == "playssf" {
            // the file column names the sound, the counter column its place; PinUP plays nothing
            // without all three of x, y and z
            if parts[10].matches(',').count() < 2 {
                crate::script::log::once(&format!("PinUP: a PlaySSF trigger without its x,y,z: {line}"));
                return None;
            }
            let volume = if parts[7].is_empty() { 100.0 } else { string_to_float(&parts[7], 100.0) };
            return Some(without_playlist(PlayAction::PlaySSF, volume, 0, &parts[10]));
        }
        if action_name == "customfunc" {
            // the playlist column holds the function (JSON), the file column a message to send
            return Some(without_playlist(PlayAction::CustomFunction, 100.0, string_to_int(&parts[10], 0), &parts[5]));
        }
        // an empty playlist with a description: a comment
        if parts[5].is_empty() {
            return None;
        }
        let playlist = playlists.iter().position(|p| p.folder.eq_ignore_ascii_case(&parts[5]))?;
        let mut play_file = parts[6].clone();
        if !play_file.is_empty() {
            play_file = playlists[playlist].play_file(&play_file)?;
        }
        let action = match action_name.as_str() {
            "loop" => PlayAction::Loop,
            "splashreset" => PlayAction::SplashReset,
            "splashreturn" => PlayAction::SplashReturn,
            "stopplayer" => PlayAction::StopPlayer,
            "stopfile" => PlayAction::StopFile,
            "setbg" => PlayAction::SetBG,
            "skipsameprty" => PlayAction::SkipSamePriority,
            "queuesamep" => PlayAction::QueueSameP,
            "queuealways" => PlayAction::QueueAlways,
            _ => PlayAction::Normal,
        };
        let p = &playlists[playlist];
        Some(Trigger {
            description: parts[2].clone(),
            trigger: parts[3].clone(),
            playlist: Some(playlist),
            play_file,
            volume: if parts[7].is_empty() { p.volume } else { string_to_float(&parts[7], 100.0) },
            priority: string_to_int(&parts[8], 0),
            length: string_to_int(&parts[9], 0),
            counter: string_to_int(&parts[10], 0),
            rest_seconds: string_to_int(&parts[11], 0),
            action,
            conditions: conditions(&parts[3]),
            last_triggered: None,
            use_defaults: string_to_int(&parts[13], 0) == 1,
            hit_count: 0,
            delay_pending: false,
            custom_function: String::new(),
        })
    }

    /// Every event has its value.
    pub fn is_triggered(&self) -> bool {
        self.conditions.iter().all(Condition::is_triggered)
    }

    pub fn is_resting(&self, now: u64) -> bool {
        self.rest_seconds > 0 && self.last_triggered.is_some_and(|t| now.saturating_sub(t) < self.rest_seconds as u64 * 1000)
    }

    /// A negative rest time's delay, in milliseconds: seconds from -1 to -98, milliseconds from
    /// -99 down (VPX's `Trigger`).
    pub fn delay_ms(&self) -> Option<u64> {
        (self.rest_seconds < 0).then(|| if self.rest_seconds > -99 { (-self.rest_seconds) as u64 * 1000 } else { (-self.rest_seconds) as u64 })
    }

    /// A hit counted (VPX's `Fire`): true when it plays — hits 1, N+1, 2N+1… with a counter N
    /// over 1 (PinUP Player never fires a counter of 1; VPX fires it every hit, as here).
    pub fn counts(&mut self) -> bool {
        if self.counter > 1 {
            self.hit_count += 1;
            if self.hit_count >= self.counter {
                self.hit_count = 0;
                return false;
            }
            if self.hit_count > 1 {
                return false;
            }
        }
        true
    }
}

/// A trigger's events (`W5=1,S10`).
fn conditions(text: &str) -> Vec<Condition> {
    let mut out = Vec::new();
    for token in text.split(',') {
        let mut chars = token.chars();
        let Some(kind) = chars.next() else { continue };
        let rest = chars.as_str();
        let (number, expected) = match token.find('=') {
            Some(eq) if eq > 1 => (string_to_int(&token[kind.len_utf8()..eq], -1), string_to_int(&token[eq + 1..], 1)),
            _ if token.chars().count() >= 2 => (string_to_int(rest, -1), 1),
            _ => continue,
        };
        out.push(Condition { kind, number, expected, value: 0 });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_read_with_and_without_a_value() {
        let c = conditions("W5=1,S10,D0");
        assert_eq!(c.len(), 3);
        assert_eq!((c[0].kind, c[0].number, c[0].expected), ('W', 5, 1));
        assert_eq!((c[1].kind, c[1].number, c[1].expected), ('S', 10, 1));
        let mut t = c[1];
        assert!(!t.is_triggered());
        t.value = 1;
        assert!(t.is_triggered());
        // a value other than 1 isn't the one a bare event waits for (PinUP Player's rule)
        t.value = 2;
        assert!(!t.is_triggered());
        let mut w = c[0];
        w.value = 2;
        assert!(!w.is_triggered());
        w.value = 1;
        assert!(w.is_triggered());
    }

    #[test]
    fn counters_fire_on_the_first_hit_then_every_nth() {
        let mut t = Trigger {
            description: String::new(),
            trigger: String::new(),
            playlist: Some(0),
            play_file: String::new(),
            volume: 100.0,
            priority: 0,
            length: 0,
            counter: 3,
            rest_seconds: -150,
            action: PlayAction::Normal,
            conditions: Vec::new(),
            last_triggered: None,
            use_defaults: false,
            hit_count: 0,
            delay_pending: false,
            custom_function: String::new(),
        };
        let fired: Vec<bool> = (0..7).map(|_| t.counts()).collect();
        assert_eq!(fired, [true, false, false, true, false, false, true]);
        assert_eq!(t.delay_ms(), Some(150));
        t.rest_seconds = -2;
        assert_eq!(t.delay_ms(), Some(2000));
        assert_eq!((PlayAction::from_number(10), PlayAction::from_number(12)), (None, Some(PlayAction::QueueAlways)));
    }

    #[test]
    fn sounds_and_custom_functions_need_no_playlist() {
        let line = r#"1,1,Bumper,S10,2,,bumper.wav,80,1,0,"-5,0,0",0,PlaySSF,0"#;
        let t = Trigger::from_csv(2, &[], line).expect("a sound trigger");
        assert_eq!((t.action, t.playlist, t.play_file.as_str(), t.volume, t.custom_function.as_str()), (PlayAction::PlaySSF, None, "bumper.wav", 80.0, "-5,0,0"));
        assert!(Trigger::from_csv(2, &[], r#"1,1,Bumper,S10,2,,bumper.wav,80,1,0,"-5,0",0,PlaySSF,0"#).is_none());
        let line = r#"2,1,Hud off,E5,2,"{""mt"":301,""SN"":2,""FN"":3,""OT"":0}",,,,,0,0,CustomFunc,0"#;
        let t = Trigger::from_csv(2, &[], line).expect("a custom function trigger");
        // the CSV reader drops the doubled quotes, as VPX's does; the JSON is read without them
        assert_eq!((t.action, t.custom_function.as_str()), (PlayAction::CustomFunction, "{mt:301,SN:2,FN:3,OT:0}"));
        let json = super::super::lenient_json(&t.custom_function).expect("read");
        assert_eq!((json["mt"].as_i64(), json["SN"].as_i64(), json["FN"].as_i64(), json["OT"].as_i64()), (Some(301), Some(2), Some(3), Some(0)));
    }
}
