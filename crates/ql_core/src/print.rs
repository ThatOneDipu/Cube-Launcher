use std::{
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    sync::{LazyLock, Mutex},
};

use chrono::{Datelike, Timelike};

use crate::file_utils;

#[derive(Clone, Copy)]
pub enum LogType {
    Info,
    Error,
    Point,
}

pub struct LoggingState {
    thread: Option<std::thread::JoinHandle<()>>,
    writer: Option<BufWriter<File>>,
    sender: Option<std::sync::mpsc::Sender<String>>,
    pub text: Vec<(String, LogType)>,
}

impl LoggingState {
    #[must_use]
    pub fn create() -> Option<Mutex<LoggingState>> {
        Some(Mutex::new(LoggingState {
            thread: None,
            writer: None,
            sender: None,
            text: Vec::new(),
        }))
    }

    pub fn write_to_storage(&mut self, s: &str, t: LogType) {
        self.text.push((s.to_owned(), t));
    }

    pub fn write_str(&mut self, s: &str, t: LogType) {
        self.write_to_storage(s, t);

        if self.sender.is_none() {
            let (sender, receiver) = std::sync::mpsc::channel::<String>();

            if self.writer.is_none() {
                if let Some(file) = get_logs_file() {
                    self.writer = Some(BufWriter::new(file));
                }
            }

            if let Some(writer) = self.writer.take() {
                let thread = std::thread::spawn(move || {
                    let mut writer = writer;

                    while let Ok(msg) = receiver.recv() {
                        _ = writer.write_all(msg.as_bytes());
                        _ = writer.flush();
                    }
                });
                self.thread = Some(thread);
            }

            self.sender = Some(sender);
        }

        if let Some(sender) = &self.sender {
            _ = sender.send(s.to_owned());
        }
    }

    pub fn finish(&self) {
        if let Some(writer) = &self.writer {
            _ = writer.get_ref().sync_all();
        }
    }
}

fn get_logs_file() -> Option<File> {
    let logs_dir = file_utils::get_launcher_dir().ok()?.join("logs");
    std::fs::create_dir_all(&logs_dir).ok()?;
    let now = chrono::Local::now();
    let log_file_name = format!(
        "{}-{}-{}-{}-{}-{}.log",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute(),
        now.second()
    );
    let log_file_path = logs_dir.join(log_file_name);
    let file = OpenOptions::new()
        .create(true) // Create file if it doesn't exist
        .append(true) // Append to the file instead of overwriting
        .open(&log_file_path)
        .ok()?;
    Some(file)
}

pub static LOGGER: LazyLock<Option<Mutex<LoggingState>>> = LazyLock::new(LoggingState::create);

pub fn print_to_file(msg: &str, t: LogType) {
    if let Some(logger) = LOGGER.as_ref() {
        if let Ok(mut lock) = logger.lock() {
            lock.write_str(msg, t);
        } else {
            eprintln!("ql_core::print::print_to_file(): Logger thread panicked!\n[msg]: {msg}");
        }
    }
}

pub fn logger_finish() {
    if let Some(logger) = LOGGER.as_ref() {
        if let Ok(lock) = logger.lock() {
            lock.finish();
        } else {
            eprintln!("ql_core::print::logger_finish(): Logger thread panicked!");
        }
    }
}

pub fn print_to_storage(msg: &str, t: LogType) {
    if let Some(logger) = LOGGER.as_ref() {
        if let Ok(mut lock) = logger.lock() {
            lock.write_to_storage(msg, t);
        } else {
            eprintln!("ql_core::print::print_to_storage(): Logger thread panicked!");
        }
    }
}

/// Print an informational message.
/// Saved to a log file.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => {{
        let plain_text = format!("[info] {}\n", format_args!($($arg)*));
        println!("{} {}", colored::Colorize::yellow("[info]"), format_args!($($arg)*));
        $crate::print::print_to_file(&plain_text, $crate::print::LogType::Info);
    }};
}

/// Print an informational message.
/// Not saved to a log file.
#[macro_export]
macro_rules! info_no_log {
    ($($arg:tt)*) => {{
        let plain_text = format!("[info] {}\n", format_args!($($arg)*));
        println!("{} {}", colored::Colorize::yellow("[info]"), format_args!($($arg)*));
        $crate::print::print_to_storage(&plain_text, $crate::print::LogType::Info);
    }};
}

/// Print an error message.
/// Not saved to a log file.
#[macro_export]
macro_rules! err_no_log {
    ($($arg:tt)*) => {{
        let plain_text = format!("[error] {}\n", format_args!($($arg)*));
        eprintln!("{} {}", colored::Colorize::red("[error]"), format_args!($($arg)*));
        $crate::print::print_to_storage(&plain_text, $crate::print::LogType::Error);
    }};
}

/// Print an error message.
/// Saved to a log file.
#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {{
        let plain_text = format!("[error] {}\n", format_args!($($arg)*));
        eprintln!("{} {}", colored::Colorize::red("[error]"), format_args!($($arg)*));
        $crate::print::print_to_file(&plain_text, $crate::print::LogType::Error);
    }};
}

/// Print a point message, ie. a small step in some process.
/// Saved to a log file.
#[macro_export]
macro_rules! pt {
    ($($arg:tt)*) => {{
        let plain_text = format!("- {}\n", format_args!($($arg)*));
        println!("{} {}", colored::Colorize::bold("-"), format_args!($($arg)*));
        $crate::print::print_to_file(&plain_text, $crate::print::LogType::Point);
    }};
}
