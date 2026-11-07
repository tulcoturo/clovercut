// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Clovercut contributors

//! One cancellable thumbnail worker, independent of the project and monitor.
use crate::{studio::Studio, ui::TransitionPreview};
use concat_core::frame::Frame;
use concat_host::cards::{self, Card, Painter};
use slint::ComponentHandle;
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
    mpsc,
};

static GENERATION: AtomicU64 = AtomicU64::new(0);
static WORKER: OnceLock<mpsc::Sender<(u64, Card)>> = OnceLock::new();

pub fn request(studio: &mut Studio, id: &str) {
    let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
    let name = id.to_owned();
    crate::host::on_ui(move |_, app, _| {
        if GENERATION.load(Ordering::Relaxed) != generation {
            return;
        }
        let editor = app.global::<TransitionPreview>();
        editor.set_id(name.into());
        editor.set_frames(Default::default());
    });
    if id.is_empty() || !studio.host.monitor.has_gpu() {
        return;
    }
    let dir = Studio::cards_dir(&studio.host.dirs);
    let catalogue = concat_effects::Catalogue::builtin();
    let Some(package) = catalogue.get(id) else {
        super::gl_transitions::download(studio, id, Some(generation), None);
        return;
    };
    if !matches!(
        package.kind(),
        concat_effects::Kind::Effect
            | concat_effects::Kind::Filter
            | concat_effects::Kind::Transition
    ) {
        return;
    }
    let Some(card) = Card::of(catalogue, package, &dir) else {
        return;
    };
    let sender = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<(u64, Card)>();
        let compositor = studio.host.monitor.off_thread();
        let _ = std::thread::Builder::new()
            .name("catalogue thumbnails".into())
            .spawn(move || {
                let mut painter: Option<Painter> = None;
                let mut compositor = compositor;
                let mut cache: std::collections::VecDeque<(std::path::PathBuf, Vec<Frame>)> =
                    Default::default();
                while let Ok((mut generation, mut card)) = receiver.recv() {
                    while let Ok(latest) = receiver.try_recv() {
                        (generation, card) = latest;
                    }
                    if GENERATION.load(Ordering::Relaxed) != generation {
                        continue;
                    }
                    let frames = if let Some((_, frames)) =
                        cache.iter().find(|(key, _)| key == &card.path)
                    {
                        frames.clone()
                    } else {
                        if painter.is_none() {
                            let Some(gpu) = compositor
                                .take()
                                .or_else(concat_host::export::WgpuCompositor::new)
                            else {
                                continue;
                            };
                            // Hover must never start expensive software rendering.
                            if gpu.on_software_adapter() {
                                break;
                            }
                            let Ok(still) = cards::still_in(&dir) else {
                                continue;
                            };
                            let city = cards::city_still_in(&dir).ok();
                            match Painter::with_city(gpu, &still, city.as_deref(), 192, 108) {
                                Ok(value) => painter = Some(value),
                                Err(error) => {
                                    log::warn!("catalogue preview: {error}");
                                    break;
                                }
                            }
                        }
                        let mut frames = Vec::with_capacity(24);
                        for index in 0..24 {
                            if GENERATION.load(Ordering::Relaxed) != generation {
                                break;
                            }
                            match painter
                                .as_mut()
                                .unwrap()
                                .preview_frame(&card, index as f32 / 23.0)
                            {
                                Ok(frame) => frames.push(frame),
                                Err(error) => {
                                    log::warn!("catalogue preview: {error}");
                                    break;
                                }
                            }
                        }
                        if frames.len() != 24 {
                            continue;
                        }
                        if cache.len() == 4 {
                            cache.pop_front();
                        }
                        cache.push_back((card.path.clone(), frames.clone()));
                        frames
                    };
                    crate::host::on_ui(move |_, app, _| {
                        if GENERATION.load(Ordering::Relaxed) != generation {
                            return;
                        }
                        let images: Vec<slint::Image> = frames
                            .iter()
                            .map(|frame| {
                                slint::Image::from_rgba8(
                                    slint::SharedPixelBuffer::clone_from_slice(
                                        frame.pixels(),
                                        frame.width(),
                                        frame.height(),
                                    ),
                                )
                            })
                            .collect();
                        app.global::<TransitionPreview>()
                            .set_frames(std::rc::Rc::new(slint::VecModel::from(images)).into());
                    });
                }
            });
        sender
    });
    let _ = sender.send((generation, card));
}

/// Resumes a downloaded preview only while its original hover is current.
pub fn resume(studio: &mut Studio, id: &str, generation: u64) {
    if GENERATION.load(Ordering::Relaxed) == generation {
        request(studio, id);
    }
}
