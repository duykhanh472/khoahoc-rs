# `khoahoc-rs`

A high-performance, lightweight, and generic Static Site Generator (SSG) built in Rust, specifically designed to migrate The Odin Project's curriculum from a Ruby on Rails application to a zero-database static website.

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Features](#features)
- [Project Structure](#project-structure)
- [License](#license)

## Installation

### 1. Clone the repository

```bash
git clone https://github.com/duykhanh472/khoahoc-rs.git
cd khoahoc-rs
```

### 2. Build the project

```bash
cargo build --release
```

The binary will be available at `./target/release/odin-ssg`.

## Quick Start

### 1. Generate Manifests

Before building, you must generate the `manifest.yaml` files that describe your curriculum structure.

```bash
# Using Ruby (Recommended for TOP fixtures)
ruby scripts/generate_manifests.rb
```

### 2. Build the Static Site

Generate the full curriculum into the `out/` directory:

```bash
cargo run -- build --source curriculum-main --out ./out --templates templates
```

### 3. Run the Dev Server

Launch the local server with auto-rebuild:

```bash
cargo run -- serve --source curriculum-main --out out --port 8080
```

Visit `http://localhost:8080` in your browser.

## Features

- Blazing Fast: Parallelized lesson rendering using [Rayon](https://github.com/rayon-rs/rayon), allowing thousands of pages to be generated in seconds.
- Zero Database: No PostgreSQL or Rails overhead. The curriculum structure is defined via simple `manifest.yaml` files.
- Modern Design: Premium, dark-mode first vanilla CSS design system with a responsive 3-column layout.
- Smart Navigation: Automatic resolution of breadcrumbs and Prev/Next lesson logic.
- Client-Side Search: Instant search functionality powered by a pre-computed JSON index.
- Dev Server: Built-in HTTP server with file-watching for automatic rebuilds on changes.
- Syntax Highlighting: Integrated [Highlight.js](https://highlightjs.org/) for clean, readable code snippets.

## Tech Stack

- Core: [Rust](https://www.rust-lang.org/)
- CLI: [Clap v4](https://github.com/clap-rs/clap)
- Templating: [Tera](https://keats.github.io/tera/) (Jinja2-like engine)
- Markdown: [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark)
- Dev Server: `tiny_http` + `notify`
- Frontend: Vanilla HTML/CSS/JS (Zero frameworks)

## Project Structure

```text
odin-ssg/
├── src/
│   ├── main.rs          # CLI Entry point
│   ├── models.rs        # Core data structures (Curriculum, Path, Lesson)
│   ├── parser/          # Markdown parsing & hierarchy resolution
│   ├── renderer.rs      # Tera template integration
│   └── server.rs        # Dev server & file watcher
├── templates/
│   ├── static/          # CSS, JS, and font assets
│   ├── base.html        # Shared layout
│   ├── index.html       # Home page (Paths list)
│   └── lesson.html      # Lesson page (Content + Sidebar)
└── scripts/
   ├── generate_manifests.rb  # Migration script (Ruby)
   └── generate_manifests.py  # Migration script (Python)
```

## License

This project is licensed under the MIT License. Curriculum content is licensed under [CC BY-NC-SA 4.0](https://creativecommons.org/licenses/by-nc-sa/4.0/).
