//! Where QuickChat's windows open: on the screen the pointer is on, inside
//! the part the menu bar and the Dock leave free.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSEvent, NSScreen};
use objc2_foundation::NSRect;

/// The free part of the screen under the pointer, in the points egui places
/// windows in. None off the main thread.
pub fn under_pointer() -> Option<egui::Rect> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    // The first screen holds the menu bar; the others are placed around it.
    let primary = screens.firstObject()?.frame();
    let pointer = NSEvent::mouseLocation();
    let screen = screens
        .iter()
        .find(|screen| contains(screen.frame(), pointer.x, pointer.y))
        .or_else(|| screens.firstObject())?;
    Some(flip(primary.size.height, screen.visibleFrame()))
}

fn contains(frame: NSRect, x: f64, y: f64) -> bool {
    x >= frame.origin.x
        && x < frame.origin.x + frame.size.width
        && y >= frame.origin.y
        && y < frame.origin.y + frame.size.height
}

/// AppKit measures from the primary screen's bottom left, upwards; egui
/// from its top left, downwards.
fn flip(primary_height: f64, frame: NSRect) -> egui::Rect {
    let top = primary_height - (frame.origin.y + frame.size.height);
    egui::Rect::from_min_size(
        egui::pos2(frame.origin.x as f32, top as f32),
        egui::vec2(frame.size.width as f32, frame.size.height as f32),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSSize};

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    /// The primary screen's free part starts under its menu bar, and a
    /// second screen above and to the right lands above and to the right.
    #[test]
    fn screens_are_measured_from_the_primary_screens_top_left() {
        let menu_bar = 25.0;
        let below_menu = flip(900.0, rect(0.0, 0.0, 1440.0, 900.0 - menu_bar));
        assert_eq!(below_menu.min, egui::pos2(0.0, 25.0));
        let above_right = flip(900.0, rect(1440.0, 900.0, 1920.0, 1080.0));
        assert_eq!(above_right.min, egui::pos2(1440.0, -1080.0));
        assert!(contains(
            rect(1440.0, 900.0, 1920.0, 1080.0),
            2000.0,
            1000.0
        ));
        assert!(!contains(rect(0.0, 0.0, 1440.0, 900.0), 2000.0, 1000.0));
    }
}
