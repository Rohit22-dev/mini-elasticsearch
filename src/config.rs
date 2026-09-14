use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3000
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexConfig {
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,
    #[serde(default = "default_flush_threshold")]
    pub flush_threshold: usize,
    #[serde(default = "default_merge_max_segments")]
    pub merge_max_segments: usize,
    #[serde(default = "default_result_limit")]
    pub default_result_limit: usize,
}

fn default_data_dir() -> PathBuf {
    PathBuf::from("data")
}

fn default_flush_threshold() -> usize {
    1000
}

fn default_merge_max_segments() -> usize {
    10
}

fn default_result_limit() -> usize {
    10
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            flush_threshold: default_flush_threshold(),
            merge_max_segments: default_merge_max_segments(),
            default_result_limit: default_result_limit(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringConfig {
    #[serde(default = "default_bm25_k1")]
    pub bm25_k1: f64,
    #[serde(default = "default_bm25_b")]
    pub bm25_b: f64,
}

fn default_bm25_k1() -> f64 {
    1.2
}

fn default_bm25_b() -> f64 {
    0.75
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            bm25_k1: default_bm25_k1(),
            bm25_b: default_bm25_b(),
        }
    }
}

/// Global engine configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub scoring: ScoringConfig,
}

impl Config {
    /// Load configuration from an optional file path or default search paths (`config.toml`).
    /// If no file is found, returns the default configuration.
    pub fn load(path: Option<&Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = if let Some(p) = path {
            if p.exists() {
                Some(p.to_path_buf())
            } else {
                return Err(format!("Config file not found at {:?}", p).into());
            }
        } else {
            let default_path = Path::new("config.toml");
            if default_path.exists() {
                Some(default_path.to_path_buf())
            } else {
                None
            }
        };

        if let Some(path) = config_path {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }
}
