//! The annotation editor object Swift holds while the editor window is open (M4-07).
//!
//! The shell turns gestures into [`AnnotationCommand`]s and draws its live preview from
//! [`AnnotationDocument::shapes`] with [`annotation_style_constants`]; pixel effects and text
//! can be previewed with [`AnnotationDocument::render_region`], and the final image comes from
//! [`AnnotationDocument::export`].

use std::sync::{Arc, Mutex, MutexGuard};

use cy_annotate::{AnnotateError, BaseImage, Document, Editor, Font, STYLE_CONSTANTS};

use crate::annotation_types::ids;
use crate::{
    AnnotationCommand, AnnotationPoint, AnnotationRect, AnnotationShape, ExportImageFormat,
    FileNameParts, RenderedRegion,
};

/// What went wrong in the editor, with a message the shell can show. Messages never contain
/// the user's text or pixels.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AnnotationError {
    /// The image could not be read or is too large.
    #[error("{message}")]
    InvalidImage {
        /// What happened.
        message: String,
    },
    /// A command, region or style value is not valid; nothing changed.
    #[error("{message}")]
    InvalidInput {
        /// What is wrong.
        message: String,
    },
    /// The shape no longer exists.
    #[error("{message}")]
    UnknownShape {
        /// What happened.
        message: String,
    },
    /// Too many shapes, points or steps.
    #[error("{message}")]
    LimitExceeded {
        /// What happened.
        message: String,
    },
    /// A saved annotation is damaged or from a newer Supercy.
    #[error("{message}")]
    InvalidDocument {
        /// What happened.
        message: String,
    },
    /// The font could not be read.
    #[error("{message}")]
    InvalidFont {
        /// What happened.
        message: String,
    },
    /// The image could not be exported, or the file name is not valid.
    #[error("{message}")]
    Export {
        /// What happened.
        message: String,
    },
}

impl From<AnnotateError> for AnnotationError {
    fn from(error: AnnotateError) -> Self {
        let message = error.to_string();
        match error {
            AnnotateError::InvalidImage | AnnotateError::ImageTooLarge => {
                Self::InvalidImage { message }
            }
            AnnotateError::UnknownShape => Self::UnknownShape { message },
            AnnotateError::LimitExceeded => Self::LimitExceeded { message },
            AnnotateError::InvalidDocument => Self::InvalidDocument { message },
            AnnotateError::InvalidFont => Self::InvalidFont { message },
            AnnotateError::Encode
            | AnnotateError::InvalidQuality
            | AnnotateError::InvalidFileName => Self::Export { message },
            _ => Self::InvalidInput { message },
        }
    }
}

/// One screenshot being annotated. Safe to call from any thread; calls are serialized.
#[derive(uniffi::Object)]
pub struct AnnotationDocument {
    inner: Mutex<Editor>,
}

impl std::fmt::Debug for AnnotationDocument {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AnnotationDocument(..)")
    }
}

impl AnnotationDocument {
    fn editor(&self) -> MutexGuard<'_, Editor> {
        // A panic cannot leave the editor half-changed (commands build new documents), so a
        // poisoned lock still holds a consistent editor.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wrap(document: Document) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Editor::new(document)),
        })
    }
}

#[uniffi::export]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
impl AnnotationDocument {
    /// A new annotation over a PNG; `scale` is pixels per point (2 on Retina).
    ///
    /// # Errors
    ///
    /// `InvalidImage`.
    #[uniffi::constructor]
    pub fn from_png(png: Vec<u8>, scale: f32) -> Result<Arc<Self>, AnnotationError> {
        Ok(Self::wrap(Document::new(BaseImage::from_png(&png, scale)?)))
    }

    /// A saved annotation (from [`AnnotationDocument::save`]) over the PNG it was made on.
    ///
    /// # Errors
    ///
    /// `InvalidImage` or `InvalidDocument`.
    #[uniffi::constructor]
    pub fn from_saved(
        png: Vec<u8>,
        scale: f32,
        saved: Vec<u8>,
    ) -> Result<Arc<Self>, AnnotationError> {
        let base = BaseImage::from_png(&png, scale)?;
        Ok(Self::wrap(Document::decode(&saved, base)?))
    }

    /// Sets the font for text and step numbers (the bytes of a system font; `index` picks a
    /// face in a collection), or clears it with `None`.
    ///
    /// # Errors
    ///
    /// `InvalidFont`; the previous font stays.
    pub fn set_font(&self, font: Option<Vec<u8>>, index: u32) -> Result<(), AnnotationError> {
        let font = font.map(|bytes| Font::new(bytes, index)).transpose()?;
        self.editor().set_font(font);
        Ok(())
    }

    /// Applies an edit; returns the id of an added shape.
    ///
    /// # Errors
    ///
    /// `InvalidInput`, `UnknownShape` or `LimitExceeded`; nothing changes then.
    pub fn apply(&self, command: AnnotationCommand) -> Result<Option<u64>, AnnotationError> {
        Ok(self.editor().apply(&command.into())?.map(|id| id.0))
    }

    /// Goes back one edit; returns whether there was one.
    pub fn undo(&self) -> bool {
        self.editor().undo()
    }

    /// Repeats the last undone edit; returns whether there was one.
    pub fn redo(&self) -> bool {
        self.editor().redo()
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        self.editor().can_undo()
    }

    /// Whether there is an edit to redo.
    pub fn can_redo(&self) -> bool {
        self.editor().can_redo()
    }

    /// Width of the image in pixels.
    pub fn image_width(&self) -> u32 {
        self.editor().document().base().width()
    }

    /// Height of the image in pixels.
    pub fn image_height(&self) -> u32 {
        self.editor().document().base().height()
    }

    /// Pixels per point.
    pub fn image_scale(&self) -> f32 {
        self.editor().document().base().scale()
    }

    /// Every shape, bottom first.
    pub fn shapes(&self) -> Vec<AnnotationShape> {
        self.editor().document().shapes().map(Into::into).collect()
    }

    /// One shape.
    pub fn shape(&self, id: u64) -> Option<AnnotationShape> {
        self.editor()
            .document()
            .shape(cy_annotate::ShapeId(id))
            .map(Into::into)
    }

    /// The crop box, if one is set.
    pub fn crop(&self) -> Option<AnnotationRect> {
        self.editor().document().crop().map(Into::into)
    }

    /// The number the next step marker gets.
    pub fn next_step_number(&self) -> u32 {
        self.editor().document().next_step_number()
    }

    /// The topmost shape at `point` (within the hit tolerance).
    pub fn hit_test(&self, point: AnnotationPoint) -> Option<u64> {
        self.editor().hit_test(point.into()).map(|id| id.0)
    }

    /// The box around the drawn shapes, for the selection frame.
    pub fn selection_bounds(&self, shape_ids: Vec<u64>) -> Option<AnnotationRect> {
        self.editor()
            .selection_bounds(&ids(shape_ids))
            .map(Into::into)
    }

    /// Shapes whose drawn box overlaps `area`, bottom first (rubber-band selection).
    pub fn shapes_in(&self, area: AnnotationRect) -> Vec<u64> {
        self.editor()
            .shapes_in(area.into())
            .into_iter()
            .map(|id| id.0)
            .collect()
    }

    /// The composite inside `region`, for previews of mosaic, blur and text.
    ///
    /// # Errors
    ///
    /// `InvalidInput` for a region outside the image.
    pub fn render_region(&self, region: AnnotationRect) -> Result<RenderedRegion, AnnotationError> {
        let rendered = self.editor().render_region(region.into())?;
        Ok(RenderedRegion {
            x: rendered.x,
            y: rendered.y,
            width: rendered.width(),
            height: rendered.height(),
            rgba: rendered.to_rgba(),
        })
    }

    /// The final image: composited, cropped and encoded.
    ///
    /// # Errors
    ///
    /// `Export`.
    pub fn export(&self, format: ExportImageFormat) -> Result<Vec<u8>, AnnotationError> {
        Ok(self.editor().export(format.into())?)
    }

    /// The annotation in its saved form (shapes, crop box, counters; not the image).
    ///
    /// # Errors
    ///
    /// `InvalidDocument` when it is too large to save.
    pub fn save(&self) -> Result<Vec<u8>, AnnotationError> {
        Ok(self.editor().document().encode()?)
    }
}

/// The drawing constants, so the shell's preview matches the export.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct AnnotationStyleConstants {
    /// Arrow head length per point of stroke width.
    pub arrow_head_length_per_width: f32,
    /// Shortest arrow head, in points.
    pub arrow_head_min_length: f32,
    /// Half the opening angle of the arrow head, in degrees.
    pub arrow_head_half_angle_degrees: f32,
    /// Rectangle corner radius per point of stroke width (corners are quadratic curves with
    /// the corner as control point, capped at half the shorter side).
    pub rect_corner_radius_per_width: f32,
    /// Highlighter opacity before the style's own; it blends with multiply.
    pub highlighter_alpha: f32,
    /// Highlighter stroke width per point of style width.
    pub highlighter_width_factor: f32,
    /// Step marker radius at zero stroke width, in points.
    pub step_base_radius: f32,
    /// Extra step radius per point of stroke width.
    pub step_radius_per_width: f32,
    /// Step number font size per point of radius (the digits are centred on their outline).
    pub step_font_size_per_radius: f32,
    /// Text line height per point of font size; the first baseline is at the font's ascent.
    pub text_line_height: f32,
    /// Click tolerance around shapes, in points.
    pub hit_tolerance: f32,
    /// Strokes use round caps and joins.
    pub round_caps: bool,
}

/// The drawing constants of the core's renderer.
#[uniffi::export]
#[must_use]
pub fn annotation_style_constants() -> AnnotationStyleConstants {
    let c = STYLE_CONSTANTS;
    AnnotationStyleConstants {
        arrow_head_length_per_width: c.arrow_head_length_per_width,
        arrow_head_min_length: c.arrow_head_min_length,
        arrow_head_half_angle_degrees: c.arrow_head_half_angle_degrees,
        rect_corner_radius_per_width: c.rect_corner_radius_per_width,
        highlighter_alpha: c.highlighter_alpha,
        highlighter_width_factor: c.highlighter_width_factor,
        step_base_radius: c.step_base_radius,
        step_radius_per_width: c.step_radius_per_width,
        step_font_size_per_radius: c.step_font_size_per_radius,
        text_line_height: c.text_line_height,
        hit_tolerance: c.hit_tolerance,
        round_caps: c.round_caps,
    }
}

/// Expands a file name template (`{date}`, `{time}`, `{n}`) into a safe file name with the
/// format's extension.
///
/// # Errors
///
/// `Export` for an impossible date or time.
#[uniffi::export]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
pub fn annotation_file_name(
    template: String,
    parts: FileNameParts,
    format: ExportImageFormat,
) -> Result<String, AnnotationError> {
    Ok(cy_annotate::expand_file_name(
        &template,
        &parts.into(),
        format.into(),
    )?)
}

/// Joins an absolute folder and a plain file name, after checking both.
///
/// # Errors
///
/// `Export` for a relative folder or a name with separators.
#[uniffi::export]
#[allow(
    clippy::needless_pass_by_value,
    reason = "UniFFI hands over owned values"
)]
pub fn annotation_target_path(
    folder: String,
    file_name: String,
) -> Result<String, AnnotationError> {
    Ok(cy_annotate::target_path(&folder, &file_name)?)
}
