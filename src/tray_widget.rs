use gtk4::prelude::*;
use gtk4::{Box, Button, Image, Orientation};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use system_tray::client::Client;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TrayItem {
    pub id: String,
    pub title: String,
    pub icon_name: Option<String>,
    pub icon_pixmap: Option<Vec<u8>>,
    pub menu: Option<String>,
}

pub struct TrayWidget {
    pub container: Box,
    items: Arc<Mutex<HashMap<String, TrayItem>>>,
    item_buttons: Arc<Mutex<HashMap<String, Button>>>,
}

impl TrayWidget {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 5);
        container.add_css_class("tray-widget");

        let tray_widget = TrayWidget {
            container,
            items: Arc::new(Mutex::new(HashMap::new())),
            item_buttons: Arc::new(Mutex::new(HashMap::new())),
        };

        // Start monitoring for tray applications using the new method
        tray_widget.listen_for_tray_items();

        tray_widget
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    fn listen_for_tray_items(&self) {
        println!("Listening for tray items...");

        let items = Arc::clone(&self.items);
        let item_buttons = Arc::clone(&self.item_buttons);
        let container = self.container.clone();

        // Spawn the async task on the main context with a proper Tokio runtime
        glib::MainContext::default().spawn_local(async move {
            // Create a Tokio runtime in a blocking thread for the system tray client
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<HashMap<String, TrayItem>>();

            thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

                rt.block_on(async {
                    println!("Connecting to system tray client...");

                    match Client::new().await {
                        Ok(client) => {
                            let mut tray_rx = client.subscribe();
                            let initial_items = client.items();

                            // Process initial items
                            let mut tray_items = HashMap::new();
                            for (key, (sni_item, menu)) in initial_items.lock().unwrap().iter() {
                                println!("Initial tray item: {key}: {sni_item:?}");

                                let tray_item = TrayItem {
                                    id: key.clone(),
                                    title: sni_item.title.clone().unwrap_or_else(|| key.clone()),
                                    icon_name: sni_item.icon_name.clone(),
                                    icon_pixmap: None,
                                    menu: menu.as_ref().map(|m| format!("{:?}", m)),
                                };
                                tray_items.insert(key.clone(), tray_item);
                            }

                            // Send initial items
                            if !tray_items.is_empty() {
                                let _ = tx.send(tray_items);
                            }

                            // Listen for updates
                            while let Ok(ev) = tray_rx.recv().await {
                                println!("Tray event: {ev:?}");

                                match ev {
                                    system_tray::client::Event::Add(key, item) => {
                                        println!("Tray item added: {key}: {item:?}");
                                    }
                                    system_tray::client::Event::Remove(key) => {
                                        println!("Tray item removed: {key}");
                                    }
                                    system_tray::client::Event::Update(key, event) => {
                                        println!("Tray item updated: {key}: {event:?}");
                                    }
                                }

                                let current_items = client.items();
                                let mut updated_tray_items = HashMap::new();

                                for (key, (sni_item, menu)) in current_items.lock().unwrap().iter()
                                {
                                    let tray_item = TrayItem {
                                        id: key.clone(),
                                        title: sni_item
                                            .title
                                            .clone()
                                            .unwrap_or_else(|| key.clone()),
                                        icon_name: sni_item.icon_name.clone(),
                                        icon_pixmap: None,
                                        menu: menu.as_ref().map(|m| format!("{:?}", m)),
                                    };
                                    updated_tray_items.insert(key.clone(), tray_item);
                                }

                                let _ = tx.send(updated_tray_items);
                            }
                        }
                        Err(e) => {
                            println!("Failed to connect to system tray client: {}", e);
                            println!("No system tray items available - make sure you have applications with tray icons running");
                        }
                    }
                });
            });

            // Process updates from the Tokio thread
            while let Some(tray_items) = rx.recv().await {
                Self::update_tray_ui(
                    Arc::clone(&items),
                    Arc::clone(&item_buttons),
                    container.clone(),
                    tray_items,
                );
            }
        });
    }

    fn update_tray_ui(
        items: Arc<Mutex<HashMap<String, TrayItem>>>,
        item_buttons: Arc<Mutex<HashMap<String, Button>>>,
        container: Box,
        new_items: HashMap<String, TrayItem>,
    ) {
        let mut items_guard = items.lock().unwrap();
        let mut buttons_guard = item_buttons.lock().unwrap();

        // Remove items that no longer exist
        let removed_items: Vec<String> = items_guard
            .keys()
            .filter(|id| !new_items.contains_key(*id))
            .cloned()
            .collect();

        for id in removed_items {
            items_guard.remove(&id);
            if let Some(button) = buttons_guard.remove(&id) {
                container.remove(&button);
            }
        }

        // Add or update existing items
        for (id, item) in new_items {
            if !items_guard.contains_key(&id) {
                // New item - create button
                let button = Self::create_tray_button(&item);
                buttons_guard.insert(id.clone(), button.clone());
                container.append(&button);
            } else {
                // Existing item - update if needed
                if let Some(button) = buttons_guard.get(&id) {
                    Self::update_tray_button(button, &item);
                }
            }
            items_guard.insert(id, item);
        }
    }

    fn create_tray_button(item: &TrayItem) -> Button {
        let button = Button::new();
        button.add_css_class("tray-button");

        // Create icon or fallback to text
        if let Some(icon_name) = &item.icon_name {
            if !icon_name.is_empty() {
                let image = Image::from_icon_name(icon_name);
                image.set_pixel_size(16);
                button.set_child(Some(&image));
            } else {
                // Fallback to first character of title
                let fallback_text = item.title.chars().next().unwrap_or('?').to_string();
                button.set_label(&fallback_text);
            }
        } else {
            // Fallback to first character of title
            let fallback_text = item.title.chars().next().unwrap_or('?').to_string();
            button.set_label(&fallback_text);
        }

        // Set tooltip
        button.set_tooltip_text(Some(&item.title));

        // Handle left-click (primary button)
        let item_title = item.title.clone();
        let item_id = item.id.clone();

        button.connect_clicked(move |_| {
            println!("Left-clicked tray item: {} ({})", item_title, item_id);
            Self::handle_tray_item_activate(&item_id, &item_title);
        });

        // Handle right-click (secondary button) using gesture
        let right_click = gtk4::GestureClick::new();
        right_click.set_button(3); // Right mouse button (button 3)

        let item_title_right = item.title.clone();
        let item_id_right = item.id.clone();
        let menu_data = item.menu.clone();
        let button_weak = button.downgrade();

        right_click.connect_pressed(move |_, _, x, y| {
            println!(
                "Right-clicked tray item: {} ({})",
                item_title_right, item_id_right
            );
            if let Some(button) = button_weak.upgrade() {
                Self::show_context_menu(
                    &button,
                    &item_id_right,
                    &item_title_right,
                    &menu_data,
                    x,
                    y,
                );
            }
        });

        button.add_controller(right_click);

        button
    }

    fn handle_tray_item_activate(item_id: &str, item_title: &str) {
        println!("Activating tray item: {} ({})", item_title, item_id);

        // Try to activate the application or bring it to focus
        // This will find the application window and bring it to the foreground
        // or send activation signal to the StatusNotifierItem
        Self::try_activate_application_window(item_id, item_title);
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
        let menu_box = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        menu_box.add_css_class("menu");

        // Add menu items based on the tray item
        let show_hide_button = gtk4::Button::with_label("Show/Hide");
        show_hide_button.add_css_class("flat");
        show_hide_button.add_css_class("menu-item");

        let item_id_clone = item_id.to_string();
        let item_title_clone = item_title.to_string();
        show_hide_button.connect_clicked(move |_| {
            println!("Show/Hide clicked for: {}", item_title_clone);
            Self::try_activate_application_window(&item_id_clone, &item_title_clone);
        });

        let preferences_button = gtk4::Button::with_label("Preferences");
        preferences_button.add_css_class("flat");
        preferences_button.add_css_class("menu-item");

        let item_id_clone2 = item_id.to_string();
        preferences_button.connect_clicked(move |_| {
            println!("Preferences clicked for: {}", item_id_clone2);
            // Try to open preferences for the application
            Self::try_open_preferences(&item_id_clone2);
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
            Self::try_quit_application(&item_id_clone3, &item_title_clone3);
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

    fn try_open_preferences(item_id: &str) {
        use std::process::Command;

        println!("Trying to open preferences for: {}", item_id);

        // Try common preference opening patterns
        let preference_commands = vec![
            format!("{} --preferences", item_id),
            format!("{} --settings", item_id),
            format!("{} --config", item_id),
            format!("{}-preferences", item_id),
            format!("{}-settings", item_id),
        ];

        for cmd_str in preference_commands {
            let parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if parts.len() >= 1 {
                let mut command = Command::new(parts[0]);
                if parts.len() > 1 {
                    command.args(&parts[1..]);
                }

                if let Ok(_) = command.spawn() {
                    println!("Successfully opened preferences with: {}", cmd_str);
                    return;
                }
            }
        }

        println!("Could not find a way to open preferences for: {}", item_id);
    }

    fn try_quit_application(item_id: &str, item_title: &str) {
        use std::process::Command;

        println!("Trying to quit application: {} ({})", item_title, item_id);

        // First try to find the process and terminate it gracefully
        if let Ok(output) = Command::new("pgrep").arg("-f").arg(item_id).output() {
            let pids = String::from_utf8_lossy(&output.stdout);
            for pid_str in pids.lines() {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    println!("Found process {} for {}, sending SIGTERM", pid, item_id);

                    // Try SIGTERM first (graceful shutdown)
                    if let Ok(_) = Command::new("kill")
                        .args(&["-TERM", &pid.to_string()])
                        .output()
                    {
                        println!("Sent SIGTERM to process {}", pid);

                        // Give it a moment to shut down gracefully
                        std::thread::sleep(std::time::Duration::from_millis(500));

                        // Check if it's still running
                        if let Ok(check_output) = Command::new("kill")
                            .args(&["-0", &pid.to_string()])
                            .output()
                        {
                            if !check_output.status.success() {
                                println!("Process {} terminated gracefully", pid);
                                return;
                            }
                        }

                        // If still running, try SIGKILL
                        if let Ok(_) = Command::new("kill")
                            .args(&["-KILL", &pid.to_string()])
                            .output()
                        {
                            println!("Sent SIGKILL to process {}", pid);
                            return;
                        }
                    }
                }
            }
        }

        // Try application-specific quit commands
        let quit_commands = vec![
            format!("{} --quit", item_id),
            format!("{} --exit", item_id),
            format!("pkill -f {}", item_id),
        ];

        for cmd_str in quit_commands {
            let parts: Vec<&str> = cmd_str.split_whitespace().collect();
            if parts.len() >= 1 {
                let mut command = Command::new(parts[0]);
                if parts.len() > 1 {
                    command.args(&parts[1..]);
                }

                if let Ok(_) = command.output() {
                    println!("Attempted quit command: {}", cmd_str);
                    return;
                }
            }
        }

        println!("Could not quit application: {}", item_id);
    }

    fn try_activate_application_window(item_id: &str, item_title: &str) {
        // This is a simplified approach to try to bring an application window to focus
        // In practice, this would need more sophisticated window management

        use std::process::Command;

        // Try to use wmctrl to bring the window to focus if available
        if let Ok(_) = Command::new("wmctrl").arg("-l").output() {
            // wmctrl is available, try to find and activate the window
            let search_terms = vec![
                item_title.to_lowercase(),
                item_id.to_lowercase(),
                item_id.replace("-", " ").to_lowercase(),
            ];

            for term in search_terms {
                let result = Command::new("wmctrl").args(&["-a", &term]).output();

                match result {
                    Ok(output) => {
                        if output.status.success() {
                            println!("Successfully activated window for: {}", term);
                            return;
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        // Try to use xdotool as fallback if wmctrl didn't work
        if let Ok(_) = Command::new("xdotool").arg("version").output() {
            let search_terms = vec![item_title, item_id];

            for term in search_terms {
                let result = Command::new("xdotool")
                    .args(&["search", "--name", term, "windowactivate"])
                    .output();

                match result {
                    Ok(output) => {
                        if output.status.success() {
                            println!("Successfully activated window with xdotool for: {}", term);
                            return;
                        }
                    }
                    Err(_) => continue,
                }
            }
        }

        // If window management tools aren't available, try to launch the application
        // This is a last resort and may not be ideal
        println!("Could not find existing window, application might need to be launched");

        // Try common application launch patterns
        let launch_commands = vec![
            item_id.to_string(),
            item_id.replace("-", ""),
            format!("{}-desktop", item_id),
        ];

        for cmd in launch_commands {
            if let Ok(_) = Command::new(&cmd).spawn() {
                println!("Attempted to launch: {}", cmd);
                return;
            }
        }

        println!("Could not activate or launch application for: {}", item_id);
    }

    fn update_tray_button(button: &Button, item: &TrayItem) {
        // Update tooltip
        button.set_tooltip_text(Some(&item.title));

        // Update icon if needed
        if let Some(icon_name) = &item.icon_name {
            if !icon_name.is_empty() {
                let image = Image::from_icon_name(icon_name);
                image.set_pixel_size(16);
                button.set_child(Some(&image));
            }
        }
    }
}
