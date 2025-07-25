use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use system_tray::client::{Client, Event as TrayEvent};
use system_tray::error::Error;
use system_tray::item::StatusNotifierItem;
use tokio::sync::broadcast;

/// The main tray widget that manages system tray items
pub struct TrayWidget {
    pub container: GtkBox,
    items: Arc<Mutex<HashMap<String, StatusNotifierItem>>>,
    item_buttons: Arc<Mutex<HashMap<String, Button>>>,
    item_menus: Arc<Mutex<HashMap<String, gtk4::PopoverMenu>>>,
    // Store manual popovers with icon support
    item_manual_popovers: Arc<Mutex<HashMap<String, gtk4::Popover>>>,
    // Store action groups to keep them alive
    action_groups: Arc<Mutex<HashMap<String, gio::SimpleActionGroup>>>,
    // Map from item ID to service key for activation
    item_to_service_key: Arc<Mutex<HashMap<String, String>>>,
    pub system_tray_client: Arc<Client>,
    shutdown_tx: broadcast::Sender<()>,
    thread_handle: Arc<JoinHandle<()>>,
}

impl TrayWidget {
    /// Create a new TrayWidget
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
            item_manual_popovers: Arc::new(Mutex::new(HashMap::new())),
            action_groups: Arc::new(Mutex::new(HashMap::new())),
            item_to_service_key: Arc::new(Mutex::new(HashMap::new())),
            system_tray_client: client,
            shutdown_tx,
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
        tokio::sync::mpsc::UnboundedReceiver<TrayEvent>,
    ) {
        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<TrayEvent>();

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
                        .send(TrayEvent::Add(key.clone(), Box::new(sni_item.clone())))
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

        (thread_handle, shutdown_tx, event_rx)
    }

    /// Get the GTK widget container
    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    /// Handle a tray event and update the UI
    fn handle_tray_event(self: &Arc<Self>, event: TrayEvent) {
        match event {
            TrayEvent::Add(service_key, item) => {
                self.add_tray_item(&service_key, &item, self);
            }
            TrayEvent::Update(service_key, update_event) => {
                self.update_tray_item(&service_key, &update_event);
            }
            TrayEvent::Remove(service_key) => {
                self.remove_tray_item(&service_key);
            }
        }
    }

    /// Add a new tray item
    fn add_tray_item(
        &self,
        service_key: &str,
        item: &StatusNotifierItem,
        tray_widget_arc: &Arc<Self>,
    ) {
        println!("Adding tray item: {} (id: {})", service_key, item.id);

        // Store the item
        if let Ok(mut items) = self.items.lock() {
            items.insert(service_key.to_string(), item.clone());
        }

        // Store the item ID to service key mapping
        if let Ok(mut mapping) = self.item_to_service_key.lock() {
            mapping.insert(item.id.clone(), service_key.to_string());
        }

        // Create button using the controls module
        let button = crate::tray_widget::controls::create_tray_button(
            item,
            service_key,
            Arc::clone(tray_widget_arc),
        );

        // Store the button
        if let Ok(mut buttons) = self.item_buttons.lock() {
            buttons.insert(service_key.to_string(), button.clone());
        }

        // Create a basic menu for the tray item
        self.create_menu_for_item(service_key, item, &button);

        // Add to container
        self.container.append(&button);
    }

    /// Update an existing tray item
    fn update_tray_item(
        &self,
        service_key: &str,
        _update_event: &system_tray::client::UpdateEvent,
    ) {
        println!("Updating tray item: {}", service_key);

        // For now, just update the button if it exists
        if let Ok(buttons) = self.item_buttons.lock() {
            if let Some(button) = buttons.get(service_key) {
                // Get the current item to extract icon information
                if let Ok(items) = self.items.lock() {
                    if let Some(item) = items.get(service_key) {
                        // Update button icon and tooltip using the current item data
                        crate::tray_widget::controls::set_button_icon(
                            item.icon_name.as_deref(),
                            item.icon_pixmap.clone(),
                            button,
                        );
                        crate::tray_widget::controls::set_tooltip(
                            button,
                            item.tool_tip.clone(),
                            item.title.as_deref(),
                        );
                    }
                }
            }
        }
    }

    /// Remove a tray item
    fn remove_tray_item(&self, service_key: &str) {
        println!("Removing tray item: {}", service_key);

        // Remove from container
        if let Ok(mut buttons) = self.item_buttons.lock() {
            if let Some(button) = buttons.remove(service_key) {
                self.container.remove(&button);
            }
        }

        // Remove menu and action group
        if let Ok(mut menus) = self.item_menus.lock() {
            menus.remove(service_key);
        }
        if let Ok(mut manual_popovers) = self.item_manual_popovers.lock() {
            manual_popovers.remove(service_key);
        }
        if let Ok(mut action_groups) = self.action_groups.lock() {
            action_groups.remove(service_key);
        }

        // Remove from items
        if let Ok(mut items) = self.items.lock() {
            if let Some(item) = items.remove(service_key) {
                // Remove from item ID mapping
                if let Ok(mut mapping) = self.item_to_service_key.lock() {
                    mapping.remove(&item.id);
                }
            }
        }
    }

    /// Get the service key for a given item ID (needed for activation)
    pub fn get_service_key_for_item(&self, item_id: &str) -> Option<String> {
        if let Ok(mapping) = self.item_to_service_key.lock() {
            mapping.get(item_id).cloned()
        } else {
            None
        }
    }

    /// Get the PopoverMenu for a given service key
    pub fn get_menu_for_service_key(&self, service_key: &str) -> Option<gtk4::PopoverMenu> {
        if let Ok(menus) = self.item_menus.lock() {
            menus.get(service_key).cloned()
        } else {
            None
        }
    }

    /// Get the manual Popover for a given service key (with icon support)
    pub fn get_manual_popover_for_service_key(&self, service_key: &str) -> Option<gtk4::Popover> {
        if let Ok(manual_popovers) = self.item_manual_popovers.lock() {
            manual_popovers.get(service_key).cloned()
        } else {
            None
        }
    }

    /// Create a basic menu for a tray item
    fn create_menu_for_item(&self, service_key: &str, item: &StatusNotifierItem, button: &Button) {
        // Check if the system-tray client has menu data for this item
        if let Ok(items) = self.system_tray_client.items().lock() {
            if let Some((_item, menu_opt)) = items.get(service_key) {
                if let Some(menu) = menu_opt {
                    // Create a menu from actual menu data using manual approach for better icon support
                    println!(
                        "Creating manual menu from system-tray data for {}",
                        service_key
                    );
                    let popover = crate::tray_widget::manual_menu::create_manual_popover_menu(
                        button,
                        &menu.submenus,
                        service_key,
                        Arc::clone(&self.system_tray_client),
                    );

                    // Store the manual popover for display
                    if let Ok(mut manual_popovers) = self.item_manual_popovers.lock() {
                        manual_popovers.insert(service_key.to_string(), popover);
                    }
                    return;
                }
            }
        }

        // Fallback: create a basic menu using menu helpers
        println!("Creating basic fallback menu for {}", service_key);
        let popover = crate::tray_widget::menu_helpers::create_basic_popover_menu(
            button,
            &format!("/MenuBar/{}", item.id),
        );

        if let Ok(mut menus) = self.item_menus.lock() {
            menus.insert(service_key.to_string(), popover);
        }
    }

    /// Create a PopoverMenu from system-tray menu data
    fn create_popover_from_menu(
        &self,
        button: &Button,
        menu: &system_tray::menu::TrayMenu,
        service_key: &str,
    ) -> gtk4::PopoverMenu {
        use gio::Menu as GMenu;

        // Create a GMenu from the TrayMenu structure
        let gmenu = GMenu::new();

        // Create an action group for this menu
        let action_group = gio::SimpleActionGroup::new();

        // Add menu items recursively
        self.add_menu_items_recursive(
            &gmenu,
            &action_group,
            &menu.submenus,
            service_key,
            String::new(),
        );

        // If no items were added, add a placeholder
        if gmenu.n_items() == 0 {
            gmenu.append(Some("No menu items"), None);
        }

        // Create a PopoverMenu
        let popover = gtk4::PopoverMenu::from_model(Some(&gmenu));
        popover.set_parent(button);

        // Associate the action group with the popover
        popover.insert_action_group("menu", Some(&action_group));

        // Enable icons in PopoverMenu (GTK4 feature)
        popover.set_has_arrow(true);
        // Try to enable icons (this may not work with GMenu approach)
        if let Some(settings) = gtk4::Settings::default() {
            // Some GTK themes may support menu icons
            settings.set_property("gtk-menu-images", &true);
        }

        println!(
            "Inserted action group 'menu' with {} actions into popover for service: {}",
            action_group.list_actions().len(),
            service_key
        );

        // Store the action group to keep it alive
        if let Ok(mut action_groups) = self.action_groups.lock() {
            action_groups.insert(service_key.to_string(), action_group);
        }

        println!(
            "PopoverMenu created with {} items for service key: {}",
            gmenu.n_items(),
            service_key
        );
        popover
    }

    /// Recursively add menu items and submenus to a GMenu
    fn add_menu_items_recursive(
        &self,
        gmenu: &gio::Menu,
        action_group: &gio::SimpleActionGroup,
        menu_items: &[system_tray::menu::MenuItem],
        service_key: &str,
        path_prefix: String,
    ) {
        for (index, menu_item) in menu_items.iter().enumerate() {
            if !menu_item.visible {
                continue;
            }

            // Handle separator items - check menu_type field
            if format!("{:?}", menu_item.menu_type).contains("Separator") {
                // GTK doesn't have direct separator support in GMenu, but we can add a disabled item
                let separator = gio::MenuItem::new(Some("---"), None);
                separator.set_attribute_value(
                    "custom",
                    Some(&format!("separator_{}", index).to_variant()),
                );
                gmenu.append_item(&separator);
                continue;
            }

            if let Some(label) = &menu_item.label {
                if !label.is_empty() {
                    // Make action names unique by including service key
                    let action_name = format!(
                        "{}__item_{}",
                        service_key.replace(":", "_").replace(".", "_"),
                        menu_item.id
                    );

                    // Check if this item has children (submenus)
                    if !menu_item.submenu.is_empty() {
                        println!(
                            "Creating submenu '{}' with {} children",
                            label,
                            menu_item.submenu.len()
                        );

                        // Create a submenu
                        let submenu = gio::Menu::new();
                        let submenu_path = format!("{}{}_", path_prefix, index);

                        // Recursively add children to the submenu
                        self.add_menu_items_recursive(
                            &submenu,
                            action_group,
                            &menu_item.submenu,
                            service_key,
                            submenu_path,
                        );

                        // Create a submenu item
                        let submenu_item = gio::MenuItem::new_submenu(Some(label), &submenu);

                        // Add icon if available
                        crate::tray_widget::menu_helpers::add_icon_to_menu_item(
                            &submenu_item,
                            menu_item,
                            label,
                        );

                        gmenu.append_item(&submenu_item);
                    } else {
                        // Regular menu item (leaf node)
                        let action = gio::SimpleAction::new(&action_name, None);

                        // Store the menu item information for the action callback
                        let item_id = menu_item.id;
                        let label_clone = label.clone();
                        let service_key_clone = service_key.to_string();
                        let system_tray_client = Arc::clone(&self.system_tray_client);

                        println!(
                            "Creating action '{}' for menu item '{}'",
                            action_name, label
                        );

                        action.connect_activate(move |_, _| {
                            println!("Menu item activated: '{}' (id: {})", label_clone, item_id);

                            // Trigger menu item activation via the system-tray client
                            let service_key = service_key_clone.clone();
                            let client = system_tray_client.clone();

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
                                    println!(
                                        "Successfully triggered menu event for item: {}",
                                        item_id
                                    );
                                }
                            });
                        });

                        // Set action sensitivity based on item.enabled
                        action.set_enabled(menu_item.enabled);
                        action_group.add_action(&action);

                        // Create a menu item with icon support
                        let g_menu_item =
                            gio::MenuItem::new(Some(label), Some(&format!("menu.{}", action_name)));

                        println!(
                            "Created GMenuItem '{}' with action 'menu.{}'",
                            label, action_name
                        );

                        // Add icon if available
                        crate::tray_widget::menu_helpers::add_icon_to_menu_item(
                            &g_menu_item,
                            menu_item,
                            label,
                        );

                        gmenu.append_item(&g_menu_item);
                    }
                }
            }
        }
    }

    /// Helper method to clone self for controls module use
    fn clone_for_controls(&self) -> TrayWidget {
        TrayWidget {
            container: self.container.clone(),
            items: Arc::clone(&self.items),
            item_buttons: Arc::clone(&self.item_buttons),
            item_menus: Arc::clone(&self.item_menus),
            item_manual_popovers: Arc::clone(&self.item_manual_popovers),
            action_groups: Arc::clone(&self.action_groups),
            item_to_service_key: Arc::clone(&self.item_to_service_key),
            system_tray_client: Arc::clone(&self.system_tray_client),
            shutdown_tx: self.shutdown_tx.clone(),
            thread_handle: Arc::clone(&self.thread_handle),
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
