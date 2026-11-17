//! Project templates with replaceable placeholder slots.
//!
//! Inspired by modern NLE workflows (such as CapCut and ReaCut templates),
//! a [`ProjectTemplate`] captures an entire project structure while designating
//! specific media assets and text titles as replacement slots.
//!
//! Instantiating a template isolates IDs so subsequent edits never corrupt
//! the template, and batch slot replacement preserves all transition timing,
//! keyframe animations, audio mixing, and geometric transforms.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::model::{ClipKind, Project};

/// The kind of content a template slot holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TemplateSlotKind {
    /// A media file on the timeline (video, image, audio).
    Media,
    /// A text or subtitle title.
    Text,
}

/// A placeholder slot exposed by a template for quick substitution.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateSlot {
    /// Unique identifier of the slot within this template.
    pub id: String,
    /// Whether this slot replaces media or text.
    pub kind: TemplateSlotKind,
    /// ID of the target MediaItem (for Media) or Clip (for Text).
    pub target_id: String,
    /// Human-readable label displayed in the inspector / template UI.
    pub label: String,
}

/// A reusable project template with dynamic replacement slots.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTemplate {
    /// Template display name.
    pub name: String,
    /// Exposed placeholder slots.
    pub slots: Vec<TemplateSlot>,
    /// Underlying project model.
    pub project: Project,
}

impl ProjectTemplate {
    /// Creates a template from an existing project, automatically identifying
    /// media assets and text clips used in the active timeline as slots.
    pub fn from_project(project: &Project, name: &str) -> Self {
        let active = project.active();
        let mut slots = Vec::new();
        let mut seen_media = std::collections::HashSet::new();

        // Collect media slots for assets actually placed on the timeline
        let mut slot_counter = 1;
        for clip in &active.clips {
            if !clip.media_id.is_empty() && seen_media.insert(clip.media_id.clone()) {
                let label = project
                    .media
                    .iter()
                    .find(|m| m.id == clip.media_id)
                    .map(|m| m.name.clone())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| format!("Media Slot {slot_counter}"));
                slots.push(TemplateSlot {
                    id: format!("slot_media_{slot_counter}"),
                    kind: TemplateSlotKind::Media,
                    target_id: clip.media_id.clone(),
                    label,
                });
                slot_counter += 1;
            }
        }

        // Collect text slots for titles and captions on the active timeline
        let mut text_counter = 1;
        for clip in &active.clips {
            if clip.kind == ClipKind::Text {
                let label = clip
                    .text
                    .as_ref()
                    .and_then(|t| t.content.lines().next())
                    .filter(|l| !l.is_empty())
                    .map(|l| l.chars().take(40).collect())
                    .unwrap_or_else(|| format!("Text Slot {text_counter}"));
                slots.push(TemplateSlot {
                    id: format!("slot_text_{text_counter}"),
                    kind: TemplateSlotKind::Text,
                    target_id: clip.id.clone(),
                    label,
                });
                text_counter += 1;
            }
        }

        Self {
            name: name.to_owned(),
            slots,
            project: project.clone(),
        }
    }

    /// Instantiates an independent project copy from the template.
    ///
    /// Mints fresh unique IDs for all media items, tracks, clips, and detached audio references,
    /// ensuring the instantiated project can be safely edited without affecting
    /// the template or other instances.
    pub fn instantiate(&self, new_active_name: &str) -> Project {
        let mut instance = self.project.clone();

        let mut media_map = HashMap::new();
        for (i, media) in instance.media.iter_mut().enumerate() {
            let new_id = format!("m_inst_{i}");
            media_map.insert(media.id.clone(), new_id.clone());
            media.id = new_id;
        }

        let mut track_map = HashMap::new();
        let mut clip_map = HashMap::new();
        let timeline = instance.active_mut();
        timeline.name = new_active_name.to_owned();

        for (i, track) in timeline.tracks.iter_mut().enumerate() {
            let new_id = format!("t_inst_{i}");
            track_map.insert(track.id.clone(), new_id.clone());
            track.id = new_id;
        }

        for (i, clip) in timeline.clips.iter_mut().enumerate() {
            let clip_mut = Arc::make_mut(clip);
            let new_id = format!("c_inst_{i}");
            clip_map.insert(clip_mut.id.clone(), new_id.clone());
            clip_mut.id = new_id;
            if let Some(new_tid) = track_map.get(&clip_mut.track_id) {
                clip_mut.track_id = new_tid.clone();
            }
            if !clip_mut.media_id.is_empty()
                && let Some(new_mid) = media_map.get(&clip_mut.media_id)
            {
                clip_mut.media_id = new_mid.clone();
            }
        }

        // Remap detached_from audio dependencies
        for clip in &mut timeline.clips {
            if let Some(parent_id) = &clip.detached_from
                && let Some(new_parent) = clip_map.get(parent_id)
            {
                let clip_mut = Arc::make_mut(clip);
                clip_mut.detached_from = Some(new_parent.clone());
            }
        }

        instance
    }

    /// Replaces slots in-place across the template's project.
    ///
    /// - For `text_replacements`: maps `slot_id` -> new text content.
    /// - For `media_paths`: maps `slot_id` -> new file path.
    ///
    /// Returns the total number of successfully updated slots.
    pub fn apply_replacements(
        &mut self,
        text_replacements: &HashMap<String, String>,
        media_paths: &HashMap<String, String>,
    ) -> usize {
        let mut modified = 0;

        for slot in &self.slots {
            match slot.kind {
                TemplateSlotKind::Text => {
                    if let Some(new_text) = text_replacements.get(&slot.id) {
                        let timeline = self.project.active_mut();
                        if let Some(clip) = timeline.clip_mut(&slot.target_id)
                            && let Some(text_style) = &mut clip.text
                        {
                            text_style.content = new_text.clone();
                            clip.name = new_text
                                .lines()
                                .next()
                                .unwrap_or("Text")
                                .chars()
                                .take(30)
                                .collect();
                            modified += 1;
                        }
                    }
                }
                TemplateSlotKind::Media => {
                    if let Some(new_path) = media_paths.get(&slot.id)
                        && let Some(media) = self
                            .project
                            .media
                            .iter_mut()
                            .find(|m| m.id == slot.target_id)
                    {
                        media.path = new_path.clone();
                        media.name = std::path::Path::new(new_path)
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or("Media")
                            .to_owned();
                        modified += 1;
                    }
                }
            }
        }

        modified
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Clip, ClipKind, MediaItem, MediaKind, TextStyle};

    #[test]
    fn template_extraction_and_slot_replacement() {
        let mut proj = Project::new();
        proj.media.push(MediaItem {
            id: "m1".into(),
            name: "Background.mp4".into(),
            path: "/path/to/Background.mp4".into(),
            duration: Some(10.0),
            kind: MediaKind::Video,
            width: Some(1920),
            height: Some(1080),
            frame_rate: None,
            frame_rate_fraction: None,
            video_codec: None,
            audio_codec: None,
            has_audio: false,
            audio_tracks: vec![],
            placeholder: false,
            color_range: None,
            color_space: Default::default(),
            origin: None,
            extra: Default::default(),
        });

        let tl = proj.active_mut();
        let mut vid_clip = Clip::blank("c1", "T1", ClipKind::Video, "Video", 0.0, 5.0);
        vid_clip.media_id = "m1".into();
        tl.clips.push(Arc::new(vid_clip));

        let mut txt_clip = Clip::blank("c2", "T1", ClipKind::Text, "Title", 0.0, 3.0);
        txt_clip.text = Some(TextStyle {
            content: "Welcome to Clovercut".into(),
            ..Default::default()
        });
        tl.clips.push(Arc::new(txt_clip));

        let mut template = ProjectTemplate::from_project(&proj, "Intro Template");
        assert_eq!(template.slots.len(), 2);
        assert_eq!(template.slots[0].kind, TemplateSlotKind::Media);
        assert_eq!(template.slots[0].label, "Background.mp4");
        assert_eq!(template.slots[1].kind, TemplateSlotKind::Text);
        assert_eq!(template.slots[1].label, "Welcome to Clovercut");

        let mut text_map = HashMap::new();
        text_map.insert(template.slots[1].id.clone(), "New Channel Intro".into());

        let mut media_map = HashMap::new();
        media_map.insert(
            template.slots[0].id.clone(),
            "/new/path/BrandIntro.mov".into(),
        );

        let count = template.apply_replacements(&text_map, &media_map);
        assert_eq!(count, 2);

        let active = template.project.active();
        assert_eq!(
            active.clip("c2").unwrap().text.as_ref().unwrap().content,
            "New Channel Intro"
        );
        assert_eq!(template.project.media[0].path, "/new/path/BrandIntro.mov");

        // Instantiation produces an independent project with fresh IDs
        let instance = template.instantiate("My Custom Project");
        assert_eq!(instance.active().name, "My Custom Project");
        assert_ne!(instance.media[0].id, "m1");
        assert_ne!(instance.active().clips[0].id, "c1");
        assert_eq!(instance.active().clips[0].media_id, instance.media[0].id);
    }
}
