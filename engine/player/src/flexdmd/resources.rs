//! FlexDMD's own resources, built in as FlexDMD carries them in itself: its
//! fonts and two pictures, which scripts name `FlexDMD.Resources.<name>`
//! (`engine/player/assets/flexdmd`, where their authors and terms are noted).
//! A name is matched
//! exactly, as .NET finds a resource; a folder's path inside FlexDMD is
//! joined with dots (`dmds.black.png`).

/// Each resource by its name after `FlexDMD.Resources.`.
const RESOURCES: &[(&str, &[u8])] = &[
    ("bm_army-12.fnt", include_bytes!("../../assets/flexdmd/bm_army-12.fnt")),
    ("bm_army-12.png", include_bytes!("../../assets/flexdmd/bm_army-12.png")),
    ("colors.png", include_bytes!("../../assets/flexdmd/colors.png")),
    ("teeny_tiny_pixls-5.fnt", include_bytes!("../../assets/flexdmd/teeny_tiny_pixls-5.fnt")),
    ("teeny_tiny_pixls-5.png", include_bytes!("../../assets/flexdmd/teeny_tiny_pixls-5.png")),
    ("udmd-f12by24.fnt", include_bytes!("../../assets/flexdmd/udmd-f12by24.fnt")),
    ("udmd-f12by24.png", include_bytes!("../../assets/flexdmd/udmd-f12by24.png")),
    ("udmd-f14by26.fnt", include_bytes!("../../assets/flexdmd/udmd-f14by26.fnt")),
    ("udmd-f14by26.png", include_bytes!("../../assets/flexdmd/udmd-f14by26.png")),
    ("udmd-f4by5.fnt", include_bytes!("../../assets/flexdmd/udmd-f4by5.fnt")),
    ("udmd-f4by5.png", include_bytes!("../../assets/flexdmd/udmd-f4by5.png")),
    ("udmd-f5by7.fnt", include_bytes!("../../assets/flexdmd/udmd-f5by7.fnt")),
    ("udmd-f5by7.png", include_bytes!("../../assets/flexdmd/udmd-f5by7.png")),
    ("udmd-f6by12.fnt", include_bytes!("../../assets/flexdmd/udmd-f6by12.fnt")),
    ("udmd-f6by12.png", include_bytes!("../../assets/flexdmd/udmd-f6by12.png")),
    ("udmd-f7by13.fnt", include_bytes!("../../assets/flexdmd/udmd-f7by13.fnt")),
    ("udmd-f7by13.png", include_bytes!("../../assets/flexdmd/udmd-f7by13.png")),
    ("udmd-f7by5.fnt", include_bytes!("../../assets/flexdmd/udmd-f7by5.fnt")),
    ("udmd-f7by5.png", include_bytes!("../../assets/flexdmd/udmd-f7by5.png")),
    ("zx_spectrum-7.fnt", include_bytes!("../../assets/flexdmd/zx_spectrum-7.fnt")),
    ("zx_spectrum-7.png", include_bytes!("../../assets/flexdmd/zx_spectrum-7.png")),
    ("dmds.black.png", include_bytes!("../../assets/flexdmd/dmds/black.png")),
];

/// A built-in resource's bytes, by its name after `FlexDMD.Resources.`.
pub fn get(name: &str) -> Option<&'static [u8]> {
    RESOURCES.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_font_and_its_picture_is_built_in() {
        for f in ["bm_army-12", "teeny_tiny_pixls-5", "zx_spectrum-7", "udmd-f4by5", "udmd-f5by7", "udmd-f6by12", "udmd-f7by13", "udmd-f7by5", "udmd-f12by24", "udmd-f14by26"] {
            let fnt = super::get(&format!("{f}.fnt")).expect("the font");
            assert!(String::from_utf8_lossy(fnt).contains(&format!("file=\"{f}.png\"")), "{f} names its picture");
            assert!(super::get(&format!("{f}.png")).is_some(), "{f}'s picture");
        }
        assert!(super::get("dmds.black.png").is_some() && super::get("colors.png").is_some());
        // names match exactly, as .NET's resources do
        assert!(super::get("UDMD-F5BY7.FNT").is_none());
    }
}
