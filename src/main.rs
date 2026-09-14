use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use mini_elasticsearch::config::Config;
use mini_elasticsearch::engine::EngineManager;
use mini_elasticsearch::server;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "mini-es", version = "0.1.0", about = "Mini Elasticsearch in Rust")]
struct Cli {
    /// Path to configuration file (TOML)
    #[arg(short, long)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Start the HTTP search engine server
    Serve {
        /// Host address to bind to
        #[arg(long)]
        host: Option<String>,

        /// Port to listen on
        #[arg(short, long)]
        port: Option<u16>,
    },

    /// Bulk index documents from a JSON or JSONL file
    Index {
        /// File containing documents (JSON array or newline-delimited JSON)
        #[arg(short, long)]
        file: PathBuf,

        /// Target index name
        #[arg(short, long, default_value = "default")]
        index: String,
    },

    /// Search an index from the command line
    Search {
        /// Query string (supports phrases "...", prefixes *, fuzzy ~N)
        query: String,

        /// Target index name
        #[arg(short, long, default_value = "default")]
        index: String,

        /// Maximum number of hits to return
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    /// Display statistics for an index
    Stats {
        /// Target index name
        #[arg(short, long, default_value = "default")]
        index: String,
    },

    /// Merge all segments into a single segment (compaction)
    Compact {
        /// Target index name
        #[arg(short, long, default_value = "default")]
        index: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize structured logging
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init();

    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;

    match cli.command.unwrap_or(Commands::Serve { host: None, port: None }) {
        Commands::Serve { host, port } => {
            let bind_host = host.unwrap_or(config.server.host.clone());
            let bind_port = port.unwrap_or(config.server.port);
            let addr = format!("{}:{}", bind_host, bind_port);

            println!("=== Mini Elasticsearch Server ===");
            println!("Starting on {}", addr);
            server::start_server_with_config(&addr, config).await;
        }

        Commands::Index { file, index } => {
            let mut manager = EngineManager::new(config.index.data_dir.clone(), config)?;
            let engine_lock = manager.get_or_create(&index)?;

            let f = File::open(&file)?;
            let reader = BufReader::new(f);
            let mut count = 0;

            // Check if file is JSON array or JSON lines
            let content = std::fs::read_to_string(&file)?;
            let trimmed = content.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let docs: Vec<serde_json::Value> = serde_json::from_str(trimmed)?;
                let mut engine = engine_lock.write().unwrap();
                for doc in docs {
                    if let (Some(id), Some(title), Some(body)) = (
                        doc.get("id").and_then(|v| v.as_str()),
                        doc.get("title").and_then(|v| v.as_str()),
                        doc.get("body").and_then(|v| v.as_str()),
                    ) {
                        engine.add_document(id, title, body);
                        count += 1;
                    }
                }
            } else {
                let mut engine = engine_lock.write().unwrap();
                for line in reader.lines() {
                    let line = line?;
                    let line_str = line.trim();
                    if line_str.is_empty() {
                        continue;
                    }
                    if let Ok(doc) = serde_json::from_str::<serde_json::Value>(line_str) {
                        if let (Some(id), Some(title), Some(body)) = (
                            doc.get("id").and_then(|v| v.as_str()),
                            doc.get("title").and_then(|v| v.as_str()),
                            doc.get("body").and_then(|v| v.as_str()),
                        ) {
                            engine.add_document(id, title, body);
                            count += 1;
                        }
                    }
                }
            }

            {
                let mut engine = engine_lock.write().unwrap();
                let _ = engine.flush();
            }

            println!("Successfully indexed {} documents into index '{}'", count, index);
        }

        Commands::Search { query, index, limit } => {
            let manager = EngineManager::new(config.index.data_dir.clone(), config)?;
            let engine_lock = manager.get(&index).ok_or_else(|| format!("Index '{}' not found", index))?;

            let engine = engine_lock.read().unwrap();
            let results = engine.search(&query);

            println!("Search results for {:?} in index '{}':", query, index);
            println!("Found {} total hits (showing up to {}):\n", results.len(), limit);

            for (i, r) in results.into_iter().take(limit).enumerate() {
                println!("{}. [{:.4}] (id: {}) {}", i + 1, r.score, r.doc_id, r.title);
            }
        }

        Commands::Stats { index } => {
            let manager = EngineManager::new(config.index.data_dir.clone(), config)?;
            let engine_lock = manager.get(&index).ok_or_else(|| format!("Index '{}' not found", index))?;

            let engine = engine_lock.read().unwrap();
            let s = engine.stats();

            println!("Statistics for index '{}':", index);
            println!("- Total Documents: {}", s.total_docs);
            println!("- Total Terms:     {}", s.total_terms);
            println!("- Avg Doc Length:  {:.2} tokens", s.avg_doc_length);
            println!("- Segment Count:   {}", s.segment_count);
        }

        Commands::Compact { index } => {
            let manager = EngineManager::new(config.index.data_dir.clone(), config)?;
            let engine_lock = manager.get(&index).ok_or_else(|| format!("Index '{}' not found", index))?;

            let mut engine = engine_lock.write().unwrap();
            engine.compact()?;
            println!("Compacted segments for index '{}'", index);
        }
    }

    Ok(())
}