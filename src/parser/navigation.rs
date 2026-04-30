/// Navigation resolver.
///
/// After the full Curriculum tree is built, this module stitches together
/// the `prev` / `next` links and `breadcrumbs` for every lesson.
///
/// Strategy:
///   Build a single flat `Vec<LessonPtr>` ordered by:
///     path.position → course.position → section.position → lesson.position
///   Then walk adjacent pairs and write back into the mutable tree.
use crate::models::{LessonRef, Path};

/// Resolve all navigation links in-place across the entire curriculum.
/// Must be called *after* the full tree has been built.
pub fn resolve_all(paths: &mut Vec<Path>) {
    // 1. Collect a flat list of (path_idx, course_idx, section_idx, lesson_idx)
    //    in display order.
    let mut refs: Vec<(usize, usize, usize, usize)> = Vec::new();

    for (pi, path) in paths.iter().enumerate() {
        for (ci, course) in path.courses.iter().enumerate() {
            for (si, section) in course.sections.iter().enumerate() {
                for (li, _lesson) in section.lessons.iter().enumerate() {
                    refs.push((pi, ci, si, li));
                }
            }
        }
    }

    // 2. For each lesson, look up the previous and next entries and patch.
    for idx in 0..refs.len() {
        let prev_ref = if idx > 0 {
            let (pi, ci, si, li) = refs[idx - 1];
            let l = &paths[pi].courses[ci].sections[si].lessons[li];
            Some(LessonRef {
                title: l.display_title.clone(),
                url: l.url.clone(),
            })
        } else {
            None
        };

        let next_ref = if idx + 1 < refs.len() {
            let (pi, ci, si, li) = refs[idx + 1];
            let l = &paths[pi].courses[ci].sections[si].lessons[li];
            Some(LessonRef {
                title: l.display_title.clone(),
                url: l.url.clone(),
            })
        } else {
            None
        };

        let (pi, ci, si, li) = refs[idx];
        let lesson = &mut paths[pi].courses[ci].sections[si].lessons[li];
        lesson.prev = prev_ref;
        lesson.next = next_ref;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::*;
    use std::path::PathBuf;

    fn make_lesson(title: &str, url: &str) -> Lesson {
        Lesson {
            slug: slug::slugify(title),
            title: title.to_string(),
            description: String::new(),
            position: 1,
            is_project: false,
            source_path: PathBuf::new(),
            output_path: format!("{}/index.html", url.trim_matches('/')),
            url: url.to_string(),
            html_content: None,
            prev: None,
            next: None,
            breadcrumbs: vec![],
        }
    }

    #[test]
    fn test_prev_next_resolved() {
        let mut paths = vec![Path {
            slug: "foundations".to_string(),
            title: "Foundations".to_string(),
            description: String::new(),
            position: 1,
            url: "/foundations/".to_string(),
            courses: vec![Course {
                slug: "intro".to_string(),
                title: "Introduction".to_string(),
                description: String::new(),
                position: 1,
                url: "/foundations/introduction/".to_string(),
                badge_uri: None,
                sections: vec![Section {
                    title: "Start".to_string(),
                    description: String::new(),
                    position: 1,
                    lessons: vec![
                        make_lesson("Lesson A", "/foundations/introduction/lesson-a/"),
                        make_lesson("Lesson B", "/foundations/introduction/lesson-b/"),
                        make_lesson("Lesson C", "/foundations/introduction/lesson-c/"),
                    ],
                }],
            }],
        }];

        resolve_all(&mut paths);

        let lessons = &paths[0].courses[0].sections[0].lessons;
        assert!(lessons[0].prev.is_none());
        assert_eq!(lessons[0].next.as_ref().unwrap().url, "/foundations/introduction/lesson-b/");
        assert_eq!(lessons[1].prev.as_ref().unwrap().url, "/foundations/introduction/lesson-a/");
        assert_eq!(lessons[1].next.as_ref().unwrap().url, "/foundations/introduction/lesson-c/");
        assert!(lessons[2].next.is_none());
    }
}
