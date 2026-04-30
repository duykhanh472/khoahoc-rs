/// Content Discovery parser.
///
/// Entry point: `parse(source_root) -> Result<Curriculum>`
///
/// Workflow:
///   1. Read `curriculum-main/manifest.yaml` for root metadata.
///   2. For each path listed, read `<path>/manifest.yaml`.
///   3. For each course in the path manifest, read its section/lesson entries.
///   4. Locate each lesson `.md` file, read + render Markdown.
///   5. Resolve prev/next/breadcrumbs across the global flat lesson list.
pub mod markdown;
pub mod navigation;

use crate::manifest::{CurriculumManifest, PathManifest};
use crate::models::{
    Breadcrumb, Course, Curriculum, Lesson, Path, SearchEntry, Section,
};
use anyhow::{Context, Result};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a curriculum source directory and return the fully-resolved tree.
///
/// # Errors
/// Returns an error if any required `manifest.yaml` is missing or malformed,
/// or if a referenced lesson `.md` file cannot be read.
pub fn parse(source_root: &std::path::Path) -> Result<(Curriculum, Vec<SearchEntry>)> {
    // 1. Root manifest
    let root_manifest_path = source_root.join("manifest.yaml");
    let root_manifest: CurriculumManifest = read_yaml(&root_manifest_path)
        .with_context(|| format!("Missing root manifest: {}", root_manifest_path.display()))?;

    // 2. Build paths
    let mut paths: Vec<Path> = root_manifest
        .paths
        .iter()
        .enumerate()
        .map(|(i, dir_name)| {
            let path_dir = source_root.join(dir_name);
            parse_path(&path_dir, dir_name, i as u32 + 1)
                .with_context(|| format!("Failed to parse path '{dir_name}'"))
        })
        .collect::<Result<Vec<_>>>()?;

    // 3. Resolve navigation across ALL lessons globally
    navigation::resolve_all(&mut paths);

    // 4. Build search index
    let search_entries = build_search_index(&paths);

    let curriculum = Curriculum {
        title: root_manifest.title,
        description: root_manifest.description,
        paths,
    };

    Ok((curriculum, search_entries))
}

// ─────────────────────────────────────────────────────────────────────────────
// Path parsing
// ─────────────────────────────────────────────────────────────────────────────

fn parse_path(path_dir: &std::path::Path, _dir_name: &str, _fallback_pos: u32) -> Result<Path> {
    let manifest_path = path_dir.join("manifest.yaml");
    let manifest: PathManifest = read_yaml(&manifest_path)
        .with_context(|| format!("Missing path manifest: {}", manifest_path.display()))?;

    let path_slug = slug::slugify(&manifest.title);
    let path_url = format!("/{}/", path_slug);

    let courses: Vec<Course> = manifest
        .courses
        .iter()
        .map(|cm| {
            let course_dir = path_dir.join(&cm.slug);
            parse_course(cm, &course_dir, &path_slug, &path_url)
                .with_context(|| format!("Failed to parse course '{}'", cm.slug))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Path {
        slug: path_slug.clone(),
        title: manifest.title,
        description: manifest.description,
        position: manifest.position,
        url: path_url,
        courses,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Course parsing
// ─────────────────────────────────────────────────────────────────────────────

fn parse_course(
    cm: &crate::manifest::CourseManifest,
    course_dir: &std::path::Path,
    path_slug: &str,
    path_url: &str,
) -> Result<Course> {
    let course_slug = slug::slugify(&cm.title);
    let course_url = format!("{}{}/", path_url, course_slug);

    let sections: Vec<Section> = cm
        .sections
        .iter()
        .map(|sm| {
            parse_section(sm, course_dir, path_slug, &course_slug, &course_url, path_url)
                .with_context(|| format!("Failed to parse section '{}'", sm.title))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Course {
        slug: course_slug,
        title: cm.title.clone(),
        description: cm.description.clone(),
        position: cm.position,
        url: course_url,
        badge_uri: cm.badge_uri.clone(),
        sections,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Section parsing
// ─────────────────────────────────────────────────────────────────────────────

fn parse_section(
    sm: &crate::manifest::SectionManifest,
    course_dir: &std::path::Path,
    path_slug: &str,
    course_slug: &str,
    course_url: &str,
    path_url: &str,
) -> Result<Section> {
    let lessons: Vec<Lesson> = sm
        .lessons
        .iter()
        .enumerate()
        .map(|(i, lm)| {
            parse_lesson(
                lm,
                course_dir,
                path_slug,
                course_slug,
                course_url,
                path_url,
                i as u32 + 1,
                sm,
            )
            .with_context(|| format!("Failed to parse lesson '{}'", lm.file))
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Section {
        title: sm.title.clone(),
        description: sm.description.clone(),
        position: sm.position,
        lessons,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Lesson parsing
// ─────────────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn parse_lesson(
    lm: &crate::manifest::LessonManifest,
    course_dir: &std::path::Path,
    path_slug: &str,
    course_slug: &str,
    course_url: &str,
    path_url: &str,
    position: u32,
    section: &crate::manifest::SectionManifest,
) -> Result<Lesson> {
    let lesson_slug = slug::slugify(&lm.title);
    let lesson_url = format!("{}{}/", course_url, lesson_slug);
    let output_path = format!(
        "{}/{}/{}/index.html",
        path_slug, course_slug, lesson_slug
    );

    // Resolve the .md file — it may live directly in course_dir or in a subdirectory
    let source_path = resolve_lesson_file(course_dir, &lm.file)?;

    // Read and render Markdown
    let raw_md = std::fs::read_to_string(&source_path)
        .with_context(|| format!("Cannot read lesson file: {}", source_path.display()))?;
    let html_content = markdown::render(&raw_md);

    let display_title = if lm.is_project {
        format!("Project: {}", lm.title)
    } else {
        lm.title.clone()
    };

    // Breadcrumbs — navigation and is_current will be filled by navigation resolver
    let breadcrumbs = vec![
        Breadcrumb {
            label: "Curriculum".to_string(),
            url: "/".to_string(),
            is_current: false,
        },
        Breadcrumb {
            label: path_slug_to_title(path_slug),
            url: path_url.to_string(),
            is_current: false,
        },
        Breadcrumb {
            label: course_slug_to_title(course_slug),
            url: course_url.to_string(),
            is_current: false,
        },
        Breadcrumb {
            label: section.title.clone(),
            url: course_url.to_string(),
            is_current: false,
        },
        Breadcrumb {
            label: display_title.clone(),
            url: lesson_url.clone(),
            is_current: true,
        },
    ];

    Ok(Lesson {
        slug: lesson_slug,
        title: lm.title.clone(),
        display_title,
        description: lm.description.clone(),
        position,
        is_project: lm.is_project,
        source_path,
        output_path,
        url: lesson_url,
        html_content: Some(html_content),
        prev: None, // resolved later
        next: None, // resolved later
        breadcrumbs,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Try to find the lesson file in the course dir or one level deeper.
fn resolve_lesson_file(course_dir: &std::path::Path, file: &str) -> Result<PathBuf> {
    // Direct: course_dir/file
    let direct = course_dir.join(file);
    if direct.exists() {
        return Ok(direct);
    }
    // One level deeper: walk subdirectories
    if let Ok(entries) = std::fs::read_dir(course_dir) {
        for entry in entries.flatten() {
            let candidate = entry.path().join(file);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
    }
    anyhow::bail!(
        "Lesson file '{}' not found under '{}'",
        file,
        course_dir.display()
    );
}

/// Read and deserialize a YAML file.
pub fn read_yaml<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Cannot read {}", path.display()))?;
    serde_yaml::from_str(&content)
        .with_context(|| format!("Cannot parse YAML in {}", path.display()))
}

/// Convert a slug back to a readable title (best-effort).
fn path_slug_to_title(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn course_slug_to_title(slug: &str) -> String {
    path_slug_to_title(slug)
}

/// Build the search index from all rendered lessons.
fn build_search_index(paths: &[Path]) -> Vec<SearchEntry> {
    let mut entries = Vec::new();
    for path in paths {
        for course in &path.courses {
            for section in &course.sections {
                for lesson in &section.lessons {
                    let excerpt = lesson
                        .html_content
                        .as_deref()
                        .map(strip_html_tags)
                        .unwrap_or_default()
                        .chars()
                        .take(200)
                        .collect::<String>();

                    entries.push(SearchEntry {
                        title: lesson.display_title.clone(),
                        url: lesson.url.clone(),
                        path: path.title.clone(),
                        course: course.title.clone(),
                        excerpt,
                    });
                }
            }
        }
    }
    entries
}

/// Naïve HTML tag stripper for search excerpts.
fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut inside_tag = false;
    for ch in html.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => out.push(ch),
            _ => {}
        }
    }
    out
}
