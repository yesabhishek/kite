//! Cached native Markdown and Mermaid previews. No browser or script runtime.
use crate::appearance::Appearance;
use iced::{
    Border, Color, Element, Font, Length,
    widget::{column, container, markdown, rich_text, scrollable, svg, text},
};
use std::{collections::HashMap, path::Path};

const MAX_DOCUMENT_BYTES: usize = 512 * 1024;
const MAX_DIAGRAM_BYTES: usize = 64 * 1024;
const MAX_DIAGRAMS: usize = 24;
const MAX_SVG_BYTES: usize = 2 * 1024 * 1024;

/// Actions are routed to the app; rendering never opens a URL itself.
#[derive(Debug, Clone)]
pub enum PreviewEvent {
    Link(String),
}

/// Worker result contains only owned, thread-safe data.
#[derive(Debug, Clone)]
pub struct PreparedPreview {
    content: PreparedContent,
    diagrams: HashMap<String, Result<String, String>>,
}

#[derive(Debug, Clone)]
enum PreparedContent {
    Markdown(String),
    Mermaid(Result<String, String>),
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Preview {
    content: Content,
    diagrams: HashMap<String, Diagram>,
}

#[derive(Debug, Clone)]
enum Content {
    Markdown(Vec<markdown::Item>),
    Mermaid(Diagram),
    Error(String),
}

#[derive(Debug, Clone)]
enum Diagram {
    Ready(svg::Handle),
    Error(String),
}

pub fn is_supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(is_supported_extension)
}

fn is_supported_extension(extension: &str) -> bool {
    matches!(
        extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "md" | "markdown" | "mdown" | "mmd" | "mermaid"
    )
}

/// Parse and lay out diagrams on a blocking worker, before the view is built.
/// Oversized and malformed inputs become readable preview states.
pub fn prepare(source: &str, extension: &str, appearance: Appearance) -> PreparedPreview {
    let mut prepared = PreparedPreview {
        content: PreparedContent::Error("This file does not have a native preview.".into()),
        diagrams: HashMap::new(),
    };
    if source.len() > MAX_DOCUMENT_BYTES {
        prepared.content = PreparedContent::Error(
            "Preview is available for files up to 512 KiB. You can continue editing the source."
                .into(),
        );
        return prepared;
    }
    match extension
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "mmd" | "mermaid" => {
            prepared.content = PreparedContent::Mermaid(render_diagram(source, appearance));
        }
        "md" | "markdown" | "mdown" => {
            // Use the same parser as the native viewer, so quoted, indented, and
            // list-nested fences are handled consistently with the document.
            let items: Vec<_> = markdown::parse(source).collect();
            collect_diagrams(&items, appearance, &mut prepared.diagrams);
            prepared.content = PreparedContent::Markdown(source.to_owned());
        }
        _ => {}
    }
    prepared
}

fn collect_diagrams(
    items: &[markdown::Item],
    appearance: Appearance,
    diagrams: &mut HashMap<String, Result<String, String>>,
) {
    for item in items {
        match item {
            markdown::Item::CodeBlock { language, code, .. }
                if language.as_deref().is_some_and(is_mermaid_language) =>
            {
                if !diagrams.contains_key(code) {
                    let diagram = if diagrams.len() < MAX_DIAGRAMS {
                        render_diagram(code, appearance)
                    } else {
                        Err("A preview can render up to 24 distinct Mermaid diagrams.".into())
                    };
                    diagrams.insert(code.clone(), diagram);
                }
            }
            markdown::Item::Quote(items) => collect_diagrams(items, appearance, diagrams),
            markdown::Item::List { bullets, .. } => {
                for bullet in bullets {
                    let (markdown::Bullet::Point { items } | markdown::Bullet::Task { items, .. }) =
                        bullet;
                    collect_diagrams(items, appearance, diagrams);
                }
            }
            _ => {}
        }
    }
}

fn is_mermaid_language(language: &str) -> bool {
    language
        .split_whitespace()
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("mermaid"))
}

fn render_diagram(source: &str, appearance: Appearance) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("Write a Mermaid diagram to see its preview.".into());
    }
    if source.len() > MAX_DIAGRAM_BYTES {
        return Err("This diagram exceeds the 64 KiB preview limit.".into());
    }
    // Keep exceptionally large layouts from monopolizing the preview worker.
    if source.lines().count() > 1000 || source.matches(';').count() > 1000 {
        return Err("This diagram has too many statements for a live preview.".into());
    }
    let options = mermaid_rs_renderer::RenderOptions {
        theme: diagram_theme(appearance),
        ..Default::default()
    };
    let rendered =
        std::panic::catch_unwind(|| mermaid_rs_renderer::render_with_options(source, options))
            .map_err(|_| "The diagram could not be rendered. Check its Mermaid syntax.".to_owned())?
            .map_err(|error| error.to_string().chars().take(1200).collect::<String>())?;
    if rendered.len() > MAX_SVG_BYTES {
        return Err("The rendered diagram is too large for a live preview.".into());
    }
    // Generated diagrams are self-contained vectors. Refuse image elements,
    // including filesystem references that an SVG image resolver could load.
    if rendered.to_ascii_lowercase().contains("<image") {
        return Err("Image-based Mermaid nodes are not supported in the native preview.".into());
    }
    Ok(rendered)
}

fn diagram_theme(appearance: Appearance) -> mermaid_rs_renderer::Theme {
    let c = appearance.colors();
    let mut theme = mermaid_rs_renderer::Theme::modern();
    theme.font_family = "Helvetica Neue, Segoe UI, sans-serif".into();
    theme.font_size = 14.;
    theme.background = hex(c.bg);
    theme.primary_color = hex(c.panel);
    theme.primary_text_color = hex(c.ink);
    theme.primary_border_color = hex(c.subtle);
    theme.text_color = hex(c.ink);
    theme.line_color = hex(c.muted);
    theme.secondary_color = hex(c.raised);
    theme.tertiary_color = hex(c.hover);
    theme.edge_label_background = hex(c.bg);
    theme.cluster_background = hex(c.panel);
    theme.cluster_border = hex(c.edge);
    theme.sequence_actor_fill = hex(c.panel);
    theme.sequence_actor_border = hex(c.subtle);
    theme.sequence_actor_line = hex(c.subtle);
    theme.sequence_note_fill = hex(c.raised);
    theme.sequence_note_border = hex(c.subtle);
    theme.sequence_activation_fill = hex(c.selection);
    theme.sequence_activation_border = hex(c.muted);
    theme.git_colors = [
        c.accent, c.success, c.muted, c.danger, c.subtle, c.accent, c.success, c.muted,
    ]
    .map(hex);
    theme.git_inv_colors = std::array::from_fn(|_| hex(c.panel));
    theme.git_branch_label_colors = std::array::from_fn(|_| hex(c.bg));
    theme.git_commit_label_color = hex(c.ink);
    theme.git_commit_label_background = hex(c.panel);
    theme.git_tag_label_color = hex(c.ink);
    theme.git_tag_label_background = hex(c.raised);
    theme.git_tag_label_border = hex(c.subtle);
    theme.pie_title_text_color = hex(c.ink);
    // The default pie fills are light in every theme; keep slice labels dark.
    theme.pie_section_text_color = "#1b1d21".into();
    theme.pie_legend_text_color = hex(c.ink);
    theme.pie_stroke_color = hex(c.edge);
    theme.pie_outer_stroke_color = hex(c.edge);
    theme
}

fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02x}{g:02x}{b:02x}")
}

impl Preview {
    /// Materialize cached native widgets once, when accepting a worker result.
    pub fn new(prepared: PreparedPreview) -> Self {
        let content = match prepared.content {
            PreparedContent::Markdown(source) => {
                Content::Markdown(markdown::parse(&source).collect())
            }
            PreparedContent::Mermaid(diagram) => Content::Mermaid(Diagram::new(diagram)),
            PreparedContent::Error(error) => Content::Error(error),
        };
        Self {
            content,
            diagrams: prepared
                .diagrams
                .into_iter()
                .map(|(code, result)| (code, Diagram::new(result)))
                .collect(),
        }
    }

    /// Return document content; the app controls the surrounding scroll view.
    pub fn view(&self, appearance: Appearance) -> Element<'_, PreviewEvent> {
        let c = appearance.colors();
        let content = match &self.content {
            Content::Markdown(items) if items.is_empty() => text("This document is empty.")
                .size(14)
                .color(c.muted)
                .into(),
            Content::Markdown(items) => {
                let mut style = markdown::Style::from_palette(iced::theme::Palette {
                    background: c.bg,
                    text: c.ink,
                    primary: c.accent,
                    success: c.success,
                    warning: c.muted,
                    danger: c.danger,
                });
                style.inline_code_color = c.ink;
                style.inline_code_highlight = markdown::Highlight {
                    background: c.panel.into(),
                    border: Border::default().rounded(3),
                };
                let mut settings = markdown::Settings::with_text_size(15, style);
                settings.code_size = 13.into();
                settings.spacing = 15.into();
                markdown::view_with(
                    items,
                    settings,
                    &NativeViewer {
                        diagrams: &self.diagrams,
                        appearance,
                    },
                )
            }
            Content::Mermaid(diagram) => diagram.view(appearance),
            Content::Error(error) => error_view("Preview unavailable", error, appearance),
        };
        container(content)
            .width(Length::Fill)
            .style(move |_| container::Style {
                text_color: Some(c.ink),
                ..Default::default()
            })
            .into()
    }
}

impl Diagram {
    fn new(result: Result<String, String>) -> Self {
        match result {
            Ok(svg) => Self::Ready(svg::Handle::from_memory(svg.into_bytes())),
            Err(error) => Self::Error(error),
        }
    }

    fn view(&self, appearance: Appearance) -> Element<'_, PreviewEvent> {
        match self {
            Self::Ready(handle) => diagram_view(handle.clone(), appearance),
            Self::Error(error) => error_view("Mermaid preview", error, appearance),
        }
    }
}

struct NativeViewer<'a> {
    diagrams: &'a HashMap<String, Diagram>,
    appearance: Appearance,
}

impl<'a> markdown::Viewer<'a, PreviewEvent> for NativeViewer<'a> {
    fn on_link_click(url: markdown::Uri) -> PreviewEvent {
        PreviewEvent::Link(url)
    }

    fn image(
        &self,
        settings: markdown::Settings,
        _url: &'a markdown::Uri,
        _title: &'a str,
        alt: &markdown::Text,
    ) -> Element<'a, PreviewEvent> {
        let c = self.appearance.colors();
        container(
            column![
                text("IMAGE REFERENCE").size(10).color(c.subtle),
                rich_text(alt.spans(settings.style))
                    .size(settings.text_size)
                    .on_link_click(Self::on_link_click),
                text("Images are not loaded in the native preview.")
                    .size(12)
                    .color(c.muted),
            ]
            .spacing(7),
        )
        .padding(14)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(c.panel.into()),
            border: Border::default().rounded(5).width(1).color(c.edge),
            ..Default::default()
        })
        .into()
    }

    fn code_block(
        &self,
        settings: markdown::Settings,
        language: Option<&'a str>,
        code: &'a str,
        _lines: &'a [markdown::Text],
    ) -> Element<'a, PreviewEvent> {
        if language.is_some_and(is_mermaid_language) {
            let diagrams: &'a HashMap<String, Diagram> = self.diagrams;
            return match diagrams.get(code) {
                Some(diagram) => diagram.view(self.appearance),
                None => error_view(
                    "Mermaid preview",
                    "This Mermaid block could not be prepared.",
                    self.appearance,
                ),
            };
        }
        let c = self.appearance.colors();
        let code = scrollable(
            text(code.trim_end_matches('\n'))
                .font(Font::MONOSPACE)
                .size(settings.code_size)
                .color(c.ink)
                .wrapping(text::Wrapping::None),
        )
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::new().width(5).scroller_width(5),
        ));
        container(
            column![
                text(language.unwrap_or("Plain text"))
                    .size(11)
                    .color(c.muted),
                code,
            ]
            .spacing(10),
        )
        .padding(14)
        .width(Length::Fill)
        .style(move |_| container::Style {
            background: Some(c.panel.into()),
            border: Border::default().rounded(5).width(1).color(c.edge),
            ..Default::default()
        })
        .into()
    }
}

fn diagram_view<'a>(handle: svg::Handle, appearance: Appearance) -> Element<'a, PreviewEvent> {
    let c = appearance.colors();
    container(svg(handle).width(Length::Fill))
        .padding(16)
        .width(Length::Fill)
        .max_height(1000)
        .style(move |_| container::Style {
            background: Some(c.bg.into()),
            border: Border::default().rounded(6).width(1).color(c.edge),
            ..Default::default()
        })
        .into()
}

fn error_view<'a>(
    title: &'a str,
    error: &'a str,
    appearance: Appearance,
) -> Element<'a, PreviewEvent> {
    let c = appearance.colors();
    container(
        column![
            text(title).size(13).color(c.ink),
            text(error).size(13).color(c.muted),
        ]
        .spacing(8),
    )
    .padding(16)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(c.panel.into()),
        border: Border::default().rounded(5).width(1).color(c.edge),
        ..Default::default()
    })
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_result_is_send_and_sync() {
        fn assert_thread_safe<T: Send + Sync + Clone>() {}
        assert_thread_safe::<PreparedPreview>();
    }

    #[test]
    fn supported_file_extensions_are_case_insensitive() {
        for path in [
            "README.md",
            "notes.MARKDOWN",
            "notes.mdown",
            "graph.MMD",
            "graph.mermaid",
        ] {
            assert!(is_supported(Path::new(path)), "{path}");
        }
        assert!(!is_supported(Path::new("source.rs")));
        assert!(!is_supported(Path::new("README")));
    }

    #[test]
    fn markdown_renders_tables_tasks_links_and_nested_mermaid() {
        let source = "# Native preview\n\n[Documentation](https://example.com)\n\n- [x] Done\n- [ ] Pending\n\n| Name | Status |\n| --- | --- |\n| Preview | Ready |\n\n> ```mermaid\n> flowchart LR\n> A[Source] --> B[Preview]\n> ```\n\n```rust\nfn main() {}\n```\n";
        let prepared = prepare(source, "md", Appearance::Dark);
        assert_eq!(prepared.diagrams.len(), 1);
        assert!(prepared.diagrams.values().all(Result::is_ok));
        let preview = Preview::new(prepared);
        let Content::Markdown(items) = &preview.content else {
            panic!("expected Markdown");
        };
        assert!(
            items
                .iter()
                .any(|item| matches!(item, markdown::Item::Table { .. }))
        );
        assert!(items.iter().any(|item| matches!(item, markdown::Item::List { bullets, .. } if bullets.iter().any(|bullet| matches!(bullet, markdown::Bullet::Task { .. })))));
        let _ = preview.view(Appearance::Dark);
    }

    #[test]
    fn flowchart_and_sequence_diagrams_render_for_dark_and_light_themes() {
        use iced::advanced::svg::Renderer as _;

        let renderer = iced_tiny_skia::Renderer::new(Font::default(), 14.into());
        for appearance in [Appearance::Dark, Appearance::Porcelain] {
            for source in [
                "flowchart LR\nA[Editor] --> B{Preview?}\nB -->|Yes| C[Native SVG]",
                "sequenceDiagram\nparticipant User\nparticipant Editor\nUser->>Editor: Open Markdown\nEditor-->>User: Native preview",
            ] {
                let result = render_diagram(source, appearance).unwrap();
                assert!(result.contains("<svg"));
                assert!(result.contains(&hex(appearance.colors().bg)));
                assert!(!result.contains("<image"));
                assert!(!result.contains("<script"));
                assert!(!result.contains("<foreignObject"));
                let handle = svg::Handle::from_memory(result.into_bytes());
                let dimensions = renderer.measure_svg(&handle);
                assert!(dimensions.width > 0 && dimensions.height > 0);
                let prepared = prepare(source, ".mmd", appearance);
                let preview = Preview::new(prepared);
                assert!(matches!(
                    preview.content,
                    Content::Mermaid(Diagram::Ready(_))
                ));
            }
        }
    }

    #[test]
    fn malformed_and_oversized_inputs_return_readable_states() {
        assert!(render_diagram("this is not a Mermaid diagram", Appearance::Dark).is_err());
        assert!(
            render_diagram("flowchart TD\nsubgraph Unclosed\nA-->B", Appearance::Dark).is_err()
        );
        assert!(
            render_diagram(&"x".repeat(MAX_DIAGRAM_BYTES + 1), Appearance::Dark)
                .unwrap_err()
                .contains("64 KiB")
        );
        let prepared = prepare(&"x".repeat(MAX_DOCUMENT_BYTES + 1), "md", Appearance::Dark);
        assert!(
            matches!(prepared.content, PreparedContent::Error(error) if error.contains("512 KiB"))
        );
    }

    #[test]
    fn repeated_fences_share_a_cached_diagram_and_plain_code_is_preserved() {
        let source = "```mermaid\nflowchart LR\nA-->B\n```\n\n```mermaid\nflowchart LR\nA-->B\n```\n\n```text\nmermaid: this is ordinary code\n```";
        let prepared = prepare(source, "markdown", Appearance::Dark);
        assert_eq!(prepared.diagrams.len(), 1);
        let preview = Preview::new(prepared);
        let Content::Markdown(items) = &preview.content else {
            panic!("expected Markdown");
        };
        assert_eq!(
            items
                .iter()
                .filter(|item| matches!(item, markdown::Item::CodeBlock { .. }))
                .count(),
            3
        );
    }
}
