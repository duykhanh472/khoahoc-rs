/// Markdown → HTML renderer using pulldown-cmark.
///
/// Handles:
/// - GitHub-Flavoured Markdown (tables, strikethrough, task lists)
/// - Passthrough of TOP-specific HTML blocks (`<div class="lesson-note">`, etc.)
/// - Syntax-highlighted fenced code blocks (language class added for highlight.js)
/// - `<pre class="mermaid">` passthrough for diagrams
use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

/// Render a Markdown string to an HTML string.
pub fn render(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);
    options.insert(Options::ENABLE_GFM);

    let parser = Parser::new_ext(markdown, options);

    // Post-process events to add language classes to <code> blocks
    let events: Vec<Event> = parser
        .into_iter()
        .collect::<Vec<_>>();
    let events = add_code_language_classes(events);

    let mut html_output = String::new();
    html::push_html(&mut html_output, events.into_iter());
    html_output
}

/// Wrap fenced code blocks with a language-aware class so highlight.js can
/// pick them up: `<code class="language-rust">`.
fn add_code_language_classes(events: Vec<Event>) -> Vec<Event> {
    let mut out = Vec::with_capacity(events.len());
    let mut current_lang: Option<String> = None;

    for event in events {
        match &event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(lang))) => {
                let lang_str = lang.to_string();
                if !lang_str.is_empty() {
                    current_lang = Some(lang_str.clone());
                    // Emit a custom open tag with class
                    out.push(Event::Html(
                        format!(
                            "<pre><code class=\"language-{}\">",
                            html_escape(&lang_str)
                        )
                        .into(),
                    ));
                } else {
                    current_lang = None;
                    out.push(Event::Html("<pre><code>".into()));
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                out.push(Event::Html("</code></pre>".into()));
                current_lang = None;
            }
            // Skip the default open/close if we already pushed custom HTML
            Event::Start(Tag::CodeBlock(_)) => {
                if current_lang.is_none() {
                    out.push(Event::Html("<pre><code>".into()));
                }
            }
            _ => out.push(event),
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_paragraph() {
        let html = render("Hello **world**.");
        assert!(html.contains("<strong>world</strong>"));
    }

    #[test]
    fn test_fenced_code_block_lang_class() {
        let md = "```rust\nlet x = 1;\n```";
        let html = render(md);
        assert!(html.contains("language-rust"));
        assert!(html.contains("let x = 1;"));
    }

    #[test]
    fn test_tables() {
        let md = "| A | B |\n|---|---|\n| 1 | 2 |";
        let html = render(md);
        assert!(html.contains("<table>"));
    }

    #[test]
    fn test_html_passthrough() {
        let md = "<div class=\"lesson-note\" markdown=\"1\">\n\n#### Note\n\nBody.\n\n</div>";
        let html = render(md);
        assert!(html.contains("lesson-note"));
    }
}
