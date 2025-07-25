// Manual menu creation with proper icon support for GTK4

use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Popover, Orientation};
use std::sync::Arc;
use system_tray::menu::MenuItem;

/// Create a manual popover menu with proper icon support
pub fn create_manual_popover_menu(
    button: &Button,
    menu_items: &[MenuItem],
    service_key: &str,
    system_tray_client: Arc<system_tray::client::Client>,
) -> Popover {
    let popover = Popover::new();
    popover.set_parent(button);
    popover.set_has_arrow(true);

    // Create a vertical box to hold menu items
    let menu_box = GtkBox::new(Orientation::Vertical, 0);
    menu_box.add_css_class("menu");

    // Add menu items
    for menu_item in menu_items {
        if !menu_item.visible {
            continue;
        }

        // Handle separator items
        if format!("{:?}", menu_item.menu_type).contains("Separator") {
            let separator = gtk4::Separator::new(Orientation::Horizontal);
            separator.add_css_class("menu-separator");
            menu_box.append(&separator);
            continue;
        }

        if let Some(label) = &menu_item.label {
            if !label.is_empty() {
                // Create menu item button
                let item_button = Button::new();
                item_button.add_css_class("flat");
                item_button.add_css_class("menu-item");
                item_button.set_can_focus(false);

                // Create horizontal box for icon and label
                let item_box = GtkBox::new(Orientation::Horizontal, 8);
                item_box.set_margin_start(8);
                item_box.set_margin_end(8);
                item_box.set_margin_top(4);
                item_box.set_margin_bottom(4);

                // Add icon if available
                let mut icon_added = false;
                if let Some(icon_name) = &menu_item.icon_name {
                    if !icon_name.is_empty() {
                        let icon = Image::from_icon_name(icon_name);
                        icon.set_icon_size(gtk4::IconSize::Normal);
                        item_box.append(&icon);
                        icon_added = true;
                    }
                } else if let Some(icon_data) = &menu_item.icon_data {
                    if !icon_data.is_empty() {
                        // Try to create icon from data - simplified approach
                        let icon = Image::from_icon_name("image-x-generic"); // Fallback icon for data
                        icon.set_icon_size(gtk4::IconSize::Normal);
                        item_box.append(&icon);
                        icon_added = true;
                    }
                }

                // Add placeholder space if no icon
                if !icon_added {
                    let spacer = GtkBox::new(Orientation::Horizontal, 0);
                    spacer.set_size_request(16, 16);
                    item_box.append(&spacer);
                }

                // Add label
                let label_widget = Label::new(Some(label));
                label_widget.set_halign(gtk4::Align::Start);
                item_box.append(&label_widget);

                item_button.set_child(Some(&item_box));

                // Set up click handler
                let item_id = menu_item.id;
                let label_clone = label.clone();
                let service_key_clone = service_key.to_string();
                let client = Arc::clone(&system_tray_client);
                let popover_weak = popover.downgrade();

                item_button.connect_clicked(move |_| {
                    println!("Manual menu item activated: '{}' (id: {})", label_clone, item_id);

                    // Close popover
                    if let Some(popover) = popover_weak.upgrade() {
                        popover.popdown();
                    }

                    // Trigger menu item activation
                    let service_key = service_key_clone.clone();
                    let client = client.clone();

                    gtk4::glib::spawn_future_local(async move {
                        let menu_path = "/MenuBar".to_string();
                        if let Err(e) = client
                            .activate(system_tray::client::ActivateRequest::MenuItem {
                                address: service_key.clone(),
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
                            println!("Successfully triggered menu event for item: {}", item_id);
                        }
                    });
                });

                // Set enabled state
                item_button.set_sensitive(menu_item.enabled);

                menu_box.append(&item_button);
            }
        }
    }

    // If no items were added, add a placeholder
    if menu_box.first_child().is_none() {
        let placeholder = Label::new(Some("No menu items"));
        placeholder.add_css_class("dim-label");
        placeholder.set_margin_start(8);
        placeholder.set_margin_end(8);
        placeholder.set_margin_top(8);
        placeholder.set_margin_bottom(8);
        menu_box.append(&placeholder);
    }

    popover.set_child(Some(&menu_box));
    popover
}
