use egui_term::TerminalView;

use crate::config::theme::AppTheme;
use crate::terminal::pane::{PaneId, PaneNode, SplitDir};

/// Render the pane tree recursively; returns the pane the user clicked
/// (to move focus), if any.
pub fn show_pane_tree(
    ui: &mut egui::Ui,
    node: &mut PaneNode,
    focused: PaneId,
    theme: &AppTheme,
    rect: egui::Rect,
) -> Option<PaneId> {
    let mut clicked = None;
    show_node(ui, node, focused, theme, rect, &mut clicked);
    clicked
}

fn show_node(
    ui: &mut egui::Ui,
    node: &mut PaneNode,
    focused: PaneId,
    theme: &AppTheme,
    rect: egui::Rect,
    clicked: &mut Option<PaneId>,
) {
    match node {
        PaneNode::Empty => {},
        PaneNode::Leaf(pane) => {
            let mut child = ui.new_child(
                egui::UiBuilder::new().max_rect(rect).layout(
                    egui::Layout::top_down(egui::Align::Min),
                ),
            );
            let view = TerminalView::new(&mut child, &mut pane.backend)
                .set_focus(pane.id == focused)
                .set_theme(theme.terminal_theme())
                .set_size(rect.size());
            let response = child.add(view);
            if response.clicked() && pane.id != focused {
                *clicked = Some(pane.id);
            }
            // Focused pane gets a subtle accent border.
            if pane.id == focused {
                ui.painter().rect_stroke(
                    rect,
                    0.0,
                    egui::Stroke::new(1.0, theme.accent.linear_multiply(0.6)),
                    egui::StrokeKind::Inside,
                );
            }
        },
        PaneNode::Split {
            dir,
            ratio,
            first,
            second,
        } => {
            const GAP: f32 = 4.0;
            let (first_rect, second_rect) = match dir {
                SplitDir::Horizontal => {
                    // Side-by-side.
                    let w = (rect.width() - GAP) * *ratio;
                    let first_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(w, rect.height()),
                    );
                    let second_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x + w + GAP, rect.min.y),
                        rect.max,
                    );
                    (first_rect, second_rect)
                },
                SplitDir::Vertical => {
                    // Stacked.
                    let h = (rect.height() - GAP) * *ratio;
                    let first_rect = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(rect.width(), h),
                    );
                    let second_rect = egui::Rect::from_min_max(
                        egui::pos2(rect.min.x, rect.min.y + h + GAP),
                        rect.max,
                    );
                    (first_rect, second_rect)
                },
            };

            // Divider drag handle.
            let divider_rect = match dir {
                SplitDir::Horizontal => egui::Rect::from_min_max(
                    egui::pos2(first_rect.max.x, rect.min.y),
                    egui::pos2(second_rect.min.x, rect.max.y),
                ),
                SplitDir::Vertical => egui::Rect::from_min_max(
                    egui::pos2(rect.min.x, first_rect.max.y),
                    egui::pos2(rect.max.x, second_rect.min.y),
                ),
            };
            let divider_id = ui.id().with(("divider", first.pane_ids()));
            let divider_response = ui.interact(
                divider_rect,
                divider_id,
                egui::Sense::click_and_drag(),
            );
            if divider_response.dragged() {
                let delta = divider_response.drag_delta();
                let adj = match dir {
                    SplitDir::Horizontal => delta.x / rect.width().max(1.0),
                    SplitDir::Vertical => delta.y / rect.height().max(1.0),
                };
                *ratio = (*ratio + adj).clamp(0.1, 0.9);
            }
            if divider_response.hovered() {
                ui.ctx().set_cursor_icon(match dir {
                    SplitDir::Horizontal => egui::CursorIcon::ResizeHorizontal,
                    SplitDir::Vertical => egui::CursorIcon::ResizeVertical,
                });
            }
            ui.painter().rect_filled(
                divider_rect,
                0.0,
                theme.tab_bar_bg.linear_multiply(1.5),
            );

            show_node(ui, first, focused, theme, first_rect, clicked);
            show_node(ui, second, focused, theme, second_rect, clicked);
        },
    }
}
