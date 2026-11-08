// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Effect cards: every picture package's still, drawn by the package.
//!
//! A card is one reference picture through a package at its defaults,
//! drawn by the same shader and the same compositor a clip's frames go
//! through, so a card shows what the effect does and cannot drift from it.
//! An effect or a look treats the picture; a text effect treats a title set
//! over it; a transition cuts from the picture to a mirrored, hue-turned
//! copy of it, partway through. A package with no shader - someone's own
//! format 1 package - is drawn through its FFmpeg chain. Whatever a card
//! leaves transparent - a mask, a key - shows a checkerboard. A package
//! whose defaults show little on a card says what its card shows in its
//! manifest's `[card]` table: knob values, the moment, the point of the
//! cut, a screen behind the picture for a key to take out.
//!
//! Cards are drawn once and kept as JPEGs in a folder, each named after its
//! package and a fingerprint of everything it was drawn from: the shader,
//! the knobs' defaults, the table, and how cards are drawn. A package whose
//! shader or defaults change has a new name and so a new card, and the old
//! one is swept away ([`prune`]). The window reads what is there as it opens
//! and draws the rest on a thread of its own ([`Painter`]).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use concat_core::frame::Frame;
use concat_core::shader::{RevealMap, ShaderPass, TransitionPass};
use concat_core::subject::{SubjectMask, needs_subject};
use concat_core::time::Rational;
use concat_effects::{At, Catalogue, Package};
use concat_media::{DecodeOptions, Decoder, FrameSource};
use concat_render::{Compositor, FramePlan, PlannedLayer, WgpuCompositor, detached_clip};

/// A card's width in pixels: 16:9, and twice the widest a card is shown,
/// for a high-density screen.
pub const WIDTH: u32 = 480;

/// A card's height in pixels.
pub const HEIGHT: u32 = 270;

/// The picture every card is drawn from: a serene sunset over the sea/sky,
/// giving rich golden light, color gradations, clouds, and contrast for filters and adjustments.
pub const STILL_SUNSET: &[u8] = include_bytes!("../assets/card-sunset.jpg");

/// Transparent person reference for subject-aware effect thumbnails.
pub const STILL_SUBJECT: &[u8] = include_bytes!("../assets/card-subject.png");

/// The city reference, with high-frequency geometry and crisp architectural edges.
pub const STILL_CITY: &[u8] = include_bytes!("../assets/card-city.jpg");

/// Default still for backwards compatibility.
pub const STILL: &[u8] = STILL_SUNSET;

/// Bumped whenever the way a card is drawn changes - the still, the moment,
/// the title, the size, the encoding - so every kept card is drawn again.
const EDITION: u64 = 2;

/// The instant a card shows, in seconds into its clip: late enough that a
/// reveal is under way and a pulse is lit, early enough that a one-shot
/// look has not finished.
const MOMENT: f64 = 0.9;

/// How far through its cut a transition's card shows: both pictures in
/// view, the incoming one not yet past the middle.
const CUT_AT: f32 = 0.4;

/// The words a text effect's card sets over the still.
const TITLE: &str = "Big Title";

/// The quality cards are written at: FFmpeg's JPEG quantiser, 2 the best.
const QUALITY: u8 = 3;

/// The visual theme / base picture a card is drawn from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardTheme {
    /// Serene sunset for calm effects, color grades, filters, and adjustments.
    Calm,
    /// Dynamic city skyline for abrupt distortions, glitch, blur, and motion.
    Abrupt,
}

/// What a card draws.
#[derive(Clone, Debug)]
enum Drawing {
    /// The still through a pass, over a checkerboard; before a screen of
    /// the colour given, for a key.
    Picture {
        pass: ShaderPass,
        screen: Option<[u8; 3]>,
        theme: CardTheme,
    },
    /// A title over the still, the pass on the title.
    Title(ShaderPass),
    /// A cut from the still (sunset) to the city.
    Cut(TransitionPass),
}

/// One package's card: where it is kept, and what it draws.
#[derive(Clone, Debug)]
pub struct Card {
    /// The package's id.
    pub id: String,
    /// The JPEG the card is kept in; see [`Card::is_drawn`].
    pub path: PathBuf,
    drawing: Drawing,
    /// Seconds into the clip the card shows.
    moment: f64,
}

impl Card {
    /// The card `package` has, kept under `dir`, or None for a package
    /// with nothing to draw: a sound, or a package with no shader.
    pub fn of(catalogue: &Catalogue, package: &Package, dir: &Path) -> Option<Card> {
        if !package.kind().is_visual() {
            return None;
        }
        let settings = package.manifest.card.clone().unwrap_or_default();
        let mut set = package.params_at(At::Default);
        set.extend(settings.params.clone());
        let moment = settings.moment.unwrap_or(MOMENT);
        let is_abrupt = package.category().eq_ignore_ascii_case("Distort")
            || package.category().eq_ignore_ascii_case("Glitch")
            || package.category().eq_ignore_ascii_case("Blur")
            || package.category().eq_ignore_ascii_case("Cutout")
            || package.id().contains("pixelate")
            || package.id().contains("fisheye")
            || package.id().contains("chromatic")
            || package.id().contains("halation")
            || package.id().contains("motion");
        let theme = if is_abrupt {
            CardTheme::Abrupt
        } else {
            CardTheme::Calm
        };
        let drawing = if package.transition().is_some() {
            Drawing::Cut(catalogue.transition_pass(
                package.id(),
                &settings.params,
                settings.progress.unwrap_or(f64::from(CUT_AT)),
            )?)
        } else {
            let pass = package.pass(&set, None)?;
            if package.category().eq_ignore_ascii_case("Text") {
                Drawing::Title(pass)
            } else {
                Drawing::Picture {
                    pass,
                    screen: settings.screen_rgb(),
                    theme,
                }
            }
        };
        let name = format!(
            "{}-{:016x}.jpg",
            package.id(),
            fingerprint(&drawing, moment)
        );
        Some(Card {
            id: package.id().to_owned(),
            path: dir.join(name),
            drawing,
            moment,
        })
    }

    /// Whether the card is drawn and kept.
    pub fn is_drawn(&self) -> bool {
        self.path.is_file()
    }
}

/// Every card the catalogue's picture packages have, kept under `dir`.
pub fn cards(catalogue: &Catalogue, dir: &Path) -> Vec<Card> {
    catalogue
        .packages()
        .filter_map(|package| Card::of(catalogue, package, dir))
        .collect()
}

/// Removes every kept card under `dir` that is not one of `cards`: an
/// older drawing of a package that has changed, or the card of one that
/// is gone. Anything in the folder that is not named like a card is left.
pub fn prune(dir: &Path, cards: &[Card]) {
    let keep: HashSet<&Path> = cards.iter().map(|card| card.path.as_path()).collect();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for path in entries.filter_map(Result::ok).map(|entry| entry.path()) {
        let named = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".jpg"))
            .and_then(|stem| stem.rsplit_once('-'))
            .is_some_and(|(_, print)| {
                print.len() == 16 && print.chars().all(|c| c.is_ascii_hexdigit())
            });
        if named && !keep.contains(path.as_path()) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// FNV-1a over everything a card is drawn from, so its name is the same on
/// every run and every build: a kept card is found again, and a changed
/// package is not mistaken for its old self.
fn fingerprint(drawing: &Drawing, moment: f64) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |bytes: &[u8]| {
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        // A separator, so two fields cannot run into one another.
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    feed(&EDITION.to_le_bytes());
    feed(&WIDTH.to_le_bytes());
    feed(&HEIGHT.to_le_bytes());
    feed(&moment.to_le_bytes());
    let pass = |feed: &mut dyn FnMut(&[u8]), pass: &ShaderPass| {
        feed(pass.key.as_bytes());
        feed(&pass.params);
        feed(&pass.intensity.to_le_bytes());
        feed(&pass.lut.as_ref().map_or(0, |lut| lut.id).to_le_bytes());
        // A package drawn in several passes: the size of each picture is
        // the manifest's, not the shader's, so it is fed on its own.
        for stage in &pass.stages {
            feed(stage.entry.as_bytes());
            feed(&stage.shrink[0].to_le_bytes());
            feed(&stage.shrink[1].to_le_bytes());
        }
    };
    match drawing {
        Drawing::Picture {
            pass: drawn,
            screen,
            theme,
        } => {
            feed(b"picture");
            if needs_subject(&drawn.package) {
                feed(STILL_SUBJECT);
            }
            feed(match theme {
                CardTheme::Calm => b"calm",
                CardTheme::Abrupt => b"abrupt",
            });
            feed(&screen.map_or([0; 4], |[r, g, b]| [1, r, g, b]));
            pass(&mut feed, drawn);
        }
        Drawing::Title(drawn) => {
            feed(b"title");
            feed(TITLE.as_bytes());
            pass(&mut feed, drawn);
        }
        Drawing::Cut(cut) => {
            feed(b"cut");
            feed(cut.key.as_bytes());
            feed(&cut.params);
            feed(&cut.progress.to_le_bytes());
            feed(&cut.lut.as_ref().map_or(0, |lut| lut.id).to_le_bytes());
        }
    }
    hash
}

/// What draws cards: a compositor, and the pictures every card starts from,
/// made once - the still (sunset), the city skyline (or turned copy), the
/// checkerboard, the title.
pub struct Painter {
    compositor: WgpuCompositor,
    still: Arc<Frame>,
    city: Arc<Frame>,
    checker: Arc<Frame>,
    title: Option<(Arc<Frame>, Arc<RevealMap>)>,
    subject: Option<(Arc<Frame>, Arc<SubjectMask>)>,
}

impl Painter {
    /// A painter on `compositor`, drawing from the picture at `still`
    /// scaled to `width` by `height`.
    pub fn new(
        compositor: WgpuCompositor,
        still: &Path,
        width: u32,
        height: u32,
    ) -> Result<Painter, String> {
        Self::with_city(compositor, still, None, width, height)
    }

    /// A painter on `compositor` with both calm still (sunset) and dynamic/abrupt still (city).
    pub fn with_city(
        compositor: WgpuCompositor,
        still: &Path,
        city: Option<&Path>,
        width: u32,
        height: u32,
    ) -> Result<Painter, String> {
        let mut decoder = Decoder::open(still, &DecodeOptions::default().scaled_to(width, height))
            .map_err(|error| error.to_string())?;
        let still = decoder
            .next_frame()
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("{} holds no picture", still.display()))?;

        let city_frame = if let Some(city_path) = city {
            if city_path.is_file() {
                Decoder::open(
                    city_path,
                    &DecodeOptions::default().scaled_to(width, height),
                )
                .ok()
                .and_then(|mut dec| dec.next_frame().ok().flatten())
            } else {
                None
            }
        } else {
            None
        };
        let city = city_frame.unwrap_or_else(|| turned(&still));
        let checker = checker(still.width(), still.height());
        Ok(Painter {
            compositor,
            still: Arc::new(still),
            city: Arc::new(city),
            checker: Arc::new(checker),
            title: None,
            subject: None,
        })
    }

    /// Draws `card` and writes it to its path, whole or not at all.
    pub fn draw(&mut self, card: &Card) -> Result<(), String> {
        let frame = self.frame(card)?;
        write_card(&frame, &card.path)
    }

    /// Draws `card` and writes it to `path` instead of its own.
    pub fn draw_to(&mut self, card: &Card, path: &Path) -> Result<(), String> {
        let frame = self.frame(card)?;
        write_card(&frame, path)
    }

    /// Draw one instant of a transition using the same shader as the timeline.
    pub fn transition_frame(&mut self, card: &Card, progress: f32) -> Result<Frame, String> {
        let Drawing::Cut(cut) = &card.drawing else {
            return Err("not a transition card".into());
        };
        let mut animated = card.clone();
        let mut cut = cut.clone();
        cut.progress = progress.clamp(0.0, 1.0);
        animated.drawing = Drawing::Cut(cut);
        self.frame(&animated)
    }

    /// One looping catalogue preview. Transitions advance their cut; picture
    /// effects advance clip-local time and smoothly demonstrate before/after
    /// strength. Only this cloned preview changes: saved parameters stay intact.
    pub fn preview_frame(&mut self, card: &Card, phase: f32) -> Result<Frame, String> {
        if !phase.is_finite() {
            return Err("preview phase is not finite".into());
        }
        let phase = phase.clamp(0.0, 1.0);
        if matches!(&card.drawing, Drawing::Cut(_)) {
            return self.transition_frame(card, phase);
        }
        let mut animated = card.clone();
        animated.moment = f64::from(phase) * 1.5;
        let strength = 0.5 - 0.5 * (phase * std::f32::consts::TAU).cos();
        match &mut animated.drawing {
            Drawing::Picture { pass, .. } | Drawing::Title(pass) => {
                pass.intensity *= strength;
            }
            Drawing::Cut(_) => unreachable!("transition returned above"),
        }
        self.frame(&animated)
    }

    /// `card` drawn, as an opaque frame the still's size.
    pub fn frame(&mut self, card: &Card) -> Result<Frame, String> {
        let (width, height) = (self.still.width(), self.still.height());
        let moment = Rational::approximate(card.moment).ok_or("the moment is not a time")?;
        let frame = match &card.drawing {
            Drawing::Picture {
                pass,
                screen,
                theme,
            } => {
                let mut pass = pass.clone();
                let subject = if needs_subject(&pass.package) {
                    let (reference, mask) = self.subject(width, height)?;
                    pass.subject_mask = Some(mask);
                    reference
                } else {
                    let base = match theme {
                        CardTheme::Abrupt => &self.city,
                        CardTheme::Calm => &self.still,
                    };
                    match screen {
                        Some(colour) => Arc::new(screened(base, *colour)),
                        None => Arc::clone(base),
                    }
                };
                let ground = PlannedLayer::picture(detached_clip(), Arc::clone(&self.checker));
                let mut picture = PlannedLayer::picture(detached_clip(), subject);
                picture.track = 1;
                picture.effects = vec![pass];
                self.compositor.render(&FramePlan {
                    time: moment,
                    layers: vec![ground, picture],
                    ..FramePlan::empty(width, height)
                })
            }
            Drawing::Title(pass) => {
                let (title, reveal) = self.title(width, height)?;
                let ground = PlannedLayer::picture(detached_clip(), Arc::clone(&self.still));
                let overscan = (title.width() as f32 / width as f32)
                    .max(title.height() as f32 / height as f32);
                let mut words = PlannedLayer::picture(detached_clip(), title);
                words.track = 1;
                words.overscan = overscan;
                let mut pass = pass.clone();
                pass.reveal_map = Some(reveal);
                words.effects = vec![pass];
                self.compositor.render(&FramePlan {
                    time: moment,
                    layers: vec![ground, words],
                    ..FramePlan::empty(width, height)
                })
            }
            Drawing::Cut(cut) => self
                .compositor
                .combine(
                    width,
                    height,
                    card.moment as f32,
                    &self.still,
                    &self.city,
                    cut,
                )
                .ok_or("the transition's shader would not run")?,
        };
        if self.compositor.lost() {
            return Err("the GPU device was lost".to_owned());
        }
        Ok(frame)
    }

    /// A known-alpha person composited over the city. Thumbnail rendering
    /// needs no model download/inference and uses the real subject shader.
    fn subject(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<(Arc<Frame>, Arc<SubjectMask>), String> {
        if let Some((frame, mask)) = &self.subject {
            return Ok((Arc::clone(frame), Arc::clone(mask)));
        }
        let mut decoder = png::Decoder::new(std::io::Cursor::new(STILL_SUBJECT));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
        let mut pixels = vec![0; reader.output_buffer_size()];
        let info = reader
            .next_frame(&mut pixels)
            .map_err(|error| error.to_string())?;
        if info.color_type != png::ColorType::Rgba {
            return Err("subject reference needs alpha".into());
        }
        pixels.truncate(info.buffer_size());
        let image =
            Frame::from_rgba(info.width, info.height, pixels).ok_or("invalid subject reference")?;
        let cutout = Arc::new(self.compositor.render(&FramePlan {
            canvas: [0; 4],
            layers: vec![PlannedLayer::picture(detached_clip(), Arc::new(image))],
            ..FramePlan::empty(width, height)
        }));
        let scale = (concat_core::subject::MAX_SIDE as f32 / width.max(height) as f32).min(1.0);
        let (mw, mh) = (
            ((width as f32 * scale).round() as u32).max(1),
            ((height as f32 * scale).round() as u32).max(1),
        );
        let mut gray = Vec::with_capacity(mw as usize * mh as usize);
        for y in 0..mh {
            for x in 0..mw {
                gray.push(
                    cutout
                        .pixel(
                            (u64::from(x) * u64::from(width) / u64::from(mw)) as u32,
                            (u64::from(y) * u64::from(height) / u64::from(mh)) as u32,
                        )
                        .map_or(0, |pixel| pixel[3]),
                );
            }
        }
        let mask = Arc::new(SubjectMask::new(mw, mh, gray).ok_or("invalid reference matte")?);
        let mut person = PlannedLayer::picture(detached_clip(), cutout);
        person.track = 1;
        let frame = Arc::new(self.compositor.render(&FramePlan {
            layers: vec![
                PlannedLayer::picture(detached_clip(), Arc::clone(&self.city)),
                person,
            ],
            ..FramePlan::empty(width, height)
        }));
        self.subject = Some((Arc::clone(&frame), Arc::clone(&mask)));
        Ok((frame, mask))
    }

    /// White title words and their reveal order, prepared once.
    fn title(&mut self, width: u32, height: u32) -> Result<(Arc<Frame>, Arc<RevealMap>), String> {
        if let Some((frame, reveal)) = &self.title {
            return Ok((Arc::clone(frame), Arc::clone(reveal)));
        }
        let style = concat_text::TitleStyle {
            content: TITLE.to_owned(),
            font_family: concat_text::BUNDLED_FAMILY.to_owned(),
            font_size: 0.26,
            font_weight: 800.0,
            italic: false,
            underline: false,
            strikethrough: false,
            vertical_align: 1,
            vertical: false,
            color: "#ffffff".to_owned(),
            align: concat_text::Align::Center,
            stroke_width: 0.0,
            stroke_color: "#000000".to_owned(),
            shadow: true,
            background: String::new(),
            background_radius: 0.0,
            background_padding_x: 0.0,
            background_padding_y: 0.0,
            line_height: 1.1,
            tracking: 0.0,
            max_width: 0.0,
            max_height: 0.0,
            runs: Vec::new(),
        };
        let fonts = concat_text::Fonts::new();
        let rendered = concat_text::render_frame(&fonts, &style, width, height)
            .map_err(|error| error.to_string())?;
        let rects: Vec<(i32, i32, u32, u32)> = rendered
            .words
            .iter()
            .map(|word| (word.x, word.y, word.width, word.height))
            .collect();
        // The canvas's size, which is the still's unless the words ran
        // past its edges.
        let reveal = Arc::new(RevealMap::from_rects(
            rendered.width,
            rendered.height,
            &rects,
        ));
        let frame = Arc::new(
            Frame::from_rgba(rendered.width, rendered.height, rendered.rgba)
                .ok_or("the title came back the wrong size")?,
        );
        self.title = Some((Arc::clone(&frame), Arc::clone(&reveal)));
        Ok((frame, reveal))
    }
}

/// The still a transition cuts to: mirrored, and every hue turned half way
/// round, so the two pictures of a cut can be told apart anywhere on it.
fn turned(still: &Frame) -> Frame {
    let (width, height) = (still.width(), still.height());
    let mut out = still.clone();
    for y in 0..height {
        for x in 0..width {
            let [r, g, b, a] = still.pixel(width - 1 - x, y).unwrap_or([0, 0, 0, 255]);
            let [r, g, b] = [r, g, b].map(|c| f32::from(c) / 255.0);
            // Half a turn in YIQ: the two chroma axes negated, the
            // brightness kept.
            let luma = 0.299 * r + 0.587 * g + 0.114 * b;
            let [r, g, b] = [2.0 * luma - r, 2.0 * luma - g, 2.0 * luma - b]
                .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
            out.set_pixel(x, y, [r, g, b, a]);
        }
    }
    out
}

/// Draws `cards` on `compositor` - the window's device, else whatever
/// adapter is to be had - handing their ids to `report` a few at a time as
/// they land, so a shelf fills as it goes, and a last time with `done`. A
/// card that fails is logged and skipped: its package shows no picture.
///
/// On a software adapter - WARP, lavapipe, the Android emulator's
/// SwiftShader, a safe start's CONCAT_GPU=software - none are drawn unless
/// CONCAT_REQUIRE_GPU is set: there the hundred cards are every pixel
/// through the CPU, an hour and more on WARP, and on CI's API 30 emulator
/// they froze the whole device. The shelves show their names alone.
pub fn draw_all(
    dir: &Path,
    cards: &[Card],
    compositor: Option<WgpuCompositor>,
    mut report: impl FnMut(Vec<String>, bool),
) {
    const BATCH: usize = 12;
    let painter = compositor
        .or_else(WgpuCompositor::new)
        .ok_or_else(|| "no GPU or software renderer is available to draw with".to_owned())
        .and_then(|compositor| {
            if compositor.on_software_adapter() && std::env::var_os("CONCAT_REQUIRE_GPU").is_none()
            {
                return Err("a software adapter only; the cards are left undrawn".to_owned());
            }
            let still_path = still_in(dir)?;
            let city_path = city_still_in(dir).ok();
            Painter::with_city(compositor, &still_path, city_path.as_deref(), WIDTH, HEIGHT)
        });
    let mut painter = match painter {
        Ok(painter) => painter,
        Err(error) => {
            log::warn!("cards: {error}");
            report(Vec::new(), true);
            return;
        }
    };
    let mut batch = Vec::new();
    for card in cards {
        match painter.draw(card) {
            Ok(()) => batch.push(card.id.clone()),
            Err(error) => log::warn!("card {}: {error}", card.id),
        }
        if batch.len() >= BATCH {
            report(std::mem::take(&mut batch), false);
        }
    }
    report(batch, true);
}

/// What shows through where a card is transparent: grey squares a
/// twentieth of the height, the way every editor says "nothing here".
fn checker(width: u32, height: u32) -> Frame {
    let side = (height / 20).max(1);
    let mut frame = Frame::transparent(width, height);
    for y in 0..height {
        for x in 0..width {
            let light = ((x / side) + (y / side)).is_multiple_of(2);
            let level = if light { 0x5a } else { 0x44 };
            frame.set_pixel(x, y, [level, level, level, 255]);
        }
    }
    frame
}

/// The still before a screen of `colour`: everything left of the person
/// in the middle, and right of him, painted that colour, as a key's card
/// needs something to take out.
fn screened(still: &Frame, [r, g, b]: [u8; 3]) -> Frame {
    let (width, height) = (still.width(), still.height());
    let (left, right) = (width * 33 / 100, width * 80 / 100);
    let mut out = still.clone();
    for y in 0..height {
        for x in (0..left).chain(right..width) {
            out.set_pixel(x, y, [r, g, b, 255]);
        }
    }
    out
}

/// `frame` written to `path` as a JPEG, through a file beside it that is
/// renamed into place, so a reader never meets half a card.
fn write_card(frame: &Frame, path: &Path) -> Result<(), String> {
    let bytes = concat_media::jpeg(frame, QUALITY).map_err(|error| error.to_string())?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let part = path.with_extension("jpg.part");
    std::fs::write(&part, bytes).map_err(|error| error.to_string())?;
    std::fs::rename(&part, path).map_err(|error| error.to_string())
}

/// The still (sunset), written into `dir` for a decoder to read, and its path.
pub fn still_in(dir: &Path) -> Result<PathBuf, String> {
    let path = dir.join("still-sunset.jpg");
    if std::fs::read(&path).ok().as_deref() != Some(STILL_SUNSET) {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        std::fs::write(&path, STILL_SUNSET).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

/// The city still, written into `dir` for a decoder to read, and its path.
pub fn city_still_in(dir: &Path) -> Result<PathBuf, String> {
    let path = dir.join("still-city.jpg");
    if std::fs::read(&path).ok().as_deref() != Some(STILL_CITY) {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        std::fs::write(&path, STILL_CITY).map_err(|error| error.to_string())?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("concat-cards-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    #[test]
    fn transition_thumbnail_frames_follow_the_shader_progress() {
        let Some(gpu) = WgpuCompositor::new() else {
            return;
        };
        if gpu.on_software_adapter() {
            return;
        }
        let dir = scratch("transition-animation");
        let still = still_in(&dir).unwrap();
        let city = city_still_in(&dir).unwrap();
        let mut painter = Painter::with_city(gpu, &still, Some(&city), 96, 54).unwrap();
        let catalogue = Catalogue::builtin();
        let card = Card::of(catalogue, catalogue.get("concat.dissolve").unwrap(), &dir).unwrap();
        let start = painter.transition_frame(&card, 0.0).unwrap();
        let middle = painter.transition_frame(&card, 0.5).unwrap();
        let end = painter.transition_frame(&card, 1.0).unwrap();
        let matches = |actual: &Frame, expected: &Frame| {
            actual
                .pixels()
                .iter()
                .zip(expected.pixels())
                .all(|(a, b)| a.abs_diff(*b) <= 1)
        };
        assert!(matches(&start, &painter.still));
        assert!(matches(&end, &painter.city));
        assert_ne!(start, middle);
        assert_ne!(middle, end);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn picture_thumbnail_previews_show_before_after_and_real_clip_time() {
        let Some(gpu) = WgpuCompositor::new() else {
            assert!(
                std::env::var_os("CONCAT_REQUIRE_GPU").is_none(),
                "required adapter missing"
            );
            return;
        };
        if gpu.on_software_adapter() && std::env::var_os("CONCAT_REQUIRE_GPU").is_none() {
            return;
        }
        let dir = scratch("effect-animation");
        let still = still_in(&dir).unwrap();
        let city = city_still_in(&dir).unwrap();
        let mut painter = Painter::with_city(gpu, &still, Some(&city), 96, 54).unwrap();
        let catalogue = Catalogue::builtin();
        let card = Card::of(catalogue, catalogue.get("concat.scanlines").unwrap(), &dir).unwrap();
        let original = painter.frame(&card).unwrap();
        let before = painter.preview_frame(&card, 0.0).unwrap();
        let full = painter.preview_frame(&card, 0.5).unwrap();
        let end = painter.preview_frame(&card, 1.0).unwrap();
        assert_eq!(before, end, "static before/after loop meets without a jump");
        assert_ne!(before, full, "static effects demonstrate their treatment");
        assert_eq!(
            full, original,
            "midpoint shows the actual default treatment"
        );
        assert_eq!(
            original,
            painter.frame(&card).unwrap(),
            "preview must not mutate the card"
        );
        assert!(painter.preview_frame(&card, f32::NAN).is_err());
        let tape = Card::of(catalogue, catalogue.get("concat.vhs-tape").unwrap(), &dir).unwrap();
        assert_ne!(
            painter.preview_frame(&tape, 0.25).unwrap(),
            painter.preview_frame(&tape, 0.75).unwrap(),
            "equal-strength tape previews advance shader time"
        );
        let transition =
            Card::of(catalogue, catalogue.get("concat.dissolve").unwrap(), &dir).unwrap();
        assert_eq!(
            painter.preview_frame(&transition, 0.4).unwrap(),
            painter.transition_frame(&transition, 0.4).unwrap()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn subject_thumbnails_use_a_real_person_matte_and_animate() {
        let Some(gpu) = WgpuCompositor::new() else {
            assert!(
                std::env::var_os("CONCAT_REQUIRE_GPU").is_none(),
                "required adapter missing"
            );
            return;
        };
        if gpu.on_software_adapter() && std::env::var_os("CONCAT_REQUIRE_GPU").is_none() {
            return;
        }
        let dir = scratch("subject-animation");
        let still = still_in(&dir).unwrap();
        let city = city_still_in(&dir).unwrap();
        let mut painter = Painter::with_city(gpu, &still, Some(&city), 480, 270).unwrap();
        let catalogue = Catalogue::builtin();
        let (reference, mask) = painter.subject(480, 270).unwrap();
        assert!(mask.gray.contains(&0));
        assert!(mask.gray.iter().any(|v| *v > 240));
        let model = concat_vision::Segmenter::selfie().unwrap();
        model.begin();
        let inferred = model.mask(&reference).unwrap();
        assert!(
            inferred.bytes().iter().any(|v| *v > 128),
            "the reference must contain a detectable person"
        );
        let (mut intersection, mut union) = (0usize, 0usize);
        for y in 0..inferred.height() {
            for x in 0..inferred.width() {
                let detected = inferred.at(i64::from(x), i64::from(y)) > 128;
                let expected = mask.gray[((y * mask.height / inferred.height()) * mask.width
                    + x * mask.width / inferred.width())
                    as usize]
                    > 128;
                intersection += usize::from(detected && expected);
                union += usize::from(detected || expected);
            }
        }
        assert!(
            intersection as f64 / union.max(1) as f64 > 0.2,
            "person inference must overlap the known silhouette, not just return foreground"
        );
        let inferred = Arc::new(
            SubjectMask::new(
                inferred.width(),
                inferred.height(),
                inferred.bytes().to_vec(),
            )
            .unwrap(),
        );
        for id in ["concat.subject-neon", "concat.subject-electric"] {
            let card = Card::of(catalogue, catalogue.get(id).unwrap(), &dir).unwrap();
            let before = painter.preview_frame(&card, 0.0).unwrap();
            let first = painter.preview_frame(&card, 0.25).unwrap();
            let later = painter.preview_frame(&card, 0.75).unwrap();
            assert_ne!(before, first, "{id} shows the effect");
            assert_ne!(first, later, "{id} advances real shader time");
            painter.subject = Some((Arc::clone(&reference), Arc::clone(&inferred)));
            let detected = painter.preview_frame(&card, 0.5).unwrap();
            painter.subject = Some((Arc::clone(&reference), Arc::clone(&mask)));
            assert_ne!(before, detected, "{id} renders a genuine inferred matte");
            if let Some(output) = std::env::var_os("CLOVERCUT_SUBJECT_PROBES").or_else(|| {
                std::env::var_os("GITHUB_WORKSPACE")
                    .map(|path| PathBuf::from(path).join("subject-probes").into_os_string())
            }) {
                let output = PathBuf::from(output);
                std::fs::create_dir_all(&output).unwrap();
                for (name, frame) in [
                    ("thumbnail", &first),
                    ("inferred", &detected),
                    ("original", &before),
                ] {
                    let mut ppm =
                        format!("P6\n{} {}\n255\n", frame.width(), frame.height()).into_bytes();
                    for pixel in frame.pixels().chunks_exact(4) {
                        ppm.extend_from_slice(&pixel[..3]);
                    }
                    std::fs::write(output.join(format!("{id}-{name}.ppm")), ppm).unwrap();
                }
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// Every picture package has a card and every sound has none, each
    /// under a name of its own.
    #[test]
    fn every_picture_package_has_a_card_of_its_own() {
        let catalogue = Catalogue::builtin();
        let dir = Path::new("cards");
        let cards = cards(catalogue, dir);
        let pictures = catalogue
            .packages()
            .filter(|package| package.kind().is_visual())
            .count();
        assert_eq!(cards.len(), pictures);
        let names: HashSet<&Path> = cards.iter().map(|card| card.path.as_path()).collect();
        assert_eq!(names.len(), cards.len(), "two cards share a name");
        for card in &cards {
            let package = catalogue.get(&card.id).expect("a package");
            assert!(package.kind().is_visual(), "{}", card.id);
            assert!(card.path.starts_with(dir), "{}", card.path.display());
        }
    }

    /// A card's name follows what it is drawn from: the same package gives
    /// the same name, another shader or another default another.
    #[test]
    fn a_card_is_named_after_what_it_draws() {
        let catalogue = Catalogue::builtin();
        let dir = Path::new("cards");
        let manifest = |default: f64| {
            format!(
                "format = 2\n[effect]\nid = \"a.gain\"\nname = \"Gain\"\nkind = \"effect\"\n\
                 [[param]]\nkey = \"gain\"\nlabel = \"Gain\"\nmax = 4\ndefault = {default}\n\
                 [wgsl]\nentry = \"effect.wgsl\"\n"
            )
        };
        let shader = |k: f64| {
            format!(
                "struct Params {{ gain: f32 }}\nfn effect(uv: vec2<f32>) -> vec4<f32> {{ let c = sample(uv); return vec4<f32>(c.rgb * params.gain * {k:.1}, c.a); }}"
            )
        };
        let card = |default: f64, k: f64| {
            let package =
                Package::from_sources(&manifest(default), None, Some(&shader(k))).expect("loads");
            Card::of(catalogue, &package, dir).expect("a card").path
        };
        assert_eq!(card(2.0, 1.0), card(2.0, 1.0));
        assert_ne!(card(2.0, 1.0), card(3.0, 1.0), "another default");
        assert_ne!(card(2.0, 1.0), card(2.0, 2.0), "another shader");
        assert!(
            card(2.0, 1.0)
                .to_string_lossy()
                .starts_with(&*dir.join("a.gain-").to_string_lossy())
        );

        // A package drawn in passes: the same shader with its picture drawn
        // at another size is another card.
        let in_passes = |shrink: &str| {
            let manifest = format!(
                "{}[[wgsl.pass]]\ntarget = \"half\"\nshrink = [\"{shrink}\", \"{shrink}\"]\n",
                manifest(2.0)
            );
            let body = "struct Params { gain: f32 }\n\
                fn half(uv: vec2<f32>) -> vec4<f32> { return sample(uv) * params.gain; }\n\
                fn effect(uv: vec2<f32>) -> vec4<f32> { return half_at(uv); }";
            let package = Package::from_sources(&manifest, None, Some(body)).expect("loads");
            Card::of(catalogue, &package, dir).expect("a card").path
        };
        assert_eq!(in_passes("2"), in_passes("2"));
        assert_ne!(in_passes("2"), in_passes("4"), "another size of picture");
    }

    /// Pruning sweeps away a card no package draws any more and leaves the
    /// cards that are current, the still and anything else in the folder.
    #[test]
    fn prune_sweeps_only_stale_cards() {
        let dir = scratch("prune");
        let catalogue = Catalogue::builtin();
        let current: Vec<Card> = cards(catalogue, &dir).into_iter().take(2).collect();
        for card in &current {
            std::fs::write(&card.path, b"card").expect("write");
        }
        let stale = dir.join("concat.glow-0123456789abcdef.jpg");
        let other = dir.join("notes.jpg");
        let still = still_in(&dir).expect("the still");
        std::fs::write(&stale, b"old").expect("write");
        std::fs::write(&other, b"mine").expect("write");
        prune(&dir, &current);
        assert!(!stale.exists(), "a stale card");
        assert!(other.exists() && still.exists(), "not cards");
        assert!(current.iter().all(Card::is_drawn), "current cards");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every card draws on the GPU, and every one shows its package at
    /// work: the picture it was drawn from - the still, or the still
    /// before its screen - comes out changed, where the FFmpeg-drawn cards
    /// of old came out all but untouched for a mask, a key or a mirror. Set
    /// `CONCAT_CARDS_DIR` to keep the cards for a look.
    ///
    /// Over a hundred cards is an hour and more on Windows' software
    /// adapter: the CI job that ran it there was killed at ninety minutes
    /// every time. So on a software adapter the test runs only where the
    /// job says the GPU tests must (`CONCAT_REQUIRE_GPU`, which Linux CI
    /// sets for its lavapipe), and skips otherwise, saying so.
    #[test]
    fn every_card_draws_and_shows_its_package_at_work() {
        let Some(compositor) = WgpuCompositor::new() else {
            eprintln!("no usable GPU adapter; skipping");
            return;
        };
        if compositor.on_software_adapter() && std::env::var_os("CONCAT_REQUIRE_GPU").is_none() {
            eprintln!("a software adapter only, and CONCAT_REQUIRE_GPU is not set; skipping");
            return;
        }
        let dir = std::env::var_os("CONCAT_CARDS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| scratch("draw"));
        let still = still_in(&dir).expect("the still");
        let city = city_still_in(&dir).ok();
        let mut painter = Painter::with_city(compositor, &still, city.as_deref(), WIDTH, HEIGHT)
            .expect("a painter");
        let mut idle = Vec::new();
        let mut failed = Vec::new();
        let cards = cards(Catalogue::builtin(), &dir);
        for card in &cards {
            let frame = match painter.frame(card) {
                Ok(frame) => frame,
                Err(error) => {
                    failed.push(format!("{}: {error}", card.id));
                    continue;
                }
            };
            assert_eq!((frame.width(), frame.height()), (WIDTH, HEIGHT));
            let untreated = match &card.drawing {
                Drawing::Picture {
                    screen: Some(colour),
                    theme,
                    ..
                } => screened(
                    match theme {
                        CardTheme::Abrupt => &painter.city,
                        CardTheme::Calm => &painter.still,
                    },
                    *colour,
                ),
                Drawing::Picture { theme, .. } => match theme {
                    CardTheme::Abrupt => (*painter.city).clone(),
                    CardTheme::Calm => (*painter.still).clone(),
                },
                _ => (*painter.still).clone(),
            };
            let change = frame
                .pixels()
                .iter()
                .zip(untreated.pixels())
                .map(|(a, b)| f64::from(a.abs_diff(*b)))
                .sum::<f64>()
                / frame.pixels().len() as f64;
            eprintln!("{:40} {change:6.2}", card.id);
            if change < 0.02 {
                idle.push(format!(
                    "{}: changes the still by {change:.2} a channel",
                    card.id
                ));
            }
            write_card(&frame, &card.path).expect("writes");
            assert!(card.is_drawn());
        }
        assert!(failed.is_empty(), "\n{}", failed.join("\n"));
        assert!(idle.is_empty(), "\n{}", idle.join("\n"));
        if std::env::var_os("CONCAT_CARDS_DIR").is_none() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}
