use std::sync::Arc;

use crate::tray_widget::TrayWidget;
use gtk4::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, GestureClick, Image, Orientation, Popover};
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
    set_tooltip(&button, item.tool_tip.clone(), Some(title));

    // Handle left-click (primary button) using gesture
    let left_click = get_button_left_click(item, &tray_widget, service_key);

    button.add_controller(left_click);

    let right_click = get_button_right_click(item, &tray_widget, Arc::from(service_key));
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

fn get_button_left_click(
    item: &StatusNotifierItem,
    tray_widget: &Arc<TrayWidget>,
    service_key: &str,
) -> gtk4::GestureClick {
    let left_click = gtk4::GestureClick::new();
    left_click.set_button(1); // Left mouse button (button 1)

    let item_id_left = item.id.clone();
    let service_key_left = service_key.to_string();
    let tray_widget_weak = Arc::downgrade(&tray_widget);

    left_click.connect_pressed(move |_, _, _x, _y| {
        if let Some(tray_widget) = tray_widget_weak.upgrade() {
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

    return left_click;
}

fn get_button_right_click(
    item: &StatusNotifierItem,
    tray_widget: &Arc<TrayWidget>,
    service_key: Arc<str>,
) -> gtk4::GestureClick {
    let right_click = gtk4::GestureClick::new();
    right_click.set_button(3);

    let service_key = service_key.clone();
    let item_id_right = item.id.clone();
    let tray_widget_weak = Arc::downgrade(&tray_widget);

    right_click.connect_pressed(move |_, _, x, y| {
        if let Some(tray_widget) = tray_widget_weak.upgrade() {
            let item_id = item_id_right.clone();
            let service_key = service_key.clone();

            // Check for manual popover first (with icon support), then fallback to PopoverMenu
            if let Some(manual_popover) =
                tray_widget.get_manual_popover_for_service_key(&service_key)
            {
                // Use popup() to show the manual popover
                manual_popover.popup();
            } else if let Some(popover_menu) = tray_widget.get_menu_for_service_key(&service_key) {
                // Use popup() to show the popover at the current position
                popover_menu.popup();
            } else {
                let service_key = service_key.clone();
                let tray_widget_clone: Arc<TrayWidget> = tray_widget.clone();
                glib::spawn_future_local(async move {
                    if let Err(e) = tray_widget_clone
                        .system_tray_client
                        .activate(ActivateRequest::Default {
                            address: service_key.clone().to_string(),
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

    right_click
}

fn show_context_menu(
    button: &Button,
    item_id: &str,
    item_title: &str,
    menu_data: &Option<String>,
    x: f64,
    y: f64,
) {
    // Create a popover menu
    let popover = gtk4::Popover::new();
    popover.set_parent(button);
    popover.set_position(gtk4::PositionType::Bottom);

    // Create a vertical box to hold menu items
    let menu_box = GtkBox::new(gtk4::Orientation::Vertical, 0);
    menu_box.add_css_class("menu");

    // If we have actual menu data from the tray item, parse and add those items
    if let Some(menu_str) = menu_data {
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

    // Show the popover
    popover.popup();
}
