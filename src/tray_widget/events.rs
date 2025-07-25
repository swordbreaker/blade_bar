// Event handling helpers for tray widgets

use gtk4::prelude::*;
use gtk4::Button;
use system_tray::item::StatusNotifierItem;

/// Setup tooltip for a button based on tray item information
pub fn setup_button_tooltip(button: &Button, item: &StatusNotifierItem) {
    // Create tooltip text from available information
    let mut tooltip_parts = Vec::new();

    if let Some(title) = &item.title {
        if !title.is_empty() {
            tooltip_parts.push(title.clone());
        }
    }

    // Set tooltip
    if !tooltip_parts.is_empty() {
        let tooltip = tooltip_parts.join("\n");
        button.set_tooltip_text(Some(&tooltip));
    } else if !item.id.is_empty() {
        // Fallback to item ID
        button.set_tooltip_text(Some(&item.id));
    }
}

/// Helper function to trigger menu item activation
pub async fn activate_menu_item(
    client: &system_tray::client::Client,
    service_key: &str,
    item_id: i32,
    label: &str,
) {
    // Use the menu interface to trigger the event
    // The menu path is typically "/MenuBar" for most applications
    let menu_path = "/MenuBar".to_string();

    if let Err(e) = client
        .activate(system_tray::client::ActivateRequest::MenuItem {
            address: service_key.to_string(),
            menu_path,
            submenu_id: item_id,
        })
        .await
    {
        eprintln!(
            "Failed to trigger menu event for item {}: {}",
            item_id, e
        );
    } else {
        println!(
            "Successfully triggered menu event for item: {} ({})",
            item_id, label
        );
    }
}
