use std::sync::Arc;

use crate::tray_widget::TrayWidget;
use gtk4::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Orientation, Popover};
use system_tray::client::ActivateRequest;
use system_tray::item::IconPixmap;
use system_tray::item::StatusNotifierItem;
use system_tray::item::Tooltip;

pub fn create_tray_button(
    item: &StatusNotifierItem,
    service_key: &str,
    tray_widget: Arc<TrayWidget>,
) -> Button {
    let button = Button::new();
    button.add_css_class("tray-button");

    let title = item.title.as_deref().clone().unwrap_or("Unknown");

    set_button_icon(item.icon_name.as_deref(), item.icon_pixmap.clone(), &button);

    // Set tooltip
    set_tooltip(&button, item.tool_tip.clone(), Some(title));

    // Handle left-click (primary button) using gesture
    let left_click = gtk4::GestureClick::new();
    left_click.set_button(1); // Left mouse button (button 1)

    let item_id_left = item.id.clone();
    let service_key_left = service_key.to_string();
    let tray_widget_weak_left = Arc::downgrade(&tray_widget);

    left_click.connect_pressed(move |_, _, _x, _y| {
        if let Some(tray_widget) = tray_widget_weak_left.upgrade() {
            let item_id = item_id_left.clone();
            let service_key = service_key_left.clone();

            println!(
                "Left-click on tray item: {} (service: {})",
                item_id, service_key
            );

            // Activate the tray item using the service key
            glib::spawn_future_local(async move {
                if let Err(e) = tray_widget
                    .system_tray_client
                    .activate(ActivateRequest::Default {
                        address: service_key.clone(),
                        x: 0,
                        y: 0,
                    })
                    .await
                {
                    eprintln!(
                        "Failed to activate tray item '{}' (service: '{}'): {}",
                        item_id, service_key, e
                    );
                } else {
                    println!(
                        "Successfully activated tray item: {} (service: {})",
                        item_id, service_key
                    );
                }
            });
        }
    });

    button.add_controller(left_click);

    // Handle middle-click (middle button) using gesture
    let middle_click = gtk4::GestureClick::new();
    middle_click.set_button(2); // Middle mouse button (button 2)

    let item_id_middle = item.id.clone();
    let service_key_middle = service_key.to_string();
    let tray_widget_weak_middle = Arc::downgrade(&tray_widget);

    middle_click.connect_pressed(move |_, _, _x, _y| {
        if let Some(tray_widget) = tray_widget_weak_middle.upgrade() {
            let item_id = item_id_middle.clone();
            let service_key = service_key_middle.clone();

            println!(
                "Middle-click on tray item: {} (service: {})",
                item_id, service_key
            );

            // For middle-click, we can use the Secondary activation (common pattern)
            glib::spawn_future_local(async move {
                if let Err(e) = tray_widget
                    .system_tray_client
                    .activate(ActivateRequest::Secondary {
                        address: service_key.clone(),
                        x: 0,
                        y: 0,
                    })
                    .await
                {
                    eprintln!(
                        "Failed to secondary activate tray item '{}' (service: '{}'): {}",
                        item_id, service_key, e
                    );
                } else {
                    println!(
                        "Successfully secondary activated tray item: {} (service: {})",
                        item_id, service_key
                    );
                }
            });
        }
    });

    button.add_controller(middle_click);

    // Handle right-click (secondary button) using gesture
    let right_click = gtk4::GestureClick::new();
    right_click.set_button(3); // Right mouse button (button 3)

    let item_id_right = item.id.clone();
    let service_key_right = service_key.to_string();
    let tray_widget_weak = Arc::downgrade(&tray_widget);

    right_click.connect_pressed(move |_, _, x, y| {
        println!("Right-click detected at coordinates ({}, {})", x, y);

        if let Some(tray_widget) = tray_widget_weak.upgrade() {
            let item_id = item_id_right.clone();
            let service_key = service_key_right.clone();

            println!(
                "Processing right-click for item: {} (service: {})",
                item_id, service_key
            );

            // Check for manual popover first (with icon support), then fallback to PopoverMenu
            if let Some(manual_popover) = tray_widget.get_manual_popover_for_service_key(&service_key) {
                println!(
                    "Found manual popover for item: {} (service: {}), showing it",
                    item_id, service_key
                );

                // Use popup() to show the manual popover
                manual_popover.popup();

                println!("Manual popover popup() called successfully");
            } else if let Some(popover_menu) = tray_widget.get_menu_for_service_key(&service_key) {
                println!(
                    "Found PopoverMenu for item: {} (service: {}), showing it",
                    item_id, service_key
                );

                // Use popup() to show the popover at the current position
                popover_menu.popup();

                println!("PopoverMenu popup() called successfully");
            } else {
                println!(
                    "No PopoverMenu found for item: {} (service: {})",
                    item_id, service_key
                );

                // Fallback: try to activate the item using the service key
                println!(
                    "Right-click fallback: using service key '{}' for item '{}'",
                    service_key, item_id
                );

                let tray_widget_clone = tray_widget.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = tray_widget_clone
                        .system_tray_client
                        .activate(ActivateRequest::Default {
                            address: service_key.clone(),
                            x: 0,
                            y: 0,
                        })
                        .await
                    {
                        eprintln!(
                            "Failed to activate tray item '{}' (service: '{}'): {}",
                            item_id, service_key, e
                        );
                    } else {
                        println!(
                            "Fallback activation successful for item: {} (service: {})",
                            item_id, service_key
                        );
                    }
                });
            }
        } else {
            println!("TrayWidget weak reference upgrade failed in right-click handler");
        }
    });

    button.add_controller(right_click);

    button
}

fn create_button_icon(
    icon_name: Option<&str>,
    icon_pixmap: Option<Vec<IconPixmap>>,
) -> Option<Image> {
    match (icon_name, icon_pixmap.as_deref()) {
        (Some(icon_name), _) if !icon_name.is_empty() => {
            let image = Image::from_icon_name(icon_name);
            image.set_pixel_size(16);
            return Some(image);
        }
        (_, Some(pixmap)) if pixmap.len() > 0 => {
            let pixels = &pixmap[0];
            let data = &pixmap[0].pixels;

            let mut rgba_data = Vec::with_capacity(data.len());
            // Convert ARGB32 (network byte order) to RGBA
            for chunk in data.chunks_exact(4) {
                let argb = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                let a = ((argb >> 24) & 0xff) as u8;
                let r = ((argb >> 16) & 0xff) as u8;
                let g = ((argb >> 8) & 0xff) as u8;
                let b = (argb & 0xff) as u8;
                rgba_data.extend_from_slice(&[r, g, b, a]);
            }

            let pixbuf = Pixbuf::from_mut_slice(
                rgba_data,
                Colorspace::Rgb,
                true, // has_alpha
                8,    // bits_per_sample
                pixels.width as i32,
                pixels.height as i32,
                (pixels.width * 4) as i32, // rowstride (width * 4 bytes per pixel)
            );

            let image = Image::from_pixbuf(Some(&pixbuf));
            image.set_pixel_size(16);
            return Some(image);
        }
        _ => {
            return None;
        }
    }
}

pub fn set_button_icon(
    icon_name: Option<&str>,
    icon_pixmap: Option<Vec<IconPixmap>>,
    button: &Button,
) {
    match create_button_icon(icon_name, icon_pixmap) {
        Some(image) => {
            button.set_child(Some(&image));
        }
        None => {
            // Fallback to text label if no icon is available
            button.set_label("");
        }
    }
}

pub fn set_tooltip(button: &Button, tooltip: Option<Tooltip>, title: Option<&str>) {
    let tooltip_ref = tooltip.as_ref();

    // Use simple tooltip for text-only cases
    let tooltip_text = tooltip_ref.map(|t| t.title.as_str());
    let description = tooltip_ref.map(|t| t.description.as_str()).unwrap_or("");
    let final_text = tooltip_text.or(title).unwrap_or("");

    let combined_text = if !description.is_empty() && !final_text.is_empty() {
        format!("{}\n{}", final_text, description)
    } else {
        final_text.to_string()
    };

    button.set_tooltip_text(Some(&combined_text));
}

fn setup_button_left_click(item: &StatusNotifierItem, button: &Button) {
    button.connect_clicked(move |_| {});
}

fn show_context_menu(
    button: &Button,
    item_id: &str,
    item_title: &str,
    menu_data: &Option<String>,
    x: f64,
    y: f64,
) {
    println!(
        "Showing context menu for: {} ({}) at ({}, {})",
        item_title, item_id, x, y
    );

    // Create a popover menu
    let popover = gtk4::Popover::new();
    popover.set_parent(button);
    popover.set_position(gtk4::PositionType::Bottom);

    // Create a vertical box to hold menu items
    let menu_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
    menu_box.add_css_class("menu");

    // Add menu items based on the tray item
    let show_hide_button = gtk4::Button::with_label("Show/Hide");
    show_hide_button.add_css_class("flat");
    show_hide_button.add_css_class("menu-item");

    let item_id_clone = item_id.to_string();
    let item_title_clone = item_title.to_string();
    show_hide_button.connect_clicked(move |_| {
        println!("Show/Hide clicked for: {}", item_title_clone);
        // try_activate_application_window(&item_id_clone, &item_title_clone);
    });

    let preferences_button = gtk4::Button::with_label("Preferences");
    preferences_button.add_css_class("flat");
    preferences_button.add_css_class("menu-item");

    let item_id_clone2 = item_id.to_string();
    preferences_button.connect_clicked(move |_| {
        println!("Preferences clicked for: {}", item_id_clone2);
        // Try to open preferences for the application
        // try_open_preferences(&item_id_clone2);
    });

    let about_button = gtk4::Button::with_label("About");
    about_button.add_css_class("flat");
    about_button.add_css_class("menu-item");

    let item_title_clone2 = item_title.to_string();
    about_button.connect_clicked(move |_| {
        println!("About clicked for: {}", item_title_clone2);
        // Could show an about dialog or open help
    });

    let quit_button = gtk4::Button::with_label("Quit");
    quit_button.add_css_class("flat");
    quit_button.add_css_class("menu-item");

    let item_id_clone3 = item_id.to_string();
    let item_title_clone3 = item_title.to_string();
    quit_button.connect_clicked(move |_| {
        println!("Quit clicked for: {}", item_title_clone3);
        // try_quit_application(&item_id_clone3, &item_title_clone3);
    });

    // Add all buttons to the menu
    menu_box.append(&show_hide_button);

    // Add separator
    let separator1 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    separator1.add_css_class("menu-separator");
    menu_box.append(&separator1);

    menu_box.append(&preferences_button);
    menu_box.append(&about_button);

    // Add separator
    let separator2 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
    separator2.add_css_class("menu-separator");
    menu_box.append(&separator2);

    menu_box.append(&quit_button);

    // If we have actual menu data from the tray item, parse and add those items
    if let Some(menu_str) = menu_data {
        println!("Tray item has menu data: {}", menu_str);
        // Add separator for custom menu items
        let separator3 = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        separator3.add_css_class("menu-separator");
        menu_box.append(&separator3);

        // TODO: Parse the actual menu structure and add custom items
        // For now, just show that custom menu data is available
        let custom_info = gtk4::Label::new(Some("Custom menu available"));
        custom_info.add_css_class("menu-info");
        menu_box.append(&custom_info);
    }

    popover.set_child(Some(&menu_box));

    // Close popover when any menu item is clicked
    let popover_clone = popover.clone();
    show_hide_button.connect_clicked(move |_| {
        popover_clone.popdown();
    });

    let popover_clone = popover.clone();
    preferences_button.connect_clicked(move |_| {
        popover_clone.popdown();
    });

    let popover_clone = popover.clone();
    about_button.connect_clicked(move |_| {
        popover_clone.popdown();
    });

    let popover_clone = popover.clone();
    quit_button.connect_clicked(move |_| {
        popover_clone.popdown();
    });

    // Show the popover
    popover.popup();
}
