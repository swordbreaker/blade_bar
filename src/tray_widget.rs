use gtk4::prelude::*;
use gtk4::{Box, Button, Image, Orientation};
use glib::timeout_add_local;
use glib::ControlFlow;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::process::Command;

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

        // Start monitoring for tray applications
        tray_widget.start_monitoring();

        tray_widget
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    fn start_monitoring(&self) {
        let items = Arc::clone(&self.items);
        let item_buttons = Arc::clone(&self.item_buttons);
        let container = self.container.clone();

        // Poll for existing tray items periodically (every 5 seconds)
        timeout_add_local(Duration::from_secs(5), move || {
            Self::scan_existing_items(
                Arc::clone(&items),
                Arc::clone(&item_buttons),
                container.clone(),
            );
            ControlFlow::Continue
        });
    }

    fn scan_existing_items(
        items: Arc<Mutex<HashMap<String, TrayItem>>>,
        item_buttons: Arc<Mutex<HashMap<String, Button>>>,
        container: Box,
    ) {
        let mut current_items = HashMap::new();

        // First, check for applications with known tray capability
        let tray_capable_apps = vec![
            // System utilities
            ("nm-applet", "Network Manager", "network-wireless"),
            ("blueman-applet", "Bluetooth Manager", "bluetooth-active"),
            ("pasystray", "PulseAudio System Tray", "audio-volume-high"),
            ("volumeicon", "Volume Icon", "audio-volume-medium"),
            ("cbatticon", "Battery Icon", "battery-good"),
            ("udiskie", "Disk Manager", "drive-removable-media"),
            ("redshift-gtk", "Redshift", "redshift-status-on"),
            ("flameshot", "Flameshot", "flameshot"),
            ("solaar", "Logitech Solaar", "solaar"),
            ("barrier", "Barrier", "barrier"),
            ("synergy", "Synergy", "synergy"),
            
            // Productivity apps
            ("keepassxc", "KeePassXC", "keepassxc"),
            ("bitwarden", "Bitwarden", "bitwarden"),
            ("1password", "1Password", "1password"),
            ("copyq", "CopyQ Clipboard Manager", "edit-copy"),
            ("parcellite", "Parcellite", "edit-paste"),
            ("clipit", "ClipIt", "edit-paste"),
            ("nextcloud", "Nextcloud", "nextcloud"),
            ("dropbox", "Dropbox", "dropbox"),
            ("insync", "Insync", "insync"),
            ("rclone", "RClone", "folder-remote"),
            
            // Communication apps
            ("discord", "Discord", "discord"),
            ("slack", "Slack", "slack"),
            ("telegram-desktop", "Telegram", "telegram"),
            ("signal-desktop", "Signal", "signal"),
            ("whatsapp-for-linux", "WhatsApp", "whatsapp"),
            ("teams", "Microsoft Teams", "teams"),
            ("zoom", "Zoom", "zoom"),
            ("skype", "Skype", "skype"),
            ("element-desktop", "Element", "element"),
            ("thunderbird", "Thunderbird", "thunderbird"),
            ("evolution", "Evolution", "evolution"),
            
            // Media apps
            ("spotify", "Spotify", "spotify"),
            ("vlc", "VLC Media Player", "vlc"),
            ("clementine", "Clementine", "clementine"),
            ("audacious", "Audacious", "audacious"),
            ("banshee", "Banshee", "banshee"),
            ("rhythmbox", "Rhythmbox", "rhythmbox"),
            ("strawberry", "Strawberry", "strawberry"),
            
            // Development apps
            ("code", "Visual Studio Code", "code"),
            ("code-insiders", "VS Code Insiders", "code-insiders"),
            ("atom", "Atom", "atom"),
            ("sublime_text", "Sublime Text", "sublime-text"),
            ("jetbrains-idea", "IntelliJ IDEA", "intellij-idea"),
            ("pycharm", "PyCharm", "pycharm"),
            ("android-studio", "Android Studio", "android-studio"),
            
            // Gaming
            ("steam", "Steam", "steam"),
            ("lutris", "Lutris", "lutris"),
            ("heroic", "Heroic Games Launcher", "heroic"),
            ("minecraft-launcher", "Minecraft", "minecraft"),
            ("discord", "Discord", "discord"),
            
            // Web browsers (some have tray support)
            ("firefox", "Firefox", "firefox"),
            ("chromium", "Chromium", "chromium"),
            ("google-chrome", "Google Chrome", "google-chrome"),
            ("brave", "Brave Browser", "brave-browser"),
            ("opera", "Opera", "opera"),
            ("vivaldi", "Vivaldi", "vivaldi"),
            
            // System monitoring
            ("htop", "Htop", "utilities-system-monitor"),
            ("iotop", "IOTop", "utilities-system-monitor"),
            ("nvidia-settings", "NVIDIA Settings", "nvidia-settings"),
            ("corectrl", "CoreCtrl", "corectrl"),
            
            // Security & VPN
            ("openvpn", "OpenVPN", "network-vpn"),
            ("nordvpn", "NordVPN", "nordvpn"),
            ("expressvpn", "ExpressVPN", "expressvpn"),
            ("mullvad-vpn", "Mullvad VPN", "mullvad"),
            ("protonvpn", "ProtonVPN", "protonvpn"),
            
            // Virtualization
            ("virtualbox", "VirtualBox", "virtualbox"),
            ("vmware", "VMware", "vmware"),
            ("qemu", "QEMU", "qemu"),
            
            // Other utilities
            ("caffeine", "Caffeine", "caffeine-cup-full"),
            ("xfce4-power-manager", "Power Manager", "battery-good"),
            ("print-manager", "Print Manager", "printer"),
            ("blueman-manager", "Blueman", "bluetooth"),
            ("gufw", "Firewall", "security-high"),
        ];

        // Check for all tray-capable applications
        for (process_name, title, icon_name) in tray_capable_apps {
            if Self::is_process_running(process_name) {
                let item = TrayItem {
                    id: process_name.to_string(),
                    title: title.to_string(),
                    icon_name: Some(icon_name.to_string()),
                    icon_pixmap: None,
                    menu: None,
                };
                current_items.insert(process_name.to_string(), item);
            }
        }

        // Additionally, scan for processes that might have tray icons but aren't in our list
        if let Ok(processes) = Self::get_all_gui_processes() {
            for (pid, name, cmd) in processes {
                // Skip if we already have this process
                if current_items.contains_key(&name) {
                    continue;
                }
                
                // Look for processes that might have tray functionality
                if Self::might_have_tray_icon(&name, &cmd) {
                    let item = TrayItem {
                        id: format!("{}:{}", name, pid),
                        title: Self::format_process_title(&name),
                        icon_name: Self::guess_icon_name(&name),
                        icon_pixmap: None,
                        menu: None,
                    };
                    current_items.insert(format!("{}:{}", name, pid), item);
                }
            }
        }

        // Add demo items if no real ones found (for testing)
        if current_items.is_empty() {
            let mock_items = vec![
                ("demo-app1", "Demo System App", "applications-system"),
                ("demo-app2", "Demo Network App", "applications-internet"),
                ("demo-app3", "Demo Media App", "applications-multimedia"),
            ];

            for (id, title, icon_name) in mock_items {
                let item = TrayItem {
                    id: id.to_string(),
                    title: title.to_string(),
                    icon_name: Some(icon_name.to_string()),
                    icon_pixmap: None,
                    menu: None,
                };
                current_items.insert(id.to_string(), item);
            }
        }

        // Update UI on main thread
        Self::update_tray_ui(items, item_buttons, container, current_items);
    }

    fn is_process_running(process_name: &str) -> bool {
        if let Ok(output) = Command::new("pgrep").arg("-f").arg(process_name).output() {
            !output.stdout.is_empty()
        } else {
            false
        }
    }

    fn get_all_gui_processes() -> Result<Vec<(u32, String, String)>, std::io::Error> {
        // Get all processes with their command lines
        let output = Command::new("ps")
            .args(&["axo", "pid,comm,cmd"])
            .output()?;
        
        let mut processes = Vec::new();
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        for line in output_str.lines().skip(1) { // Skip header
            if let Some((pid_str, rest)) = line.split_once(' ') {
                if let Ok(pid) = pid_str.trim().parse::<u32>() {
                    if let Some((comm, cmd)) = rest.trim().split_once(' ') {
                        let comm = comm.trim().to_string();
                        let cmd = cmd.trim().to_string();
                        
                        // Only include GUI applications (those with DISPLAY or running under Wayland)
                        if Self::is_gui_process(&cmd) {
                            processes.push((pid, comm, cmd));
                        }
                    }
                }
            }
        }
        
        Ok(processes)
    }

    fn is_gui_process(cmd: &str) -> bool {
        // Check if this is likely a GUI process
        cmd.contains("DISPLAY") || 
        cmd.contains("WAYLAND") ||
        cmd.contains("--no-sandbox") ||
        cmd.contains("gtk") ||
        cmd.contains("qt") ||
        cmd.contains("electron") ||
        cmd.contains("X11") ||
        cmd.contains("/usr/bin/") && (
            cmd.contains("gnome") ||
            cmd.contains("kde") ||
            cmd.contains("xfce") ||
            cmd.contains("-gtk") ||
            cmd.contains("-qt")
        )
    }

    fn might_have_tray_icon(process_name: &str, cmd: &str) -> bool {
        // Check if this process might have a tray icon
        let tray_indicators = [
            "tray", "systray", "applet", "indicator", "notification",
            "status", "background", "daemon", "service", "manager",
            "gtk", "qt", "electron", "java"
        ];
        
        let name_lower = process_name.to_lowercase();
        let cmd_lower = cmd.to_lowercase();
        
        // Skip obviously non-tray processes
        if name_lower.contains("kernel") || 
           name_lower.contains("kthread") ||
           name_lower.starts_with("dbus") ||
           name_lower.contains("systemd") ||
           name_lower.contains("bash") ||
           name_lower.contains("zsh") ||
           name_lower.contains("fish") {
            return false;
        }
        
        // Check if process name or command contains tray indicators
        tray_indicators.iter().any(|indicator| {
            name_lower.contains(indicator) || cmd_lower.contains(indicator)
        }) ||
        // Or if it's a known GUI application type
        cmd_lower.contains("electron") ||
        cmd_lower.contains("--type=renderer") ||
        (cmd_lower.contains("/usr/bin/") && (
            cmd_lower.contains("gnome") ||
            cmd_lower.contains("kde") ||
            cmd_lower.contains("xfce")
        ))
    }

    fn format_process_title(process_name: &str) -> String {
        // Convert process name to a nice title
        process_name
            .replace("-", " ")
            .replace("_", " ")
            .split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn guess_icon_name(process_name: &str) -> Option<String> {
        // Try to guess appropriate icon names
        let name_lower = process_name.to_lowercase();
        
        if name_lower.contains("network") || name_lower.contains("nm-") {
            Some("network-wireless".to_string())
        } else if name_lower.contains("audio") || name_lower.contains("pulse") || name_lower.contains("volume") {
            Some("audio-volume-high".to_string())
        } else if name_lower.contains("blue") {
            Some("bluetooth".to_string())
        } else if name_lower.contains("battery") || name_lower.contains("power") {
            Some("battery-good".to_string())
        } else if name_lower.contains("file") || name_lower.contains("folder") {
            Some("folder".to_string())
        } else if name_lower.contains("terminal") || name_lower.contains("shell") {
            Some("utilities-terminal".to_string())
        } else if name_lower.contains("text") || name_lower.contains("edit") {
            Some("text-editor".to_string())
        } else if name_lower.contains("web") || name_lower.contains("browser") {
            Some("web-browser".to_string())
        } else if name_lower.contains("media") || name_lower.contains("video") || name_lower.contains("music") {
            Some("applications-multimedia".to_string())
        } else if name_lower.contains("game") {
            Some("applications-games".to_string())
        } else if name_lower.contains("develop") || name_lower.contains("code") {
            Some("applications-development".to_string())
        } else if name_lower.contains("system") || name_lower.contains("manager") {
            Some("applications-system".to_string())
        } else if name_lower.contains("internet") || name_lower.contains("net") {
            Some("applications-internet".to_string())
        } else {
            // Fallback to generic application icon
            Some("application-x-executable".to_string())
        }
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

        // Connect click handler - for now just print info
        let item_title = item.title.clone();
        let item_id = item.id.clone();
        button.connect_clicked(move |_| {
            println!("Clicked tray item: {} ({})", item_title, item_id);
            // In a real implementation, this would try to bring the application to focus
            // or trigger its main window
        });

        button
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
