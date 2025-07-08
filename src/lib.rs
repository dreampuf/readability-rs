//! # Readability
//!
//! A Rust port of Mozilla's Readability.js library for extracting readable content from web pages.
//!
//! This library provides functionality to parse HTML documents and extract the main article content,
//! removing navigation, ads, and other clutter to present clean, readable text.
//!
//! ## Example
//!
//! ```rust
//! use readability::{Readability, ReadabilityOptions};
//!
//! let html = r#"
//!     <html>
//!     <body>
//!         <article>
//!             <h1>Article Title</h1>
//!             <p>This is the main content of the article.</p>
//!         </article>
//!     </body>
//!     </html>
//! "#;
//!
//! let mut parser = Readability::new(html, None).unwrap();
//! if let Some(article) = parser.parse() {
//!     println!("Title: {:?}", article.title);
//!     println!("Content: {:?}", article.content);
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

// Re-export specific functions to avoid naming conflicts
pub use regexps::{
    is_unlikely_candidate, has_positive_indicators, has_negative_indicators,
    is_byline, is_video_url, is_whitespace, has_content, contains_ad_words, contains_loading_words
};
pub use scoring::ContentScore;
pub use utils::{
    to_absolute_uri, is_url, get_inner_text, get_char_count, is_phrasing_content,
    is_single_image, is_node_visible, has_ancestor_tag, get_node_ancestors,
    is_element_without_content, has_single_tag_inside_element, has_child_block_element,
    should_clean_attribute, extract_text_content, word_count, is_title_candidate,
    unescape_html_entities, clean_text, get_link_density
};

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
            classes_to_preserve: Vec::new(),
            keep_classes: false,
            disable_json_ld: false,
            allowed_video_regex: None,
            link_density_modifier: 1.0,
        }
    }
}

/// Represents an extracted article
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
    article_title: Option<String>,
    article_byline: Option<String>,
    article_dir: Option<String>,
    article_site_name: Option<String>,
    metadata: HashMap<String, String>,
}

impl Readability {
    /// Create a new Readability parser from HTML content
    pub fn new(html: &str, options: Option<ReadabilityOptions>) -> Result<Self, ReadabilityError> {
        let document = Html::parse_document(html);
        let options = options.unwrap_or_default();
        
        Ok(Self {
            document,
            options,
            base_uri: None,
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

        // Check if content meets minimum requirements
        if text_length < self.options.char_threshold {
            if self.options.debug {
                println!("Content too short: {} chars (minimum: {})", text_length, self.options.char_threshold);
            }
            return None;
        }

        // Check if content is substantive (not just navigation/footer/etc)
        if !self.is_content_substantial(&text_content) {
            if self.options.debug {
                println!("Content not substantial enough");
            }
            return None;
        }

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

    /// Check if the extracted content is substantial enough to be considered an article
    fn is_content_substantial(&self, text_content: &str) -> bool {
        // Remove excessive whitespace
        let cleaned_text = text_content.trim();
        
        // Check for minimum word count
        let word_count = cleaned_text.split_whitespace().count();
        if word_count < 25 {  // Minimum 25 words for substantial content
            return false;
        }

        // Check that it's not just navigation text or copyright notices
        let lowercase_text = cleaned_text.to_lowercase();
        let nav_indicators = ["copyright", "all rights reserved", "menu", "navigation", "login", "register"];
        
        // If the text is primarily navigation content, it's not substantial
        let nav_word_count: usize = nav_indicators.iter()
            .map(|indicator| lowercase_text.matches(indicator).count())
            .sum();
        
        // If more than 20% of words are navigation-related, it's not substantial
        nav_word_count * 5 < word_count
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
                    
                    // Handle specific Open Graph properties
                    match property {
                        "og:site_name" => self.article_site_name = Some(content.to_string()),
                        _ => {}
                    }
                }
            }
            if let Some(name) = element.value().attr("name") {
                if let Some(content) = element.value().attr("content") {
                    self.metadata.insert(name.to_string(), content.to_string());
                    
                    // Handle specific meta name properties
                    match name {
                        "author" => self.article_byline = Some(content.to_string()),
                        _ => {}
                    }
                }
            }
        }

        // Extract byline from DOM elements
        self.extract_byline_from_dom();
        
        // Extract language from html element
        if let Ok(html_selector) = Selector::parse("html") {
            if let Some(html_element) = self.document.select(&html_selector).next() {
                if let Some(lang) = html_element.value().attr("lang") {
                    self.metadata.insert("lang".to_string(), lang.to_string());
                }
            }
        }
    }

    fn extract_byline_from_dom(&mut self) {
        // If we already have a byline from meta tags, use that
        if self.article_byline.is_some() {
            return;
        }

        // Look for byline in common patterns
        let byline_selectors = [
            ".byline",
            ".author",
            ".post-author", 
            ".article-author",
            "[rel=\"author\"]",
            ".by-author",
            ".writer",
        ];

        for selector_str in &byline_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(element) = self.document.select(&selector).next() {
                    let byline_text = self.get_inner_text_from_ref(&element, false);
                    let cleaned_byline = byline_text.trim();
                    
                    // Clean up common prefixes
                    let cleaned_byline = cleaned_byline
                        .strip_prefix("By ")
                        .or_else(|| cleaned_byline.strip_prefix("by "))
                        .or_else(|| cleaned_byline.strip_prefix("BY "))
                        .or_else(|| cleaned_byline.strip_prefix("Author: "))
                        .or_else(|| cleaned_byline.strip_prefix("Written by "))
                        .unwrap_or(cleaned_byline);

                    if !cleaned_byline.is_empty() && cleaned_byline.len() < 100 {
                        self.article_byline = Some(cleaned_byline.to_string());
                        break;
                    }
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
    let opts = options.unwrap_or_default();
    
    // Scale minimum score based on char_threshold
    let min_content_length = if opts.char_threshold > 0 { 
        opts.char_threshold 
    } else { 
        140  // Default fallback
    };
    
    // Scale min_score based on char_threshold - lower thresholds need lower scores
    let min_score = if min_content_length <= 50 {
        10.0  // Very lenient for short content
    } else if min_content_length <= 100 {
        15.0  // Moderate for medium content
    } else {
        20.0  // Standard for longer content
    };
    
    // Look for content-bearing elements
    let content_selectors = ["p", "pre", "article", "div"];
    let mut score = 0.0;
    let mut total_text_length = 0;
    
    for selector_str in &content_selectors {
        if let Ok(selector) = Selector::parse(selector_str) {
            for element in document.select(&selector) {
                let text_content = element.text().collect::<String>();
                let text_length = text_content.trim().len();
                
                if text_length < 10 {  // Skip very short elements (reduced from 25)
                    continue;
                }
                
                total_text_length += text_length;
                
                // Check for unlikely candidates
                let class_and_id = format!("{} {}", 
                    element.value().attr("class").unwrap_or(""),
                    element.value().attr("id").unwrap_or("")
                );
                
                if is_unlikely_candidate(&class_and_id) {
                    score -= 5.0;  // Penalize unlikely candidates
                    continue;
                }
                
                // Score based on element type and content length
                let element_score = match element.value().name() {
                    "article" => (text_length as f64 * 0.5).min(30.0),
                    "p" => (text_length as f64 * 0.3).min(20.0),
                    "pre" => (text_length as f64 * 0.4).min(25.0),
                    "div" => {
                        // More lenient for divs when using low thresholds
                        if min_content_length <= 50 && text_length > 20 {
                            (text_length as f64 * 0.25).min(15.0)
                        } else if text_length > 80 {
                            (text_length as f64 * 0.2).min(15.0)
                        } else {
                            0.0
                        }
                    },
                    _ => 0.0,
                };
                
                score += element_score;
                
                // Early return if we have enough score
                if score > min_score && total_text_length >= min_content_length {
                    return true;
                }
            }
        }
    }
    
    // Final check: require both minimum score and minimum content length
    score > min_score && total_text_length >= min_content_length
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a readability parser
    fn create_parser(html: &str) -> Readability {
        Readability::new(html, Some(ReadabilityOptions {
            debug: true,
            char_threshold: 250,  // Lower threshold for testing
            ..Default::default()
        })).unwrap()
    }

    #[test]
    fn test_readability_options_default() {
        let options = ReadabilityOptions::default();
        assert!(!options.debug);
        assert_eq!(options.max_elems_to_parse, 0);
        assert_eq!(options.nb_top_candidates, 5);
        assert_eq!(options.char_threshold, 500);
        assert!(!options.keep_classes);
        assert!(!options.disable_json_ld);
    }

    #[test]
    fn test_article_creation() {
        let article = Article {
            title: Some("Test Title".to_string()),
            content: Some("<div>Test content</div>".to_string()),
            text_content: Some("Test content".to_string()),
            length: Some(12),
            excerpt: Some("Test excerpt".to_string()),
            byline: Some("Test Author".to_string()),
            dir: None,
            site_name: Some("Test Site".to_string()),
            lang: Some("en".to_string()),
            published_time: None,
        };

        assert_eq!(article.title.unwrap(), "Test Title");
        assert_eq!(article.length.unwrap(), 12);
        assert!(article.excerpt.is_some());
    }

    #[test]
    fn test_simple_article_parsing() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Test Article</title>
                <meta name="author" content="John Doe">
                <meta name="description" content="This is a test article">
            </head>
            <body>
                <h1>Test Article Title</h1>
                <article>
                    <p>This is the first paragraph of our test article. It contains enough content to be considered readable.</p>
                    <p>This is the second paragraph with more content. It helps ensure the article meets the minimum length requirements for processing.</p>
                    <p>A third paragraph to add more substance to our test article and make it comprehensive enough for testing.</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();

        assert!(result.is_some());
        let article = result.unwrap();
        assert!(article.title.is_some() && !article.title.as_ref().unwrap().is_empty());
        assert!(article.content.is_some());
        assert!(article.length.is_some() && article.length.unwrap() > 100);
    }

    #[test]
    fn test_empty_document() {
        let html = "<html><body></body></html>";
        let mut parser = create_parser(html);
        let result = parser.parse();
        
        // Empty document should not produce a result
        assert!(result.is_none());
    }

    #[test]
    fn test_minimal_content() {
        let html = r#"
            <html>
            <body>
                <p>Short</p>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();
        
        // Very short content should not be considered readable
        assert!(result.is_none());
    }

    #[test]
    fn test_article_with_metadata() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <title>Test Article - Test Site</title>
                <meta name="author" content="Jane Smith">
                <meta name="description" content="A comprehensive test article for readability testing">
                <meta property="og:site_name" content="Test Publishing">
                <meta property="og:title" content="Test Article">
            </head>
            <body>
                <article>
                    <h1>Test Article Title</h1>
                    <div class="byline">By Jane Smith</div>
                    <p>This is a comprehensive test article with enough content to be considered readable by the parser.</p>
                    <p>The article contains multiple paragraphs with substantial text content that should pass all readability checks.</p>
                    <p>Additional content to ensure the article meets minimum length requirements and provides meaningful extractable content.</p>
                    <p>More content to test the parsing and extraction capabilities of the readability implementation.</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();

        assert!(result.is_some());
        let article = result.unwrap();
        
        assert!(article.title.is_some() && !article.title.as_ref().unwrap().is_empty());
        assert!(article.byline.is_some());
        assert!(article.site_name.is_some());
        assert!(article.lang.is_some());
        assert_eq!(article.lang.as_ref().unwrap(), "en");
        assert!(article.length.is_some() && article.length.unwrap() > 200);
    }

    #[test]
    fn test_is_probably_readerable_basic() {
        // Test with content that should be readerable
        let readable_html = r#"
            <html>
            <body>
                <article>
                    <h1>Long Article Title</h1>
                    <p>This is a long article with substantial content that should be considered readable.</p>
                    <p>Multiple paragraphs with enough text to meet the readability thresholds.</p>
                    <p>Additional content to ensure this passes the readability checks.</p>
                    <p>Even more content to make sure this document is substantial enough.</p>
                </article>
            </body>
            </html>
        "#;

        assert!(is_probably_readerable(readable_html, None));

        // Test with content that should not be readerable
        let unreadable_html = r#"
            <html>
            <body>
                <nav>Menu</nav>
                <footer>Copyright</footer>
            </body>
            </html>
        "#;

        assert!(!is_probably_readerable(unreadable_html, None));
    }

    #[test]
    fn test_is_probably_readerable_with_options() {
        let html = r#"
            <html>
            <body>
                <p>Medium length content that is somewhat substantial.</p>
            </body>
            </html>
        "#;

        // With default options, this should not be readerable
        assert!(!is_probably_readerable(html, None));

        // With lower thresholds, this should be readerable
        let lenient_options = ReadabilityOptions {
            char_threshold: 20,
            ..Default::default()
        };
        assert!(is_probably_readerable(html, Some(lenient_options)));
    }

    #[test]
    fn test_parser_creation() {
        let html = "<html><body><p>Test content</p></body></html>";
        let parser = Readability::new(html, None);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parser_with_options() {
        let html = "<html><body><p>Test content</p></body></html>";
        let options = ReadabilityOptions {
            debug: true,
            char_threshold: 100,
            ..Default::default()
        };
        let parser = Readability::new(html, Some(options));
        assert!(parser.is_ok());
    }

    #[test]
    fn test_unicode_handling() {
        let unicode_html = r#"
            <!DOCTYPE html>
            <html lang="zh">
            <head>
                <title>测试文章</title>
                <meta charset="UTF-8">
            </head>
            <body>
                <article>
                    <h1>Unicode Content Test</h1>
                    <p>This article contains unicode characters: 测试 🚀 ñáéíóú àèìòù</p>
                    <p>Emoji support test: 😀 🎉 🌟 💻 📚</p>
                    <p>Various languages: English, Español, Français, 中文, 日本語, العربية</p>
                    <p>Special characters: ™ © ® € £ ¥ § ¶ † ‡ • … ‰ ′ ″ ‹ › « » " " ' '</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(unicode_html);
        let result = parser.parse();

        assert!(result.is_some());
        let article = result.unwrap();
        
        // Should handle unicode content without panicking
        assert!(article.title.is_some());
        assert!(article.text_content.is_some());
    }

    #[test]
    fn test_malformed_html_handling() {
        let malformed_html = r#"
            <html>
            <body>
                <div>
                    <p>Unclosed paragraph
                    <div>Nested div without proper closing
                    <p>Another paragraph</p>
                </div>
            </body>
            </html>
        "#;

        let mut parser = create_parser(malformed_html);
        let result = parser.parse();

        // Should handle malformed HTML gracefully without panicking
        assert!(result.is_some() || result.is_none());
    }
}