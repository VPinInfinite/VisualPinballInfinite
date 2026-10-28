// Ported from FlexDMD 1.9.1 (https://github.com/vbousquet/flexdmd): FlexDMD/Actors/Layout.cs, itself adapted from
// libGDX's Scaling (Nathan Sweet). Copyright 2019 Vincent Bousquet, licensed under the Apache License, Version 2.0
// (LICENSE-FlexDMD.txt beside this file). Changed in the porting, from C# to Rust, for Visual Pinball Infinite;
// the port is GPL-3.0-or-later.

//! How a picture is sized into its actor's box (`Scaling`) and where
//! something sits inside a box (`Alignment`), as scripts number them.

/// How a source is scaled to its target box; the script's numbers, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scaling {
    /// Fits inside, keeping its shape (smaller than the box one way).
    Fit,
    /// Fills the box, keeping its shape (larger than the box one way).
    Fill,
    /// As wide as the box, keeping its shape.
    FillX,
    /// As tall as the box, keeping its shape.
    FillY,
    /// Exactly the box, its shape changed.
    #[default]
    Stretch,
    /// As wide as the box, its height kept.
    StretchX,
    /// As tall as the box, its width kept.
    StretchY,
    /// Its own size.
    None,
    /// A number outside the list, which a script can set: no size at all.
    Other(i32),
}

impl Scaling {
    /// The script's number for it.
    pub fn from_i32(n: i32) -> Scaling {
        use Scaling::*;
        let known = [Fit, Fill, FillX, FillY, Stretch, StretchX, StretchY, None];
        usize::try_from(n).ok().and_then(|i| known.get(i).copied()).unwrap_or(Other(n))
    }

    /// Its number, as the script reads it back.
    pub fn to_i32(self) -> i32 {
        use Scaling::*;
        match self {
            Fit => 0,
            Fill => 1,
            FillX => 2,
            FillY => 3,
            Stretch => 4,
            StretchX => 5,
            StretchY => 6,
            None => 7,
            Other(n) => n,
        }
    }
}

/// Where something sits in a box; the script's numbers, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    TopLeft,
    Top,
    TopRight,
    Left,
    #[default]
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// A number outside the list, which a script can set: the top left.
    Other(i32),
}

const ALIGNMENTS: [Alignment; 9] = {
    use Alignment::*;
    [TopLeft, Top, TopRight, Left, Center, Right, BottomLeft, Bottom, BottomRight]
};

impl Alignment {
    /// The script's number for it.
    pub fn from_i32(n: i32) -> Alignment {
        usize::try_from(n).ok().and_then(|i| ALIGNMENTS.get(i).copied()).unwrap_or(Alignment::Other(n))
    }

    /// Its number, as the script reads it back.
    pub fn to_i32(self) -> i32 {
        match self {
            Alignment::Other(n) => n,
            a => ALIGNMENTS.iter().position(|&k| k == a).unwrap_or(0) as i32,
        }
    }

    /// A left-hand alignment (top, middle or bottom).
    pub fn is_left(self) -> bool {
        matches!(self, Alignment::TopLeft | Alignment::Left | Alignment::BottomLeft)
    }
}

/// The size a source of `source` takes in a box of `target` (FlexDMD's `Layout.Scale`).
pub fn scale(mode: Scaling, source: (f32, f32), target: (f32, f32)) -> (f32, f32) {
    let ((sw, sh), (tw, th)) = (source, target);
    match mode {
        Scaling::Fit | Scaling::Fill => {
            let target_ratio = th / tw;
            let source_ratio = sh / sw;
            let wide = if mode == Scaling::Fit { target_ratio > source_ratio } else { target_ratio < source_ratio };
            let scale = if wide { tw / sw } else { th / sh };
            (sw * scale, sh * scale)
        }
        Scaling::FillX => {
            let scale = tw / sw;
            (sw * scale, sh * scale)
        }
        Scaling::FillY => {
            let scale = th / sh;
            (sw * scale, sh * scale)
        }
        Scaling::Stretch => (tw, th),
        Scaling::StretchX => (tw, sh),
        Scaling::StretchY => (sw, th),
        Scaling::None => (sw, sh),
        Scaling::Other(_) => (0.0, 0.0),
    }
}

/// Where something of `size` sits in a box of `container`, from its top left
/// (FlexDMD's `Layout.Align`).
pub fn align(mode: Alignment, size: (f32, f32), container: (f32, f32)) -> (f32, f32) {
    use Alignment::*;
    let ((w, h), (cw, ch)) = (size, container);
    let x = match mode {
        TopLeft | Left | BottomLeft | Other(_) => 0.0,
        Top | Center | Bottom => (cw - w) * 0.5,
        TopRight | Right | BottomRight => cw - w,
    };
    let y = match mode {
        TopLeft | Top | TopRight | Other(_) => 0.0,
        Left | Center | Right => (ch - h) * 0.5,
        BottomLeft | Bottom | BottomRight => ch - h,
    };
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_and_alignment_follow_flexdmd() {
        // a 128×32 picture in a 64×64 box
        assert_eq!(scale(Scaling::Fit, (128.0, 32.0), (64.0, 64.0)), (64.0, 16.0));
        assert_eq!(scale(Scaling::Fill, (128.0, 32.0), (64.0, 64.0)), (256.0, 64.0));
        assert_eq!(scale(Scaling::FillX, (128.0, 32.0), (64.0, 64.0)), (64.0, 16.0));
        assert_eq!(scale(Scaling::FillY, (128.0, 32.0), (64.0, 64.0)), (256.0, 64.0));
        assert_eq!(scale(Scaling::StretchX, (128.0, 32.0), (64.0, 64.0)), (64.0, 32.0));
        assert_eq!(scale(Scaling::None, (128.0, 32.0), (64.0, 64.0)), (128.0, 32.0));
        assert_eq!(align(Alignment::Center, (10.0, 5.0), (128.0, 32.0)), (59.0, 13.5));
        assert_eq!(align(Alignment::BottomRight, (10.0, 5.0), (128.0, 32.0)), (118.0, 27.0));
        assert_eq!(Alignment::from_i32(8), Alignment::BottomRight);
        assert_eq!(Alignment::from_i32(8).to_i32(), 8);
        assert_eq!(scale(Scaling::from_i32(8), (128.0, 32.0), (64.0, 64.0)), (0.0, 0.0));
    }
}
