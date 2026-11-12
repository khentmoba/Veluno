use crate::ui;
use tao::{
    dpi::{LogicalPosition, LogicalSize, Position, Size},
    window::{CursorIcon, ResizeDirection},
};
use wry::Rect;

/// Takes the side web panel from the content edge away from the sidebar.
pub(crate) fn side_panel_bounds(
    content: Rect,
    scale: f64,
    sidebar_right: bool,
    progress: f64,
) -> (Rect, Rect) {
    let position = content.position.to_logical::<f64>(scale);
    let size = content.size.to_logical::<f64>(scale);
    let width = ui::SIDE_PANEL_WIDTH.min(size.width * 0.45);
    let offset = (width + ui::WINDOW_MARGIN) * progress;
    let rest = (size.width - offset).max(10.);
    let (panel_x, content_x) = if sidebar_right {
        (position.x - width + width * progress, position.x + offset)
    } else {
        (position.x + size.width - width * progress, position.x)
    };
    let rect = |x: f64, width: f64| Rect {
        position: Position::Logical(LogicalPosition::new(x, position.y)),
        size: Size::Logical(LogicalSize::new(width, size.height)),
    };
    (rect(content_x, rest), rect(panel_x, width))
}

/// Splits the content area into two side-by-side panes with a window-margin gap.
pub(crate) fn split_bounds(content: Rect, scale: f64) -> (Rect, Rect) {
    let position = content.position.to_logical::<f64>(scale);
    let size = content.size.to_logical::<f64>(scale);
    let width = ((size.width - ui::WINDOW_MARGIN) / 2.).max(10.);
    let pane = |x: f64| Rect {
        position: Position::Logical(LogicalPosition::new(x, position.y)),
        size: Size::Logical(LogicalSize::new(width, size.height)),
    };
    (
        pane(position.x),
        pane(position.x + width + ui::WINDOW_MARGIN),
    )
}

pub(crate) fn step(progress: f64, target: f64, dt: f64) -> f64 {
    let amount = dt / if target > progress { 0.30 } else { 0.22 };
    if target > progress {
        (progress + amount).min(target)
    } else {
        (progress - amount).max(target)
    }
}

pub(crate) fn ease(progress: f64) -> f64 {
    let p = progress.clamp(0., 1.);
    p * p * p * (p * (p * 6. - 15.) + 10.)
}

pub(crate) fn parse_resize_direction(direction: &str) -> Option<ResizeDirection> {
    match direction {
        "n" => Some(ResizeDirection::North),
        "s" => Some(ResizeDirection::South),
        "e" => Some(ResizeDirection::East),
        "w" => Some(ResizeDirection::West),
        "ne" => Some(ResizeDirection::NorthEast),
        "nw" => Some(ResizeDirection::NorthWest),
        "se" => Some(ResizeDirection::SouthEast),
        "sw" => Some(ResizeDirection::SouthWest),
        _ => None,
    }
}

pub(crate) fn border_resize_direction(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<(ResizeDirection, CursorIcon)> {
    let border = ui::WINDOW_MARGIN;
    let corner = 12.;
    let left = x < border;
    let right = x >= width - border;
    let top = y < border;
    let bottom = y >= height - border;
    if !(left || right || top || bottom) {
        return None;
    }
    let near_left = x < corner;
    let near_right = x >= width - corner;
    let near_top = y < corner;
    let near_bottom = y >= height - corner;
    Some(match (near_left, near_right, near_top, near_bottom) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::NwResize),
        (_, true, true, _) => (ResizeDirection::NorthEast, CursorIcon::NeResize),
        (true, _, _, true) => (ResizeDirection::SouthWest, CursorIcon::SwResize),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::SeResize),
        _ if left => (ResizeDirection::West, CursorIcon::WResize),
        _ if right => (ResizeDirection::East, CursorIcon::EResize),
        _ if top => (ResizeDirection::North, CursorIcon::NResize),
        _ => (ResizeDirection::South, CursorIcon::SResize),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_directions_accept_only_compass_names() {
        assert_eq!(
            parse_resize_direction("nw"),
            Some(ResizeDirection::NorthWest)
        );
        assert_eq!(parse_resize_direction("e"), Some(ResizeDirection::East));
        assert_eq!(parse_resize_direction("NW"), None);
        assert_eq!(parse_resize_direction(""), None);
    }

    #[test]
    fn border_hit_testing_picks_edges_and_corners() {
        let direction =
            |x, y| border_resize_direction(x, y, 800., 600.).map(|(direction, _)| direction);
        assert_eq!(direction(400., 300.), None);
        assert_eq!(direction(0., 300.), Some(ResizeDirection::West));
        assert_eq!(direction(400., 2.), Some(ResizeDirection::North));
        assert_eq!(direction(2., 2.), Some(ResizeDirection::NorthWest));
        assert_eq!(direction(797., 597.), Some(ResizeDirection::SouthEast));
    }

    #[test]
    fn split_panes_share_the_content_width_around_a_margin() {
        let content = Rect {
            position: Position::Logical(LogicalPosition::new(100., 50.)),
            size: Size::Logical(LogicalSize::new(1000., 600.)),
        };
        let (left, right) = split_bounds(content, 1.);
        let width = (1000. - ui::WINDOW_MARGIN) / 2.;
        assert_eq!(left.position.to_logical::<f64>(1.).x, 100.);
        assert_eq!(
            right.position.to_logical::<f64>(1.).x,
            100. + width + ui::WINDOW_MARGIN
        );
        assert_eq!(left.size.to_logical::<f64>(1.).width, width);
        assert_eq!(right.size.to_logical::<f64>(1.).width, width);
    }
}
