use gio::Menu as GMenu;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, PopoverMenu};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use system_tray::client::Event;
use system_tray::client::{Client, UpdateEvent};
use system_tray::error::Error;
use system_tray::item::StatusNotifierItem;
use tokio::sync::broadcast;

use crate::tray_widget::controls::{create_tray_button, set_button_icon, set_tooltip};

pub mod controls;

pub struct TrayWidget {
    pub container: GtkBox,
    items: Arc<Mutex<HashMap<String, StatusNotifierItem>>>,
    item_buttons: Arc<Mutex<HashMap<String, Button>>>,
    item_menus: Arc<Mutex<HashMap<String, PopoverMenu>>>,
    // Map from item ID to service key for activation
    item_to_service_key: Arc<Mutex<HashMap<String, String>>>,
    pub system_tray_client: Arc<Client>,
    shutdown_tx: broadcast::Sender<()>,
    thread_handle: Arc<JoinHandle<()>>,
}

impl TrayWidget {
    pub async fn new() -> Result<Arc<Self>, Error> {
        let container = GtkBox::new(Orientation::Horizontal, 5);
        container.add_css_class("tray-widget");

        let client = Arc::new(Client::new().await?);
        let client_copy = Arc::clone(&client);

        let (thread_handle, shutdown_tx, mut event_rx) = Self::start_event_listener(&client_copy);

        let tray_widget = Arc::new(TrayWidget {
            container,
            items: Arc::new(Mutex::new(HashMap::new())),
            item_buttons: Arc::new(Mutex::new(HashMap::new())),
            item_menus: Arc::new(Mutex::new(HashMap::new())),
            item_to_service_key: Arc::new(Mutex::new(HashMap::new())),
            system_tray_client: client,
            shutdown_tx: shutdown_tx,
            thread_handle: Arc::new(thread_handle),
        });

        let tray_ptr = tray_widget.clone();

        // Handle events on the main thread
        glib::MainContext::default().spawn_local(async move {
            while let Some(event) = event_rx.recv().await {
                let tray_ptr = tray_ptr.clone();
                tray_ptr.handle_tray_event(event);
            }
        });

        Ok(tray_widget)
    }

    fn start_event_listener(
        system_tray_client: &Arc<Client>,
    ) -> (
        JoinHandle<()>,
        broadcast::Sender<()>,
        tokio::sync::mpsc::UnboundedReceiver<Event>,
    ) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();

        let client = system_tray_client.clone();

        let thread_handle = thread::spawn(move || {
            let rt: tokio::runtime::Runtime =
                tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async {
                let mut tray_rx = client.subscribe();
                let initial_items = client.items();

                // Process initial items
                for (key, (sni_item, _menu)) in initial_items.lock().unwrap().iter() {
                    if event_tx
                        .send(Event::Add(key.clone(), Box::new(sni_item.clone())))
                        .is_err()
                    {
                        break;
                    }
                }

                // Listen for updates with cancellation
                let mut shutdown_rx = shutdown_rx;
                loop {
                    tokio::select! {
                        event = tray_rx.recv() => {
                            match event {
                                Ok(ev) => {
                                    if event_tx.send(ev).is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            println!("Shutting down tray listener");
                            break;
                        }
                    }
                }
            });
        });

        return (thread_handle, shutdown_tx, event_rx);
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    // Get the PopoverMenu for a given item ID
    pub fn get_menu_for_item(&self, item_id: &str) -> Option<PopoverMenu> {
        // Since item_id is not unique, we need to find the service key first
        if let Ok(mapping) = self.item_to_service_key.lock() {
            if let Some(service_key) = mapping.get(item_id) {
                if let Ok(menus) = self.item_menus.lock() {
                    return menus.get(service_key).cloned();
                }
            }
        }
        None
    }
    
    // Get the PopoverMenu for a given service key (unique identifier)
    pub fn get_menu_for_service_key(&self, service_key: &str) -> Option<PopoverMenu> {
        if let Ok(menus) = self.item_menus.lock() {
            menus.get(service_key).cloned()
        } else {
            None
        }
    }

    // Get the service key for a given item ID (needed for activation)
    pub fn get_service_key_for_item(&self, item_id: &str) -> Option<String> {
        if let Ok(mapping) = self.item_to_service_key.lock() {
            mapping.get(item_id).cloned()
        } else {
            None
        }
    }

    // Store a PopoverMenu using the item ID as key (called after menu creation)
    fn store_menu_for_item(&self, item_id: &str, service_key: &str) {
        if let Ok(menus) = self.item_menus.lock() {
            // Try to find an existing menu stored with the service key and move it to item_id key
            if let Some(menu) = menus.get(service_key).cloned() {
                drop(menus); // Release the lock before getting a mutable lock
                if let Ok(mut menus_mut) = self.item_menus.lock() {
                    menus_mut.remove(service_key); // Remove from service key
                    menus_mut.insert(item_id.to_string(), menu); // Store with item ID
                    println!(
                        "Moved PopoverMenu from key '{}' to item ID '{}'",
                        service_key, item_id
                    );
                }
            }
        }
    }

    // Attach a DBus menu to a button as a PopoverMenu
    fn attach_context_menu_to_button(
        &self,
        button: &Button,
        menu_path: &str,
        item_id: Option<&str>,
    ) {
        println!(
            "Attaching context menu to button with menu path: {}",
            menu_path
        );

        // For GTK4, we create a PopoverMenu from the DBus menu path
        // The menu_path contains the DBus object path for the menu
        self.create_popover_menu_for_button(button, menu_path, item_id);
    }

    // Create a PopoverMenu for a specific service key (used during MenuConnect)
    fn create_popover_menu_for_service_key(
        &self,
        button: &Button,
        menu_path: &str,
        item_id: &str,
        service_key: &str,
    ) {
        println!("Creating PopoverMenu for service key: {} (item: {}, menu_path: {})", service_key, item_id, menu_path);

        // Try to get the actual menu data for this specific service key
        if let Ok(items) = self.system_tray_client.items().lock() {
            if let Some((_item, menu_opt)) = items.get(service_key) {
                if let Some(menu) = menu_opt {
                    // Create a GMenu from the actual menu structure
                    let gmenu = GMenu::new();

                    // Create an action group for this menu
                    let action_group = gio::SimpleActionGroup::new();
                    self.populate_gmenu_from_menu_items(&gmenu, &menu.submenus, &action_group);

                    // Create a PopoverMenu
                    let popover = PopoverMenu::from_model(Some(&gmenu));
                    popover.set_parent(button);

                    // Associate the action group with the popover
                    popover.insert_action_group("menu", Some(&action_group));

                    // Store the PopoverMenu using the service key (unique identifier)
                    if let Ok(mut menus) = self.item_menus.lock() {
                        menus.insert(service_key.to_string(), popover);
                        println!("Stored PopoverMenu with service key: '{}'", service_key);
                    }

                    println!(
                        "PopoverMenu created with {} items for service key: {}",
                        menu.submenus.len(),
                        service_key
                    );
                    return;
                } else {
                    println!("No menu data found for service key: {}", service_key);
                }
            } else {
                println!("Service key not found in items: {}", service_key);
            }
        }

        // Fallback to basic menu if no menu data found
        self.create_basic_popover_menu(button, menu_path);
    }

    // Create a PopoverMenu for the button based on the DBus menu path
    fn create_popover_menu_for_button(
        &self,
        button: &Button,
        menu_path: &str,
        item_id: Option<&str>,
    ) {
        println!("Creating PopoverMenu for menu path: {}", menu_path);

        // Try to get the actual menu data from the system-tray client
        if let Ok(items) = self.system_tray_client.items().lock() {
            // Find the specific menu that matches this menu_path or item_id
            let mut target_menu = None;
            let mut target_key = None;

            println!(
                "Looking for menu with item_id: {:?}, menu_path: {}",
                item_id, menu_path
            );
            println!("Available items: {:?}", items.keys().collect::<Vec<_>>());

            for (key, (_item, menu_opt)) in items.iter() {
                if let Some(menu) = menu_opt {
                    println!(
                        "Checking item: key={}, item.id={}, has_menu={}",
                        key,
                        _item.id,
                        menu_opt.is_some()
                    );

                    // Try multiple matching strategies
                    let matches = if let Some(id) = item_id {
                        // Strategy 1: Direct item ID match
                        let id_match = _item.id == id;
                        // Strategy 2: Check if menu_path contains the item ID
                        let path_match = menu_path.contains(id);
                        // Strategy 3: Check if the service key is in the menu_path
                        let service_match = menu_path.contains(key);

                        println!(
                            "  Matching strategies for item_id '{}': id_match={}, path_match={}, service_match={}",
                            id, id_match, path_match, service_match
                        );

                        id_match || path_match || service_match
                    } else {
                        // Fallback: check if the service key is in the menu_path
                        menu_path.contains(key)
                    };

                    if matches {
                        target_menu = Some(menu);
                        target_key = Some(key);
                        println!("Found matching menu for key: {}", key);
                        break;
                    }
                }
            }

            // Create PopoverMenu for the found target menu
            if let (Some(menu), Some(key)) = (target_menu, target_key) {
                // Create a GMenu from the actual menu structure
                let gmenu = GMenu::new();

                // Create an action group for this menu
                let action_group = gio::SimpleActionGroup::new();
                self.populate_gmenu_from_menu_items(&gmenu, &menu.submenus, &action_group);

                // Create a PopoverMenu
                let popover = PopoverMenu::from_model(Some(&gmenu));
                popover.set_parent(button);

                // Associate the action group with the popover
                popover.insert_action_group("menu", Some(&action_group));

                // Store the PopoverMenu using the service key (unique identifier)
                if let Ok(mut menus) = self.item_menus.lock() {
                    menus.insert(key.to_string(), popover);
                    println!("Stored PopoverMenu with service key: '{}'", key);
                }

                println!(
                    "PopoverMenu created with {} items for key: {}",
                    menu.submenus.len(),
                    key
                );
                return;
            } else {
                println!(
                    "No matching menu found for item_id: {:?}, menu_path: {}",
                    item_id, menu_path
                );
            }
        }

        // Fallback to basic menu if no menu data found
        self.create_basic_popover_menu(button, menu_path);
    }

    // Create a PopoverMenu directly from TrayMenu data
    fn create_popover_menu_from_tray_menu(
        &self,
        button: &Button,
        tray_menu: &system_tray::menu::TrayMenu,
        item_id: &str,
        service_key: &str,
    ) {
        println!("Creating PopoverMenu from TrayMenu for item: {} (service: {})", item_id, service_key);

        // Create a GMenu from the TrayMenu structure
        let gmenu = GMenu::new();

        // Create an action group for this menu
        let action_group = gio::SimpleActionGroup::new();
        self.populate_gmenu_from_menu_items(&gmenu, &tray_menu.submenus, &action_group);

        // Create a PopoverMenu
        let popover = PopoverMenu::from_model(Some(&gmenu));
        popover.set_parent(button);

        // Associate the action group with the popover
        popover.insert_action_group("menu", Some(&action_group));

        // Store the PopoverMenu using the service key (unique identifier)
        if let Ok(mut menus) = self.item_menus.lock() {
            menus.insert(service_key.to_string(), popover);
            println!("Stored PopoverMenu from TrayMenu with service key: '{}'", service_key);
        }

        println!(
            "PopoverMenu created from TrayMenu with {} items for item: {} (service: {})",
            tray_menu.submenus.len(),
            item_id,
            service_key
        );
    }

    // Populate GMenu from actual menu items
    fn populate_gmenu_from_menu_items(
        &self,
        gmenu: &GMenu,
        menu_items: &[system_tray::menu::MenuItem],
        action_group: &gio::SimpleActionGroup,
    ) {
        for item in menu_items {
            // Check if the item should be displayed
            if !item.visible {
                continue;
            }

            // For separators, we typically have no label
            if item.label.is_none() || item.label.as_ref().map_or(true, |l| l.is_empty()) {
                // Likely a separator - add a section if we have items already
                if gmenu.n_items() > 0 {
                    let section = GMenu::new();
                    gmenu.append_section(None, &section);
                }
                continue;
            }

            // Add regular menu items
            if let Some(label) = &item.label {
                if !label.is_empty() {
                    // Create action for this menu item
                    let action_name = format!("menu_item_{}", item.id);
                    let action = gio::SimpleAction::new(&action_name, None);

                    // Store the menu item information for the action callback
                    let item_id = item.id;
                    let label_clone = label.clone();

                    action.connect_activate(move |_, _| {
                        println!("Menu item activated: '{}' (id: {})", label_clone, item_id);
                        // TODO: Implement actual menu item activation via DBus
                        // This would typically involve calling the dbusmenu event_triggered method
                    });

                    // Set action sensitivity based on item.enabled
                    action.set_enabled(item.enabled);

                    action_group.add_action(&action);

                    println!(
                        "Adding menu item: '{}' (enabled: {}, id: {})",
                        label, item.enabled, item.id
                    );
                    gmenu.append(Some(label), Some(&format!("menu.{}", action_name)));
                }
            }
        }

        // If no items were added, add a placeholder
        if gmenu.n_items() == 0 {
            gmenu.append(Some("No menu items"), None);
        }
    }

    // Create a basic fallback PopoverMenu
    fn create_basic_popover_menu(&self, button: &Button, menu_path: &str) {
        println!("Creating basic PopoverMenu for menu path: {}", menu_path);

        // Create a GMenu
        let gmenu = GMenu::new();
        gmenu.append(Some("Show/Hide"), Some("app.show_hide"));
        gmenu.append(Some("Preferences"), Some("app.preferences"));
        gmenu.append(Some("About"), Some("app.about"));
        gmenu.append(Some("Quit"), Some("app.quit"));

        // Create a PopoverMenu
        let popover = PopoverMenu::from_model(Some(&gmenu));
        popover.set_parent(button);

        println!("Basic PopoverMenu created for menu path: {}", menu_path);
    }

    // Static method to handle events (no self reference needed)
    fn handle_tray_event(self: Arc<Self>, event: Event) {
        match event {
            Event::Add(key, item) => {
                let item_id = item.id.clone();

                // Store the item
                self.items
                    .lock()
                    .unwrap()
                    .insert(key.clone(), *item.clone());

                // Store the mapping from item ID to service key for activation
                self.item_to_service_key
                    .lock()
                    .unwrap()
                    .insert(item_id, key.clone());

                let button = create_tray_button(&item, &key, Arc::clone(&self));
                self.item_buttons
                    .lock()
                    .unwrap()
                    .insert(key.clone(), button.clone());
                self.container.append(&button);
                button.connect_clicked(move |_| {});
            }
            Event::Remove(key) => {
                self.items.lock().unwrap().remove(&key);
                if let Some(button) = self.item_buttons.lock().unwrap().remove(&key) {
                    self.container.remove(&button);
                }
            }
            Event::Update(key, event) => {
                let guard = self.item_buttons.lock().unwrap();

                match event {
                    UpdateEvent::AttentionIcon(_) => {
                        println!("Attention icon update not handled yet")
                    }
                    UpdateEvent::Icon {
                        icon_name,
                        icon_pixmap,
                    } => {
                        if let Some(button) = guard.get(&key) {
                            set_button_icon(icon_name.as_deref(), icon_pixmap.clone(), button);
                            print!("Updated icon for key: {}", key);
                        } else {
                            eprintln!("Button for key {} not found", key);
                        }
                    }
                    UpdateEvent::OverlayIcon(_) => println!("Overlay icon update not handled yet"),
                    UpdateEvent::Status(_status) => println!("Status update not handled yet"),
                    UpdateEvent::Title(_) => println!("Title update not handled yet"),
                    UpdateEvent::Tooltip(tooltip) => {
                        if let Some(button) = guard.get(&key) {
                            set_tooltip(button, tooltip.clone(), None);
                        } else {
                            eprintln!("Button for key {} not found", key);
                        }
                    }
                    UpdateEvent::Menu(tray_menu) => {
                        println!("Menu update received for key {}", key);

                        // Try to find the button for this key and create/update its PopoverMenu
                        if let Some(button) = guard.get(&key) {
                            // Get the item to find its ID
                            if let Ok(items_guard) = self.items.lock() {
                                if let Some(item) = items_guard.get(&key) {
                                    let item_id = item.id.clone();

                                    // Check if we already have a menu for this service key
                                    let needs_creation = if let Ok(menus) = self.item_menus.lock() {
                                        !menus.contains_key(&key)
                                    } else {
                                        true
                                    };

                                    if needs_creation {
                                        println!(
                                            "Creating PopoverMenu from Menu update for item: {}",
                                            item_id
                                        );
                                        // Create a PopoverMenu directly from the TrayMenu
                                        self.create_popover_menu_from_tray_menu(
                                            button, &tray_menu, &item_id, &key,
                                        );
                                    } else {
                                        println!(
                                            "PopoverMenu already exists for service key: {}",
                                            key
                                        );
                                    }
                                }
                            }
                        }
                    }
                    UpdateEvent::MenuDiff(menu_diffs) => {
                        println!("Menu diff update not handled yet: {:#?}", menu_diffs)
                    }
                    UpdateEvent::MenuConnect(menu) => {
                        println!("MenuConnect received for key {}", key);

                        // Store the menu for this item - we need to find the item ID for this key
                        if let Some(button) = guard.get(&key) {
                            // Get the item to find its ID
                            if let Ok(items_guard) = self.items.lock() {
                                if let Some(item) = items_guard.get(&key) {
                                    let item_id = item.id.clone();
                                    println!("Storing PopoverMenu for item: {} (service: {})", item_id, key);

                                    // Directly create menu for this specific service key
                                    self.create_popover_menu_for_service_key(
                                        button,
                                        &menu,
                                        &item_id,
                                        &key,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl Drop for TrayWidget {
    fn drop(&mut self) {
        // Send shutdown signal to the thread
        let _ = self.shutdown_tx.send(());

        let thread_handle = self.thread_handle.clone();

        if let Ok(thread_handle) = Arc::try_unwrap(thread_handle) {
            // If we can unwrap, it means there are no other references to the thread handle
            // and we can safely join it.
            if let Err(e) = thread_handle.join() {
                eprintln!("Error joining tray thread: {:?}", e);
            }
        }

        // Clear items and buttons
        self.items.lock().unwrap().clear();
        self.item_buttons.lock().unwrap().clear();
    }
}
