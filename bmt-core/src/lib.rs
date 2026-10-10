//! Bulk Model Test - lõi đo thời gian phản hồi của mọi model trên một endpoint
//! OpenAI-compatible.
//!
//! Thư viện này dùng chung cho ứng dụng Tauri (`src-tauri`) và bản dòng lệnh
//! (`src/bin/bmt-cli.rs`), nên toàn bộ logic nằm ở đây chứ không nằm trong lớp
//! giao diện.

pub mod api;
pub mod config;
pub mod params;
pub mod profiles;
pub mod runner;
pub mod service;
