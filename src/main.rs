// src/main.rs

use axum::{
    routing::get,
    Json,
    Router,
    response::IntoResponse,
    http::StatusCode,  
};
use chrono::{DateTime, FixedOffset};
use reqwest::Error as ReqwestError;
use rss::Error as RssError;
use serde::Serialize;
use std::collections::HashSet;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;
use axum::http::Method;
use tracing::{error, info, instrument, warn};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Serialize, Clone)]
struct AggregatedNewsItem {
    title: String,
    link: String,
    source_name: String,
    publication_date: Option<DateTime<FixedOffset>>,
    description: Option<String>,
}

#[derive(Debug)]
enum NewsError {
    Http(ReqwestError),
    Parse(RssError),
}

impl From<ReqwestError> for NewsError {
    fn from(err: ReqwestError) -> NewsError {
        NewsError::Http(err)
    }
}

impl From<RssError> for NewsError {
    fn from(err: RssError) -> NewsError {
        NewsError::Parse(err)
    }
}

impl IntoResponse for NewsError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            NewsError::Http(err) => {
                error!("External HTTP Error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("External service error: {}", err),
                )
            }
            NewsError::Parse(err) => {
                error!("Feed Parse Error: {}", err);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error parsing feed data: {}", err),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

async fn get_processed_news() -> Result<Vec<AggregatedNewsItem>, NewsError> {
    info!("Processing request to get news...");
    let feed_sources = vec![
        ("BBC Sport - Football", "https://feeds.bbci.co.uk/sport/football/rss.xml"),
        ("Football News Views - PL", "https://www.football-news-views.co.uk/premier-league-rss.xml"),
        ("Football News Views - Spurs", "https://www.football-news-views.co.uk/tottenham-hotspurrss.xml"),
    ];
    let mut all_news_items: Vec<AggregatedNewsItem> = Vec::new();

    for (source_name, url) in feed_sources {
        info!("Fetching from: {} ({})", source_name, url);
        match fetch_and_parse_feed(url, source_name).await {
            Ok(items) => {
                info!("Fetched {} items from {}", items.len(), source_name);
                all_news_items.extend(items);
            }
            Err(e) => {
                error!("Skipping source {}: {:?}", source_name, e);
            }
        }
    }
    info!("Total items before sorting: {}", all_news_items.len());

    all_news_items.sort_by(|a, b| {
        match (a.publication_date, b.publication_date) {
            (Some(date_a), Some(date_b)) => date_b.cmp(&date_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });

    let mut unique_links: HashSet<String> = HashSet::new();
    let deduplicated_news_items: Vec<AggregatedNewsItem> = all_news_items
        .into_iter()
        .filter(|item| unique_links.insert(item.link.clone()))
        .collect();
    info!("Total items after deduplication: {}", deduplicated_news_items.len());
    Ok(deduplicated_news_items)
}

async fn news_api_handler() -> Result<Json<Vec<AggregatedNewsItem>>, NewsError> {
    info!("API request received for /api/news");
    let news_items = get_processed_news().await?;
    info!("Responding with {} news items.", news_items.len());
    Ok(Json(news_items))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("News Aggregator API Server starting up!");

    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5173".parse::<axum::http::HeaderValue>().unwrap()) 
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS]) 
        .allow_headers(tower_http::cors::Any); 
    let app = Router::new()
        .route("/api/news", get(news_api_handler))
        .layer(cors);

    let addr_str = "0.0.0.0:3030";
    let addr: SocketAddr = addr_str.parse().unwrap_or_else(|e| {
        error!("Failed to parse address '{}': {}", addr_str, e);
        panic!("Invalid server address format");
    });
    
    info!("Server configured to listen on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        error!("Failed to bind to address {}: {}", addr, e);
        panic!("Failed to bind server to address");
    });

    if let Err(e) = axum::serve(listener, app.into_make_service()).await {
        error!("Server error: {}", e);
        panic!("Server encountered a fatal error");
    }
}

#[instrument(skip(url, source_name), fields(feed_url = %url, source = %source_name))]
async fn fetch_and_parse_feed(url: &str, source_name: &str) -> Result<Vec<AggregatedNewsItem>, NewsError> {
    info!("Fetching content...");
    let client = reqwest::Client::builder()
        .user_agent("FPL-News-Aggregator/1.0")
        .build()?;
    
    let response = client.get(url).send().await?;

    if !response.status().is_success() {
        error!("Request to {} failed with status: {}", url, response.status());
        return Err(NewsError::Http(response.error_for_status().unwrap_err()));
    }

    let content_bytes = response.bytes().await?;
    let channel = rss::Channel::read_from(&content_bytes[..])?;
    info!("Successfully parsed RSS feed: '{}' from {}", channel.title(), url);

    let news_items: Vec<AggregatedNewsItem> = channel
        .into_items()
        .into_iter()
        .filter_map(|item| {
            let title = match item.title() {
                Some(t) => t.to_string(),
                None => {
                    warn!("RSS item from source {} (URL: {}) has no title. Skipping.", source_name, url);
                    return None;
                }
            };
            let link = match item.link() {
                Some(l) if !l.is_empty() => l.to_string(),
                _ => {
                    warn!("RSS item '{}' from source {} (URL: {}) has no link or empty link. Skipping.", title, source_name, url);
                    return None;
                }
            };

            let parsed_date: Option<DateTime<FixedOffset>> = item.pub_date().and_then(|date_str| {
                DateTime::parse_from_rfc2822(date_str)
                    .or_else(|_| DateTime::parse_from_rfc3339(date_str))
                    .map_err(|e| warn!("Failed to parse date string '{}' from source {} (URL: {}): {}. Using None.", date_str, source_name, url, e))
                    .ok()
            });

            Some(AggregatedNewsItem {
                title,
                link,
                source_name: source_name.to_string(),
                publication_date: parsed_date,
                description: item.description().map(String::from),
            })
        })
        .collect();

    info!("Transformed {} valid items from feed: {}", news_items.len(), url);
    Ok(news_items)
}
