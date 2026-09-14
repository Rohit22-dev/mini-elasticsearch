use std::sync::{Arc, RwLock};
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::engine::{EngineManager, SearchEngine};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

/// Thread-safe shared search engine, wrapped for axum's async handlers.
pub type SharedEngine = Arc<RwLock<SearchEngine>>;

/// Thread-safe shared engine manager for multi-index deployments.
pub type SharedManager = Arc<RwLock<EngineManager>>;

// ---------------------------------------------------------------------------
// Request / Response schemas
// ---------------------------------------------------------------------------

/// POST /documents — request body.
#[derive(Debug, Deserialize)]
pub struct IndexDocumentRequest {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// POST /documents — response body.
#[derive(Debug, Serialize)]
pub struct IndexDocumentResponse {
    pub status: String,
    pub id: String,
}

/// GET /documents/:id — response body.
#[derive(Debug, Serialize)]
pub struct GetDocumentResponse {
    pub id: String,
    pub title: String,
    pub body: String,
}

/// DELETE /documents/:id — response body.
#[derive(Debug, Serialize)]
pub struct DeleteDocumentResponse {
    pub status: String,
    pub id: String,
}

/// GET /search — query parameters.
#[derive(Debug, Deserialize)]
pub struct SearchQueryParams {
    pub q: String,
    pub limit: Option<usize>,
}

/// A single hit in the search response.
#[derive(Debug, Serialize)]
pub struct SearchHit {
    pub id: String,
    pub score: f64,
    pub title: String,
}

/// GET /search — response body.
#[derive(Debug, Serialize)]
pub struct SearchResponse {
    pub query: String,
    pub total_hits: usize,
    pub hits: Vec<SearchHit>,
    pub took_ms: u128,
}

/// GET /_stats — response body.
#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub total_docs: u64,
    pub total_terms: usize,
    pub avg_doc_length: f64,
    pub segment_count: usize,
}

/// Generic operation response.
#[derive(Debug, Serialize, Deserialize)]
pub struct SimpleStatusResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// GET /indexes response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListIndexesResponse {
    pub indexes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Single-engine route handlers (Used by tests and standalone mode)
// ---------------------------------------------------------------------------

async fn index_document(
    State(engine): State<SharedEngine>,
    Json(req): Json<IndexDocumentRequest>,
) -> (StatusCode, Json<IndexDocumentResponse>) {
    let id = req.id.clone();
    {
        let mut engine = engine.write().unwrap();
        engine.add_document(&req.id, &req.title, &req.body);
    }

    (
        StatusCode::CREATED,
        Json(IndexDocumentResponse {
            status: "indexed".to_string(),
            id,
        }),
    )
}

async fn get_document(
    State(engine): State<SharedEngine>,
    Path(id): Path<String>,
) -> Result<Json<GetDocumentResponse>, StatusCode> {
    let engine = engine.read().unwrap();
    match engine.get_document(&id) {
        Some(doc) => Ok(Json(GetDocumentResponse {
            id: doc.id.clone(),
            title: doc.title.clone(),
            body: doc.body.clone(),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn delete_document(
    State(engine): State<SharedEngine>,
    Path(id): Path<String>,
) -> Result<Json<DeleteDocumentResponse>, StatusCode> {
    let mut engine = engine.write().unwrap();
    if engine.delete_document(&id) {
        Ok(Json(DeleteDocumentResponse {
            status: "deleted".to_string(),
            id,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn search(
    State(engine): State<SharedEngine>,
    Query(params): Query<SearchQueryParams>,
) -> Json<SearchResponse> {
    let start = Instant::now();
    let results = {
        let engine = engine.read().unwrap();
        engine.search(&params.q)
    };
    let took_ms = start.elapsed().as_millis();

    let total_hits = results.len();
    let limit = params.limit.unwrap_or(10);
    let hits: Vec<SearchHit> = results
        .into_iter()
        .take(limit)
        .map(|r| SearchHit {
            id: r.doc_id,
            score: r.score,
            title: r.title,
        })
        .collect();

    Json(SearchResponse {
        query: params.q,
        total_hits,
        hits,
        took_ms,
    })
}

async fn stats(State(engine): State<SharedEngine>) -> Json<StatsResponse> {
    let engine = engine.read().unwrap();
    let s = engine.stats();
    Json(StatsResponse {
        total_docs: s.total_docs,
        total_terms: s.total_terms,
        avg_doc_length: s.avg_doc_length,
        segment_count: s.segment_count,
    })
}

async fn flush_handler(State(engine): State<SharedEngine>) -> (StatusCode, Json<SimpleStatusResponse>) {
    let mut engine = engine.write().unwrap();
    match engine.flush() {
        Ok(opt) => (
            StatusCode::OK,
            Json(SimpleStatusResponse {
                status: "flushed".to_string(),
                details: opt.map(|p| p.to_string_lossy().to_string()),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(e.to_string()),
            }),
        ),
    }
}

async fn compact_handler(State(engine): State<SharedEngine>) -> (StatusCode, Json<SimpleStatusResponse>) {
    let mut engine = engine.write().unwrap();
    match engine.compact() {
        Ok(()) => (
            StatusCode::OK,
            Json(SimpleStatusResponse {
                status: "compacted".to_string(),
                details: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(e.to_string()),
            }),
        ),
    }
}

// ---------------------------------------------------------------------------
// EngineManager Route Handlers (Multi-Index)
// ---------------------------------------------------------------------------

async fn mgr_list_indexes(State(manager): State<SharedManager>) -> Json<ListIndexesResponse> {
    let mgr = manager.read().unwrap();
    Json(ListIndexesResponse {
        indexes: mgr.list_indexes(),
    })
}

async fn mgr_create_index(
    State(manager): State<SharedManager>,
    Path(name): Path<String>,
) -> (StatusCode, Json<SimpleStatusResponse>) {
    let mut mgr = manager.write().unwrap();
    match mgr.get_or_create(&name) {
        Ok(_) => (
            StatusCode::CREATED,
            Json(SimpleStatusResponse {
                status: "created".to_string(),
                details: Some(name),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(e.to_string()),
            }),
        ),
    }
}

async fn mgr_delete_index(
    State(manager): State<SharedManager>,
    Path(name): Path<String>,
) -> (StatusCode, Json<SimpleStatusResponse>) {
    let mut mgr = manager.write().unwrap();
    match mgr.delete_index(&name) {
        Ok(true) => (
            StatusCode::OK,
            Json(SimpleStatusResponse {
                status: "deleted".to_string(),
                details: Some(name),
            }),
        ),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(SimpleStatusResponse {
                status: "not_found".to_string(),
                details: Some(name),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(e.to_string()),
            }),
        ),
    }
}

async fn mgr_index_doc(
    State(manager): State<SharedManager>,
    name: Option<String>,
    req: IndexDocumentRequest,
) -> (StatusCode, Json<IndexDocumentResponse>) {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mut mgr = manager.write().unwrap();
        match mgr.get_or_create(&index_name) {
            Ok(e) => e,
            Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(IndexDocumentResponse { status: "error".to_string(), id: req.id })),
        }
    };

    let id = req.id.clone();
    {
        let mut e = engine.write().unwrap();
        e.add_document(&req.id, &req.title, &req.body);
    }

    (
        StatusCode::CREATED,
        Json(IndexDocumentResponse {
            status: "indexed".to_string(),
            id,
        }),
    )
}

async fn mgr_get_doc(
    State(manager): State<SharedManager>,
    name: Option<String>,
    id: String,
) -> Result<Json<GetDocumentResponse>, StatusCode> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or(StatusCode::NOT_FOUND)?
    };

    let e = engine.read().unwrap();
    match e.get_document(&id) {
        Some(doc) => Ok(Json(GetDocumentResponse {
            id: doc.id.clone(),
            title: doc.title.clone(),
            body: doc.body.clone(),
        })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn mgr_delete_doc(
    State(manager): State<SharedManager>,
    name: Option<String>,
    id: String,
) -> Result<Json<DeleteDocumentResponse>, StatusCode> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or(StatusCode::NOT_FOUND)?
    };

    let mut e = engine.write().unwrap();
    if e.delete_document(&id) {
        Ok(Json(DeleteDocumentResponse {
            status: "deleted".to_string(),
            id,
        }))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn mgr_search_impl(
    State(manager): State<SharedManager>,
    name: Option<String>,
    params: SearchQueryParams,
) -> Result<Json<SearchResponse>, StatusCode> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or(StatusCode::NOT_FOUND)?
    };

    let start = Instant::now();
    let results = {
        let e = engine.read().unwrap();
        e.search(&params.q)
    };
    let took_ms = start.elapsed().as_millis();

    let total_hits = results.len();
    let limit = params.limit.unwrap_or(10);
    let hits: Vec<SearchHit> = results
        .into_iter()
        .take(limit)
        .map(|r| SearchHit {
            id: r.doc_id,
            score: r.score,
            title: r.title,
        })
        .collect();

    Ok(Json(SearchResponse {
        query: params.q,
        total_hits,
        hits,
        took_ms,
    }))
}

async fn mgr_stats_impl(
    State(manager): State<SharedManager>,
    name: Option<String>,
) -> Result<Json<StatsResponse>, StatusCode> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or(StatusCode::NOT_FOUND)?
    };

    let e = engine.read().unwrap();
    let s = e.stats();
    Ok(Json(StatsResponse {
        total_docs: s.total_docs,
        total_terms: s.total_terms,
        avg_doc_length: s.avg_doc_length,
        segment_count: s.segment_count,
    }))
}

async fn mgr_flush_impl(
    State(manager): State<SharedManager>,
    name: Option<String>,
) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(SimpleStatusResponse {
                    status: "not_found".to_string(),
                    details: Some(format!("Index '{}' not found", index_name)),
                }),
            )
        })?
    };

    let mut e = engine.write().unwrap();
    match e.flush() {
        Ok(opt) => Ok(Json(SimpleStatusResponse {
            status: "flushed".to_string(),
            details: opt.map(|p| p.to_string_lossy().to_string()),
        })),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(err.to_string()),
            }),
        )),
    }
}

async fn mgr_compact_impl(
    State(manager): State<SharedManager>,
    name: Option<String>,
) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    let index_name = name.unwrap_or_else(|| "default".to_string());
    let engine = {
        let mgr = manager.read().unwrap();
        mgr.get(&index_name).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(SimpleStatusResponse {
                    status: "not_found".to_string(),
                    details: Some(format!("Index '{}' not found", index_name)),
                }),
            )
        })?
    };

    let mut e = engine.write().unwrap();
    match e.compact() {
        Ok(()) => Ok(Json(SimpleStatusResponse {
            status: "compacted".to_string(),
            details: None,
        })),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SimpleStatusResponse {
                status: "error".to_string(),
                details: Some(err.to_string()),
            }),
        )),
    }
}

// ---------------------------------------------------------------------------
// Manager route wrappers
// ---------------------------------------------------------------------------

async fn default_index_doc(state: State<SharedManager>, Json(req): Json<IndexDocumentRequest>) -> (StatusCode, Json<IndexDocumentResponse>) {
    mgr_index_doc(state, None, req).await
}

async fn default_get_doc(state: State<SharedManager>, Path(id): Path<String>) -> Result<Json<GetDocumentResponse>, StatusCode> {
    mgr_get_doc(state, None, id).await
}

async fn default_delete_doc(state: State<SharedManager>, Path(id): Path<String>) -> Result<Json<DeleteDocumentResponse>, StatusCode> {
    mgr_delete_doc(state, None, id).await
}

async fn default_search(state: State<SharedManager>, Query(params): Query<SearchQueryParams>) -> Result<Json<SearchResponse>, StatusCode> {
    mgr_search_impl(state, None, params).await
}

async fn default_stats(state: State<SharedManager>) -> Result<Json<StatsResponse>, StatusCode> {
    mgr_stats_impl(state, None).await
}

async fn default_flush(state: State<SharedManager>) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    mgr_flush_impl(state, None).await
}

async fn default_compact(state: State<SharedManager>) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    mgr_compact_impl(state, None).await
}

// Named index handlers
async fn named_index_doc(state: State<SharedManager>, Path(name): Path<String>, Json(req): Json<IndexDocumentRequest>) -> (StatusCode, Json<IndexDocumentResponse>) {
    mgr_index_doc(state, Some(name), req).await
}

async fn named_get_doc(state: State<SharedManager>, Path((name, id)): Path<(String, String)>) -> Result<Json<GetDocumentResponse>, StatusCode> {
    mgr_get_doc(state, Some(name), id).await
}

async fn named_delete_doc(state: State<SharedManager>, Path((name, id)): Path<(String, String)>) -> Result<Json<DeleteDocumentResponse>, StatusCode> {
    mgr_delete_doc(state, Some(name), id).await
}

async fn named_search(state: State<SharedManager>, Path(name): Path<String>, Query(params): Query<SearchQueryParams>) -> Result<Json<SearchResponse>, StatusCode> {
    mgr_search_impl(state, Some(name), params).await
}

async fn named_stats(state: State<SharedManager>, Path(name): Path<String>) -> Result<Json<StatsResponse>, StatusCode> {
    mgr_stats_impl(state, Some(name)).await
}

async fn named_flush(state: State<SharedManager>, Path(name): Path<String>) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    mgr_flush_impl(state, Some(name)).await
}

async fn named_compact(state: State<SharedManager>, Path(name): Path<String>) -> Result<Json<SimpleStatusResponse>, (StatusCode, Json<SimpleStatusResponse>)> {
    mgr_compact_impl(state, Some(name)).await
}

// ---------------------------------------------------------------------------
// Routers
// ---------------------------------------------------------------------------

/// Build router for a single `SharedEngine` (used in tests and standalone mode).
pub fn build_router(engine: SharedEngine) -> Router {
    Router::new()
        .route("/documents", post(index_document))
        .route("/documents/{id}", get(get_document).delete(delete_document))
        .route("/search", get(search))
        .route("/_stats", get(stats))
        .route("/_flush", post(flush_handler))
        .route("/_compact", post(compact_handler))
        .with_state(engine)
}

/// Build router for `SharedManager` supporting multi-index and default index routes.
pub fn build_manager_router(manager: SharedManager) -> Router {
    Router::new()
        // Default index routes
        .route("/documents", post(default_index_doc))
        .route("/documents/{id}", get(default_get_doc).delete(default_delete_doc))
        .route("/search", get(default_search))
        .route("/_stats", get(default_stats))
        .route("/_flush", post(default_flush))
        .route("/_compact", post(default_compact))
        // Multi-index management
        .route("/indexes", get(mgr_list_indexes))
        .route("/indexes/{name}", put(mgr_create_index).delete(mgr_delete_index))
        // Named index routes
        .route("/indexes/{name}/documents", post(named_index_doc))
        .route("/indexes/{name}/documents/{id}", get(named_get_doc).delete(named_delete_doc))
        .route("/indexes/{name}/search", get(named_search))
        .route("/indexes/{name}/_stats", get(named_stats))
        .route("/indexes/{name}/_flush", post(named_flush))
        .route("/indexes/{name}/_compact", post(named_compact))
        .with_state(manager)
}

/// Start the HTTP server on the given address with default config.
pub async fn start_server(addr: &str) {
    let config = Config::load(None).unwrap_or_default();
    start_server_with_config(addr, config).await;
}

/// Start the HTTP server on the given address with custom config.
pub async fn start_server_with_config(addr: &str, config: Config) {
    let data_dir = config.index.data_dir.clone();
    let manager = Arc::new(RwLock::new(
        EngineManager::new(data_dir, config).expect("Failed to initialize EngineManager"),
    ));

    let app = build_manager_router(manager);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .unwrap_or_else(|e| panic!("Failed to bind to {}: {}", addr, e));

    tracing::info!("Mini Elasticsearch server listening on {}", addr);
    println!("Mini Elasticsearch listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
