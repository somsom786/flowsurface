//! News Panel - displays real-time crypto news from Tree of Alpha
//!
//! Each news item shows ticker buttons with price changes and mini sparklines.

use crate::style::{self, Icon, icon_text};
use exchange::adapter::treeofalpha::{NewsItem, NewsSymbol};

use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, row, scrollable, text, Space},
};
use std::collections::VecDeque;

/// Maximum number of news items to keep in memory
const MAX_NEWS_ITEMS: usize = 100;

/// Row height for each news item
const NEWS_ROW_MIN_HEIGHT: f32 = 80.0;

#[derive(Debug, Clone)]
pub enum Message {
    /// User clicked on a ticker button
    TickerClicked(String, String), // (coin, symbol)
    /// Toggle expanded view for a news item
    ToggleExpand(String), // news id
    /// Scroll the news feed
    Scrolled(f32),
}

/// Action that the news panel wants the parent to perform
#[derive(Debug, Clone)]
pub enum Action {
    /// Navigate to a specific ticker chart
    NavigateToTicker {
        exchange: String,
        symbol: String,
    },
    /// Open a specific ticker chart in a new window
    OpenChartInNewWindow {
        exchange: String,
        symbol: String,
    },
}

/// Price data for displaying alongside ticker buttons
#[derive(Debug, Clone, Default)]
pub struct TickerPriceData {
    /// Current price
    pub price: f64,
    /// Price 5 minutes ago
    pub price_5m_ago: f64,
    /// Price history for sparkline (last 5 minutes)
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

/// The News Panel state
pub struct NewsPanel {
    /// News items, newest first
    news_items: VecDeque<NewsItem>,
    /// Currently expanded news item ID
    expanded_id: Option<String>,
    /// Price data cache for tickers
    price_cache: rustc_hash::FxHashMap<String, TickerPriceData>,
}

impl NewsPanel {
    pub fn new() -> Self {
        Self {
            news_items: VecDeque::new(),
            expanded_id: None,
            price_cache: rustc_hash::FxHashMap::default(),
        }
    }

    /// Add a new news item to the panel
    pub fn add_news(&mut self, news: NewsItem) {
        // Add to front (newest first)
        self.news_items.push_front(news);
        
        // Prune old items
        while self.news_items.len() > MAX_NEWS_ITEMS {
            self.news_items.pop_back();
        }
    }

    /// Update price data for a ticker
    pub fn update_price(&mut self, symbol: &str, price: f64) {
        let entry = self.price_cache.entry(symbol.to_string()).or_default();
        
        // Update price history
        entry.price_history.push(price);
        if entry.price_history.len() > 30 {
            // Keep last 30 samples (5 minutes at 10 second intervals)
            entry.price_history.remove(0);
        }
        
        entry.price = price;
        if entry.price_5m_ago == 0.0 && !entry.price_history.is_empty() {
            entry.price_5m_ago = entry.price_history[0];
        } else if entry.price_history.len() >= 30 {
            entry.price_5m_ago = entry.price_history[0];
        }
    }

    /// Process a message and return an optional action
    pub fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::TickerClicked(exchange, symbol) => {
                Some(Action::OpenChartInNewWindow { exchange, symbol }) // Use OpenChartInNewWindow
            }
            Message::ToggleExpand(id) => {
                if self.expanded_id.as_ref() == Some(&id) {
                    self.expanded_id = None;
                } else {
                    self.expanded_id = Some(id);
                }
                None
            }
            Message::Scrolled(_) => None,
        }
    }

    /// Render the news panel
    pub fn view(&self) -> Element<'_, Message> {
        if self.news_items.is_empty() {
            return container(
                column![
                    icon_text(Icon::ExternalLink, 32),
                    text("Waiting for news...").size(14),
                ]
                .align_x(Alignment::Center)
                .spacing(8),
            )
            .center(Length::Fill)
            .into();
        }

        let news_list: Vec<Element<'_, Message>> = self
            .news_items
            .iter()
            .map(|news| self.view_news_item(news))
            .collect();

        scrollable(
            column(news_list)
                .spacing(4)
                .padding(4),
        )
        .height(Length::Fill)
        .into()
    }

    /// Render a single news item
    fn view_news_item<'a>(&'a self, news: &'a NewsItem) -> Element<'a, Message> {
        let is_expanded = self.expanded_id.as_ref() == Some(&news.id);

        // Time and source header
        let time_str = news.display_time();
        let source_str = news.source.as_deref().unwrap_or("News");

        let header = row![
            text(time_str)
                .size(10)
                .color(iced::Color::from_rgb(0.6, 0.6, 0.6)),
            Space::new().width(Length::Fixed(8.0)),
            text(source_str)
                .size(10)
                .color(iced::Color::from_rgb(0.5, 0.7, 0.9)),
        ]
        .align_y(Alignment::Center);

        // Title
        let title = text(&news.title)
            .size(12)
            .font(style::AZERET_MONO);

        // Body (truncated unless expanded)
        let body_text = if is_expanded {
            news.body.clone()
        } else {
            let preview = news.body_preview(150);
            if preview.len() < news.body.len() {
                format!("{}...", preview)
            } else {
                preview.to_string()
            }
        };

        let body = text(body_text)
            .size(11)
            .color(iced::Color::from_rgb(0.8, 0.8, 0.8));

        // Ticker buttons
        let ticker_buttons = self.view_ticker_buttons(news);

        // Combine all parts
        let content = column![
            header,
            title,
            body,
            ticker_buttons,
        ]
        .spacing(4)
        .padding(8);

        // Wrap in a container with click handler
        let news_id = news.id.clone();
        button(content)
            .on_press(Message::ToggleExpand(news_id))
            .style(move |theme, status| style::button::transparent(theme, status, is_expanded))
            .width(Length::Fill)
            .into()
    }

    /// Render ticker buttons with price changes
    fn view_ticker_buttons<'a>(&'a self, news: &'a NewsItem) -> Element<'a, Message> {
        if news.suggestions.is_empty() {
            return Space::new().into();
        }

        let buttons: Vec<Element<'a, Message>> = news
            .suggestions
            .iter()
            .take(5) // Limit to 5 tickers per news item
            .filter_map(|suggestion| {
                // Prefer binance-futures, fall back to first available
                let symbol = suggestion
                    .symbols
                    .iter()
                    .find(|s| s.exchange == "binance-futures")
                    .or_else(|| suggestion.symbols.first())?;

                Some(self.view_ticker_button(&suggestion.coin, symbol))
            })
            .collect();

        if buttons.is_empty() {
            return Space::new().into();
        }

        row(buttons)
            .spacing(4)
            .wrap()
            .into()
    }

    /// Render a single ticker button
    fn view_ticker_button<'a>(&'a self, coin: &'a str, symbol: &'a NewsSymbol) -> Element<'a, Message> {
        let price_data = self.price_cache.get(&symbol.symbol);
        
        let change_text = if let Some(data) = price_data {
            let pct = data.change_percent();
            if pct >= 0.0 {
                format!("+{:.2}%", pct)
            } else {
                format!("{:.2}%", pct)
            }
        } else {
            "---".to_string()
        };

        let change_color = if let Some(data) = price_data {
            if data.is_positive() {
                iced::Color::from_rgb(0.2, 0.8, 0.4)
            } else {
                iced::Color::from_rgb(0.9, 0.3, 0.3)
            }
        } else {
            iced::Color::from_rgb(0.6, 0.6, 0.6)
        };

        let content = row![
            text(coin).size(10).font(style::AZERET_MONO),
            Space::new().width(Length::Fixed(4.0)),
            text(change_text).size(9).color(change_color),
        ]
        .align_y(Alignment::Center)
        .padding([2, 6]);

        let exchange = symbol.exchange.clone();
        let sym = symbol.symbol.clone();

        button(content)
            .on_press(Message::TickerClicked(exchange, sym))
            .style(|theme, status| style::button::transparent(theme, status, false))
            .into()
    }

    /// Check if the panel has any news
    pub fn is_empty(&self) -> bool {
        self.news_items.is_empty()
    }

    /// Get the number of news items
    pub fn len(&self) -> usize {
        self.news_items.len()
    }
}

impl Default for NewsPanel {
    fn default() -> Self {
        Self::new()
    }
}
