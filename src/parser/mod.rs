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

use crate::manifest::{CurriculumManifest, PathManifest, NestedCurriculumManifest};
use crate::models::{
    Breadcrumb, Course, Curriculum, Lesson, Path, SearchEntry, Section, Theme,
};
use anyhow::{Context, Result};
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Public API
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a curriculum source directory and return the fully-resolved tree.
///
/// Supports both:
///   - New nested format: single manifest.yaml with "nav" field
///   - Old format: root manifest.yaml lists paths, each path has separate manifest.yaml
///
/// # Errors
/// Returns an error if the manifest is malformed or referenced files cannot be read.
pub fn parse(source_root: &std::path::Path) -> Result<(Curriculum, Vec<SearchEntry>)> {
    // 1. Read root manifest
    let root_manifest_path = source_root.join("manifest.yaml");
    let root_content = std::fs::read_to_string(&root_manifest_path)
        .with_context(|| format!("Missing root manifest: {}", root_manifest_path.display()))?;

    // 2. Try to detect which format is being used
    let (curriculum, search_entries) = if root_content.contains("nav:") {
        // New nested format
        let nested_manifest: NestedCurriculumManifest = serde_yaml::from_str(&root_content)
            .context("Failed to parse nested manifest format")?;
        parse_nested_format(source_root, nested_manifest)?
    } else {
        // Old format
        let root_manifest: CurriculumManifest = serde_yaml::from_str(&root_content)
            .context("Failed to parse root manifest")?;
        parse_old_format(source_root, root_manifest)?
    };

    Ok((curriculum, search_entries))
}

// ─────────────────────────────────────────────────────────────────────────────
// New nested format parser
// ─────────────────────────────────────────────────────────────────────────────

fn parse_nested_format(
    source_root: &std::path::Path,
    manifest: NestedCurriculumManifest,
) -> Result<(Curriculum, Vec<SearchEntry>)> {
    let mut paths: Vec<Path> = Vec::new();

    for (position, nav_map) in manifest.nav.iter().enumerate() {
        for (path_slug, path_data) in nav_map {
            let path_dir = source_root.join(path_slug);

            let mut courses: Vec<Course> = Vec::new();

            for (course_position, course_map) in path_data.courses.iter().enumerate() {
                for (course_slug, course_data) in course_map {
                    let course_dir = path_dir.join(course_slug);
                    let course_url = format!("/{}/{}/", path_slug, course_slug);

                    let mut sections: Vec<Section> = Vec::new();

                    for (section_position, section_data) in
                        course_data.sections.iter().enumerate()
                    {
                        let mut lessons: Vec<Lesson> = Vec::new();

                        for (lesson_position, (lesson_title, lesson_value)) in
                            section_data.lessons.iter().enumerate()
                        {
                            let (filename, is_project) = match lesson_value {
                                crate::manifest::NestedLessonValue::File(f) => (f.clone(), false),
                                crate::manifest::NestedLessonValue::WithMeta {
                                    file,
                                    is_project,
                                } => (file.clone(), *is_project),
                            };

                            let lesson_slug = slug::slugify(lesson_title);
                            let lesson_url = format!("{}{}/", course_url, lesson_slug);
                            let output_path = format!(
                                "{}/{}/{}/index.html",
                                path_slug, course_slug, lesson_slug
                            );

                            // Resolve lesson file
                            let source_path = resolve_lesson_file(&course_dir, &filename)?;

                            // Read and render markdown
                            let raw_md = std::fs::read_to_string(&source_path).with_context(
                                || format!("Cannot read lesson file: {}", source_path.display()),
                            )?;
                            let html_content = markdown::render(&raw_md);

                            let display_title = if is_project {
                                format!("Project: {}", lesson_title)
                            } else {
                                lesson_title.clone()
                            };

                            // Breadcrumbs
                            let breadcrumbs = vec![
                                Breadcrumb {
                                    label: "Curriculum".to_string(),
                                    url: "/".to_string(),
                                    is_current: false,
                                },
                                Breadcrumb {
                                    label: path_slug_to_title(path_slug),
                                    url: format!("/{}/", path_slug),
                                    is_current: false,
                                },
                                Breadcrumb {
                                    label: path_slug_to_title(course_slug),
                                    url: course_url.clone(),
                                    is_current: false,
                                },
                                Breadcrumb {
                                    label: section_data.title.clone(),
                                    url: course_url.clone(),
                                    is_current: false,
                                },
                                Breadcrumb {
                                    label: display_title.clone(),
                                    url: lesson_url.clone(),
                                    is_current: true,
                                },
                            ];

                            let lesson = Lesson {
                                slug: lesson_slug,
                                title: lesson_title.clone(),
                                display_title,
                                description: String::new(),
                                position: (lesson_position + 1) as u32,
                                is_project,
                                source_path,
                                output_path,
                                url: lesson_url,
                                html_content: Some(html_content),
                                prev: None,
                                next: None,
                                breadcrumbs,
                            };

                            lessons.push(lesson);
                        }

                        sections.push(Section {
                            title: section_data.title.clone(),
                            description: section_data.description.clone(),
                            position: (section_position + 1) as u32,
                            lessons,
                        });
                    }

                    courses.push(Course {
                        slug: course_slug.clone(),
                        title: course_data.title.clone(),
                        description: course_data.description.clone(),
                        position: (course_position + 1) as u32,
                        url: course_url,
                        badge_uri: None,
                        sections,
                    });
                }
            }

            let path_url = format!("/{}/", path_slug);
            paths.push(Path {
                slug: path_slug.clone(),
                title: path_data.title.clone(),
                description: path_data.description.clone(),
                position: (position + 1) as u32,
                url: path_url,
                courses,
            });
        }
    }

    // Resolve navigation
    navigation::resolve_all(&mut paths);

    // Build search index
    let search_entries = build_search_index(&paths);

    // Theme resolution
    let theme = resolve_theme_nested(&manifest);

    let curriculum = Curriculum {
        title: manifest.title,
        description: manifest.description,
        theme,
        paths,
    };

    Ok((curriculum, search_entries))
}

// ─────────────────────────────────────────────────────────────────────────────
// Old format parser (existing logic)
// ─────────────────────────────────────────────────────────────────────────────

fn parse_old_format(
    source_root: &std::path::Path,
    root_manifest: CurriculumManifest,
) -> Result<(Curriculum, Vec<SearchEntry>)> {
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

    // 5. Theme resolution
    let theme = resolve_theme(source_root, &root_manifest);

    let curriculum = Curriculum {
        title: root_manifest.title,
        description: root_manifest.description,
        theme,
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
// Theme resolution
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_theme_nested(manifest: &NestedCurriculumManifest) -> Theme {
    // 1. Priority: custom_colors in manifest
    if let Some(custom) = &manifest.custom_colors {
        return custom.clone();
    }

    // 2. Priority: theme_preset in manifest
    // Note: themes.yml lookup would require source_root which we don't have here
    // For now, just return default
    if let Some(_preset_name) = &manifest.theme_preset {
        // Could look up in themes.yml if we had source_root
        // For now, fall through to default
    }

    // 3. Fallback: Built-in Biophilic preset
    Theme::biophilic()
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

/// Resolve the theme based on priority logic.
fn resolve_theme(source_root: &std::path::Path, manifest: &CurriculumManifest) -> Theme {
    // 1. Priority: custom_colors in manifest
    if let Some(custom) = &manifest.custom_colors {
        return custom.clone();
    }

    // 2. Priority: theme_preset in manifest (looked up in themes.yml)
    if let Some(preset_name) = &manifest.theme_preset {
        let themes_path = source_root.join("themes.yml");
        if themes_path.exists() {
            if let Ok(themes_map) = read_yaml::<std::collections::HashMap<String, Theme>>(&themes_path) {
                if let Some(theme) = themes_map.get(preset_name) {
                    return theme.clone();
                }
            }
        }
    }

    // 3. Fallback: Built-in Biophilic preset
    Theme::biophilic()
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
