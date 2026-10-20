use iced::{
    Element, Font, Length, Point, Rectangle, Renderer, Theme, alignment,
    widget::{
        canvas::{self, Canvas, Frame, Geometry, Text},
        text_editor::Content,
    },
};

pub fn gutter<'a, Message: 'a>(content: &'a Content, font_size: f32) -> Element<'a, Message> {
    // Keep a little breathing room around the numbers, including in large files
    // and when the editor font size changes.
    let digits = content.line_count().max(1).ilog10() + 1;
    let width = (digits.max(3) as f32 * font_size * 0.62 + 26.0).ceil();
    Canvas::new(Gutter { content, font_size })
        .width(Length::Fixed(width))
        .height(Length::Fill)
        .into()
}
struct Gutter<'a> {
    content: &'a Content,
    font_size: f32,
}
impl<Message> canvas::Program<Message> for Gutter<'_> {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let active = self.content.cursor().position.line;
        let palette = theme.palette();
        let muted = iced::Color {
            r: palette.background.r + (palette.text.r - palette.background.r) * 0.43,
            g: palette.background.g + (palette.text.g - palette.background.g) * 0.43,
            b: palette.background.b + (palette.text.b - palette.background.b) * 0.43,
            a: 1.0,
        };
        self.content.with_editor(|editor| {
            let mut previous_line = None;
            for run in editor.buffer().layout_runs() {
                // A wrapped continuation belongs to the same source line.
                if previous_line == Some(run.line_i) {
                    continue;
                }
                previous_line = Some(run.line_i);
                frame.fill_text(Text {
                    content: (run.line_i + 1).to_string(),
                    position: Point::new(bounds.width - 16.0, run.line_top + 8.0),
                    color: if run.line_i == active {
                        palette.text
                    } else {
                        muted
                    },
                    font: Font::MONOSPACE,
                    size: self.font_size.into(),
                    line_height: iced::widget::text::LineHeight::Relative(1.55),
                    align_x: alignment::Horizontal::Right.into(),
                    align_y: alignment::Vertical::Top,
                    ..Default::default()
                });
            }
        });
        vec![frame.into_geometry()]
    }
}
