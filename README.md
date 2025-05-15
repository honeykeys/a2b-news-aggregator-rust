# FPL News Aggregator - Rust Microservice

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg?style=for-the-badge)](https://opensource.org/licenses/MIT)
A high-performance microservice built in Rust for fetching, parsing, aggregating, and serving football-related news from multiple RSS feeds. This service is designed to be a component of the larger FPL Assistant project.

**Part of the FPL Assistant Project:**
* **Main Project Repository (MERN App):** [github.com/honeykeys/a2b-mernapp]
* **Live Frontend Application:** [fplandml.netlify.app]

---

## Table of Contents

* [Project Overview](#project-overview)
* [How It Works](#how-it-works)
* [Key Features](#key-features)
* [Technology Stack](#technology-stack)
* [API Endpoint](#api-endpoint)

---

## Project Overview

The FPL News Aggregator is a Rust-based microservice responsible for providing a consolidated stream of football news relevant to Fantasy Premier League managers. It fetches data from pre-configured RSS feeds, processes these articles by parsing, standardizing, sorting by date, and deduplicating them, and then makes this aggregated news feed available via a simple JSON API endpoint.

This service was developed to explore Rust for backend development, leveraging its performance, memory safety, and concurrency features.

---

## How It Works

1.  **RSS Feed Fetching:** The service uses the `reqwest` library to asynchronously fetch XML data from a list of defined football news RSS feeds.
2.  **Parsing:** The fetched XML data is parsed using the `rss` crate to extract individual news items (title, link, publication date, description, etc.).
3.  **Data Transformation & Standardization:** Each item is transformed into a common internal structure (`AggregatedNewsItem`). Publication dates are parsed and standardized using the `chrono` crate.
4.  **Aggregation & Sorting:** Items from all feeds are collected into a single list and sorted by publication date in descending order (newest first).
5.  **Deduplication:** Duplicate articles (identified by their link/URL) are removed to provide a cleaner feed.
6.  **API Serving:** An HTTP API endpoint, built with the `axum` web framework, serves the processed and aggregated news items as a JSON response.
7.  **CORS Handling:** Implemented using `tower-http` to allow requests from specified frontend origins.
8.  **Logging:** Utilizes the `tracing` ecosystem for structured application logging.

---

## Key Features

* **Multi-Source Aggregation:** Fetches news from multiple pre-defined RSS feeds.
* **Asynchronous Operations:** Built with `tokio` and `async/await` for non-blocking I/O when fetching feeds.
* **Data Processing:** Includes parsing, date standardization, sorting, and deduplication.
* **JSON API:** Exposes a single GET endpoint (`/api/news`) to retrieve the aggregated news feed.
* **Error Handling:** Basic error handling for feed fetching and parsing.
* **CORS Enabled:** Configured to allow cross-origin requests from the FPL Assistant frontend.
* **Containerized:** Includes a `Dockerfile` for easy building and deployment.

---

## Technology Stack

* **Language:** Rust (Stable, e.g., 1.79.0+ or 1.85+)
* **Asynchronous Runtime:** Tokio
* **Web Framework:** Axum
* **HTTP Client:** Reqwest
* **RSS Parsing:** `rss` crate
* **Date & Time:** `chrono` crate
* **Serialization/Deserialization:** Serde (for JSON)
* **Logging:** `tracing`, `tracing-subscriber`
* **CORS:** `tower-http`
* **Containerization:** Docker

---

## API Endpoint

* **`GET /api/news`**
    * **Description:** Retrieves the latest aggregated, sorted, and deduplicated football news items.
    * **Response:** A JSON array of `AggregatedNewsItem` objects. Each item includes:
        ```json
        {
          "title": "string",
          "link": "string (URL)",
          "source_name": "string",
          "publication_date": "string (ISO 8601 format, optional)",
          "description": "string (optional)"
        }
        ```
