/// Serde-compatible schema for `manifest.yaml` files.
///
/// One manifest lives at the root of each **path** directory
/// (e.g. `curriculum-main/foundations/manifest.yaml`) and
/// describes the full path → course → section → lesson hierarchy.
///
/// Example:
/// ```yaml
/// title: "Foundations"
/// description: "Start here if you are new to web development."
/// position: 1
/// courses:
///   - slug: introduction
///     title: "Introduction"
///     description: "..."
///     position: 1
///     badge_uri: ~
///     sections:
///       - title: "Getting Started"
///         description: "..."
///         position: 1
///         lessons:
///           - file: how_this_course_will_work.md
///             title: "How This Course Will Work"
///             description: "..."
///             is_project: false
/// ```
use serde::{Deserialize, Serialize};
use crate::models::Theme;

// ─────────────────────────────────────────────────────────────────────────────
// Path manifest  (root of each path directory)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathManifest {
    pub title: String,
    pub description: String,
    pub position: u32,
    pub courses: Vec<CourseManifest>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Course manifest
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourseManifest {
    pub slug: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub position: u32,
    #[serde(default)]
    pub badge_uri: Option<String>,
    pub sections: Vec<SectionManifest>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Section manifest
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionManifest {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub position: u32,
    pub lessons: Vec<LessonManifest>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson manifest entry
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonManifest {
    /// Filename relative to the course directory (e.g. `how_this_course_will_work.md`).
    pub file: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub is_project: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Curriculum manifest  (root manifest.yaml at curriculum-main/)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurriculumManifest {
    pub title: String,
    pub description: String,
    /// Ordered list of path directory names to include.
    pub paths: Vec<String>,
    #[serde(default)]
    pub custom_colors: Option<Theme>,
    #[serde(default)]
    pub theme_preset: Option<String>,
}
