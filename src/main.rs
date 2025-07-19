use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, CssProvider, Label, gdk::Display, Box, Orientation};
use gtk4 as gtk;
use gtk4_layer_shell::{Edge, Layer, LayerShell};

mod system_monitor;
use system_monitor::SystemMonitor;

mod notification_widget;
use notification_widget::NotificationWidget;

fn load_css() {
    let css_provider = CssProvider::new();

    // Load CSS from file
    css_provider.load_from_data(include_str!("style.css"));

    // Apply CSS to the default display
    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

fn main() {
    let app = Application::builder()
        .application_id("org.swordi.BladeBar")
        .build();

    app.connect_activate(|app| {
        load_css();

        let window = ApplicationWindow::builder()
            .application(app)
            .title("Wayland Bar")
            .build();

        // Initialize layer shell for this window
        LayerShell::init_layer_shell(&window);

        // Set the desired layer
        LayerShell::set_layer(&window, Layer::Top);

        // Reserve space so your bar is not covered
        LayerShell::set_exclusive_zone(&window, 30); // height in pixels

        // Anchor to the top, left, right edges
        LayerShell::set_anchor(&window, Edge::Top, true);
        LayerShell::set_anchor(&window, Edge::Left, true);
        LayerShell::set_anchor(&window, Edge::Right, true);

        // Optional: set a fixed height
        window.set_default_size(800, 30); // width x height

        // Create main container
        let main_box = Box::new(Orientation::Horizontal, 10);
        main_box.set_hexpand(true);
        main_box.add_css_class("main-container");

        // Create system monitor widget
        let system_monitor = SystemMonitor::new();
        
        // Create notification widget (if swaync is available)
        let notification_widget = NotificationWidget::new();
        
        // Add some spacing and the widgets to the right side
        let spacer = Label::new(None);
        spacer.set_hexpand(true);
        
        let title_label = Label::new(Some("BladeBar"));
        title_label.add_css_class("title-label");
        
        main_box.append(&title_label);
        main_box.append(&spacer);
        
        // Add notification widget if available
        if let Some(notification) = notification_widget {
            main_box.append(notification.widget());
        }
        
        main_box.append(system_monitor.widget());

        window.set_child(Some(&main_box));
        window.present();
    });

    app.run();
}
