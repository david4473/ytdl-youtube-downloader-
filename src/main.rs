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

// Embed FFmpeg DLLs - adjust version numbers to match your files
/* const AVCODEC_DLL: &[u8] = include_bytes!("../avcodec-62.dll");
const AVDEVICE_DLL: &[u8] = include_bytes!("../avdevice-62.dll");
const AVFILTER_DLL: &[u8] = include_bytes!("../avfilter-11.dll");
const AVFORMAT_DLL: &[u8] = include_bytes!("../avformat-62.dll");
const AVUTIL_DLL: &[u8] = include_bytes!("../avutil-60.dll");
const SWRESAMPLE_DLL: &[u8] = include_bytes!("../swresample-6.dll");
const SWSCALE_DLL: &[u8] = include_bytes!("../swscale-9.dll"); */

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

        *progress.lock().unwrap() = 0.0;
        *is_downloading.lock().unwrap() = true;
        *status.lock().unwrap() = "Starting download...".to_string();

        thread::spawn(move || {
            *status.lock().unwrap() = "Preparing...".to_string();

            let temp_dir = std::env::temp_dir();
            let ytdlp_path = temp_dir.join("yt-dlp.exe");
            let deno_path = temp_dir.join("deno.exe");
            let ffmpeg_path = temp_dir.join("ffmpeg.exe");

            // Helper to extract files
            let extract_file =
                |path: &std::path::Path, bytes: &[u8], name: &str| -> Result<(), String> {
                    if !path.exists() {
                        std::fs::write(path, bytes)
                            .map_err(|e| format!("Failed to extract {}: {}", name, e))?;
                    }
                    Ok(())
                };

            // Extract yt-dlp
            if let Err(e) = extract_file(&ytdlp_path, YTDLP_BYTES, "yt-dlp") {
                *status.lock().unwrap() = e;
                *is_downloading.lock().unwrap() = false;
                return;
            }

            // Extract Deno
            if let Err(e) = extract_file(&deno_path, DENO_BYTES, "deno") {
                *status.lock().unwrap() = e;
                *is_downloading.lock().unwrap() = false;
                return;
            }

            // Extract FFmpeg
            if let Err(e) = extract_file(&ffmpeg_path, FFMPEG_BYTES, "ffmpeg") {
                *status.lock().unwrap() = e;
                *is_downloading.lock().unwrap() = false;
                return;
            }

            *status.lock().unwrap() = "Downloading...".to_string();

            // Spawn the command
            let mut child = match Command::new(&ytdlp_path)
                .arg(&url)
                .arg("-f")
                .arg(quality.format_to_ytdlp())
                .arg("-o")
                .arg("%(title)s.%(ext)s")
                .arg("--merge-output-format")
                .arg("mp4") // Force MP4 output
                .arg("--remux-video")
                .arg("mp4") // Remux to MP4 if needed
                .arg("--js-runtimes")
                .arg(format!("deno:{}", deno_path.display()))
                .arg("--ffmpeg-location")
                .arg(&ffmpeg_path)
                .arg("--newline")
                .arg("--no-warnings") // Reduce noise in output
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    *status.lock().unwrap() = format!("Failed to start yt-dlp: {}", e);
                    *is_downloading.lock().unwrap() = false;
                    return;
                }
            };

            // Clone for the stderr thread
            let status_clone = Arc::clone(&status);
            let progress_clone = Arc::clone(&progress);

            // Read stdout in main thread
            let stdout = child.stdout.take().unwrap();
            let stdout_handle = thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if let Ok(line) = line {
                        // Update progress
                        if let Some(percent) = parse_progress(&line) {
                            *progress_clone.lock().unwrap() = percent;
                        }

                        // Check for merge/conversion status
                        if line.contains("[Merger]") {
                            *status_clone.lock().unwrap() =
                                "Merging audio and video...".to_string();
                        } else if line.contains("[ExtractAudio]") {
                            *status_clone.lock().unwrap() = "Extracting audio...".to_string();
                        } else if line.contains("[ffmpeg]") && line.contains("Merging") {
                            *status_clone.lock().unwrap() = "Merging streams...".to_string();
                        } else if line.contains("[ffmpeg]") && line.contains("Converting") {
                            *status_clone.lock().unwrap() = "Converting to MP4...".to_string();
                        }
                    }
                }
            });

            // Read stderr in separate thread (to catch any errors)
            let stderr = child.stderr.take().unwrap();
            let mut has_error = false;
            let mut error_message = String::new();

            let stderr_reader = BufReader::new(stderr);
            for line in stderr_reader.lines() {
                if let Ok(line) = line {
                    // Only capture actual errors, not warnings
                    if line.contains("ERROR") {
                        has_error = true;
                        error_message = line.clone();
                    }
                }
            }

            // Wait for stdout thread to finish
            let _ = stdout_handle.join();

            // Wait for process to complete
            match child.wait() {
                Ok(exit_status) => {
                    if exit_status.success() {
                        *status.lock().unwrap() = "Download complete!".to_string();
                        *progress.lock().unwrap() = 100.0;
                    } else if has_error {
                        *status.lock().unwrap() = format!("Download failed: {}", error_message);
                    } else {
                        // Exit code was non-zero but we didn't catch an error
                        *status.lock().unwrap() = "Download completed with warnings".to_string();
                        *progress.lock().unwrap() = 100.0;
                    }
                }
                Err(e) => {
                    *status.lock().unwrap() = format!("Process error: {}", e);
                }
            }

            *is_downloading.lock().unwrap() = false;
        });
    }
}

fn parse_progress(line: &str) -> Option<f32> {
    // yt-dlp outputs progress like: "[download]  45.2% of 123.45MiB at 1.23MiB/s ETA 00:15"

    // Check for merging status
    if line.contains("[Merger]") || line.contains("Merging formats into") {
        return Some(99.0);
    }

    // Check for ffmpeg processing
    if line.contains("[ffmpeg]") {
        return Some(99.5);
    }

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
