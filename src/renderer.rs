/// HTML renderer: wraps Tera templates and writes output files.
///
/// For performance, lesson pages are rendered in parallel via rayon.
use crate::models::{Curriculum, Course, Lesson, Path, SearchEntry};
use anyhow::{Context, Result};
use rayon::prelude::*;

use tera::{Context as TeraCtx, Tera};

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Render the entire curriculum to `out_dir`.
///
/// Output structure:
/// ```
/// out/
///   index.html
///   search-index.json
///   static/          (copied verbatim from templates_dir/static/)
///   <path-slug>/
///     index.html
///     <course-slug>/
///       index.html
///       <lesson-slug>/
///         index.html
/// ```
pub fn render_site(
    curriculum: &Curriculum,
    search_entries: &[SearchEntry],
    templates_dir: &std::path::Path,
    out_dir: &std::path::Path,
) -> Result<()> {
    // Load templates
    let glob = templates_dir.join("**/*.html");
    let glob_str = glob.to_string_lossy();
    let tera = Tera::new(&glob_str)
        .with_context(|| format!("Failed to load templates from {}", templates_dir.display()))?;

    // Ensure output dir exists
    std::fs::create_dir_all(out_dir)?;

    // Copy static assets
    let static_src = templates_dir.join("static");
    if static_src.exists() {
        let static_dst = out_dir.join("static");
        copy_dir_all(&static_src, &static_dst)?;

        // Inject theme CSS into style.css
        let style_css_path = static_dst.join("style.css");
        if style_css_path.exists() {
            let original_css = std::fs::read_to_string(&style_css_path)
                .with_context(|| format!("Failed to read {}", style_css_path.display()))?;
            let theme_css = curriculum.theme.to_css();
            let new_css = format!("{}\n\n/* --- Theme Variables --- */\n{}", original_css, theme_css);
            std::fs::write(&style_css_path, new_css)
                .with_context(|| format!("Failed to write {}", style_css_path.display()))?;
        }
    }

    // Write search index
    let search_json = serde_json::to_string_pretty(search_entries)?;
    std::fs::write(out_dir.join("search-index.json"), &search_json)?;

    // Root index
    render_root(curriculum, &tera, out_dir)?;

    // Path + course + lesson pages
    for path in &curriculum.paths {
        render_path(path, curriculum, &tera, out_dir)?;
        for course in &path.courses {
            render_course(course, path, curriculum, &tera, out_dir)?;
            render_lessons_parallel(course, path, curriculum, &tera, out_dir)?;
        }
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Page renderers
// ─────────────────────────────────────────────────────────────────────────────

fn render_root(curriculum: &Curriculum, tera: &Tera, out_dir: &std::path::Path) -> Result<()> {
    let mut ctx = TeraCtx::new();
    ctx.insert("curriculum", curriculum);
    ctx.insert("page_title", &curriculum.title);
    ctx.insert("site_root", "/");
    let html = tera.render("index.html", &ctx).context("Render index.html")?;
    write_page(out_dir, "index.html", &html)
}

fn render_path(
    path: &Path,
    curriculum: &Curriculum,
    tera: &Tera,
    out_dir: &std::path::Path,
) -> Result<()> {
    let mut ctx = TeraCtx::new();
    ctx.insert("curriculum", curriculum);
    ctx.insert("path", path);
    ctx.insert("page_title", &format!("{} — {}", path.title, curriculum.title));
    ctx.insert("site_root", "/");
    let html = tera.render("path.html", &ctx).context("Render path.html")?;
    write_page(out_dir, &format!("{}/index.html", path.slug), &html)
}

fn render_course(
    course: &Course,
    path: &Path,
    curriculum: &Curriculum,
    tera: &Tera,
    out_dir: &std::path::Path,
) -> Result<()> {
    let mut ctx = TeraCtx::new();
    ctx.insert("curriculum", curriculum);
    ctx.insert("path", path);
    ctx.insert("course", course);
    ctx.insert(
        "page_title",
        &format!("{} — {} — {}", course.title, path.title, curriculum.title),
    );
    ctx.insert("site_root", "/");
    let html = tera.render("course.html", &ctx).context("Render course.html")?;
    write_page(
        out_dir,
        &format!("{}/{}/index.html", path.slug, course.slug),
        &html,
    )
}

/// Render all lessons in a course in parallel.
fn render_lessons_parallel(
    course: &Course,
    path: &Path,
    curriculum: &Curriculum,
    tera: &Tera,
    out_dir: &std::path::Path,
) -> Result<()> {
    // Collect flat list to allow parallel iteration
    let lessons: Vec<&Lesson> = course.all_lessons();

    // Parallel render — each produces (output_path, html)
    let results: Vec<Result<(String, String)>> = lessons
        .par_iter()
        .map(|lesson| render_lesson(lesson, course, path, curriculum, tera))
        .collect();

    // Write sequentially (disk I/O, no contention benefit)
    for result in results {
        let (output_path, html) = result?;
        write_page(out_dir, &output_path, &html)?;
    }
    Ok(())
}

fn render_lesson(
    lesson: &Lesson,
    course: &Course,
    path: &Path,
    curriculum: &Curriculum,
    tera: &Tera,
) -> Result<(String, String)> {
    let mut ctx = TeraCtx::new();
    ctx.insert("curriculum", curriculum);
    ctx.insert("path", path);
    ctx.insert("course", course);
    ctx.insert("lesson", lesson);
    ctx.insert("page_title", &format!("{} — {}", lesson.display_title, curriculum.title));
    ctx.insert("site_root", "/");

    let html = tera.render("lesson.html", &ctx)
        .with_context(|| format!("Render lesson '{}'", lesson.slug))?;
    Ok((lesson.output_path.clone(), html))
}

// ─────────────────────────────────────────────────────────────────────────────
// I/O helpers
// ─────────────────────────────────────────────────────────────────────────────

fn write_page(out_dir: &std::path::Path, rel_path: &str, html: &str) -> Result<()> {
    let full_path = out_dir.join(rel_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full_path, html)
        .with_context(|| format!("Write {}", full_path.display()))?;
    Ok(())
}

/// Recursively copy a directory.
fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)?;
        }
    }
    Ok(())
}
