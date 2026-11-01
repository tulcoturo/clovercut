// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer and Concat contributors

//! Optional Hyprland/grim screenshot fixture for editor states and layouts.
//!
//! Import readiness is state-driven and bounded; layout captures still need visual review.
//! Missing-media and save-warning dialogs are staged; export steps configure
//! the dialog without completing an export. Captures require visual review.
//! Fixture or screenshot failure stops the driver, but reaching its last step
//! does not qualify recovery, playback, export, keyboard or accessibility.
//! Use docs/DESKTOP_QA.md for the interactive qualification checklist.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use concat_host::projects;
use concat_project::Command;
use concat_project::commands::ClipPatch;
use concat_project::model::{MissingMedia, TextStyle};
use slint::ComponentHandle;

use crate::host::Shell;
use crate::panes::Msg;
use crate::panes::captions::CaptionsMsg;
use crate::panes::export::ExportMsg;
use crate::panes::media_bin::MediaMsg;
use crate::panes::relink::RelinkMsg;
use crate::panes::start::StartMsg;
use crate::ui::{App, Theme, TimelineTool};

fn ensure_synthetic_media(dir: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let video_a = dir.join("landscape_nature.mp4");
    let video_b = dir.join("portrait_vlog.mp4");
    let audio = dir.join("soundtrack.wav");

    if !video_a.exists() {
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=1920x1080:rate=30",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000",
                "-t",
                "5",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&video_a)
            .status()
            .expect("ffmpeg must be available for the walkthrough");
        assert!(status.success(), "could not create landscape fixture");
    }

    if !video_b.exists() {
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=size=1080x1920:rate=30",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=880:sample_rate=48000",
                "-t",
                "4",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "aac",
            ])
            .arg(&video_b)
            .status()
            .expect("ffmpeg must be available for the walkthrough");
        assert!(status.success(), "could not create portrait fixture");
    }

    if !audio.exists() {
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=220:sample_rate=48000",
                "-t",
                "8",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&audio)
            .status()
            .expect("ffmpeg must be available for the walkthrough");
        assert!(status.success(), "could not create audio fixture");
    }

    (video_a, video_b, audio)
}

fn window_geometry() -> Option<(i32, i32, u32, u32)> {
    let output = std::process::Command::new("hyprctl")
        .args(["clients", "-j"])
        .output()
        .ok()?;
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let my_pid = std::process::id();
    for client in parsed.as_array()? {
        if client.get("pid").and_then(|p| p.as_u64()) == Some(my_pid as u64) {
            let at = client.get("at")?.as_array()?;
            let size = client.get("size")?.as_array()?;
            let x = at.first()?.as_i64()? as i32;
            let y = at.get(1)?.as_i64()? as i32;
            let w = size.first()?.as_u64()? as u32;
            let h = size.get(1)?.as_u64()? as u32;
            return Some((x, y, w, h));
        }
    }
    None
}

fn capture_screenshot(name: &str) {
    let dir_override = std::env::var("CLOVERCUT_SCREENSHOT_DIR").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join(format!("clovercut-ui-walkthrough-{}", std::process::id()))
            .to_string_lossy()
            .into_owned()
    });
    let screenshot_dir = Path::new(&dir_override);
    std::fs::create_dir_all(screenshot_dir).expect("create walkthrough screenshot directory");
    let dest = screenshot_dir.join(format!("{name}.png"));

    if let Some((x, y, w, h)) = window_geometry() {
        let geom = format!("{x},{y} {w}x{h}");
        let res = std::process::Command::new("grim")
            .args(["-g", &geom, dest.to_str().unwrap()])
            .output();
        if let Ok(out) = res
            && out.status.success()
        {
            println!(
                "WALKTHROUGH: Captured {name} at {geom} -> {}",
                dest.display()
            );
            return;
        }
    }
    panic!("WALKTHROUGH: Failed to capture {name}; requires Hyprland and grim");
}

pub fn start(shell: Rc<Shell>, app_weak: slint::Weak<App>) -> slint::Timer {
    println!("WALKTHROUGH: Initializing Phase 5 UI/UX Walkthrough Driver");

    let temp_root =
        std::env::temp_dir().join(format!("clovercut-p5-walkthrough-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp_root);
    let media_dir = temp_root.join("media");
    let _ = std::fs::create_dir_all(&media_dir);
    let (video_a, video_b, audio) = ensure_synthetic_media(&media_dir);

    let state = Rc::new(RefCell::new(0usize));
    let step_entered = Rc::new(RefCell::new(Instant::now()));
    let first_clip_id = Rc::new(RefCell::new(String::new()));
    let text_clip_id = Rc::new(RefCell::new(String::new()));

    let timer = slint::Timer::default();
    let shell_clone = shell.clone();

    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(600),
        move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let mut current_step = state.borrow_mut();
            let step = *current_step;
            // Imports complete on workers. Never capture an empty bin or skip
            // the editing sequence merely because a slow import exceeds 600 ms.
            if step == 3 {
                let studio = shell_clone.studio.borrow();
                let ready = [&video_a, &video_b, &audio].iter().all(|path| {
                    studio.project().media.iter().any(|item| Path::new(&item.path) == path.as_path())
                });
                if !ready {
                    assert!(!studio.toast.failed, "walkthrough import failed: {}", studio.toast.message);
                    assert!(step_entered.borrow().elapsed() < Duration::from_secs(60),
                        "walkthrough import timed out; {} media ready", studio.project().media.len());
                    return;
                }
            }
            *current_step += 1;
            *step_entered.borrow_mut() = Instant::now();

            match step {
                // Step 0: Initial launch screen
                0 => {
                    println!("WALKTHROUGH [0]: Capturing 01_start_launch_screen");
                    capture_screenshot("01_start_launch_screen");
                }

                // Step 1: Start compose dialog
                1 => {
                    println!("WALKTHROUGH [1]: Opening New Project compose dialog");
                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Start(StartMsg::Compose));
                    studio.handle(Msg::Start(StartMsg::NameEdited("Walkthrough_Demo".into())));
                    studio.handle(Msg::Start(StartMsg::AspectChanged(0))); // 16:9
                    studio.handle(Msg::Start(StartMsg::SizeChanged(1)));   // 1080p
                    studio.handle(Msg::Start(StartMsg::RateChanged(2)));   // 30 fps
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 2: Capture compose dialog, open project & import media
                2 => {
                    println!("WALKTHROUGH [2]: Capturing 01b_new_project_dialog & opening project");
                    capture_screenshot("01b_new_project_dialog");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Start(StartMsg::Dismiss));

                    let proj_path = temp_root.join("Walkthrough_Demo");
                    let info = projects::create(
                        proj_path.to_str().unwrap(),
                        "Walkthrough_Demo",
                        1920,
                        1080,
                        30,
                        1,
                    )
                    .expect("failed to create project");
                    studio.open_project(info).expect("failed to open project");
                    studio.on_start = false;

                    // Import synthetic media
                    studio.handle(Msg::Media(MediaMsg::Import(vec![
                        video_a.clone(),
                        video_b.clone(),
                        audio.clone(),
                    ])));
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 3: Capture media bin, add clips to timeline, razor split & undo/redo
                3 => {
                    println!("WALKTHROUGH [3]: Capturing 02_media_library_import & editing timeline");
                    capture_screenshot("02_media_library_import");

                    let mut studio = shell_clone.studio.borrow_mut();
                    // Bind by fixture path, not document insertion order.
                    let media_ids: Vec<String> = [&video_a, &video_b, &audio].iter().map(|path| {
                        studio.project().media.iter().find(|item| Path::new(&item.path) == path.as_path())
                            .expect("fixture import completed").id.clone()
                    }).collect();

                    if media_ids.len() >= 2 {
                        let track_video = studio.timeline().tracks[0].id.clone();
                        let cid1 = studio
                            .apply(Command::AddClip {
                                media_id: media_ids[0].clone(),
                                track_id: track_video.clone(),
                                start: 0.0,
                                ripple: false,
                            })
                            .expect("add clip 1");
                        *first_clip_id.borrow_mut() = cid1.clone();

                        let _ = studio.apply(Command::AddClip {
                            media_id: media_ids[1].clone(),
                            track_id: track_video.clone(),
                            start: 3.5,
                            ripple: false,
                        });

                        if studio.timeline().tracks.len() > 1 {
                            let track_audio = studio.timeline().tracks[1].id.clone();
                            let _ = studio.apply(Command::AddClip {
                                media_id: media_ids.last().unwrap().clone(),
                                track_id: track_audio,
                                start: 0.0,
                                ripple: false,
                            });
                        }

                        // Move playhead to 1.5s
                        studio.seek(1.5);
                        // Activate razor tool
                        studio.lanes.tool = TimelineTool::Split;
                        // Select clip 1
                        studio.selection = vec![cid1.clone()];
                        // Split clip at playhead
                        studio.split_at(1.5, true);
                        // Test undo and redo
                        studio.undo();
                        studio.redo();
                        // Reselect first clip half
                        studio.selection = vec![cid1];
                    }
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 4: Capture razor & timeline, open inspector and adjust transform
                4 => {
                    println!("WALKTHROUGH [4]: Capturing 03_timeline_editing_razor & editing inspector");
                    capture_screenshot("03_timeline_editing_razor");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.lanes.tool = TimelineTool::Select;

                    let cid = first_clip_id.borrow().clone();
                    if !cid.is_empty() {
                        studio.selection = vec![cid.clone()];
                        let _ = studio.apply(Command::SetClipTransform {
                            clip_id: cid.clone(),
                            scale: Some(1.25),
                            offset_x: Some(0.04),
                            offset_y: Some(-0.03),
                            rotation: Some(8.5),
                            stretch_x: None,
                            stretch_y: None,
                        });
                        let _ = studio.apply(Command::UpdateClip {
                            clip_id: cid,
                            patch: ClipPatch {
                                opacity: Some(0.85),
                                ..Default::default()
                            },
                        });
                    }
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 5: Capture inspector, create normal text clip
                5 => {
                    println!("WALKTHROUGH [5]: Capturing 04_inspector_transform_effects & creating normal text clip");
                    capture_screenshot("04_inspector_transform_effects");

                    let mut studio = shell_clone.studio.borrow_mut();
                    let t_cid = studio
                        .apply(Command::AddTextClip {
                            track_id: None,
                            above: true,
                            start: 1.0,
                            style: Some(TextStyle {
                                content: "Clovercut 4K HDR Walkthrough".into(),
                                font_size: 44.0,
                                ..Default::default()
                            }),
                            duration: Some(3.5),
                            offset_y: Some(0.35),
                        })
                        .expect("add text clip");
                    *text_clip_id.borrow_mut() = t_cid.clone();
                    studio.selection = vec![t_cid];
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 6: Capture normal text inspector, then create dedicated caption collection & place on timeline
                6 => {
                    println!("WALKTHROUGH [6]: Capturing 05_normal_text_inspector & creating dedicated caption collection");
                    capture_screenshot("05_caption_styling_overlay");

                    let mut studio = shell_clone.studio.borrow_mut();
                    let srt_content = "1\n00:00:00,000 --> 00:00:02,000\nWelcome to Clovercut\n\n2\n00:00:02,000 --> 00:00:04,500\nDual Text & Captions System\n";
                    if let Some(src_id) = studio.apply(Command::AddCaptionSource {
                        name: "demo_subtitles.srt".into(),
                        srt: srt_content.into(),
                        style: TextStyle::default(),
                        offset_y: 0.35,
                        place_at: Some(0.0),
                    }) {
                        // Select the first placed caption clip so the Captions | Style | Effects Inspector shows
                        if let Some(cap_clip) = studio
                            .timeline()
                            .clips
                            .iter()
                            .find(|c| concat_project::captions::is_caption(c))
                            .map(|c| c.id.clone())
                        {
                            studio.selection = vec![cap_clip];
                        }
                        let _ = src_id;
                    }
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 7: Capture dedicated captions inspector, then open centered CaptionsDialog
                7 => {
                    println!("WALKTHROUGH [7]: Capturing 05b_captions_collection_inspector & opening CaptionsDialog");
                    capture_screenshot("05b_captions_collection_inspector");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Captions(CaptionsMsg::Open));
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 8: Capture centered CaptionsDialog, simulate missing media and open relink dialog
                8 => {
                    println!("WALKTHROUGH [8]: Capturing 05c_captions_dialog_centered & simulating missing media");
                    capture_screenshot("05c_captions_dialog_centered");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Captions(CaptionsMsg::Close));

                    studio.handle(Msg::Relink(RelinkMsg::Show(vec![MissingMedia {
                        id: "m_missing".into(),
                        name: "portrait_vlog.mp4".into(),
                        path: video_b.to_string_lossy().to_string(),
                    }])));
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 9: Capture relink dialog, simulate save durability warning
                9 => {
                    println!("WALKTHROUGH [9]: Capturing 06_missing_media_relink & triggering durability warning");
                    capture_screenshot("06_missing_media_relink");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Relink(RelinkMsg::Dismiss));

                    studio.offer_quit_without_saving(
                        "Disk I/O error: simulated read-only filesystem or full disk (EACCES)".into(),
                    );
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 10: Capture durability warning, start playback
                10 => {
                    println!("WALKTHROUGH [10]: Capturing 07_save_durability_warning & starting playback");
                    capture_screenshot("07_save_durability_warning");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.confirm = None;
                    if !studio.playing {
                        studio.play_toggle();
                    }
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 11: Capture active playback with monitor & VU meters, open export dialog
                11 => {
                    println!("WALKTHROUGH [11]: Capturing 08_playback_monitor_seeking & opening export dialog");
                    capture_screenshot("08_playback_monitor_seeking");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.pause();
                    studio.handle(Msg::Export(ExportMsg::Open));
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 12: Capture export dialog, maximize window for 1080p comparison
                12 => {
                    println!("WALKTHROUGH [12]: Capturing 09_export_configuration_dialog & maximizing window");
                    capture_screenshot("09_export_configuration_dialog");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Export(ExportMsg::Close));
                    let _ = std::process::Command::new("hyprctl")
                        .args(["dispatch", "fullscreen", "1"])
                        .status();
                    crate::platform::toggle_maximize(app.window());
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 13: Capture maximized layout, open export at 1080p
                13 => {
                    println!("WALKTHROUGH [13]: Capturing 10_editor_workspace_maximized_1080p & opening export at 1080p");
                    capture_screenshot("10_editor_workspace_maximized_1080p");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Export(ExportMsg::Open));
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 14: Capture 1080p export modal, close it & switch to Light theme
                14 => {
                    println!("WALKTHROUGH [14]: Capturing 11_export_modal_1080p & switching to Light theme");
                    capture_screenshot("11_export_modal_1080p");

                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.handle(Msg::Export(ExportMsg::Close));
                    app.global::<Theme>().set_dark(false);
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 15: Capture Light theme 1080p workspace & resize to narrow 927x1025
                15 => {
                    println!("WALKTHROUGH [15]: Capturing 12_editor_workspace_light_1080p & resizing to narrow 927x1025");
                    capture_screenshot("12_editor_workspace_light_1080p");

                    app.global::<Theme>().set_dark(true);
                    let _ = std::process::Command::new("hyprctl")
                        .args(["dispatch", "fullscreen", "0"])
                        .status();
                    app.window().set_size(slint::LogicalSize::new(927.0, 1025.0));
                    let mut studio = shell_clone.studio.borrow_mut();
                    studio.refresh_art();
                    studio.publish(&app, &shell_clone.models);
                }

                // Step 16: Capture narrow 927x1025 workspace & exit
                16 => {
                    println!("WALKTHROUGH [16]: Capturing 13_editor_workspace_narrow_927x1025 & finishing");
                    capture_screenshot("13_editor_workspace_narrow_927x1025");

                    println!("WALKTHROUGH: All Phase 5 screenshots successfully captured!");
                    let _ = std::fs::remove_dir_all(&temp_root);
                    slint::quit_event_loop().ok();
                }

                _ => {}
            }
        },
    );

    timer
}
