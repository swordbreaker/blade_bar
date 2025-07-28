// Menu-related helper functions for tray widgets

use gio::Menu as GMenu;
use gtk4::prelude::*;
use gtk4::{Button, PopoverMenu};

/// Helper function to create an icon from PNG data
pub fn create_icon_from_data(
    icon_data: &[u8],
) -> Result<gio::BytesIcon, Box<dyn std::error::Error>> {
    // Create a GBytes object from the icon data using gtk4::glib
    let bytes = gtk4::glib::Bytes::from(icon_data);

    // Create a BytesIcon from the PNG data
    let icon = gio::BytesIcon::new(&bytes);

    Ok(icon)
}

/// Add icon to a menu item from the MenuItem data
pub fn add_icon_to_menu_item(
    menu_item: &gio::MenuItem,
    item: &system_tray::menu::MenuItem,
    label: &str,
) {
    if let Some(icon_name) = &item.icon_name {
        if !icon_name.is_empty() {
            // For GTK4 PopoverMenu, use the proper way to set icon attribute
            menu_item.set_attribute_value("icon", Some(&icon_name.to_variant()));
            println!("Added icon '{}' to menu item '{}'", icon_name, label);
        }
    } else if let Some(icon_data) = &item.icon_data {
        if !icon_data.is_empty() {
            // Create icon from PNG data
            match create_icon_from_data(icon_data) {
                Ok(_icon) => {
                    // For data icons, we'll use a generic icon name as fallback
                    menu_item.set_attribute_value("icon", Some(&"image-x-generic".to_variant()));
                    println!("Added icon from data to menu item '{}'", label);
                }
                Err(e) => {
                    eprintln!(
                        "Failed to create icon from data for item '{}': {}",
                        label, e
                    );
                }
            }
        }
    }
}


