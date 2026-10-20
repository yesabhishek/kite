use iced::{
    Color, Element, Length, Point, Rectangle, Renderer, Theme,
    widget::canvas::{self, Canvas, Frame, Geometry, LineCap, LineJoin, Path, Stroke},
};

/// A quiet outline family, drawn on a consistent 16-point grid.
#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Files,
    Folder,
    Code,
    Document,
    Config,
    Panel,
    Search,
    Plus,
    Close,
    Refresh,
    ChevronRight,
    ChevronDown,
    Terminal,
    Settings,
    Sidebar,
    Command,
    Save,
    Check,
    Minus,
    Info,
}

pub fn icon<'a, Message: 'a>(kind: Icon) -> Element<'a, Message> {
    icon_sized(kind, 16.0)
}

pub fn icon_sized<'a, Message: 'a>(kind: Icon, size: f32) -> Element<'a, Message> {
    glyph(kind, size, None)
}

pub fn icon_tinted<'a, Message: 'a>(kind: Icon, color: Color) -> Element<'a, Message> {
    icon_sized_tinted(kind, 16.0, color)
}

pub fn icon_sized_tinted<'a, Message: 'a>(
    kind: Icon,
    size: f32,
    color: Color,
) -> Element<'a, Message> {
    glyph(kind, size, Some(color))
}

fn glyph<'a, Message: 'a>(kind: Icon, size: f32, color: Option<Color>) -> Element<'a, Message> {
    Canvas::new(Glyph { kind, color })
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .into()
}

struct Glyph {
    kind: Icon,
    color: Option<Color>,
}

impl<Message> canvas::Program<Message> for Glyph {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        draw(self.kind, self.color, renderer, theme, bounds)
    }
}

// Preserve support for Canvas::new(Icon::...) alongside the helper API.
impl<Message> canvas::Program<Message> for Icon {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        draw(*self, None, renderer, theme, bounds)
    }
}

fn draw(
    kind: Icon,
    color: Option<Color>,
    renderer: &Renderer,
    theme: &Theme,
    bounds: Rectangle,
) -> Vec<Geometry> {
    let mut frame = Frame::new(renderer, bounds.size());
    frame.scale(bounds.width.min(bounds.height) / 16.0);
    let color = color.unwrap_or(Color {
        a: 0.74,
        ..theme.palette().text
    });
    let stroke = Stroke::default()
        .with_width(1.25)
        .with_line_cap(LineCap::Round)
        .with_line_join(LineJoin::Round)
        .with_color(color);
    let segments: &[&[(f32, f32)]] = match kind {
        Icon::Folder => &[
            &[
                (1.75, 4.25),
                (6.25, 4.25),
                (7.75, 6.0),
                (14.25, 6.0),
                (14.25, 12.75),
                (1.75, 12.75),
                (1.75, 4.25),
            ],
            &[(7.75, 6.0), (1.75, 6.0)],
        ],
        Icon::Code => &[
            &[(5.0, 4.25), (1.75, 8.0), (5.0, 11.75)],
            &[(11.0, 4.25), (14.25, 8.0), (11.0, 11.75)],
            &[(9.25, 3.0), (6.75, 13.0)],
        ],
        Icon::Document => &[
            &[
                (3.25, 1.75),
                (9.25, 1.75),
                (12.75, 5.25),
                (12.75, 14.25),
                (3.25, 14.25),
                (3.25, 1.75),
            ],
            &[(9.25, 1.75), (9.25, 5.25), (12.75, 5.25)],
            &[(5.5, 8.5), (10.5, 8.5)],
            &[(5.5, 11.25), (9.0, 11.25)],
        ],
        Icon::Config => &[
            &[(4.0, 2.0), (4.0, 4.0)],
            &[(4.0, 7.5), (4.0, 14.0)],
            &[(8.0, 2.0), (8.0, 8.5)],
            &[(8.0, 12.0), (8.0, 14.0)],
            &[(12.0, 2.0), (12.0, 4.0)],
            &[(12.0, 7.5), (12.0, 14.0)],
        ],
        Icon::Files => &[
            &[
                (5.25, 4.5),
                (13.25, 4.5),
                (13.25, 14.25),
                (5.25, 14.25),
                (5.25, 4.5),
            ],
            &[(2.75, 11.5), (2.75, 1.75), (10.75, 1.75)],
        ],
        Icon::Panel | Icon::Sidebar => &[
            &[
                (1.75, 2.75),
                (14.25, 2.75),
                (14.25, 13.25),
                (1.75, 13.25),
                (1.75, 2.75),
            ],
            match kind {
                Icon::Sidebar => &[(5.75, 2.75), (5.75, 13.25)],
                _ => &[(1.75, 9.5), (14.25, 9.5)],
            },
        ],
        Icon::Plus => &[&[(8.0, 3.5), (8.0, 12.5)], &[(3.5, 8.0), (12.5, 8.0)]],
        Icon::Minus => &[&[(3.5, 8.0), (12.5, 8.0)]],
        Icon::Close => &[&[(4.5, 4.5), (11.5, 11.5)], &[(11.5, 4.5), (4.5, 11.5)]],
        Icon::Refresh => &[
            &[(12.0, 2.0), (12.0, 5.25), (8.75, 5.25)],
            &[(4.0, 14.0), (4.0, 10.75), (7.25, 10.75)],
        ],
        Icon::ChevronRight => &[&[(6.25, 4.5), (9.75, 8.0), (6.25, 11.5)]],
        Icon::ChevronDown => &[&[(4.5, 6.25), (8.0, 9.75), (11.5, 6.25)]],
        Icon::Terminal => &[
            &[(2.5, 4.5), (6.0, 8.0), (2.5, 11.5)],
            &[(8.0, 11.5), (13.5, 11.5)],
        ],
        Icon::Settings => &[&[
            (6.5, 1.75),
            (9.5, 1.75),
            (10.0, 3.5),
            (11.25, 4.25),
            (13.0, 3.9),
            (14.5, 6.5),
            (13.25, 7.75),
            (13.25, 9.0),
            (14.0, 10.5),
            (12.5, 13.0),
            (10.75, 12.5),
            (9.5, 13.25),
            (9.0, 14.25),
            (6.0, 14.25),
            (5.5, 12.75),
            (4.25, 12.0),
            (2.75, 12.5),
            (1.25, 10.0),
            (2.5, 8.75),
            (2.5, 7.25),
            (1.75, 5.75),
            (3.25, 3.25),
            (5.0, 3.75),
            (6.0, 3.0),
            (6.5, 1.75),
        ]],
        Icon::Command => &[
            &[(6.0, 4.0), (6.0, 12.0)],
            &[(10.0, 4.0), (10.0, 12.0)],
            &[(4.0, 6.0), (12.0, 6.0)],
            &[(4.0, 10.0), (12.0, 10.0)],
        ],
        Icon::Save => &[
            &[
                (2.25, 2.25),
                (11.25, 2.25),
                (13.75, 4.75),
                (13.75, 13.75),
                (2.25, 13.75),
                (2.25, 2.25),
            ],
            &[(5.0, 2.25), (5.0, 6.0), (10.5, 6.0), (10.5, 2.25)],
            &[(5.0, 13.75), (5.0, 9.0), (11.0, 9.0), (11.0, 13.75)],
        ],
        Icon::Check => &[&[(3.0, 8.25), (6.5, 11.75), (13.0, 4.25)]],
        Icon::Info => &[&[(8.0, 7.5), (8.0, 10.75)]],
        Icon::Search => &[],
    };
    for points in segments {
        frame.stroke(
            &Path::new(|path| {
                path.move_to(Point::new(points[0].0, points[0].1));
                for point in &points[1..] {
                    path.line_to(Point::new(point.0, point.1));
                }
            }),
            stroke,
        );
    }
    let circle = |frame: &mut Frame, x, y, radius| {
        frame.stroke(&Path::circle(Point::new(x, y), radius), stroke);
    };
    match kind {
        Icon::Search => {
            circle(&mut frame, 6.75, 6.75, 4.5);
            frame.stroke(
                &Path::line(Point::new(10.0, 10.0), Point::new(13.75, 13.75)),
                stroke,
            );
        }
        Icon::Settings => circle(&mut frame, 8.0, 8.0, 2.5),
        Icon::Config => {
            circle(&mut frame, 4.0, 5.75, 1.75);
            circle(&mut frame, 8.0, 10.25, 1.75);
            circle(&mut frame, 12.0, 5.75, 1.75);
        }
        Icon::Command => {
            for (x, y) in [(4.0, 4.0), (12.0, 4.0), (4.0, 12.0), (12.0, 12.0)] {
                circle(&mut frame, x, y, 2.0);
            }
        }
        Icon::Info => {
            circle(&mut frame, 8.0, 8.0, 6.0);
            frame.fill(&Path::circle(Point::new(8.0, 5.25), 0.65), color);
        }
        Icon::Refresh => frame.stroke(
            &Path::new(|path| {
                path.move_to(Point::new(12.0, 5.25));
                path.bezier_curve_to(
                    Point::new(8.5, -0.25),
                    Point::new(1.0, 2.25),
                    Point::new(2.0, 8.0),
                );
                path.move_to(Point::new(4.0, 10.75));
                path.bezier_curve_to(
                    Point::new(7.5, 16.25),
                    Point::new(15.0, 13.75),
                    Point::new(14.0, 8.0),
                );
            }),
            stroke,
        ),
        _ => {}
    }
    vec![frame.into_geometry()]
}

pub fn file_icon(path: &std::path::Path) -> Icon {
    match path.extension().and_then(|s| s.to_str()).unwrap_or("") {
        "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "go" | "html" | "css" | "scss" | "sass"
        | "less" | "swift" | "c" | "h" | "cpp" | "hpp" | "cc" | "java" | "kt" | "kts" | "rb"
        | "php" | "sh" | "bash" | "zsh" | "vue" | "svelte" | "sql" | "lua" | "ex" | "exs"
        | "dart" | "cs" | "fs" | "r" | "R" => Icon::Code,
        "toml" | "json" | "jsonc" | "yaml" | "yml" | "lock" | "ini" | "conf" | "xml" | "env"
        | "properties" => Icon::Config,
        _ if path.file_name().is_some_and(|name| {
            let name = name.to_string_lossy();
            name.starts_with('.') || matches!(name.as_ref(), "Dockerfile" | "Makefile")
        }) =>
        {
            Icon::Config
        }
        _ => Icon::Document,
    }
}
