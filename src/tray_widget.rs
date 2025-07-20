use gtk4::gdk_pixbuf::{self, Pixbuf};
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Image, Orientation};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use system_tray::client::Client;
use system_tray::client::Event;
use system_tray::error::Error;
use system_tray::item::StatusNotifierItem;
use tokio::sync::broadcast;

use crate::tray_widget;
use crate::tray_widget::controls::create_tray_button;

pub mod controls;

pub struct TrayWidget {
    pub container: GtkBox,
    items: Arc<Mutex<HashMap<String, StatusNotifierItem>>>,
    item_buttons: Arc<Mutex<HashMap<String, Button>>>,
    system_tray_client: Arc<Client>,
    shutdown_tx: broadcast::Sender<()>,
    thread_handle: JoinHandle<()>,
}

impl TrayWidget {
    pub async fn new() -> Result<Arc<Self>, Error> {
        let container = GtkBox::new(Orientation::Horizontal, 5);
        container.add_css_class("tray-widget");

        let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(1);
        let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();

        let client = Arc::new(Client::new().await?);
        let client_copy = Arc::clone(&client);

        let thread_handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");

            rt.block_on(async {
                let mut tray_rx = client_copy.subscribe();
                let initial_items = client_copy.items();

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

        let tray_widget = TrayWidget {
            container,
            items: Arc::new(Mutex::new(HashMap::new())),
            item_buttons: Arc::new(Mutex::new(HashMap::new())),
            system_tray_client: client,
            shutdown_tx: shutdown_tx,
            thread_handle: thread_handle,
        };

        let tray_ptr = Arc::new(tray_widget);
        let tray_ptr2 = Arc::clone(&tray_ptr);

        // Handle events on the main thread
        glib::MainContext::default().spawn_local(async move {
            while let Some(event) = event_rx.recv().await {
                let tray_ptr3 = Arc::clone(&tray_ptr);
                tray_ptr3.handle_tray_event(event);
            }
        });

        Ok(tray_ptr2)
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
            }
            Event::Remove(key) => {
                println!("Tray item removed: {key}");
                self.items.lock().unwrap().remove(&key);
                if let Some(button) = self.item_buttons.lock().unwrap().remove(&key) {
                    self.container.remove(&button);
                }
            }
            Event::Update(key, event) => {
                println!("Tray item updated: {key}: {event:?}");
                // Handle updates if needed
            }
        }
    }

    // TODO drop method to clean up resources
    fn drop(&mut self) {
        // Send shutdown signal to the thread
        let _ = self.shutdown_tx.send(());

        // Wait for the thread to finish
        if let Err(e) = self.thread_handle.join() {
            eprintln!("Error joining tray thread: {:?}", e);
        }

        // Clear items and buttons
        self.items.lock().unwrap().clear();
        self.item_buttons.lock().unwrap().clear();
    }
}
