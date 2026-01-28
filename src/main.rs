// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

// This is enum for add quality selection
#[derive(Debug, Clone, PartialEq)]
enum VideoQuality {
    Best,
    High1080p,
    Medium720p,
    Low480p,
    AudioOnly,
}

// This is our main application state
// Arc and Mutex allow us to safely share data between threads
struct YouTubeDownloader {
    url: String,                      // The URL input field
    status: Arc<Mutex<String>>,       // Status messages (thread-safe)
    is_downloading: Arc<Mutex<bool>>, // Whether we're currently downloading
    selected_quality: VideoQuality,   // Video quality
    progress: Arc<Mutex<f32>>,
}

const YTDLP_BYTES: &[u8] = include_bytes!("../yt-dlp.exe");
const DENO_BYTES: &[u8] = include_bytes!("../deno.exe");
const FFMPEG_BYTES: &[u8] = include_bytes!("../ffmpeg.exe");

// Default implementation - sets initial values
impl Default for YouTubeDownloader {
    fn default() -> Self {
        Self {
            url: String::new(),
            status: Arc::new(Mutex::new("Ready".to_string())),
            is_downloading: Arc::new(Mutex::new(false)),
            selected_quality: VideoQuality::Best,
            progress: Arc::new(Mutex::new(0.0)),
        }
    }
}

impl VideoQuality {
    fn format_to_ytdlp(&self) -> String {
        match self {
            VideoQuality::Best => "bestvideo+bestaudio/best".to_string(),
            VideoQuality::High1080p => {
                "bestvideo[height<=1080]+bestaudio/best[height<=1080]".to_string()
            }
            VideoQuality::Medium720p => {
                "bestvideo[height<=720]+bestaudio/best[height<=720]".to_string()
            }
            VideoQuality::Low480p => {
                "bestvideo[height<=480]+bestaudio/best[height<=480]".to_string()
            }
            VideoQuality::AudioOnly => "bestaudio/best".to_string(),
        }
    }

    /* fn display_name(&self) -> &str {
        match self {
            VideoQuality::Best => "Best Quality",
            VideoQuality::High1080p => "1080p (Full HD)",
            VideoQuality::Medium720p => "720p (HD)",
            VideoQuality::Low480p => "480p (SD)",
            VideoQuality::AudioOnly => "Audio Only (MP3)",
        }
    } */
}

// This trait tells eframe how to draw and update our app
impl eframe::App for YouTubeDownloader {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Create a central panel (the main window area)
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("YouTube Video Downloader");
            ui.add_space(10.0);

            // Text input for URL
            ui.label("YouTube URL:");
            ui.text_edit_singleline(&mut self.url);
            ui.add_space(10.0);

            // Buttons for quality selections
            ui.label("Select Quality");
            ui.horizontal(|ui| {
                ui.radio_value(
                    &mut self.selected_quality,
                    VideoQuality::Best,
                    "Best Quality",
                );
                ui.radio_value(&mut self.selected_quality, VideoQuality::High1080p, "1080p");
                ui.radio_value(&mut self.selected_quality, VideoQuality::Medium720p, "720p");
                ui.radio_value(&mut self.selected_quality, VideoQuality::Low480p, "480p");
                ui.radio_value(
                    &mut self.selected_quality,
                    VideoQuality::AudioOnly,
                    "Audio Only",
                );
            });
            ui.add_space(10.0);

            // Get download progress
            // let status = self.status.lock().unwrap().clone();
            // let is_downloading = *self.is_downloading.lock().unwrap();
            let progress = *self.progress.lock().unwrap();

            // Get current status (we need to lock the Mutex to read it)
            let status = self.status.lock().unwrap().clone();
            let is_downloading = *self.is_downloading.lock().unwrap();

            // Download button - disabled while downloading
            if ui
                .add_enabled(!is_downloading, egui::Button::new("Download"))
                .clicked()
            {
                self.start_download();
            }

            ui.add_space(10.0);

            // Progress bar
            if is_downloading {
                ui.add(
                    egui::ProgressBar::new(progress / 100.0)
                        .show_percentage()
                        .text(format!("{:.1}%", progress)),
                );
            }

            ui.add_space(5.0);
            ui.label(format!("Status: {}", status));
        });
        if *self.is_downloading.lock().unwrap() {
            ctx.request_repaint();
        }
    }
}

impl YouTubeDownloader {
    fn start_download(&mut self) {
        let url = self.url.clone();

        if url.is_empty() {
            *self.status.lock().unwrap() = "Please enter a URL".to_string();
            return;
        }

        let status = Arc::clone(&self.status);
        let is_downloading = Arc::clone(&self.is_downloading);
        let progress = Arc::clone(&self.progress);
        let quality = self.selected_quality.clone();

        // Reset progress
        *progress.lock().unwrap() = 0.0;

        *is_downloading.lock().unwrap() = true;
        *status.lock().unwrap() = "Starting download...".to_string();

        thread::spawn(move || {
            *status.lock().unwrap() = "Preparing...".to_string();

            // Extract yt-dlp to a temporary location
            let temp_dir = std::env::temp_dir();
            let ytdlp_path = temp_dir.join("yt-dlp.exe");
            let deno_path = temp_dir.join("deno.exe");
            let ffmpeg_path = temp_dir.join("ffmpeg.exe");

            // Extract yt-dlp
            if !ytdlp_path.exists() {
                if let Err(e) = std::fs::write(&ytdlp_path, YTDLP_BYTES) {
                    *status.lock().unwrap() = format!("Failed to extract yt-dlp: {}", e);
                    *is_downloading.lock().unwrap() = false;
                    return;
                }
            }

            // Do same for Deno
            if !deno_path.exists() {
                if let Err(e) = std::fs::write(&deno_path, DENO_BYTES) {
                    *status.lock().unwrap() = format!("Failed to extract deno: {}", e);
                    *is_downloading.lock().unwrap() = false;
                    return;
                }
            }

            // Do same for FFMPEG
            if !ffmpeg_path.exists() {
                if let Err(e) = std::fs::write(&ffmpeg_path, FFMPEG_BYTES) {
                    *status.lock().unwrap() = format!("Failed to extract deno: {}", e);
                    *is_downloading.lock().unwrap() = false;
                    return;
                }
            }

            *status.lock().unwrap() = "Downloading...".to_string();

            // yrdlp commad center
            // Spawn process with stdout piped to us
            let mut child = match Command::new(&ytdlp_path)
                .arg(&url)
                .arg("-f")
                .arg(quality.format_to_ytdlp()) // Use selected quality
                .arg("-o")
                .arg("%(title)s.%(ext)s")
                .arg("--js-runtimes")
                .arg(format!("deno:{}", deno_path.display()))
                .arg("--ffmpeg-location")
                .arg(&ffmpeg_path)
                .arg("--newline") // Force progress on new lines
                .arg("--progress")
                .arg("--verbose")
                .stdout(Stdio::piped()) // Capture stdout
                .stderr(Stdio::piped()) // Capture stderr
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    *status.lock().unwrap() = format!("Failed to start yt-dlp: {}", e);
                    *is_downloading.lock().unwrap() = false;
                    return;
                }
            };

            // Get stdout handle and wrap in BufReader for line-by-line reading
            let stdout = child.stdout.take().unwrap();
            let reader = BufReader::new(stdout);

            // Read output line by line
            for line in reader.lines() {
                if let Ok(line) = line {
                    // Parse progress from yt-dlp output
                    if let Some(percent) = parse_progress(&line) {
                        *progress.lock().unwrap() = percent;
                    }
                }
            }

            match child.wait() {
                Ok(status_code) => {
                    if status_code.success() {
                        *status.lock().unwrap() = "Download complete!".to_string();
                    } else {
                        *status.lock().unwrap() = "Download failed".to_string();
                    }
                }
                Err(e) => {
                    *status.lock().unwrap() = format!("Failed to run yt-dlp: {}", e);
                }
            }

            *is_downloading.lock().unwrap() = false;
        });
    }
}

fn parse_progress(line: &str) -> Option<f32> {
    // yt-dlp outputs progress like: "[download]  45.2% of 123.45MiB at 1.23MiB/s ETA 00:15"

    if line.contains("[download]") && line.contains("%") {
        // Find the percentage value
        let parts: Vec<&str> = line.split_whitespace().collect();
        for part in parts {
            if part.ends_with('%') {
                // Remove the % sign and parse as float
                let percent_str = part.trim_end_matches('%');
                if let Ok(percent) = percent_str.parse::<f32>() {
                    return Some(percent);
                }
            }
        }
    }

    None
}

// Entry point of the program
fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 200.0]),
        ..Default::default()
    };

    eframe::run_native(
        "YouTube Downloader",
        options,
        Box::new(|_cc| Ok(Box::new(YouTubeDownloader::default()))),
    )
}
