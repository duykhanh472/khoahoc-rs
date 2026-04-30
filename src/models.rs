/// Core data models for the odin-ssg static site generator.
///
/// The hierarchy mirrors The Odin Project's Rails domain:
///   Curriculum → Path → Course → Section → Lesson
///
/// All navigation (prev/next/breadcrumbs) is resolved *after* the full
/// tree is built by the parser and stored directly on each Lesson.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level curriculum container
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curriculum {
    pub title: String,
    pub description: String,
    pub theme: Theme,
    /// All learning paths, ordered by `position`.
    pub paths: Vec<Path>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Path  (e.g. "Foundations", "Full Stack JavaScript")
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Path {
    /// URL-safe identifier derived from `title` (e.g. "foundations").
    pub slug: String,
    pub title: String,
    pub description: String,
    pub position: u32,
    /// Relative URL for this path's index page.
    pub url: String,
    pub courses: Vec<Course>,
}

impl Path {
    /// Flat, ordered list of all lessons across every course in this path.
    #[allow(dead_code)]
    pub fn all_lessons(&self) -> Vec<&Lesson> {
        self.courses
            .iter()
            .flat_map(|c| c.all_lessons())
            .collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Course  (e.g. "HTML and CSS", "JavaScript")
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub position: u32,
    /// Relative URL for this course's index page.
    pub url: String,
    /// Badge SVG filename (optional).
    pub badge_uri: Option<String>,
    pub sections: Vec<Section>,
}

impl Course {
    /// Flat, ordered list of all lessons across every section.
    /// Mirrors Rails `Course#lessons` (through sections, ordered by position).
    pub fn all_lessons(&self) -> Vec<&Lesson> {
        self.sections
            .iter()
            .flat_map(|s| s.lessons.iter())
            .collect()
    }

    /// Next lesson after `current` within this course.
    /// Mirrors Rails `Course#next_lesson`.
    #[allow(dead_code)]
    pub fn next_lesson_after<'a>(&'a self, current_slug: &str) -> Option<&'a Lesson> {
        let lessons = self.all_lessons();
        let pos = lessons.iter().position(|l| l.slug == current_slug)?;
        lessons.get(pos + 1).copied()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Section  (logical grouping within a course)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub description: String,
    pub position: u32,
    pub lessons: Vec<Lesson>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    /// URL-safe slug (e.g. "how-this-course-will-work").
    pub slug: String,
    pub title: String,
    pub display_title: String,
    pub description: String,
    pub position: u32,
    /// When true, the display title is prefixed with "Project: ".
    /// Mirrors Rails `Lesson#display_title`.
    pub is_project: bool,
    /// Absolute path of the source `.md` file.
    #[serde(skip)]
    #[allow(dead_code)]
    pub source_path: PathBuf,
    /// Root-relative output path, e.g. "foundations/introduction/how-this-course-will-work/index.html".
    pub output_path: String,
    /// Root-relative URL, e.g. "/foundations/introduction/how-this-course-will-work/".
    pub url: String,
    /// Rendered HTML body (populated during the render phase).
    pub html_content: Option<String>,
    /// Navigation: previous lesson in the global flat order.
    pub prev: Option<LessonRef>,
    /// Navigation: next lesson in the global flat order.
    pub next: Option<LessonRef>,
    /// Breadcrumb trail: Curriculum → Path → Course → (this lesson).
    pub breadcrumbs: Vec<Breadcrumb>,
}


// ─────────────────────────────────────────────────────────────────────────────
// Navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A lightweight reference to an adjacent lesson for prev/next navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LessonRef {
    pub title: String,
    pub url: String,
}

/// A single step in the breadcrumb trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Breadcrumb {
    /// Human-readable label.
    pub label: String,
    /// Root-relative URL (empty string for the current page).
    pub url: String,
    /// True when this is the active (last) crumb.
    pub is_current: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Search index entry (written to search-index.json)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SearchEntry {
    pub title: String,
    pub url: String,
    pub path: String,
    pub course: String,
    /// First ~200 chars of plain-text content for snippets.
    pub excerpt: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme Management
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub bg: String,
    pub text: String,
    pub accent: String,
    pub border: String,
    #[serde(default = "default_accent_text")]
    pub accent_text: String,
}

fn default_accent_text() -> String {
    "#ffffff".to_string()
}

impl Theme {
    /// The default "Biophilic" theme preset.
    pub fn biophilic() -> Self {
        Self {
            bg: "#f9fbf9".to_string(),
            text: "#1a2e1a".to_string(),
            accent: "#4a7c44".to_string(),
            border: "#d1dbd1".to_string(),
            accent_text: "#ffffff".to_string(),
        }
    }

    /// Generate a :root CSS string containing these colors as variables.
    pub fn to_css(&self) -> String {
        format!(
            ":root {{\n  --bg: {};\n  --text: {};\n  --accent: {};\n  --border: {};\n  --accent-text: {};\n}}\n",
            self.bg, self.text, self.accent, self.border, self.accent_text
        )
    }
}

