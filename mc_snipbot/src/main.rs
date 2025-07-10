use chrono::{DateTime, Utc};
use eframe::{egui, App, Frame, CreationContext};
use reqwest::Client;
use serde_json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use tokio::runtime::Runtime;
use tokio::time::sleep;
use egui::{Button, Vec2, ViewportBuilder, TextEdit, Slider};

const TIMESTAMP_RELEASE: i64 = 1752474234;

#[derive(Default, Clone)]
struct LogLine {
    msg: String,
}

#[derive(Clone)]
struct TimeSync {
    time: DateTime<Utc>,
    latency: Duration,
    source: String,
}

#[derive(Default)]
struct SnipeApp {
    logs: Arc<Mutex<Vec<LogLine>>>,
    status: String,
    username: String,
    access_token: String,
    is_running: bool,
    ms_offset: i64,
    custom_release_time: String,
    current_utc_time: DateTime<Utc>,
    burst_count: u8, 
}

impl SnipeApp {
    fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::new())),
            status: "Ready".to_string(),
            username: String::new(),
            access_token: String::new(),
            is_running: false,
            ms_offset: 20,
            custom_release_time: String::new(),
            current_utc_time: Utc::now(),
            burst_count: 5, 
        }
    }

    fn log(&self, text: impl Into<String>) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(LogLine { msg: text.into() });
    }

    async fn get_reliable_time(&self) -> Result<DateTime<Utc>, String> {
        let time_sources = vec![
            ("https://api.frankfurter.app/latest", "frankfurter.app"),
        ];
        
        self.log("🌐 Synchronizing time from reliable sources...".to_string());
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(6))
            .connect_timeout(Duration::from_secs(4))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36")
            .tcp_keepalive(Duration::from_secs(30))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(10)
            .build()
            .map_err(|e| format!("HTTP client error: {}", e))?;

        let mut successful_syncs = Vec::new();
        let mut time_samples = Vec::new();

        for (url, name) in time_sources {
            self.log(format!("🔍 Trying {}...", name));
            
            let start_time = std::time::Instant::now();
            
            match client.get(url).send().await {
                Ok(response) => {
                    let network_latency = start_time.elapsed();
                    
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(text) => {
                                if let Ok(time) = self.parse_time_from_json(&text, name) {
                                    let compensated_time = time + chrono::Duration::from_std(network_latency / 2)
                                        .unwrap_or(chrono::Duration::zero());
                                    
                                    let time_sync = TimeSync {
                                        time: compensated_time,
                                        latency: network_latency,
                                        source: name.to_string(),
                                    };
                                    
                                    successful_syncs.push(time_sync.clone());
                                    time_samples.push(compensated_time);
                                    
                                    self.log(format!("✅ {} sync OK - {} (latency: {}ms)", 
                                                   name, 
                                                   compensated_time.format("%H:%M:%S.%3f UTC"),
                                                   network_latency.as_millis()));

                                    if successful_syncs.len() >= 5 {
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                self.log(format!("⚠️ {} response read error: {}", name, e));
                            }
                        }
                    } else {
                        self.log(format!("⚠️ {} HTTP error: {}", name, response.status()));
                    }
                }
                Err(e) => {
                    self.log(format!("⚠️ {} connection failed: {}", name, e));
                }
            }
        }

        if !successful_syncs.is_empty() {
            successful_syncs.sort_by(|a, b| a.latency.cmp(&b.latency));
            
            let best_sync = &successful_syncs[0];
            
            self.log(format!("🏆 Best server selected: {} (latency: {}ms)", 
                           best_sync.source, best_sync.latency.as_millis()));
            
            let time_differences: Vec<i64> = successful_syncs.iter()
                .map(|sync| (sync.time.timestamp_millis() - best_sync.time.timestamp_millis()).abs())
                .collect();
            
            let max_difference = time_differences.iter().max().unwrap_or(&0);
            
            if *max_difference > 2000 { 
                self.log(format!("⚠️ Large time difference detected ({}ms), using average instead", max_difference));
                
                let avg_timestamp = time_samples.iter()
                    .map(|t| t.timestamp_millis())
                    .sum::<i64>() / time_samples.len() as i64;
                    
                let averaged_time = DateTime::from_timestamp_millis(avg_timestamp)
                    .unwrap_or_else(|| Utc::now());
                    
                self.log(format!("📊 Time sync completed: {} sources, averaged time: {}", 
                               successful_syncs.len(), averaged_time.format("%H:%M:%S.%3f UTC")));
                
                return Ok(averaged_time);
            } else {
                self.log(format!("📊 Time sync completed: {} sources, using best server ({}ms max diff)", 
                               successful_syncs.len(), max_difference));
                
                return Ok(best_sync.time);
            }
        }

        self.log("⚠️ All time APIs failed - using system time (less precise)".to_string());
        Ok(Utc::now())
    }

    fn parse_time_from_json(&self, json_text: &str, source: &str) -> Result<DateTime<Utc>, String> {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_text) {
            let date_fields = ["dateTime", "datetime", "currentDateTime", "utc_datetime", "time", "current_time"];
            
            for field in &date_fields {
                if let Some(datetime_str) = json[field].as_str() {
                    if let Ok(dt) = DateTime::parse_from_rfc3339(datetime_str) {
                        return Ok(dt.with_timezone(&Utc));
                    }
                }
            }
            if let Some(timestamp) = json["unixtime"].as_i64() {
                if let Some(dt) = DateTime::from_timestamp(timestamp, 0) {
                    return Ok(dt);
                }
            }
            if let Some(timestamp_ms) = json["timestamp"].as_i64() {
                if let Some(dt) = DateTime::from_timestamp_millis(timestamp_ms) {
                    return Ok(dt);
                }
            }
        }
        if source.contains("github.com") || source.contains("httpbin.org") || 
           source.contains("coindesk.com") || source.contains("exchangerate-api.com") ||
           source.contains("frankfurter.app") || source.contains("fixer.io") ||
           source.contains("jsonplaceholder.typicode.com") || source.contains("httpstat.us") ||
           source.contains("quotable.io") || source.contains("adviceslip.com") ||
           source.contains("chucknorris.io") || source.contains("official-joke-api") ||
           source.contains("openweathermap.org") || source.contains("sunrise-sunset.org") {
            return Ok(Utc::now());
        }
        
        Err(format!("Cannot parse time from {}", source))
    }

    fn get_release_timestamp(&self) -> i64 {
        if !self.custom_release_time.trim().is_empty() {
            if let Ok(parsed_time) = DateTime::parse_from_rfc3339(&format!("{}Z", self.custom_release_time)) {
                return parsed_time.timestamp();
            }
        }
        TIMESTAMP_RELEASE
    }

    fn start_snipe(&mut self) {
        if self.username.trim().is_empty() {
            self.log("❌ Please enter a username to snipe!");
            return;
        }
        
        if self.access_token.trim().is_empty() {
            self.log("❌ Please enter your Minecraft access token!");
            return;
        }

        if self.custom_release_time.trim().is_empty() {
            self.log("❌ Please enter a custom release time!");
            return;
        }

        if DateTime::parse_from_rfc3339(&format!("{}Z", self.custom_release_time)).is_err() {
            self.log("❌ Invalid time format! Use YYYY-MM-DDTHH:MM:SS");
            return;
        }

        self.status = "In progress...".into();
        self.is_running = true;
        let logs = self.logs.clone();
        let username = self.username.clone();
        let access_token = self.access_token.clone();
        let ms_offset = self.ms_offset;
        let release_timestamp = self.get_release_timestamp();
        
        std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                let snipe_client = Client::builder()
                    .timeout(Duration::from_secs(10))
                    .connect_timeout(Duration::from_secs(5))
                    .user_agent("SnipeBot/1.0")
                    .build()
                    .unwrap();
                let release_time = UNIX_EPOCH + Duration::from_secs(release_timestamp as u64);
                let release_dt: DateTime<Utc> = release_time.into();
                let first_request_time_ms = release_dt.timestamp_millis() - ms_offset;
                logs.lock().unwrap().push(LogLine {
                    msg: format!("🎯 Target username: {}", username),
                });
                
                logs.lock().unwrap().push(LogLine {
                    msg: format!("🎯 Target release time: {}", release_dt.format("%Y-%m-%d %H:%M:%S UTC")),
                });
                logs.lock().unwrap().push(LogLine {
                    msg: format!("🚀 First request will be sent at: {} ({}ms before release)", 
                               DateTime::from_timestamp_millis(first_request_time_ms).unwrap().format("%H:%M:%S.%3f UTC"),
                               ms_offset),
                });
                
                let mut cached_time: Option<DateTime<Utc>> = None;
                let mut last_sync = std::time::Instant::now();
                let mut last_log_time = std::time::Instant::now();
                let sync_interval = Duration::from_secs(45); 
                
                loop {
                    let current_time = {
                        let need_sync = cached_time.is_none() || last_sync.elapsed() > sync_interval;                      
                        if need_sync {
                            let temp_app = SnipeApp { 
                                logs: logs.clone(), 
                                status: String::new(),
                                username: String::new(),
                                access_token: String::new(),
                                is_running: false,
                                ms_offset: 0,
                                custom_release_time: String::new(),
                                current_utc_time: Utc::now(),
                                burst_count: 0, 
                            };                            
                            match temp_app.get_reliable_time().await {
                                Ok(synced_time) => {
                                    cached_time = Some(synced_time);
                                    last_sync = std::time::Instant::now();
                                    logs.lock().unwrap().push(LogLine {
                                        msg: format!("🔄 Time synchronized to optimal server: {}", synced_time.format("%H:%M:%S.%3f UTC")),
                                    });
                                    synced_time
                                }
                                Err(e) => {
                                    logs.lock().unwrap().push(LogLine {
                                        msg: format!("⚠️ Sync failed: {}", e),
                                    });                                    
                                    if let Some(cached) = cached_time {
                                        let elapsed = last_sync.elapsed();
                                        let calculated_time = cached + chrono::Duration::from_std(elapsed).unwrap_or(chrono::Duration::zero());
                                        logs.lock().unwrap().push(LogLine {
                                            msg: format!("🕐 Using cached server time + {}ms offset", elapsed.as_millis()),
                                        });
                                        calculated_time
                                    } else {
                                        let fallback_time = Utc::now();
                                        logs.lock().unwrap().push(LogLine {
                                            msg: "⚠️ No cached time available, using system time".to_string(),
                                        });
                                        fallback_time
                                    }
                                }
                            }
                        } else {
                            let elapsed = last_sync.elapsed();
                            cached_time.unwrap() + chrono::Duration::from_std(elapsed).unwrap_or(chrono::Duration::zero())
                        }
                    };

                    let current_time_ms = current_time.timestamp_millis();
                    let time_diff_ms = first_request_time_ms - current_time_ms;
                    
                    let log_interval = if time_diff_ms < 5000 {
                        Duration::from_millis(100)  
                    } else if time_diff_ms < 30000 {
                        Duration::from_millis(1000)
                    } else if time_diff_ms < 120000 {
                        Duration::from_millis(3000) 
                    } else {
                        Duration::from_millis(5000)
                    };
                    
                    let should_log = time_diff_ms <= 0 || last_log_time.elapsed() >= log_interval;
                    if should_log {
                        last_log_time = std::time::Instant::now();
                        let time_remaining = if time_diff_ms > 0 {
                            if time_diff_ms >= 60000 {
                                format!("{}min {}s", time_diff_ms / 60000, (time_diff_ms % 60000) / 1000)
                            } else if time_diff_ms >= 1000 {
                                format!("{}.{}s", time_diff_ms / 1000, (time_diff_ms % 1000) / 100)
                            } else {
                                format!("{}ms", time_diff_ms)
                            }
                        } else {
                            "FIRING!".to_string()
                        };

                        logs.lock().unwrap().push(LogLine {
                            msg: format!("⏱️ Now: {} | Time to snipe: {}", 
                                       current_time.format("%H:%M:%S.%3f"), time_remaining),
                        });
                    }
                    
                    if current_time_ms >= first_request_time_ms {
                        logs.lock().unwrap().push(LogLine {
                            msg: format!("🚀 LAUNCHING SNIPE ATTACK NOW! ({}ms before release)", ms_offset),
                        });
                        
                        let precise_attack_time = DateTime::from_timestamp_millis(first_request_time_ms)
                            .unwrap_or(current_time);
                        
                        logs.lock().unwrap().push(LogLine {
                            msg: format!("📡 Attack time based on: {} | Precise time: {}", 
                                       if cached_time.is_some() { "Optimal server time" } else { "System time" },
                                       precise_attack_time.format("%H:%M:%S.%3f UTC")),
                        });
                        
                        // Wait until the precise moment if we're still early
                        let wait_time_ms = first_request_time_ms - current_time_ms;
                        if wait_time_ms > 0 {
                            sleep(Duration::from_millis(wait_time_ms as u64)).await;
                        }
                        
                        let mut tasks = vec![];
                        for attempt in 1..=8 {
                            let client = snipe_client.clone();
                            let logs = logs.clone();
                            let request_time = precise_attack_time;
                            let username_clone = username.clone();
                            let token_clone = access_token.clone();
                            let task = tokio::spawn(async move {
                                let actual_send_time = Utc::now();
                                let url = format!(
                                    "https://api.minecraftservices.com/minecraft/profile/name/{}",
                                    username_clone
                                );
                                let response = client
                                    .put(&url)
                                    .header("Authorization", format!("Bearer {}", token_clone))
                                    .header("Content-Type", "application/json")
                                    .body("{}")
                                    .send()
                                    .await;
                                let result = match response {
                                    Ok(resp) => {
                                        let status = resp.status();
                                        if status.is_success() {
                                            format!("[#{:02}] 🎉 SUCCESS! Status: {} | Planned: {} | Actual: {}", 
                                                   attempt, status, request_time.format("%H:%M:%S.%3f"), actual_send_time.format("%H:%M:%S.%3f"))
                                        } else {
                                            format!("[#{:02}] ❌ Failed - Status: {} | Planned: {} | Actual: {}", 
                                                   attempt, status, request_time.format("%H:%M:%S.%3f"), actual_send_time.format("%H:%M:%S.%3f"))
                                        }
                                    }
                                    Err(e) => format!("[#{:02}] ❌ Network error: {} | Planned: {} | Actual: {}", 
                                                     attempt, e, request_time.format("%H:%M:%S.%3f"), actual_send_time.format("%H:%M:%S.%3f")),
                                };
                                logs.lock().unwrap().push(LogLine { msg: result });
                            });
                            tasks.push(task);
                            sleep(Duration::from_millis(2)).await;
                        }
                        
                        for task in tasks {
                            let _ = task.await;
                        }
                        
                        let final_time = Utc::now();
                        let time_source_final = if cached_time.is_some() { "optimal server" } else { "system" };
                        logs.lock().unwrap().push(LogLine {
                            msg: format!("🏁 Snipe sequence completed at: {} ({}) | Actual offset: {}ms", 
                                       final_time.format("%H:%M:%S.%3f"),
                                       time_source_final,
                                       final_time.timestamp_millis() - release_dt.timestamp_millis()),
                        });
                        break;
                    }
                    
                    if time_diff_ms < 1000 {
                        sleep(Duration::from_millis(1)).await;   
                    } else if time_diff_ms < 5000 {
                        sleep(Duration::from_millis(10)).await;  
                    } else if time_diff_ms < 30000 {
                        sleep(Duration::from_millis(100)).await; 
                    } else {
                        sleep(Duration::from_secs(1)).await;
                    }
                }
                
                // Reset status when done
                logs.lock().unwrap().push(LogLine {
                    msg: "🔄 Snipe mission completed. Ready for next operation.".to_string(),
                });
            });
        });
    }
}

impl App for SnipeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Update current UTC time
        self.current_utc_time = Utc::now();
        
        // Reset status when not running
        if !self.is_running {
            if self.status == "In progress..." {
                self.status = "Ready".to_string();
            }
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("🎯 SnipeBot - Minecraft Username Sniper");
            ui.add_space(10.0);
            
            // Info section
            ui.group(|ui| {
                ui.label("ℹ️ Practical information");
                ui.separator();
                ui.label("• Automatic synchronization with optimal server selection (minimal latency)");
                ui.label("• Time consistency check between servers");
                ui.label("• Sends 5 simultaneous requests to maximize chances");
                ui.label("• Customizable timing from 0 to 1000ms before release");
                ui.label("• Enter the release time in format YYYY-MM-DDTHH:MM:SS");
                ui.label("• Access token: Obtain it from your Minecraft profile");
                ui.label("• Refer to the current UTC time for timing");
            });
            ui.add_space(10.0);
            
            ui.group(|ui| {
                ui.label("⚙️ Configuration");
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    ui.label("🎮 Username to snipe:");
                    ui.add(TextEdit::singleline(&mut self.username)
                        .hint_text("Enter username here...")
                        .desired_width(150.0));
                });
                
                ui.horizontal(|ui| {
                    ui.label("🔑 Access Token:");
                    ui.add(TextEdit::singleline(&mut self.access_token)
                        .hint_text("Enter your Minecraft token...")
                        .password(true)
                        .desired_width(200.0));
                });
                
                ui.horizontal(|ui| {
                    ui.label("⏱️ Timing first request:");
                    ui.add(Slider::new(&mut self.ms_offset, 0..=1000)
                        .suffix(" ms")
                        .text("ms before release"));
                });
                
                ui.horizontal(|ui| {
                    ui.label("🕐 Custom release time:");
                    ui.add(TextEdit::singleline(&mut self.custom_release_time)
                        .hint_text("YYYY-MM-DDTHH:MM:SS")
                        .desired_width(200.0));
                });
                
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    ui.label("🌐 Current UTC time:");
                    ui.monospace(self.current_utc_time.format("%Y-%m-%d %H:%M:%S UTC").to_string());
                });
            });
            
            ui.add_space(15.0);
            ui.spacing_mut().button_padding = Vec2::new(25.0, 15.0);
            
            let button_text = if self.is_running {
                "🔄 Snipe in Progress..."
            } else {
                "🚀 Start Snipe Mission"
            };
            
            let start_button = Button::new(button_text)
                .min_size(Vec2::new(200.0, 50.0))
                .fill(if self.is_running { 
                    egui::Color32::from_rgb(100, 100, 100) 
                } else { 
                    egui::Color32::from_rgb(0, 150, 0) 
                });
            
            ui.add_enabled_ui(!self.is_running, |ui| {
                if ui.add(start_button).clicked() {
                    self.start_snipe();
                }
            });
            
            ui.add_space(10.0);
            
            // Show release time info if custom time is set
            if !self.custom_release_time.trim().is_empty() {
                if let Ok(parsed_time) = DateTime::parse_from_rfc3339(&format!("{}Z", self.custom_release_time)) {
                    let release_utc = parsed_time.with_timezone(&Utc);
                    let time_diff = release_utc.signed_duration_since(self.current_utc_time);

                    ui.horizontal(|ui| {
                        ui.label("⏰ Release in:");
                        let time_text = if time_diff.num_seconds() > 0 {
                            if time_diff.num_hours() > 0 {
                                format!("{}h {}m {}s", time_diff.num_hours(), time_diff.num_minutes() % 60, time_diff.num_seconds() % 60)
                            } else if time_diff.num_minutes() > 0 {
                                format!("{}m {}s", time_diff.num_minutes(), time_diff.num_seconds() % 60)
                            } else {
                                format!("{}s", time_diff.num_seconds())
                            }
                        } else {
                            "PAST TIME".to_string()
                        };
                        ui.colored_label(
                            if time_diff.num_seconds() > 0 { egui::Color32::GREEN } else { egui::Color32::RED },
                            time_text
                        );
                    });
                }
            }
            
            ui.horizontal(|ui| {
                ui.label("📊 Status:");
                ui.colored_label(
                    if self.is_running { egui::Color32::YELLOW } else { egui::Color32::WHITE },
                    &self.status
                );
            });
            
            ui.separator();
            ui.label("📋 Activity Log:");
            
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .max_height(350.0)
                .show(ui, |ui| {
                    let logs = self.logs.lock().unwrap();
                    for log in logs.iter().rev().take(50) {
                        ui.label(&log.msg);
                    }
                });
        });
        
        // Auto-reset is_running flag when task completes
        {
            let logs = self.logs.lock().unwrap();
            if let Some(last_log) = logs.last() {
                if last_log.msg.contains("Snipe mission completed") && self.is_running {
                    self.is_running = false;
                }
            }
        }
        
        ctx.request_repaint_after(Duration::from_millis(50));
    }
}

fn main() -> eframe::Result<()> {
    let app = SnipeApp::new();
    
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([650.0, 850.0])
            .with_title("SnipeBot - Minecraft Username Sniper"),
        ..Default::default()
    };
    
    eframe::run_native(
        "SnipeBot - Minecraft Username Sniper",
        options,
        Box::new(|_cc: &CreationContext| Box::new(app)),
    )
}
