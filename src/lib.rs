//! A lightweight cross-platform library for listing installed system fonts.
//!
//! # Platform support
//!
//! | Platform | Backend               |
//! |----------|-----------------------|
//! | macOS    | CoreText              |
//! | Windows  | DirectWrite (Win 10+) |
//! | Linux    | Fontconfig            |
//! | Other    | Unsupported (empty)   |
//!
//! # Features
//!
//! By default, this crate provides a single function that returns deduplicated,
//! sorted font family names — nothing more.
//!
//! Enable the `full` feature to unlock detailed font metadata, variable-font
//! detection via the OpenType `fvar` table, and control over family merging.
//!
//! ## Default
//!
//! ```rust
//! let families: Vec<String> = list_fonts::get_font_list();
//! // => ["Arial", "Consolas", "Inter", "Times New Roman", …]
//! ```
//!
//! ## `full` feature
//!
//! ```rust
//! # #[cfg(feature = "full")] {
//! use list_fonts::{get_font_list_full, Options};
//!
//! let fonts = get_font_list_full(&Options {
//!     family: false,
//!     meta: true,
//!     variable: true,
//! });
//!
//! for font in &fonts {
//!     println!(
//!         "{} {} — weight {} — variable: {}",
//!         font.family_name, font.font_name, font.weight, font.variable
//!     );
//! }
//! # }
//! ```

// ---------------------------------------------------------------------------
// Platform backends — each exposes `family_names()` and, when `full` is
// enabled, `all_fonts()`.
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use core_text::font_collection;

    /// Return sorted, deduplicated font family names via CoreText.
    pub fn family_names() -> Vec<String> {
        let mut names: Vec<String> = font_collection::get_family_names()
            .iter()
            .map(|name| name.to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    #[cfg(feature = "full")]
    pub fn all_fonts() -> Vec<super::Font> {
        use core_text::font_descriptor::TraitAccessors;

        let collection = font_collection::create_for_all_families();
        let descs = match collection.get_descriptors() {
            Some(d) => d,
            None => return Vec::new(),
        };
        descs
            .iter()
            .map(|desc| {
                let traits = desc.traits();
                let path = desc.font_path().map(|p| p.into());
                let variable = super::fvar::check_fvar(&path);
                super::Font {
                    family_name: desc.family_name(),
                    font_name: desc.font_name(),
                    path,
                    style: style_from_ct(&traits),
                    weight: weight_from_ct(traits.normalized_weight()),
                    stretch: stretch_from_ct(traits.normalized_width()),
                    variable,
                }
            })
            .collect()
    }

    #[cfg(feature = "full")]
    fn style_from_ct(traits: &core_text::font_descriptor::CTFontTraits) -> super::Style {
        use core_text::font_descriptor::{SymbolicTraitAccessors, TraitAccessors};

        let sym = traits.symbolic_traits();
        if sym.is_italic() {
            return super::Style::Italic;
        }
        if sym.is_vertical() {
            return super::Style::Normal;
        }
        let angle = traits.normalized_slant() / 30. * 360.;
        if angle.abs() < 0.01 {
            super::Style::Normal
        } else {
            super::Style::Oblique(Some(angle as f32))
        }
    }

    #[cfg(feature = "full")]
    fn weight_from_ct(weight: f64) -> f32 {
        let w = weight as f32;
        // Normalize CoreText weight (-1..1) to CSS scale (100–950).
        ((w + 1.0) * 425.0 + 100.0).clamp(1.0, 1000.0) as f32
    }

    #[cfg(feature = "full")]
    fn stretch_from_ct(width: f64) -> f32 {
        // CoreText width: -1.0 = condensed, 0 = normal, 1.0 = expanded.
        // Map to percentage where 100 % = normal.
        ((width + 1.0) * 50.0 + 50.0).clamp(1.0, 200.0) as f32
    }
}

#[cfg(windows)]
mod imp {
    use dwrote::FontCollection;

    /// Return sorted, deduplicated font family names via DirectWrite.
    pub fn family_names() -> Vec<String> {
        let collection = FontCollection::system();
        let mut names: Vec<String> = collection
            .families_iter()
            .map(|f| f.family_name())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    #[cfg(feature = "full")]
    pub fn all_fonts() -> Vec<super::Font> {
        use dwrote::{FontStretch, FontStyle, FontWeight};

        let collection = FontCollection::system();
        let mut fonts = Vec::new();

        for family in collection.families_iter() {
            for idx in 0..family.get_font_count() {
                let font = family.get_font(idx);
                let face = font.create_font_face();
                let path = face
                    .get_files()
                    .first()
                    .and_then(|f| f.get_font_file_path().map(|p| p.into()));
                let variable = super::fvar::check_fvar(&path);

                fonts.push(super::Font {
                    family_name: font.family_name(),
                    font_name: font.face_name(),
                    path,
                    style: style_from_dw(font.style()),
                    weight: weight_from_dw(font.weight()),
                    stretch: stretch_from_dw(font.stretch()),
                    variable,
                });
            }
        }
        fonts
    }

    #[cfg(feature = "full")]
    fn style_from_dw(style: FontStyle) -> super::Style {
        match style {
            FontStyle::Normal => super::Style::Normal,
            FontStyle::Italic => super::Style::Italic,
            FontStyle::Oblique => super::Style::Oblique(None),
        }
    }

    #[cfg(feature = "full")]
    fn weight_from_dw(weight: FontWeight) -> f32 {
        match weight {
            FontWeight::Thin => 100.0,
            FontWeight::ExtraLight => 200.0,
            FontWeight::Light => 300.0,
            FontWeight::SemiLight => 350.0,
            FontWeight::Regular => 400.0,
            FontWeight::Medium => 500.0,
            FontWeight::SemiBold => 600.0,
            FontWeight::Bold => 700.0,
            FontWeight::ExtraBold => 800.0,
            FontWeight::Black => 900.0,
            FontWeight::ExtraBlack => 950.0,
            FontWeight::Unknown(v) => v as f32,
        }
    }

    #[cfg(feature = "full")]
    fn stretch_from_dw(stretch: FontStretch) -> f32 {
        use dwrote::FontStretch::*;
        match stretch {
            UltraCondensed => 50.0,
            ExtraCondensed => 62.5,
            Condensed => 75.0,
            SemiCondensed => 87.5,
            Normal => 100.0,
            SemiExpanded => 112.5,
            Expanded => 125.0,
            ExtraExpanded => 150.0,
            UltraExpanded => 200.0,
            Undefined => 100.0,
        }
    }
}

#[cfg(target_os = "linux")]
mod imp {
    use fontconfig::{Fontconfig, ObjectSet, Pattern};

    /// Return sorted, deduplicated font family names via Fontconfig.
    pub fn family_names() -> Vec<String> {
        let fc = match Fontconfig::new() {
            Some(fc) => fc,
            None => return Vec::new(),
        };
        let pattern = Pattern::new(&fc);
        let mut objects = ObjectSet::new(&fc);
        objects.add(fontconfig::FC_FAMILY);
        let fonts = fontconfig::list_fonts(&pattern, Some(&objects));

        let mut names: Vec<String> = fonts
            .iter()
            .filter_map(|f| f.get_string(fontconfig::FC_FAMILY))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    #[cfg(feature = "full")]
    pub fn all_fonts() -> Vec<super::Font> {
        let fc = match Fontconfig::new() {
            Some(fc) => fc,
            None => return Vec::new(),
        };
        let pattern = Pattern::new(&fc);
        let mut objects = ObjectSet::new(&fc);
        objects.add(fontconfig::FC_FAMILY);
        objects.add(fontconfig::FC_FULLNAME);
        objects.add(fontconfig::FC_FILE);
        objects.add(fontconfig::FC_SLANT);
        objects.add(fontconfig::FC_WEIGHT);
        objects.add(fontconfig::FC_WIDTH);
        let fonts = fontconfig::list_fonts(&pattern, Some(&objects));

        fonts
            .iter()
            .map(|font| {
                let family = font.get_string(fontconfig::FC_FAMILY).unwrap_or_default();
                let name = font.get_string(fontconfig::FC_FULLNAME).unwrap_or_default();
                let path: Option<std::path::PathBuf> =
                    font.get_string(fontconfig::FC_FILE).map(|p| p.into());
                let slant = font.slant().unwrap_or(0);
                let weight = font.weight().unwrap_or(fontconfig::FC_WEIGHT_REGULAR);
                let width = font.width().unwrap_or(fontconfig::FC_WIDTH_NORMAL);
                let variable = super::fvar::check_fvar(&path);

                super::Font {
                    family_name: family,
                    font_name: name,
                    path,
                    style: style_from_fc(slant),
                    weight: weight_from_fc(weight),
                    stretch: stretch_from_fc(width),
                    variable,
                }
            })
            .collect()
    }

    #[cfg(feature = "full")]
    fn style_from_fc(slant: i32) -> super::Style {
        match slant {
            100 => super::Style::Italic,
            110 => super::Style::Oblique(None),
            _ => super::Style::Normal,
        }
    }

    #[cfg(feature = "full")]
    fn weight_from_fc(weight: i32) -> f32 {
        // Fontconfig weight: 0–215, where 80 = regular, 200 = bold.
        // Normalize to CSS scale (100–950).
        (weight as f32 / 215.0 * 900.0 + 100.0).clamp(1.0, 1000.0)
    }

    #[cfg(feature = "full")]
    fn stretch_from_fc(width: i32) -> f32 {
        // Fontconfig width is roughly a percentage (100 = normal).
        width as f32
    }
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
mod imp {
    /// Return an empty list on unsupported platforms so dependents can still
    /// compile for mobile and other non-desktop targets.
    pub fn family_names() -> Vec<String> {
        Vec::new()
    }

    #[cfg(feature = "full")]
    pub fn all_fonts() -> Vec<super::Font> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns the list of installed font family names, sorted and deduplicated.
///
/// Weight and style variants are collapsed into their parent family (e.g.
/// `"Arial Bold"`, `"Arial Italic"` all become `"Arial"`).
///
/// This function is always available; no feature flag is required.
///
/// # Example
///
/// ```rust
/// let families = list_fonts::get_font_list();
/// assert!(!families.is_empty());
/// for name in &families[..5.min(families.len())] {
///     println!("  {name}");
/// }
/// ```
pub fn get_font_list() -> Vec<String> {
    imp::family_names()
}

// ---------------------------------------------------------------------------
// Feature-gated types and functions (`full`)
// ---------------------------------------------------------------------------

/// Controls how [`get_font_list_full`] collects and groups fonts.
///
/// All fields default to the same behaviour as [`get_font_list`]: merge by
/// family, no metadata, no variable-font distinction.
#[cfg(feature = "full")]
pub struct Options {
    /// Merge fonts that share the same family name.
    ///
    /// When `true` (the default), weight/style variants like `"Arial Bold"`
    /// and `"Arial Italic"` are collapsed into a single entry.
    pub family: bool,

    /// Populate per-face metadata (`style`, `weight`, `stretch`, `path`).
    ///
    /// When `false` (the default), only `family_name` is meaningful; other
    /// fields on [`Font`] carry default values.
    pub meta: bool,

    /// Distinguish variable fonts from static ones by inspecting the
    /// OpenType [`fvar`] table.
    ///
    /// When `true`, variable fonts that share a family name with a static
    /// counterpart will appear as separate entries (their [`Font::variable`]
    /// field will be `true`).
    pub variable: bool,
}

#[cfg(feature = "full")]
impl Default for Options {
    fn default() -> Self {
        Self {
            family: true,
            meta: false,
            variable: false,
        }
    }
}

/// A single font entry.
///
/// Returned by [`get_font_list_full`]. When the `full` feature is *not*
/// enabled, only family names are available via [`get_font_list`].
#[cfg(feature = "full")]
#[derive(Debug, Clone)]
pub struct Font {
    /// Font family name (e.g. `"Arial"`, `"Inter"`).
    pub family_name: String,

    /// Individual face name (e.g. `"Arial Bold"`, `"Inter SemiBold"`).
    pub font_name: String,

    /// Filesystem path to the font file, if the platform exposes it.
    pub path: Option<std::path::PathBuf>,

    /// Style classification.
    pub style: Style,

    /// CSS-compatible weight (100–950). `400` is normal; `700` is bold.
    pub weight: f32,

    /// Width as a percentage of normal. `100.0` is normal; smaller values
    /// are condensed, larger values are expanded.
    pub stretch: f32,

    /// Whether this font contains OpenType variation axes ([`fvar`] table).
    ///
    /// Meaningful only when [`Options::variable`] is `true`; `false`
    /// otherwise.
    pub variable: bool,
}

/// Font style classification.
#[cfg(feature = "full")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Style {
    /// Upright (Roman).
    Normal,

    /// Italic — visually distinct from the upright style.
    Italic,

    /// Oblique — a slanted version of the upright style. The field holds the
    /// slant angle in degrees when the platform provides it.
    Oblique(Option<f32>),
}

/// Enumerate installed fonts with full control over grouping and metadata.
///
/// Use [`Options`] to specify whether to merge by family, include per-face
/// metadata, and probe for variable fonts.
///
/// This function is only available with the **`full`** feature.
///
/// # Example
///
/// ```rust
/// use list_fonts::{get_font_list_full, Options};
///
/// let fonts = get_font_list_full(&Options {
///     family: false,
///     meta: true,
///     variable: true,
/// });
///
/// for f in &fonts {
///     println!(
///         "{} — {} — weight: {} — variable: {}",
///         f.family_name, f.font_name, f.weight, f.variable,
///     );
/// }
/// ```
#[cfg(feature = "full")]
pub fn get_font_list_full(options: &Options) -> Vec<Font> {
    let mut fonts: Vec<Font> = imp::all_fonts()
        .into_iter()
        .filter(|f| options.variable || !f.variable)
        .collect();

    if options.family {
        if options.meta || options.variable {
            // Keep one entry per unique family name.
            let mut seen = std::collections::HashSet::new();
            fonts.retain(|f| seen.insert(f.family_name.clone()));
        } else {
            // Fast path: just return family names, no metadata.
            let mut names: Vec<String> = fonts.into_iter().map(|f| f.family_name).collect();
            names.sort();
            names.dedup();
            return names
                .into_iter()
                .map(|n| Font {
                    family_name: n,
                    font_name: String::new(),
                    path: None,
                    style: Style::Normal,
                    weight: 400.0,
                    stretch: 100.0,
                    variable: false,
                })
                .collect();
        }
    }

    fonts.sort_by(|a, b| {
        a.family_name
            .cmp(&b.family_name)
            .then(a.font_name.cmp(&b.font_name))
    });
    fonts
}

// ---------------------------------------------------------------------------
// OpenType `fvar` table probe
// ---------------------------------------------------------------------------

#[cfg(feature = "full")]
mod fvar {
    use std::fs::File;
    use std::io::{Read, Seek, SeekFrom};
    use std::path::PathBuf;

    /// Peek at the OpenType table directory of a font file to determine
    /// whether it contains an `fvar` (Font Variations) table.
    ///
    /// Returns `false` if the file cannot be opened, is too short, or does
    /// not contain the table.
    pub fn check_fvar(path: &Option<PathBuf>) -> bool {
        let path = match path {
            Some(p) => p,
            None => return false,
        };
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        // Read the OpenType table directory header.
        let mut header = [0u8; 12];
        if file.read_exact(&mut header).is_err() {
            return false;
        }
        // Byte 4–5: numTables (u16 big-endian).
        let num_tables = u16::from_be_bytes([header[4], header[5]]) as usize;
        if num_tables == 0 || num_tables > 100 {
            return false;
        }

        // Seek past the header to the table records.
        if file.seek(SeekFrom::Start(12)).is_err() {
            return false;
        }
        let mut record = [0u8; 16];
        for _ in 0..num_tables {
            if file.read_exact(&mut record).is_err() {
                return false;
            }
            let tag = u32::from_be_bytes([record[0], record[1], record[2], record[3]]);
            // 0x66766172 = 'fvar'
            if tag == 0x66766172 {
                return true;
            }
        }
        false
    }
}
