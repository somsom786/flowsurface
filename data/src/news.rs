//! News data structures for Tree of Alpha integration
//!
//! This module contains types for representing news items received from
//! the Tree of Alpha WebSocket API.

use serde::{Deserialize, Serialize};

/// A news item received from Tree of Alpha
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItem {
    /// Unique identifier for the news item
    #[serde(rename = "_id")]
    pub id: String,
    /// News title (for Twitter, this is the account name)
    pub title: String,
    /// News content/body
    pub body: String,
    /// Timestamp in milliseconds
    pub time: u64,
    /// Source of the news (Blogs, Terminal, Binance EN, Upbit, etc.)
    #[serde(default)]
    pub source: Option<String>,
    /// Primary coin mentioned (if detected)
    #[serde(default)]
    pub coin: Option<String>,
    /// Icon URL (useful for Twitter account icons)
    #[serde(default)]
    pub icon: Option<String>,
    /// Link to the original source
    #[serde(default)]
    pub link: Option<String>,
    /// Image URL if the news contains an image
    #[serde(default)]
    pub image: Option<String>,
    /// Automatically detected coins in the news with trading symbols
    #[serde(default)]
    pub suggestions: Vec<NewsSuggestion>,
    /// News type (e.g., "direct" for Twitter)
    #[serde(rename = "type", default)]
    pub news_type: Option<String>,
    /// Additional Twitter-specific info
    #[serde(default)]
    pub info: Option<TwitterInfo>,
}

/// Twitter-specific metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TwitterInfo {
    #[serde(rename = "isQuote", default)]
    pub is_quote: bool,
    #[serde(rename = "isReply", default)]
    pub is_reply: bool,
    #[serde(rename = "isRetweet", default)]
    pub is_retweet: bool,
    #[serde(rename = "twitterId", default)]
    pub twitter_id: Option<String>,
}

/// A suggested coin found in the news with available trading symbols
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSuggestion {
    /// The coin symbol (e.g., "BTC", "ETH")
    pub coin: String,
    /// Text fragments that matched this coin
    #[serde(default)]
    pub found: Vec<String>,
    /// Available trading symbols on various exchanges
    #[serde(default)]
    pub symbols: Vec<NewsSymbol>,
}

/// A trading symbol on a specific exchange
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSymbol {
    /// Exchange name (e.g., "binance-futures", "bybit-perps")
    pub exchange: String,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub symbol: String,
}

impl NewsItem {
    /// Get the display time as a formatted string
    pub fn display_time(&self) -> String {
        use chrono::{TimeZone, Utc};
        if let Some(dt) = Utc.timestamp_millis_opt(self.time as i64).single() {
            dt.format("%H:%M:%S").to_string()
        } else {
            "??:??:??".to_string()
        }
    }

    /// Get a short preview of the body text
    pub fn body_preview(&self, max_chars: usize) -> &str {
        if self.body.len() <= max_chars {
            &self.body
        } else {
            let mut end = max_chars;
            while end > 0 && !self.body.is_char_boundary(end) {
                end -= 1;
            }
            &self.body[..end]
        }
    }

    /// Get the first suggestion's preferred symbol for Binance Futures
    pub fn preferred_symbol(&self) -> Option<&NewsSymbol> {
        self.suggestions.first().and_then(|s| {
            s.symbols
                .iter()
                .find(|sym| sym.exchange == "binance-futures")
                .or_else(|| s.symbols.first())
        })
    }
}

/// Price data for displaying alongside news ticker buttons
#[derive(Debug, Clone, Default)]
pub struct TickerPriceData {
    /// Current price
    pub price: f64,
    /// Price 5 minutes ago
    pub price_5m_ago: f64,
    /// Price history for sparkline (last 5 minutes, sampled every 10 seconds)
    pub price_history: Vec<f64>,
}

impl TickerPriceData {
    /// Calculate the 5-minute price change percentage
    pub fn change_percent(&self) -> f64 {
        if self.price_5m_ago == 0.0 {
            0.0
        } else {
            ((self.price - self.price_5m_ago) / self.price_5m_ago) * 100.0
        }
    }

    /// Returns true if price went up
    pub fn is_positive(&self) -> bool {
        self.change_percent() >= 0.0
    }
}
