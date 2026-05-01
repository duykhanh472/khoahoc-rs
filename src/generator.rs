/// Generates a full nested manifest.yaml from a directory structure.
///
/// Scans `curriculum-main/{path}/` directories to auto-detect courses, sections,
/// and lessons, then generates a hierarchical manifest ready for manual editing.

use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use serde_yaml;

use crate::manifest::NestedCurriculumManifest;

/// Generate full manifest from directory structure.
///
/// Workflow:
/// 1. Check if manifest.yaml already exists → warn user
/// 2. Scan `source/` for path directories
/// 3. For each path, scan for courses and lessons
/// 4. Build the nested structure
/// 5. Write to output file
pub fn generate_full_manifest(source: &Path, output: &Path) -> Result<()> {
    // 1. Check if output exists
    if output.exists() {
        eprintln!("⚠️  WARNING: '{}' already exists!", output.display());
        eprint!("Delete it before proceeding to avoid overwriting. Continue anyway? (y/N): ");
        io::stdout().flush()?;

        let mut response = String::new();
        io::stdin().read_line(&mut response)?;

        if !response.trim().eq_ignore_ascii_case("y") {
            bail!("Aborted by user.");
        }
    }

    // 2. Read the main manifest to get title/description
    let main_manifest_path = source.join("manifest.yaml");
    let (title, description) = if main_manifest_path.exists() {
        let content = fs::read_to_string(&main_manifest_path)?;
        let data: serde_yaml::Value = serde_yaml::from_str(&content)?;

        let title = data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("The Odin Project")
            .to_string();

        let description = data
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("A free, open-source coding curriculum.")
            .to_string();

        (title, description)
    } else {
        (
            "The Odin Project".to_string(),
            "A free, open-source coding curriculum.".to_string(),
        )
    };

    // 3. Scan source directory for path directories
    println!("🔍 Scanning directory structure...");
    let mut nav = Vec::new();

    for entry in fs::read_dir(source)
        .with_context(|| format!("Failed to read {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        // Skip files and hidden dirs
        if !path.is_dir() || path.file_name().unwrap().to_string_lossy().starts_with('.') {
            continue;
        }

        let path_slug = path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        // Skip special directories
        if ["scripts", "target", "themes.yml", "templates"].contains(&path_slug.as_str()) {
            continue;
        }

        // Try to extract title from existing manifest (if it exists)
        let path_manifest = path.join("manifest.yaml");
        let path_title = if path_manifest.exists() {
            let content = fs::read_to_string(&path_manifest)?;
            let data: serde_yaml::Value = serde_yaml::from_str(&content)?;
            data.get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
        .unwrap_or_else(|| {
            // Fallback: title-case the slug
            path_slug
                .split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        });

        let path_description = if path_manifest.exists() {
            let content = fs::read_to_string(&path_manifest)?;
            let data: serde_yaml::Value = serde_yaml::from_str(&content)?;
            data.get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        }
        .unwrap_or_default();

        // 4. Scan for courses
        let courses = scan_courses(&path, &path_slug)?;

        if !courses.is_empty() {
            let course_count = courses.len();
            let mut path_map = BTreeMap::new();
            path_map.insert(
                path_slug.clone(),
                crate::manifest::NestedPathData {
                    title: path_title,
                    description: path_description,
                    courses,
                },
            );
            nav.push(path_map);
            println!("  ✓ {}: {} course(s)", path_slug, course_count);
        }
    }

    // 5. Build and write manifest
    let manifest = NestedCurriculumManifest {
        title,
        description,
        nav,
        custom_colors: None,
        theme_preset: None,
    };

    let yaml = serde_yaml::to_string(&manifest)
        .context("Failed to serialize manifest to YAML")?;

    fs::write(output, yaml).with_context(|| format!("Failed to write {}", output.display()))?;

    println!(
        "📝 Manifest generated with {} path(s)",
        manifest.nav.len()
    );
    Ok(())
}

/// Scan a path directory for courses (subdirectories containing lesson files).
fn scan_courses(
    path_dir: &Path,
    _path_slug: &str,
) -> Result<Vec<BTreeMap<String, crate::manifest::NestedCourseData>>> {
    let mut courses = Vec::new();

    for entry in fs::read_dir(path_dir)? {
        let entry = entry?;
        let course_path = entry.path();

        // Skip files and hidden dirs
        if !course_path.is_dir()
            || course_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with('.')
        {
            continue;
        }

        let course_slug = course_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();

        // Skip manifest.yaml files
        if course_slug == "manifest.yaml" {
            continue;
        }

        // Check if it's a lesson file (ends with .md)
        if course_path.is_file() && course_path.to_string_lossy().ends_with(".md") {
            continue;
        }

        // Extract course title (title-case the slug)
        let course_title = course_slug
            .split('_')
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        // Try to read existing manifest for course
        let course_manifest = course_path.join("manifest.yaml");
        let (course_title, course_description, sections) = if course_manifest.exists() {
            // Use existing manifest structure if available
            let content = fs::read_to_string(&course_manifest)?;
            let data: serde_yaml::Value = serde_yaml::from_str(&content)?;

            let title = data
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&course_title)
                .to_string();

            let desc = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Parse sections from old manifest
            let sections = data
                .get("sections")
                .and_then(|v| v.as_sequence())
                .map(|sections| {
                    sections
                        .iter()
                        .filter_map(|section| {
                            let section_title = section
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Lessons")
                                .to_string();

                            let lessons = section
                                .get("lessons")
                                .and_then(|v| v.as_sequence())?
                                .iter()
                                .filter_map(|lesson| {
                                    let file = lesson
                                        .get("file")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let title = lesson
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let is_project = lesson
                                        .get("is_project")
                                        .and_then(|v| v.as_bool())
                                        .unwrap_or(false);

                                    if title.is_empty() || file.is_empty() {
                                        return None;
                                    }

                                    let mut lessons_map = BTreeMap::new();
                                    lessons_map.insert(
                                        title,
                                        if is_project {
                                            crate::manifest::NestedLessonValue::WithMeta {
                                                file,
                                                is_project: true,
                                            }
                                        } else {
                                            crate::manifest::NestedLessonValue::File(file)
                                        },
                                    );
                                    Some((section_title.clone(), lessons_map))
                                })
                                .collect::<Vec<_>>();

                            if lessons.is_empty() {
                                return None;
                            }

                            // Group by section title
                            let mut section_lessons = BTreeMap::new();
                            for (_, lessons_map) in lessons {
                                section_lessons.extend(lessons_map);
                            }

                            Some(crate::manifest::NestedSectionData {
                                title: section_title,
                                description: String::new(),
                                lessons: section_lessons,
                            })
                        })
                        .collect()
                })
                .unwrap_or_else(Vec::new);

            (title, desc, sections)
        } else {
            // Auto-scan for lessons
            let sections = scan_sections(&course_path)?;
            (course_title, String::new(), sections)
        };

        if !sections.is_empty() {
            let mut course_map = BTreeMap::new();
            course_map.insert(
                course_slug.clone(),
                crate::manifest::NestedCourseData {
                    title: course_title,
                    description: course_description,
                    sections,
                },
            );
            courses.push(course_map);
        }
    }

    Ok(courses)
}

/// Scan a course directory for sections (subdirectories or direct lesson files).
fn scan_sections(course_dir: &Path) -> Result<Vec<crate::manifest::NestedSectionData>> {
    let mut sections = Vec::new();

    // Check if there are subdirectories (sections) or just lesson files
    let mut has_subdirs = false;
    let mut lessons_flat = BTreeMap::new();

    for entry in fs::read_dir(course_dir)? {
        let entry = entry?;
        let item_path = entry.path();
        let item_name = item_path.file_name().unwrap().to_string_lossy().to_string();

        // Skip hidden items and manifest files
        if item_name.starts_with('.') || item_name == "manifest.yaml" {
            continue;
        }

        if item_path.is_dir() {
            has_subdirs = true;
            // Potential section directory
            let section_title = item_name
                .split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            let lessons = scan_lessons_in_dir(&item_path)?;
            if !lessons.is_empty() {
                sections.push(crate::manifest::NestedSectionData {
                    title: section_title,
                    description: String::new(),
                    lessons,
                });
            }
        } else if item_path.to_string_lossy().ends_with(".md") {
            // Lesson file at course root
            let lesson_title = item_name
                .trim_end_matches(".md")
                .replace('_', " ")
                .split_whitespace()
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            lessons_flat.insert(
                lesson_title,
                crate::manifest::NestedLessonValue::File(item_name.clone()),
            );
        }
    }

    // If we found lesson files at root and no subdirs, create "Lessons" section
    if !has_subdirs && !lessons_flat.is_empty() {
        sections.push(crate::manifest::NestedSectionData {
            title: "Lessons".to_string(),
            description: String::new(),
            lessons: lessons_flat,
        });
    }

    Ok(sections)
}

/// Scan a section directory for lesson files.
fn scan_lessons_in_dir(section_dir: &Path) -> Result<BTreeMap<String, crate::manifest::NestedLessonValue>> {
    let mut lessons = BTreeMap::new();

    let mut files: Vec<_> = fs::read_dir(section_dir)?
        .filter_map(Result::ok)
        .filter(|e| {
            let path = e.path();
            path.is_file() && path.to_string_lossy().ends_with(".md")
        })
        .collect();

    // Sort by filename (respects numeric prefixes like 01_, 02_)
    files.sort_by(|a, b| {
        let a_name = a.file_name();
        let b_name = b.file_name();
        a_name.cmp(&b_name)
    });

    for entry in files {
        let file_path = entry.path();
        let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();

        // Extract title from filename
        let lesson_title: String = file_name
            .trim_end_matches(".md")
            .chars()
            .skip_while(|c| c.is_ascii_digit() || *c == '_')
            .collect::<String>()
            .replace('_', " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if !lesson_title.is_empty() {
            lessons.insert(
                lesson_title,
                crate::manifest::NestedLessonValue::File(file_name),
            );
        }
    }

    Ok(lessons)
}
