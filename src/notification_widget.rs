use glib::ControlFlow;
use glib::timeout_add_local;
use gtk4::prelude::*;
use gtk4::{Button, Label};
use std::process::Command;
use std::time::Duration;

pub struct NotificationWidget {
    pub button: Button,
    label: Label,
}

impl NotificationWidget {
    pub fn new() -> Option<Self> {
        // Check if swaync-client is available
        if !Self::is_swaync_available() {
            return None;
        }

        let button = Button::new();
        button.add_css_class("notification-button");

        let label = Label::new(None);
        label.add_css_class("notification-label");
        button.set_child(Some(&label));

        let widget = NotificationWidget { button, label };

        widget.setup_click_handlers();
        widget.start_monitoring();

        Some(widget)
    }

    fn is_swaync_available() -> bool {
        Command::new("which")
            .arg("swaync-client")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn setup_click_handlers(&self) {
        let button = self.button.clone();

        // Left click: toggle notification panel
        button.connect_clicked(|_| {
            let _ = Command::new("swaync-client").args(["-t", "-sw"]).spawn();
        });

        // Right click: dismiss all notifications
        let gesture = gtk4::GestureClick::new();
        gesture.set_button(3); // Right mouse button

        gesture.connect_pressed(|_, _, _, _| {
            let _ = Command::new("swaync-client").args(["-d", "-sw"]).spawn();
        });

        button.add_controller(gesture);
    }

    fn start_monitoring(&self) {
        let label = self.label.clone();

        // Update every 2 seconds with a timeout to prevent hanging
        timeout_add_local(Duration::from_secs(2), move || {
            // Use a simple approach: try to get status with a short timeout
            if let Some(status) = Self::get_notification_status() {
                Self::update_display(&label, &status);
            } else {
                // If swaync is not responding, show a default state
                label.set_text("🔔");
                if let Some(parent) = label.parent() {
                    parent.set_tooltip_text(Some("Notifications unavailable"));
                }
            }
            ControlFlow::Continue
        });

        // Initial update
        if let Some(status) = Self::get_notification_status() {
            Self::update_display(&self.label, &status);
        } else {
            self.label.set_text("🔔");
            if let Some(parent) = self.label.parent() {
                parent.set_tooltip_text(Some("Notifications unavailable"));
            }
        }
    }

    fn get_notification_status() -> Option<NotificationStatus> {
        // Get notification count
        let count_output = Command::new("swaync-client").arg("--count").output().ok()?;

        if !count_output.status.success() {
            return None;
        }

        let count_str = String::from_utf8_lossy(&count_output.stdout);
        let count = count_str.trim().parse::<u32>().unwrap_or(0);

        // Get DND status
        let dnd_output = Command::new("swaync-client")
            .arg("--get-dnd")
            .output()
            .ok()?;

        if !dnd_output.status.success() {
            return None;
        }

        let dnd_str = String::from_utf8_lossy(&dnd_output.stdout);
        let dnd = dnd_str.trim().to_lowercase() == "true";

        Some(NotificationStatus { count, dnd })
    }

    fn update_display(label: &Label, status: &NotificationStatus) {
        let icon = Self::get_icon_for_status(status);
        label.set_markup(&icon);

        // Set tooltip
        let tooltip = if status.count > 0 {
            format!(
                "{} notification{}",
                status.count,
                if status.count == 1 { "" } else { "s" }
            )
        } else {
            "No notifications".to_string()
        };

        if let Some(parent) = label.parent() {
            parent.set_tooltip_text(Some(&tooltip));
        }
    }

    fn get_icon_for_status(status: &NotificationStatus) -> String {
        // Show notification indicator if there are notifications
        if status.count > 0 {
            if status.dnd {
                // DND with notifications
                "<span foreground='red'><sup>●</sup></span>".to_string()
            } else {
                // Normal notifications
                "<span foreground='red'><sup>●</sup></span>".to_string()
            }
        } else {
            if status.dnd {
                // DND without notifications
                "".to_string()
            } else {
                // No notifications
                "".to_string()
            }
        }
    }

    pub fn widget(&self) -> &Button {
        &self.button
    }
}

#[derive(Debug)]
struct NotificationStatus {
    count: u32,
    dnd: bool,
}
