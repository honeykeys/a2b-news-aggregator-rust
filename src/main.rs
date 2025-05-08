use axum::{routing::get, Json, Router}; // Axum components for routing and JSON responses
use chrono::{DateTime, FixedOffset}; // For handling dates and times
use reqwest::Error as ReqwestError; // HTTP client errors
use rss::Error as RssError;         // RSS parsing errors
use serde::Serialize; // For serializing our AggregatedNewsItem to JSON
use std::collections::HashSet; // For deduplication
use std::net::SocketAddr; // For specifying the server address
use std::sync::Arc; // For thread-safe sharing of data (if we add caching later)
use tokio::sync::Mutex; // For mutable shared state (if we add caching later)
use tracing::{error, info, instrument, warn}; // Logging macros
use tracing_subscriber::EnvFilter;    // For configuring logging levels

// Define a common structure for our aggregated news items.
#[derive(Debug, Serialize, Clone)]
struct AggregatedNewsItem {
    title: String,
    link: String,
    source_name: String,
    publication_date: Option<DateTime<FixedOffset>>,
    description: Option<String>,
}

// Define a custom error type
#[derive(Debug)]
enum NewsError {
    Http(ReqwestError),
    Parse(RssError),
    // We could add a variant for server errors if needed
}

// Implement conversions for error types
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

// Implement how NewsError can be converted into an Axum response
// This allows us to use `?` in our handlers and have errors automatically
// converted into appropriate HTTP responses.
impl axum::response::IntoResponse for NewsError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            NewsError::Http(err) => {
                error!("HTTP Error: {}", err);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("External service error: {}", err),
                )
            }
            NewsError::Parse(err) => {
                error!("Parse Error: {}", err);
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Error parsing feed data: {}", err),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": error_message }))).into_response()
    }
}

// This function will contain the core logic for fetching, parsing,
// sorting, and deduplicating news items.
async fn get_processed_news() -> Result<Vec<AggregatedNewsItem>, NewsError> {
    info!("Processing request to get news...");

    let feed_sources = vec![
        ("BBC Sport - Football", "https://feeds.bbci.co.uk/sport/football/rss.xml"),
        ("Football News Views - PL", "https://www.football-news-views.co.uk/premier-league-rss.xml"),
        ("Football News Views - Spurs", "https://www.football-news-views.co.uk/tottenham-hotspurrss.xml"),
    ];

    let mut all_news_items: Vec<AggregatedNewsItem> = Vec::new();

    // In a real application, you might want to fetch these concurrently
    // using something like `futures::future::join_all`.
    // For simplicity, we'll keep it sequential for now.
    for (source_name, url) in feed_sources {
        info!("Fetching feed from: {} ({})", source_name, url);
        match fetch_and_parse_feed(url, source_name).await {
            Ok(items) => {
                info!("Successfully fetched and parsed {} items from {}", items.len(), source_name);
                all_news_items.extend(items);
            }
            Err(e) => {
                // Log the error but continue, so one failing feed doesn't stop others.
                // The error will not be propagated to the client from here directly,
                // but the feed will be missing. Consider if partial data is acceptable.
                error!("Failed to fetch or parse feed from {}: {:?}. Skipping this source.", source_name, e);
            }
        }
    }
    info!("Total aggregated news items before sorting: {}", all_news_items.len());

    all_news_items.sort_by(|a, b| {
        match (a.publication_date, b.publication_date) {
            (Some(date_a), Some(date_b)) => date_b.cmp(&date_a),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    info!("Total items after sorting: {}", all_news_items.len());

    let mut unique_links: HashSet<String> = HashSet::new();
    let mut deduplicated_news_items: Vec<AggregatedNewsItem> = Vec::new();
    for item in all_news_items {
        if unique_links.insert(item.link.clone()) {
            deduplicated_news_items.push(item);
        } else {
            // info!("Duplicate item (link already seen): {} from {}", item.title, item.source_name);
        }
    }
    info!("Total items after deduplication: {}", deduplicated_news_items.len());
    Ok(deduplicated_news_items)
}

// Axum handler function for the /api/news route
async fn news_api_handler() -> Result<Json<Vec<AggregatedNewsItem>>, NewsError> {
    info!("Received request for /api/news");
    let news_items = get_processed_news().await?;
    info!("Successfully processed news, returning {} items.", news_items.len());
    Ok(Json(news_items)) // Serialize the vector of items into a JSON response
}

#[tokio::main]
async fn main() { // Removed Result<(), NewsError> as Axum handles its own lifecycle
    println!("Top of main()");
    panic!("Testing panic");
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    info!("News Aggregator V2.5 API Server starting up!");

    // Define the application routes
    let app = Router::new()
        .route("/api/news", get(news_api_handler)); // GET requests to /api/news will call news_api_handler

    // Define the server address
    // 0.0.0.0 means it will listen on all available network interfaces
    // You can change 3030 to any other port you prefer
    let addr = SocketAddr::from(([0, 0, 0, 0], 3030));
    info!("Server listening on {}", addr);

    // Run the Axum server
    // `axum::serve` is the new way to run servers with Axum 0.7+
    // It requires a `tokio::net::TcpListener`.
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap_or_else(|e| {
        error!("Failed to bind to address {}: {}", addr, e);
        panic!("Failed to bind to address");
    });

    axum::serve(listener, app.into_make_service())
        .await
        .unwrap_or_else(|e| {
            error!("Server error: {}", e);
            panic!("Server error");
        });
}


#[instrument(skip(url, source_name), fields(feed_url = %url, source = %source_name))]
async fn fetch_and_parse_feed(url: &str, source_name: &str) -> Result<Vec<AggregatedNewsItem>, NewsError> {
    info!("Attempting to fetch content.");
    let response = reqwest::get(url).await?;

    if !response.status().is_success() {
        error!("Request failed with status: {}", response.status());
        return Err(NewsError::Http(response.error_for_status().unwrap_err()));
    }

    info!("Successfully fetched content, now attempting to read bytes.");
    let content_bytes = response.bytes().await?;

    info!("Successfully read bytes, now attempting to parse RSS feed.");
    let channel = rss::Channel::read_from(&content_bytes[..])?;
    info!("Successfully parsed RSS feed: '{}'", channel.title());

    let mut news_items: Vec<AggregatedNewsItem> = Vec::new();
    for item in channel.items() {
        let title = item.title().unwrap_or("No Title").to_string();
        let link = item.link().unwrap_or("").to_string();
        if link.is_empty() {
            warn!("RSS item '{}' from source {} has an empty link. It might not be deduplicated correctly or displayed.", title, source_name);
        }
        if item.title().is_none() {
            warn!("RSS item found with no title from source: {}", source_name);
        }

        let parsed_date: Option<DateTime<FixedOffset>> = item.pub_date().and_then(|date_str| {
            match DateTime::parse_from_rfc2822(date_str) {
                Ok(dt) => Some(dt),
                Err(_) => {
                    match DateTime::parse_from_rfc3339(date_str) {
                        Ok(dt) => Some(dt),
                        Err(_) => {
                            warn!("Failed to parse date string '{}' from source {}. Using None.", date_str, source_name);
                            None
                        }
                    }
                }
            }
        });

        news_items.push(AggregatedNewsItem {
            title,
            link,
            source_name: source_name.to_string(),
            publication_date: parsed_date,
            description: item.description().map(String::from),
        });
    }
    info!("Transformed {} items from this feed.", news_items.len());
    Ok(news_items)
}