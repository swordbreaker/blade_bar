use std::sync::Arc;

use gtk4::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Orientation};
use system_tray::client::ActivateRequest;
use system_tray::item::{Status, StatusNotifierItem};

use crate::tray_widget::{self, TrayWidget};

pub fn create_tray_button(item: &StatusNotifierItem, tray_widget: Arc<TrayWidget>) -> Button {
    let button = Button::new();
    button.add_css_class("tray-button");

    let title = item.title.as_deref().clone().unwrap_or("Unknown");

    setup_button_icon(item, &button, title);

    // Set tooltip
    button.set_tooltip_text(Some(&title));

    button.connect_clicked(move |_| {
        // println!("Left-clicked tray item: {} ({})", t1, item_id);
    });

    // Handle right-click (secondary button) using gesture
    let right_click = gtk4::GestureClick::new();
    right_click.set_button(3); // Right mouse button (button 3)

    let item_id_right = item.id.clone();
    let tray_widget_weak = Arc::downgrade(&tray_widget);

    right_click.connect_pressed(move |_, _, x, y| {
        if let Some(tray_widget) = tray_widget_weak.upgrade() {
            let item_id = item_id_right.clone();

            tray_widget
                .system_tray_client
                .activate(ActivateRequest::Default {
                    address: item_id,
                    x: x as i32,
                    y: y as i32,
                });
            // Self::show_context_menu(&button, &item_id_right, &t2, &menu_data, x, y);
        }
    });

    button.add_controller(right_click);

    button
}

fn setup_button_icon(item: &StatusNotifierItem, button: &Button, title: &str) {
    match (&item.icon_name, item.icon_pixmap.as_deref()) {
        (Some(icon_name), _) if !icon_name.is_empty() => {
            let image = Image::from_icon_name(icon_name);
            image.set_pixel_size(16);
            button.set_child(Some(&image));
        }
        (_, Some(pixmap)) => {
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
            button.set_child(Some(&image));
        }
        _ => {
            // Fallback to text label
            button.set_label(&title);
        }
    }
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
