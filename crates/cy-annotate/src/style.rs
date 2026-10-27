//! Colours, stroke styles and the drawing constants shared with the shell's live preview.
//!
//! Every size in a [`Style`] and in [`StyleConstants`] is in points; the renderer multiplies by
//! the base image's scale (2 on a Retina screenshot), so a 4-point arrow looks the same on every
//! display. The shell draws its preview with the same constants (ADR-10).

use serde::{Deserialize, Serialize};

use crate::AnnotateError;
use crate::limits::{MAX_STROKE_WIDTH, MIN_STROKE_WIDTH};

/// A colour with straight (not premultiplied) alpha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha; 255 is opaque.
    pub a: u8,
}

impl Rgba {
    /// An opaque colour.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// The default annotation red.
    pub const RED: Self = Self::rgb(235, 64, 52);
    /// White, used for step numbers.
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    /// Yellow, the default highlighter colour.
    pub const YELLOW: Self = Self::rgb(255, 221, 0);
}

/// How a shape is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Style {
    /// Stroke, fill and text colour.
    pub color: Rgba,
    /// Line width in points ([`MIN_STROKE_WIDTH`]..=[`MAX_STROKE_WIDTH`]).
    pub stroke_width: f32,
    /// Opacity from 0 to 1, multiplied with the colour's alpha.
    pub opacity: f32,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            color: Rgba::RED,
            stroke_width: 4.0,
            opacity: 1.0,
        }
    }
}

impl Style {
    /// Checks the ranges.
    ///
    /// # Errors
    ///
    /// [`AnnotateError::InvalidStyle`] for a width or opacity out of range.
    pub fn validate(&self) -> Result<(), AnnotateError> {
        let width_ok = (MIN_STROKE_WIDTH..=MAX_STROKE_WIDTH).contains(&self.stroke_width);
        let opacity_ok = (0.0..=1.0).contains(&self.opacity);
        if width_ok && opacity_ok {
            Ok(())
        } else {
            Err(AnnotateError::InvalidStyle)
        }
    }

    /// The colour's alpha times the opacity, from 0 to 1.
    #[must_use]
    pub fn alpha(&self) -> f32 {
        f32::from(self.color.a) / 255.0 * self.opacity
    }
}

/// Arrow head length per point of stroke width.
pub const ARROW_HEAD_LENGTH_PER_WIDTH: f32 = 4.0;
/// Shortest arrow head, in points.
pub const ARROW_HEAD_MIN_LENGTH: f32 = 10.0;
/// Half the opening angle of the arrow head, in degrees.
pub const ARROW_HEAD_HALF_ANGLE_DEGREES: f32 = 25.0;
/// Corner radius of rectangles per point of stroke width (capped at half the shorter side).
pub const RECT_CORNER_RADIUS_PER_WIDTH: f32 = 1.0;
/// Highlighter opacity, before the style's own opacity.
pub const HIGHLIGHTER_ALPHA: f32 = 0.4;
/// Highlighter stroke width per point of style width.
pub const HIGHLIGHTER_WIDTH_FACTOR: f32 = 4.0;
/// Step marker radius at zero stroke width, in points.
pub const STEP_BASE_RADIUS: f32 = 10.0;
/// Extra step marker radius per point of stroke width.
pub const STEP_RADIUS_PER_WIDTH: f32 = 1.5;
/// Height of the number in a step marker per point of radius.
pub const STEP_FONT_SIZE_PER_RADIUS: f32 = 1.1;
/// Text line height per point of font size.
pub const TEXT_LINE_HEIGHT: f32 = 1.25;
/// How far from a shape a click still selects it, in points.
pub const HIT_TOLERANCE: f32 = 6.0;

/// The drawing constants, for the shell's preview (exposed over FFI).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StyleConstants {
    /// [`ARROW_HEAD_LENGTH_PER_WIDTH`].
    pub arrow_head_length_per_width: f32,
    /// [`ARROW_HEAD_MIN_LENGTH`].
    pub arrow_head_min_length: f32,
    /// [`ARROW_HEAD_HALF_ANGLE_DEGREES`].
    pub arrow_head_half_angle_degrees: f32,
    /// [`RECT_CORNER_RADIUS_PER_WIDTH`].
    pub rect_corner_radius_per_width: f32,
    /// [`HIGHLIGHTER_ALPHA`]; the highlighter blends with multiply.
    pub highlighter_alpha: f32,
    /// [`HIGHLIGHTER_WIDTH_FACTOR`].
    pub highlighter_width_factor: f32,
    /// [`STEP_BASE_RADIUS`].
    pub step_base_radius: f32,
    /// [`STEP_RADIUS_PER_WIDTH`].
    pub step_radius_per_width: f32,
    /// [`STEP_FONT_SIZE_PER_RADIUS`].
    pub step_font_size_per_radius: f32,
    /// [`TEXT_LINE_HEIGHT`].
    pub text_line_height: f32,
    /// [`HIT_TOLERANCE`].
    pub hit_tolerance: f32,
    /// Lines, arrows, pen and highlighter strokes use round caps and round joins.
    pub round_caps: bool,
}

/// The constants the renderer uses.
pub const STYLE_CONSTANTS: StyleConstants = StyleConstants {
    arrow_head_length_per_width: ARROW_HEAD_LENGTH_PER_WIDTH,
    arrow_head_min_length: ARROW_HEAD_MIN_LENGTH,
    arrow_head_half_angle_degrees: ARROW_HEAD_HALF_ANGLE_DEGREES,
    rect_corner_radius_per_width: RECT_CORNER_RADIUS_PER_WIDTH,
    highlighter_alpha: HIGHLIGHTER_ALPHA,
    highlighter_width_factor: HIGHLIGHTER_WIDTH_FACTOR,
    step_base_radius: STEP_BASE_RADIUS,
    step_radius_per_width: STEP_RADIUS_PER_WIDTH,
    step_font_size_per_radius: STEP_FONT_SIZE_PER_RADIUS,
    text_line_height: TEXT_LINE_HEIGHT,
    hit_tolerance: HIT_TOLERANCE,
    round_caps: true,
};

/// Radius of a step marker drawn with `stroke_width`, in points.
#[must_use]
pub fn step_radius(stroke_width: f32) -> f32 {
    STEP_BASE_RADIUS + STEP_RADIUS_PER_WIDTH * stroke_width
}

/// Length of an arrow head drawn with `stroke_width`, in points.
#[must_use]
pub fn arrow_head_length(stroke_width: f32) -> f32 {
    (ARROW_HEAD_LENGTH_PER_WIDTH * stroke_width).max(ARROW_HEAD_MIN_LENGTH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_style_is_valid_and_ranges_are_checked() {
        assert_eq!(Style::default().validate(), Ok(()));
        let thin = Style {
            stroke_width: 0.1,
            ..Style::default()
        };
        let see_through = Style {
            opacity: 1.5,
            ..Style::default()
        };
        let nan = Style {
            stroke_width: f32::NAN,
            ..Style::default()
        };
        for style in [thin, see_through, nan] {
            assert_eq!(style.validate(), Err(AnnotateError::InvalidStyle));
        }
    }

    #[test]
    fn alpha_combines_colour_and_opacity() {
        let style = Style {
            color: Rgba { a: 51, ..Rgba::RED },
            opacity: 0.5,
            ..Style::default()
        };
        assert!((style.alpha() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn derived_sizes_follow_the_constants() {
        assert!((step_radius(4.0) - 16.0).abs() < 1e-6);
        assert!((arrow_head_length(1.0) - ARROW_HEAD_MIN_LENGTH).abs() < 1e-6);
        assert!((arrow_head_length(5.0) - 20.0).abs() < 1e-6);
        assert!((STYLE_CONSTANTS.highlighter_alpha - HIGHLIGHTER_ALPHA).abs() < 1e-6);
    }
}
