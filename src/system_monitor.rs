use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};
use glib::timeout_add_local;
use glib::ControlFlow;
use sysinfo::System;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct SystemMonitor {
    pub container: Box,
    cpu_label: Label,
    memory_label: Label,
    temp_label: Label,
    system: Arc<Mutex<System>>,
}

impl SystemMonitor {
    pub fn new() -> Self {
        let container = Box::new(Orientation::Horizontal, 10);
        container.add_css_class("system-monitor");

        // Create labels for each metric
        let cpu_label = Label::new(Some("CPU: ---%"));
        cpu_label.add_css_class("cpu-label");
        
        let memory_label = Label::new(Some("MEM: ---%"));
        memory_label.add_css_class("memory-label");
        
        let temp_label = Label::new(Some("TEMP: ---°C"));
        temp_label.add_css_class("temp-label");

        container.append(&cpu_label);
        container.append(&memory_label);
        container.append(&temp_label);

        let system = Arc::new(Mutex::new(System::new_all()));

        let monitor = SystemMonitor {
            container,
            cpu_label,
            memory_label,
            temp_label,
            system,
        };

        monitor.start_monitoring();
        monitor
    }

    fn start_monitoring(&self) {
        let cpu_label = self.cpu_label.clone();
        let memory_label = self.memory_label.clone();
        let temp_label = self.temp_label.clone();
        let system = self.system.clone();

        // Update every 2 seconds
        timeout_add_local(Duration::from_secs(2), move || {
            if let Ok(mut sys) = system.lock() {
                sys.refresh_all();

                // CPU Usage - average of all CPUs
                if !sys.cpus().is_empty() {
                    let cpu_usage: f32 = sys.cpus().iter()
                        .map(|cpu| cpu.cpu_usage())
                        .sum::<f32>() / sys.cpus().len() as f32;
                    cpu_label.set_text(&format!("CPU: {:.1}%", cpu_usage));
                }

                // Memory Usage
                let total_memory = sys.total_memory();
                let used_memory = sys.used_memory();
                if total_memory > 0 {
                    let memory_percentage = (used_memory as f64 / total_memory as f64) * 100.0;
                    memory_label.set_text(&format!("MEM: {:.1}%", memory_percentage));
                }

                // CPU Temperature - try to read from thermal zones
                let temp = SystemMonitor::get_cpu_temperature();
                if temp > 0.0 {
                    temp_label.set_text(&format!("TEMP: {:.0}°C", temp));
                } else {
                    temp_label.set_text("TEMP: N/A");
                }
            }

            ControlFlow::Continue
        });
    }

    pub fn widget(&self) -> &Box {
        &self.container
    }

    fn get_cpu_temperature() -> f32 {
        use std::fs;
        use std::process::Command;
        
        // Method 1: Try to read CPU temperature from /sys/class/thermal
        for i in 0..10 {
            let thermal_path = format!("/sys/class/thermal/thermal_zone{}/type", i);
            let temp_path = format!("/sys/class/thermal/thermal_zone{}/temp", i);
            
            if let Ok(thermal_type) = fs::read_to_string(&thermal_path) {
                let thermal_type = thermal_type.trim().to_lowercase();
                
                if thermal_type.contains("cpu") || 
                   thermal_type.contains("x86_pkg_temp") ||
                   thermal_type.contains("coretemp") {
                    
                    if let Ok(temp_str) = fs::read_to_string(&temp_path) {
                        if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                            return temp_millic as f32 / 1000.0;
                        }
                    }
                }
            }
        }
        
        // Method 2: Try /sys/class/hwmon
        if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
            for entry in entries {
                if let Ok(entry) = entry {
                    let hwmon_path = entry.path();
                    
                    // Look for temp1_input files
                    let temp_file = hwmon_path.join("temp1_input");
                    if temp_file.exists() {
                        if let Ok(temp_str) = fs::read_to_string(&temp_file) {
                            if let Ok(temp_millic) = temp_str.trim().parse::<i32>() {
                                return temp_millic as f32 / 1000.0;
                            }
                        }
                    }
                }
            }
        }
        
        // Method 3: Try using sensors command
        if let Ok(output) = Command::new("sensors").output() {
            let output_str = String::from_utf8_lossy(&output.stdout);
            for line in output_str.lines() {
                if line.contains("°C") && (line.contains("Core") || line.contains("Package") || line.contains("CPU")) {
                    if let Some(temp_start) = line.find('+') {
                        if let Some(temp_end) = line[temp_start..].find('°') {
                            let temp_str = &line[temp_start + 1..temp_start + temp_end];
                            if let Ok(temp) = temp_str.parse::<f32>() {
                                return temp;
                            }
                        }
                    }
                }
            }
        }
        
        0.0 // Return 0 if no temperature found
    }
}
