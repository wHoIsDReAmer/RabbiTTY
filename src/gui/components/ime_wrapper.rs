use iced::advanced::input_method::{InputMethod, Preedit, Purpose};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::Widget;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Shell};
use iced::mouse;
use iced::{Element, Event, Length, Pixels, Rectangle, Size, Vector};

use std::ops::Range;

use crate::gui::pane::PaneNode;

/// Grid cell that the IME composition window should anchor under.
#[derive(Debug, Clone)]
pub struct CursorCell {
    pub col: usize,
    pub row: usize,
    pub cell_width: f32,
    pub cell_height: f32,
    pub padding: [f32; 2],
    pub layout: PaneNode,
    pub focused: u64,
}

/// A wrapper widget that enables IME input for its child.
pub struct ImeEnabled<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    preedit: Option<(String, Option<Range<usize>>)>,
    cursor_cell: Option<CursorCell>,
    text_size: f32,
}

impl<'a, Message, Theme, Renderer> ImeEnabled<'a, Message, Theme, Renderer> {
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            preedit: None,
            cursor_cell: None,
            text_size: 14.0,
        }
    }

    pub fn preedit(mut self, preedit: Option<(String, Option<Range<usize>>)>) -> Self {
        self.preedit = preedit;
        self
    }

    pub fn cursor_cell(mut self, cell: Option<CursorCell>) -> Self {
        self.cursor_cell = cell;
        self
    }

    pub fn text_size(mut self, size: f32) -> Self {
        self.text_size = size;
        self
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ImeEnabled<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        // Enable IME on every redraw
        if matches!(
            event,
            Event::Window(iced::window::Event::RedrawRequested(_))
        ) {
            let bounds = layout.bounds();
            let text_size = self.text_size;
            let preedit = self.preedit.as_ref().map(|(text, selection)| Preedit {
                content: text.as_str(),
                selection: selection.clone(),
                text_size: Some(Pixels(text_size)),
            });
            let cursor_rect = self
                .cursor_cell
                .as_ref()
                .map(|cell| cursor_pixel_rect(cell, bounds))
                .unwrap_or_else(|| {
                    Rectangle::new(
                        iced::Point::new(bounds.x, bounds.y + bounds.height),
                        Size::ZERO,
                    )
                });
            shell.request_input_method(&InputMethod::Enabled {
                cursor: cursor_rect,
                purpose: Purpose::Terminal,
                preedit,
            });
        }

        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

fn cursor_pixel_rect(cell: &CursorCell, bounds: Rectangle) -> Rectangle {
    // Anchor to the focused pane's region using the renderer's cell metrics.
    let region = cell
        .layout
        .regions(bounds)
        .into_iter()
        .find(|(id, _)| *id == cell.focused)
        .map(|(_, rect)| rect)
        .unwrap_or(bounds);
    let cell_w = cell.cell_width.max(1.0);
    let cell_h = cell.cell_height.max(1.0);
    let x = region.x + cell.padding[0] + cell.col as f32 * cell_w;
    let y = region.y + cell.padding[1] + cell.row as f32 * cell_h;
    Rectangle::new(iced::Point::new(x, y), Size::new(cell_w, cell_h))
}

impl<'a, Message, Theme, Renderer> From<ImeEnabled<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(wrapper: ImeEnabled<'a, Message, Theme, Renderer>) -> Self {
        Element::new(wrapper)
    }
}
