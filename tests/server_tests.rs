use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use mini_elasticsearch::engine::SearchEngine;
use mini_elasticsearch::server::{build_router, SharedEngine};
use tower::ServiceExt;

fn test_engine() -> SharedEngine {
    Arc::new(RwLock::new(SearchEngine::new()))
}

fn test_app() -> axum::Router {
    build_router(test_engine())
}

async fn body_string(body: Body) -> String {
    let bytes = body.collect().await.unwrap().to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

fn seeded_engine() -> SharedEngine {
    let engine = test_engine();
    {
        let mut e = engine.write().unwrap();
        e.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
        e.add_document("2", "Dragon Tale", "The dragon burned the village");
        e.add_document("3", "Escape Story", "The bride escaped from the ogre");
    }
    engine
}

// --- POST /documents ---

#[tokio::test]
async fn test_index_document() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/documents")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"id": "1", "title": "Ogre Story", "body": "The ogre kidnapped the bride"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "indexed");
    assert_eq!(json["id"], "1");
}

// --- GET /documents/:id ---

#[tokio::test]
async fn test_get_document() {
    let engine = test_engine();
    {
        let mut e = engine.write().unwrap();
        e.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
    }
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/documents/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["id"], "1");
    assert_eq!(json["title"], "Ogre Story");
    assert_eq!(json["body"], "The ogre kidnapped the bride");
}

#[tokio::test]
async fn test_get_document_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/documents/999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- DELETE /documents/:id ---

#[tokio::test]
async fn test_delete_document() {
    let engine = test_engine();
    {
        let mut e = engine.write().unwrap();
        e.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
    }
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/documents/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["status"], "deleted");
    assert_eq!(json["id"], "1");
}

#[tokio::test]
async fn test_delete_document_not_found() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/documents/999")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// --- GET /search ---

#[tokio::test]
async fn test_search() {
    let engine = seeded_engine();
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=ogre")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["query"], "ogre");
    assert_eq!(json["total_hits"], 2);
    assert!(json["took_ms"].is_number());

    let hits = json["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 2);
    // Hits should have id, score, and title
    assert!(hits[0]["id"].is_string());
    assert!(hits[0]["score"].is_number());
    assert!(hits[0]["title"].is_string());
    // Sorted by score descending
    let score0 = hits[0]["score"].as_f64().unwrap();
    let score1 = hits[1]["score"].as_f64().unwrap();
    assert!(score0 >= score1);
}

#[tokio::test]
async fn test_search_with_limit() {
    let engine = seeded_engine();
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=ogre&limit=1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 2); // total hits is still 2
    let hits = json["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1); // but only 1 returned
}

#[tokio::test]
async fn test_search_no_results() {
    let engine = test_engine();
    {
        let mut e = engine.write().unwrap();
        e.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
    }
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=wizard")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 0);
    let hits = json["hits"].as_array().unwrap();
    assert!(hits.is_empty());
}

// --- GET /_stats ---

#[tokio::test]
async fn test_stats() {
    let engine = test_engine();
    {
        let mut e = engine.write().unwrap();
        e.add_document("1", "Ogre Story", "The ogre kidnapped the bride");
        e.add_document("2", "Dragon Tale", "The dragon burned the village");
    }
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/_stats")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_docs"], 2);
    assert!(json["total_terms"].as_u64().unwrap() > 0);
    assert!(json["avg_doc_length"].as_f64().unwrap() > 0.0);
}

// --- Full workflow ---

#[tokio::test]
async fn test_index_then_search_then_delete() {
    let engine = test_engine();
    let app = build_router(engine.clone());

    // Index a document
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/documents")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"id": "1", "title": "Ogre Story", "body": "The ogre kidnapped the bride"}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);

    // Search for it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/search?q=ogre")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 1);

    // Delete it
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/documents/1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Search again — should be gone
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=ogre")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 0);
}

// =========================================================================
// Phase 4 — Phrase search via HTTP
// =========================================================================

#[tokio::test]
async fn test_http_phrase_search() {
    let engine = seeded_engine();
    let app = build_router(engine);

    // URL-encoded: q="ogre kidnapped" → q=%22ogre+kidnapped%22
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=%22ogre+kidnapped%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let hits = json["hits"].as_array().unwrap();
    // Only doc 1 has "ogre kidnapped" as adjacent phrase
    assert!(!hits.is_empty());
    let hit_ids: Vec<&str> = hits.iter().map(|h| h["id"].as_str().unwrap()).collect();
    assert!(hit_ids.contains(&"1"));
}

#[tokio::test]
async fn test_http_phrase_search_no_match() {
    let engine = seeded_engine();
    let app = build_router(engine);

    // "ogre bride" are not adjacent in any document
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=%22ogre+bride%22")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 0);
}

// =========================================================================
// Phase 4 — Prefix search via HTTP
// =========================================================================

#[tokio::test]
async fn test_http_prefix_search() {
    let engine = seeded_engine();
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=ogr*")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let total_hits = json["total_hits"].as_u64().unwrap();
    assert!(total_hits >= 2, "Should match docs containing 'ogre'");
}

#[tokio::test]
async fn test_http_prefix_search_no_match() {
    let engine = seeded_engine();
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=xyz*")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 0);
}

// =========================================================================
// Phase 4 — Fuzzy search via HTTP
// =========================================================================

#[tokio::test]
async fn test_http_fuzzy_search() {
    let engine = seeded_engine();
    let app = build_router(engine);

    // "ogr~1" should match "ogre" (distance 1)
    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=ogr~1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    let total_hits = json["total_hits"].as_u64().unwrap();
    assert!(total_hits >= 2, "Should match docs containing 'ogre'");
}

#[tokio::test]
async fn test_http_fuzzy_search_no_match() {
    let engine = seeded_engine();
    let app = build_router(engine);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/search?q=xyzzy~1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body = body_string(response.into_body()).await;
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["total_hits"], 0);
}
