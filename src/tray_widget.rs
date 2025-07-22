use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation};
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
    system_tray_client: Arc<Client>,
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

    // Static method to handle events (no self reference needed)
    fn handle_tray_event(self: Arc<Self>, event: Event) {
        match event {
            Event::Add(key, item) => {
                self.items
                    .lock()
                    .unwrap()
                    .insert(key.clone(), *item.clone());
                let button = create_tray_button(&item, Arc::clone(&self));
                self.item_buttons
                    .lock()
                    .unwrap()
                    .insert(key.clone(), button.clone());
                self.container.append(&button);
                button.connect_clicked(move |_| {
                });
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
                    UpdateEvent::Status(status) => println!("Status update not handled yet"),
                    UpdateEvent::Title(_) => println!("Title update not handled yet"),
                    UpdateEvent::Tooltip(tooltip) => {
                        if let Some(button) = guard.get(&key) {
                            set_tooltip(button, tooltip.clone(), None);
                        } else {
                            eprintln!("Button for key {} not found", key);
                        }
                    }
                    UpdateEvent::Menu(tray_menu) => {
                        println!("Menu update not handled yet: {:#?}", tray_menu)
                    }
                    UpdateEvent::MenuDiff(menu_diffs) => {
                        println!("Menu diff update not handled yet: {:#?}", menu_diffs)
                    }
                    UpdateEvent::MenuConnect(_) => println!("Menu connect not handled yet"),
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
