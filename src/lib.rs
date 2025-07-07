//! # Readability
//!
//! A Rust port of Mozilla's Readability library for extracting article content from web pages.
//! This library removes clutter like ads, navigation, and sidebars to extract the main content.
//!
//! ## Example
//!
//! ```rust
//! use readability::{Readability, ReadabilityOptions};
//!
//! let html = r#"
//! <html>
//!   <head><title>Example Article</title></head>
//!   <body>
//!     <article>
//!       <h1>Main Article Title</h1>
//!       <p>This is the main content of the article...</p>
//!     </article>
//!     <aside>This is sidebar content that should be removed.</aside>
//!   </body>
//! </html>
//! "#;
//!
//! let mut readability = Readability::new(html, None).unwrap();
//! if let Some(article) = readability.parse() {
//!     println!("Title: {}", article.title.unwrap_or_default());
//!     println!("Content: {}", article.content.unwrap_or_default());
//! }
//! ```

use regex::Regex;
use scraper::{Html, Selector, ElementRef};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

mod regexps;
mod scoring;
mod utils;

pub use regexps::*;
pub use scoring::*;
pub use utils::*;

/// Errors that can occur during readability parsing
#[derive(Error, Debug)]
pub enum ReadabilityError {
    #[error("Invalid HTML document")]
    InvalidHtml,
    #[error("No content found")]
    NoContent,
    #[error("Parsing failed: {0}")]
    ParseError(String),
}

/// Configuration options for the Readability parser
#[derive(Debug, Clone)]
pub struct ReadabilityOptions {
    /// Whether to enable debug logging
    pub debug: bool,
    /// Maximum number of elements to parse (0 = no limit)
    pub max_elems_to_parse: usize,
    /// Number of top candidates to consider
    pub nb_top_candidates: usize,
    /// Minimum character threshold for content
    pub char_threshold: usize,
    /// CSS classes to preserve during cleanup
    pub classes_to_preserve: Vec<String>,
    /// Whether to keep CSS classes
    pub keep_classes: bool,
    /// Whether to disable JSON-LD parsing
    pub disable_json_ld: bool,
    /// Custom allowed video regex pattern
    pub allowed_video_regex: Option<Regex>,
    /// Link density modifier
    pub link_density_modifier: f64,
}

impl Default for ReadabilityOptions {
    fn default() -> Self {
        Self {
            debug: false,
            max_elems_to_parse: 0,
            nb_top_candidates: 5,
            char_threshold: 500,
            classes_to_preserve: vec!["page".to_string()],
            keep_classes: false,
            disable_json_ld: false,
            allowed_video_regex: None,
            link_density_modifier: 0.0,
        }
    }
}

/// Article metadata extracted from the document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    /// Article title
    pub title: Option<String>,
    /// HTML content of the article
    pub content: Option<String>,
    /// Plain text content
    pub text_content: Option<String>,
    /// Length of the article in characters
    pub length: Option<usize>,
    /// Article excerpt or description
    pub excerpt: Option<String>,
    /// Author information
    pub byline: Option<String>,
    /// Content direction (ltr/rtl)
    pub dir: Option<String>,
    /// Site name
    pub site_name: Option<String>,
    /// Content language
    pub lang: Option<String>,
    /// Published time
    pub published_time: Option<String>,
}

/// The main Readability parser
pub struct Readability {
    document: Html,
    options: ReadabilityOptions,
    base_uri: Option<String>,
    flags: u8,
    attempts: Vec<String>,
    article_title: Option<String>,
    article_byline: Option<String>,
    article_dir: Option<String>,
    article_site_name: Option<String>,
    metadata: HashMap<String, String>,
}

// Parser flags
const FLAG_STRIP_UNLIKELYS: u8 = 0x1;
const FLAG_WEIGHT_CLASSES: u8 = 0x2;
const FLAG_CLEAN_CONDITIONALLY: u8 = 0x4;

impl Readability {
    /// Create a new Readability parser from HTML content
    pub fn new(html: &str, options: Option<ReadabilityOptions>) -> Result<Self, ReadabilityError> {
        let document = Html::parse_document(html);
        let options = options.unwrap_or_default();
        
        Ok(Self {
            document,
            options,
            base_uri: None,
            flags: FLAG_STRIP_UNLIKELYS | FLAG_WEIGHT_CLASSES | FLAG_CLEAN_CONDITIONALLY,
            attempts: Vec::new(),
            article_title: None,
            article_byline: None,
            article_dir: None,
            article_site_name: None,
            metadata: HashMap::new(),
        })
    }

    /// Create a new Readability parser with a base URI for resolving relative URLs
    pub fn new_with_base_uri(html: &str, base_uri: &str, options: Option<ReadabilityOptions>) -> Result<Self, ReadabilityError> {
        let mut parser = Self::new(html, options)?;
        parser.base_uri = Some(base_uri.to_string());
        Ok(parser)
    }

    /// Parse the document and extract the main article content
    pub fn parse(&mut self) -> Option<Article> {
        if self.options.debug {
            println!("Starting readability parsing...");
        }

        // Remove script tags
        self.remove_scripts();
        
        // Prepare the document
        self.prep_document();

        // Extract metadata
        self.get_article_metadata();

        // Get article title
        self.get_article_title();

        // Try to grab the article content
        let article_content = self.grab_article()?;
        let content_html = article_content.inner_html();
        let text_content = self.get_inner_text_from_ref(&article_content, true);
        let text_length = text_content.len();

        // Post-process would be done here if needed
        if self.options.debug {
            println!("Post-processing content...");
        }

        Some(Article {
            title: self.article_title.clone(),
            content: Some(content_html),
            text_content: Some(text_content),
            length: Some(text_length),
            excerpt: self.metadata.get("description").cloned(),
            byline: self.article_byline.clone(),
            dir: self.article_dir.clone(),
            site_name: self.article_site_name.clone(),
            lang: self.metadata.get("lang").cloned(),
            published_time: self.metadata.get("publishedTime").cloned(),
        })
    }

    fn remove_scripts(&mut self) {
        // This would require mutable DOM manipulation
        // For now, we'll handle this in the HTML preprocessing
    }

    fn prep_document(&mut self) {
        // Remove unlikely candidates and prepare the document for parsing
        if self.options.debug {
            println!("Preparing document...");
        }
    }

    fn get_article_metadata(&mut self) {
        // Extract metadata from meta tags, JSON-LD, etc.
        let meta_selector = Selector::parse("meta").unwrap();
        
        for element in self.document.select(&meta_selector) {
            if let Some(property) = element.value().attr("property") {
                if let Some(content) = element.value().attr("content") {
                    self.metadata.insert(property.to_string(), content.to_string());
                }
            }
            if let Some(name) = element.value().attr("name") {
                if let Some(content) = element.value().attr("content") {
                    self.metadata.insert(name.to_string(), content.to_string());
                }
            }
        }
    }

    fn get_article_title(&mut self) {
        let title_selector = Selector::parse("title").unwrap();
        if let Some(title_element) = self.document.select(&title_selector).next() {
            self.article_title = Some(title_element.inner_html());
        }

        // Try to get a better title from h1 elements
        let h1_selector = Selector::parse("h1").unwrap();
        for h1 in self.document.select(&h1_selector) {
            let h1_text = self.get_inner_text_from_ref(&h1, false);
            if h1_text.len() > 10 {
                self.article_title = Some(h1_text);
                break;
            }
        }
    }

    fn grab_article(&self) -> Option<ElementRef> {
        // This is the main content extraction logic
        // For now, we'll use a simplified approach
        
        // Try article tag first
        let article_selector = Selector::parse("article").unwrap();
        if let Some(article) = self.document.select(&article_selector).next() {
            return Some(article);
        }

        // Try main tag
        let main_selector = Selector::parse("main").unwrap();
        if let Some(main) = self.document.select(&main_selector).next() {
            return Some(main);
        }

        // Try content-related selectors
        let content_selectors = [
            "#content",
            ".content",
            "#main-content", 
            ".main-content",
            ".post-content",
            ".entry-content",
        ];

        for selector_str in &content_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = self.document.select(&selector).next() {
                    return Some(element);
                }
            }
        }

        // Fallback to body
        let body_selector = Selector::parse("body").unwrap();
        self.document.select(&body_selector).next()
    }

    fn get_inner_text_from_ref(&self, element: &ElementRef, normalize_spaces: bool) -> String {
        let text = element.text().collect::<Vec<_>>().join(" ");
        if normalize_spaces {
            let re = Regex::new(r"\s+").unwrap();
            re.replace_all(&text, " ").trim().to_string()
        } else {
            text
        }
    }
}

/// Check if a document is likely to be readable/parseable
pub fn is_probably_readerable(html: &str, options: Option<ReadabilityOptions>) -> bool {
    let document = Html::parse_document(html);
    let _opts = options.unwrap_or_default();
    
    let min_score = 20.0;
    let min_content_length = 140;
    
    // Look for content-bearing elements
    let content_selectors = ["p", "pre", "article"];
    let mut score = 0.0;
    
    for selector_str in &content_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                let text_content = element.text().collect::<String>();
                let text_length = text_content.trim().len();
                
                if text_length < min_content_length {
                    continue;
                }
                
                // Check for unlikely candidates
                let class_and_id = format!("{} {}", 
                    element.value().attr("class").unwrap_or(""),
                    element.value().attr("id").unwrap_or("")
                );
                
                if is_unlikely_candidate(&class_and_id) {
                    continue;
                }
                
                score += (text_length as f64 - min_content_length as f64).sqrt();
                
                if score > min_score {
                    return true;
                }
            }
        }
    }
    
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let html = r#"
        <html>
            <head><title>Test Article</title></head>
            <body>
                <article>
                    <h1>Main Title</h1>
                    <p>This is the main content of the article that should be extracted.</p>
                </article>
                <aside>This sidebar should be removed.</aside>
            </body>
        </html>
        "#;

        let mut readability = Readability::new(html, None).unwrap();
        let article = readability.parse();
        
        assert!(article.is_some());
        let article = article.unwrap();
        assert!(article.content.is_some());
        assert!(article.title.is_some());
    }

    #[test]
    fn test_is_probably_readerable() {
        let html = r#"
        <html>
            <body>
                <p>This is a paragraph with enough content to be considered readerable. It has more than 140 characters which is the minimum threshold for content length that we use to determine if something is worth reading.</p>
            </body>
        </html>
        "#;

        assert!(is_probably_readerable(html, None));
    }

    #[test]
    fn test_not_readerable() {
        let html = r#"
        <html>
            <body>
                <p>Short</p>
            </body>
        </html>
        "#;

        assert!(!is_probably_readerable(html, None));
    }
}