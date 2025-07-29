// Manual menu creation with proper icon support for GTK4

use gio::glib::translate::FromGlibPtrArrayContainerAsVec;
use gtk4::gdk_pixbuf::{InterpType, Pixbuf};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Label, Popover, Orientation};
use std::io::Cursor;
use std::sync::Arc;
use system_tray::menu::MenuItem;

/// Create a manual popover menu with proper icon support
pub fn create_popover_menu(
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

        menu_item.submenu.iter().for_each(|submenu: &MenuItem| {
            // Handle submenu items
            let submenu_popover = create_popover_menu(button, &[submenu.clone()], service_key, Arc::clone(&system_tray_client));
            let submenu_button = Button::new();
            submenu_button.add_css_class("submenu-button");
            submenu_button.set_child(Some(&Image::from_icon_name("go-next")));
            submenu_button.connect_clicked(move |_| {
                submenu_popover.popup();
            });
            menu_box.append(&submenu_button);
            return;
        });

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
                match create_icon(menu_item) {
                    Some(icon) => {
                        item_box.append(&icon);
                    },
                    None => {
                        let spacer = GtkBox::new(Orientation::Horizontal, 0);
                        spacer.set_size_request(16, 16);
                        item_box.append(&spacer);
                    }
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

fn create_icon(menu_item: &MenuItem) -> Option<Image> {
    if let Some(icon_name) = &menu_item.icon_name {
        if !icon_name.is_empty() {
            let icon = Image::from_icon_name(icon_name);
            icon.set_icon_size(gtk4::IconSize::Normal);
            return Some(icon);
        }
    } else if let Some(icon_data) = &menu_item.icon_data {
        if !icon_data.is_empty() {
            // Create icon from PNG data
            match Pixbuf::from_read(Cursor::new(icon_data.clone())) {
                Ok(pixbuf) => {
                    // Scale the pixbuf to appropriate size (16x16 for menu items)
                    let scaled_pixbuf = pixbuf.scale_simple(16, 16, InterpType::Bilinear);
                    if let Some(scaled) = scaled_pixbuf {
                        return Some(Image::from_pixbuf(Some(&scaled)));
                    } else {
                        // Fallback if scaling fails
                        return Some(Image::from_pixbuf(Some(&pixbuf)));
                    }
                }
                Err(e) => {
                    eprintln!("Failed to load icon from PNG data: {}", e);
                    // Use fallback icon
                    return Some(Image::from_icon_name("image-x-generic"));
                }
            }
        }
    }
    return None;
}