use core_graphics::display::CGDisplay;
use core_graphics::geometry::{CGPoint, CGRect, CGSize};

/// Screen regions a window can be snapped to. Covers the split layouts the
/// product wants: halves (2-split), thirds (3-split), quadrants (4-split).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Full,
    LeftHalf,
    RightHalf,
    LeftThird,
    CenterThird,
    RightThird,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Region {
    pub fn parse(label: &str) -> Option<Self> {
        match label {
            "full" => Some(Self::Full),
            "left" => Some(Self::LeftHalf),
            "right" => Some(Self::RightHalf),
            "left-third" => Some(Self::LeftThird),
            "center-third" => Some(Self::CenterThird),
            "right-third" => Some(Self::RightThird),
            "top-left" => Some(Self::TopLeft),
            "top-right" => Some(Self::TopRight),
            "bottom-left" => Some(Self::BottomLeft),
            "bottom-right" => Some(Self::BottomRight),
            _ => None,
        }
    }

    pub const ALL_LABELS: &'static str =
        "full | left | right | left-third | center-third | right-third | \
         top-left | top-right | bottom-left | bottom-right";

    /// Target frame for this region within a display frame, in the global
    /// top-left-origin coordinate space that the AX API uses.
    pub fn frame(self, display: CGRect) -> CGRect {
        let x = display.origin.x;
        let y = display.origin.y;
        let w = display.size.width;
        let h = display.size.height;

        let rect = |rx: f64, ry: f64, rw: f64, rh: f64| CGRect {
            origin: CGPoint {
                x: x + w * rx,
                y: y + h * ry,
            },
            size: CGSize {
                width: w * rw,
                height: h * rh,
            },
        };

        match self {
            Self::Full => rect(0.0, 0.0, 1.0, 1.0),
            Self::LeftHalf => rect(0.0, 0.0, 0.5, 1.0),
            Self::RightHalf => rect(0.5, 0.0, 0.5, 1.0),
            Self::LeftThird => rect(0.0, 0.0, 1.0 / 3.0, 1.0),
            Self::CenterThird => rect(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
            Self::RightThird => rect(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0),
            Self::TopLeft => rect(0.0, 0.0, 0.5, 0.5),
            Self::TopRight => rect(0.5, 0.0, 0.5, 0.5),
            Self::BottomLeft => rect(0.0, 0.5, 0.5, 0.5),
            Self::BottomRight => rect(0.5, 0.5, 0.5, 0.5),
        }
    }
}

pub fn main_display_frame() -> CGRect {
    CGDisplay::main().bounds()
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: CGRect = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize {
            width: 1600.0,
            height: 1000.0,
        },
    };

    #[test]
    fn halves_split_the_width() {
        let left = Region::LeftHalf.frame(DISPLAY);
        let right = Region::RightHalf.frame(DISPLAY);
        assert_eq!(left.size.width, 800.0);
        assert_eq!(right.origin.x, 800.0);
        assert_eq!(left.size.height, 1000.0);
    }

    #[test]
    fn quadrants_split_both_axes() {
        let bottom_right = Region::BottomRight.frame(DISPLAY);
        assert_eq!(bottom_right.origin.x, 800.0);
        assert_eq!(bottom_right.origin.y, 500.0);
        assert_eq!(bottom_right.size.width, 800.0);
        assert_eq!(bottom_right.size.height, 500.0);
    }

    #[test]
    fn thirds_tile_the_width() {
        let center = Region::CenterThird.frame(DISPLAY);
        assert!((center.origin.x - 1600.0 / 3.0).abs() < 1e-6);
        assert!((center.size.width - 1600.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn parse_rejects_unknown_label() {
        assert_eq!(Region::parse("diagonal"), None);
        assert_eq!(Region::parse("left"), Some(Region::LeftHalf));
    }
}
