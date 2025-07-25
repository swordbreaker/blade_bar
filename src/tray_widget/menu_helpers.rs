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

/// Create a basic fallback PopoverMenu with standard icons
pub fn create_basic_popover_menu(button: &Button, menu_path: &str) -> PopoverMenu {
    println!("Creating basic PopoverMenu for menu path: {}", menu_path);

    // Create a GMenu
    let gmenu = GMenu::new();

    // Create menu items with icons using attribute approach
    let show_hide_item = gio::MenuItem::new(Some("Show/Hide"), Some("app.show_hide"));
    show_hide_item.set_attribute_value("icon", Some(&"window-minimize".to_variant()));
    gmenu.append_item(&show_hide_item);

    let preferences_item = gio::MenuItem::new(Some("Preferences"), Some("app.preferences"));
    preferences_item.set_attribute_value("icon", Some(&"preferences-system".to_variant()));
    gmenu.append_item(&preferences_item);

    let about_item = gio::MenuItem::new(Some("About"), Some("app.about"));
    about_item.set_attribute_value("icon", Some(&"help-about".to_variant()));
    gmenu.append_item(&about_item);

    let quit_item = gio::MenuItem::new(Some("Quit"), Some("app.quit"));
    quit_item.set_attribute_value("icon", Some(&"application-exit".to_variant()));
    gmenu.append_item(&quit_item);

    // Create a PopoverMenu
    let popover = PopoverMenu::from_model(Some(&gmenu));
    popover.set_parent(button);

    println!("Basic PopoverMenu created for menu path: {}", menu_path);
    popover
}
