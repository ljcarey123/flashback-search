use std::path::PathBuf;
use std::sync::Mutex;

use reqwest::Client;
use rusqlite::Connection;

use crate::integrations::face::FaceEngine;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub http: Client,
    pub data_dir: PathBuf,
    pub face: FaceEngine,
}
