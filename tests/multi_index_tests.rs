use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use mini_elasticsearch::config::Config;
use mini_elasticsearch::engine::EngineManager;
use mini_elasticsearch::server::build_manager_router;
use tower::ServiceExt;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mini_es_mindex_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

async fn body_string(body: Body) -> String {
    let bytes = http_body_util::BodyExt::collect(body)
        .await
        .unwrap()
        .to_bytes();
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn test_engine_manager_isolation() {
    let dir = temp_dir("isolation");
    let config = Config::default();
    let mut manager = EngineManager::new(dir.clone(), config).unwrap();

    let books = manager.get_or_create("books").unwrap();
    let movies = manager.get_or_create("movies").unwrap();

    {
        let mut b = books.write().unwrap();
        b.add_document("b1", "Dune", "The spice must flow on Arrakis");
    }

    {
        let mut m = movies.write().unwrap();
        m.add_document("m1", "Star Wars", "In a galaxy far far away");
    }

    // Books search
    {
        let b = books.read().unwrap();
        assert_eq!(b.search("spice").len(), 1);
        assert_eq!(b.search("galaxy").len(), 0);
    }

    // Movies search
    {
        let m = movies.read().unwrap();
        assert_eq!(m.search("galaxy").len(), 1);
        assert_eq!(m.search("spice").len(), 0);
    }

    let _ = fs::remove_dir_all(dir);
}

#[tokio::test]
async fn test_multi_index_http_routes() {
    let dir = temp_dir("http_multi");
    let config = Config::default();
    let manager = Arc::new(RwLock::new(EngineManager::new(dir.clone(), config).unwrap()));
    let app = build_manager_router(manager);

    // 1. Create named index 'articles'
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/indexes/articles")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 2. List indexes
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/indexes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res.into_body()).await;
    assert!(body.contains("articles"));
    assert!(body.contains("default"));

    // 3. Index document into 'articles'
    let doc_json = r#"{"id":"art1","title":"Rust 2026","body":"Rust programming language advancements"}"#;
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/indexes/articles/documents")
                .header("content-type", "application/json")
                .body(Body::from(doc_json))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // 4. Search 'articles'
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/indexes/articles/search?q=advancements")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = body_string(res.into_body()).await;
    assert!(body.contains("art1"));

    // 5. Flush 'articles'
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/indexes/articles/_flush")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // 6. Compact 'articles'
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/indexes/articles/_compact")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let _ = fs::remove_dir_all(dir);
}
