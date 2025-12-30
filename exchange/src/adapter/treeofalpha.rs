//! Tree of Alpha News WebSocket client
//!
//! Connects to Tree of Alpha WebSocket API to receive real-time crypto news.

use crate::connect;

use iced_futures::{
    Subscription,
    futures::{SinkExt, Stream, channel::mpsc},
    stream,
};
use log::{error, info, warn};
use serde::Deserialize;
use std::time::Duration;

const WS_DOMAIN: &str = "news.treeofalpha.com";
const WS_URL: &str = "/ws";
const RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// A news item received from Tree of Alpha
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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

/// Events emitted by the Tree of Alpha news subscription
#[derive(Debug, Clone)]
pub enum Event {
    /// Connected to Tree of Alpha WebSocket
    Connected,
    /// Disconnected from Tree of Alpha WebSocket
    Disconnected(String),
    /// Received a news item
    NewsReceived(NewsItem),
}

enum State {
    Disconnected,
    Connected(
        fastwebsockets::FragmentCollector<hyper_util::rt::TokioIo<hyper::upgrade::Upgraded>>,
    ),
}

/// Create a subscription to Tree of Alpha news WebSocket
pub fn connect_news_stream(api_key: Option<String>) -> impl Stream<Item = Event> {
    stream::channel(100, async move |mut output: mpsc::Sender<Event>| {
        let mut state = State::Disconnected;

        loop {
            match &mut state {
                State::Disconnected => {
                    info!("Connecting to Tree of Alpha news...");
                    match connect::connect_ws(WS_DOMAIN, WS_URL).await {
                        Ok(mut ws) => {
                            // Send login command if API key is provided
                            if let Some(ref key) = api_key {
                                let login_msg = format!("login {}", key);
                                if let Err(e) = ws
                                    .write_frame(fastwebsockets::Frame::text(
                                        fastwebsockets::Payload::Borrowed(
                                            login_msg.as_bytes(),
                                        ),
                                    ))
                                    .await
                                {
                                    error!("Failed to send login: {}", e);
                                    tokio::time::sleep(RECONNECT_DELAY).await;
                                    continue;
                                }
                                info!("Sent login to Tree of Alpha");
                            } else {
                                warn!("No Tree of Alpha API Key provided. Connection may be rejected.");
                            }

                            info!("Connected to Tree of Alpha news");
                            // Subscribe to all news
                            if let Err(e) = ws
                                .write_frame(fastwebsockets::Frame::text(
                                    fastwebsockets::Payload::Borrowed(b"sub all"),
                                ))
                                .await
                            {
                                error!("Failed to subscribe: {}", e);
                                tokio::time::sleep(RECONNECT_DELAY).await;
                                continue;
                            }

                            let _ = output.send(Event::Connected).await;
                            state = State::Connected(ws);
                        }
                        Err(e) => {
                            let msg = e.to_string();
                            if msg.contains("401") {
                                error!("Connection rejected (401). Please check your Tree of Alpha API Key in Settings.");
                            } else {
                                warn!("Failed to connect to Tree of Alpha: {}", e);
                            }
                            tokio::time::sleep(RECONNECT_DELAY).await;
                        }
                    }
                }
                State::Connected(ws) => {
                    match ws.read_frame().await {
                        Ok(frame) => {
                            match frame.opcode {
                                fastwebsockets::OpCode::Text => {
                                    let payload = frame.payload.to_vec();
                                    if let Ok(text) = String::from_utf8(payload) {
                                        // Try to parse as a news item
                                        match serde_json::from_str::<NewsItem>(&text) {
                                            Ok(news) => {
                                                let _ = output
                                                    .send(Event::NewsReceived(news))
                                                    .await;
                                            }
                                            Err(_) => {
                                                // Not a news item, might be a status message
                                                if text.contains("auth") || text.contains("login") {
                                                    info!("Tree of Alpha: {}", text);
                                                }
                                            }
                                        }
                                    }
                                }
                                fastwebsockets::OpCode::Close => {
                                    warn!("Tree of Alpha WebSocket closed by server");
                                    let _ = output
                                        .send(Event::Disconnected(
                                            "Connection closed".to_string(),
                                        ))
                                        .await;
                                    state = State::Disconnected;
                                }
                                fastwebsockets::OpCode::Ping => {
                                    // Respond with pong
                                    let _ = ws
                                        .write_frame(fastwebsockets::Frame::pong(
                                            frame.payload,
                                        ))
                                        .await;
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            error!("Tree of Alpha read error: {}", e);
                            let _ = output
                                .send(Event::Disconnected(e.to_string()))
                                .await;
                            state = State::Disconnected;
                            tokio::time::sleep(RECONNECT_DELAY).await;
                        }
                    }
                }
            }
        }
    })
}

/// Create a subscription to Tree of Alpha news
pub fn news_subscription(api_key: Option<String>) -> Subscription<Event> {
    struct TreeOfAlphaNews(Option<String>);
    
    impl std::hash::Hash for TreeOfAlphaNews {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            std::any::TypeId::of::<Self>().hash(state);
            self.0.hash(state);
        }
    }
    
    Subscription::run_with(TreeOfAlphaNews(api_key.clone()), |cfg: &TreeOfAlphaNews| {
        connect_news_stream(cfg.0.clone())
    })
}

pub async fn fetch_news(limit: usize, api_key: Option<String>) -> Result<Vec<NewsItem>, crate::adapter::AdapterError> {
    let client = reqwest::Client::new();
    let url = format!("https://{}/api/news?limit={}", WS_DOMAIN, limit);
    let mut request = client.get(&url);
    
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }
    
    let resp = request.send().await.map_err(crate::adapter::AdapterError::FetchError)?;
    let resp = resp.error_for_status().map_err(crate::adapter::AdapterError::FetchError)?;
    let news: Vec<NewsItem> = resp.json().await.map_err(|e| crate::adapter::AdapterError::ParseError(e.to_string()))?;
    Ok(news)
}
