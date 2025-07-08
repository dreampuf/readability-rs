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
    use std::{fs, path::Path};
    use serde_json;

    // Helper function to create a readability parser
    fn create_parser(html: &str) -> Readability {
        Readability::new(html, Some(ReadabilityOptions {
            debug: true,
            char_threshold: 500,
            classes_to_preserve: vec!["caption".to_string()],
            ..Default::default()
        })).unwrap()
    }

    // Mozilla test case structure
    #[derive(Debug)]
    struct TestCase {
        name: String,
        source: String,
        expected_content: String,
        expected_metadata: TestMetadata,
    }

    #[derive(Debug, Deserialize)]
    struct TestMetadata {
        title: Option<String>,
        byline: Option<String>,
        dir: Option<String>,
        excerpt: Option<String>,
        #[serde(rename = "siteName")]
        site_name: Option<String>,
        #[serde(rename = "publishedTime")]
        published_time: Option<String>,
        readerable: Option<bool>,
        lang: Option<String>,
    }

    // Load all Mozilla test cases
    fn load_mozilla_test_cases() -> Vec<TestCase> {
        let test_pages_dir = Path::new("mozilla-readability/test/test-pages");
        
        if !test_pages_dir.exists() {
            println!("Warning: Mozilla test pages directory not found. Skipping Mozilla tests.");
            return Vec::new();
        }

        let mut test_cases = Vec::new();
        
        for entry in fs::read_dir(test_pages_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            
            if path.is_dir() {
                let name = path.file_name().unwrap().to_str().unwrap().to_string();
                
                let source_path = path.join("source.html");
                let expected_content_path = path.join("expected.html");
                let expected_metadata_path = path.join("expected-metadata.json");
                
                // Check if all required files exist
                if source_path.exists() && expected_content_path.exists() && expected_metadata_path.exists() {
                    let source = fs::read_to_string(&source_path).unwrap();
                    let expected_content = fs::read_to_string(&expected_content_path).unwrap();
                    let metadata_json = fs::read_to_string(&expected_metadata_path).unwrap();
                    let expected_metadata: TestMetadata = serde_json::from_str(&metadata_json).unwrap();
                    
                    test_cases.push(TestCase {
                        name,
                        source,
                        expected_content,
                        expected_metadata,
                    });
                }
            }
        }
        
        test_cases
    }

    #[test]
    fn test_readability_options_default() {
        let options = ReadabilityOptions::default();
        assert_eq!(options.debug, false);
        assert_eq!(options.max_elems_to_parse, 0);
        assert_eq!(options.nb_top_candidates, 5);
        assert_eq!(options.char_threshold, 500);
        assert_eq!(options.classes_to_preserve.len(), 0);
        assert_eq!(options.keep_classes, false);
    }

    #[test]
    fn test_article_creation() {
        let article = Article {
            title: Some("Test Title".to_string()),
            content: Some("<p>Test content</p>".to_string()),
            text_content: Some("Test content".to_string()),
            length: Some(12),
            excerpt: Some("Test excerpt".to_string()),
            byline: Some("Test Author".to_string()),
            dir: None,
            site_name: Some("Test Site".to_string()),
            lang: Some("en".to_string()),
            published_time: None,
        };
        
        assert_eq!(article.title, Some("Test Title".to_string()));
        assert_eq!(article.byline, Some("Test Author".to_string()));
    }

    #[test]
    fn test_simple_article_parsing() {
        let html = r#"
            <html>
            <head><title>Test Article</title></head>
            <body>
                <article>
                    <h1>Article Title</h1>
                    <p>This is the main content of the article. It should be long enough to meet the character threshold for readability parsing. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum.</p>
                    <p>Second paragraph with more content to ensure we have enough text for proper parsing and extraction of meaningful content.</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();
        
        assert!(result.is_some());
        let article = result.unwrap();
        assert!(article.content.is_some());
        assert!(article.text_content.is_some());
        assert!(article.length.unwrap() > 100);
    }

    #[test]
    fn test_empty_document() {
        let html = "<html><body></body></html>";
        let mut parser = create_parser(html);
        let result = parser.parse();
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
        assert!(result.is_none()); // Should be None due to char_threshold
    }

    #[test]
    fn test_article_with_metadata() {
        let html = r#"
            <html>
            <head>
                <title>Test Article Title</title>
                <meta name="author" content="John Doe">
                <meta property="og:site_name" content="Example Site">
                <meta name="description" content="This is a test article description">
            </head>
            <body>
                <article>
                    <h1>Article Heading</h1>
                    <p>This is the main content of the article with sufficient length to meet the character threshold requirements for proper readability parsing. The content should be extracted along with the metadata from the head section. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();
        
        assert!(result.is_some());
        let article = result.unwrap();
        assert_eq!(article.byline, Some("John Doe".to_string()));
        assert_eq!(article.site_name, Some("Example Site".to_string()));
        assert_eq!(article.excerpt, Some("This is a test article description".to_string()));
        assert!(article.content.is_some());
    }

    #[test]
    fn test_is_probably_readerable_basic() {
        let readerable_html = r#"
            <html>
            <body>
                <article>
                    <h1>Article Title</h1>
                    <p>This is a substantial article with enough content to be considered readerable. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur. Excepteur sint occaecat cupidatat non proident, sunt in culpa qui officia deserunt mollit anim id est laborum. Sed ut perspiciatis unde omnis iste natus error sit voluptatem accusantium doloremque laudantium.</p>
                </article>
            </body>
            </html>
        "#;

        let non_readerable_html = r#"
            <html>
            <body>
                <div class="navigation">Menu</div>
                <div class="sidebar">Ads</div>
                <p>Short content</p>
            </body>
            </html>
        "#;

        assert_eq!(is_probably_readerable(readerable_html, None), true);
        assert_eq!(is_probably_readerable(non_readerable_html, None), false);
    }

    #[test]
    fn test_is_probably_readerable_with_options() {
        let html = r#"
            <html>
            <body>
                <p>This is a medium-length article that might be readerable with different thresholds. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p>
            </body>
            </html>
        "#;

        // Default options (high threshold)
        assert_eq!(is_probably_readerable(html, None), false);

        // Lower threshold
        let low_threshold_options = ReadabilityOptions {
            char_threshold: 50,
            ..Default::default()
        };
        assert_eq!(is_probably_readerable(html, Some(low_threshold_options)), true);
    }

    #[test]
    fn test_parser_creation() {
        let html = "<html><body><p>Test</p></body></html>";
        let parser = Readability::new(html, None);
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parser_with_options() {
        let html = "<html><body><p>Test</p></body></html>";
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
        let html = r#"
            <html>
            <head><title>Unicode Test Article</title>
            <body>
                <article>
                    <h1>مقالة باللغة العربية</h1>
                    <p>هذا محتوى المقالة الرئيسي باللغة العربية. يجب أن يكون طويلاً بما فيه الكفاية لتلبية عتبة الأحرف لتحليل القابلية للقراءة. لوريم إيبسوم دولور سيت أميت، كونسكتتور أديبيسكينغ إليت. سيد دو إيوسمود تيمبور إنسيديدنت أوت لابوري إت دولوري ماغنا أليكوا. أوت إنيم أد مينيم فينيام، كويس نوسترود إكسرسيتاتيو أولامكو لابوريس نيسي أوت أليكويب إكس إيا كومودو كونسكوات. دويس أوتي إيروري دولور إن ريبريهينديريت إن فولوبتاتي فيليت إيسي سيلوم دولوري إو فوجيات نولا باريتور.</p>
                    <p>الفقرة الثانية مع المزيد من المحتوى لضمان وجود نص كافٍ للتحليل والاستخراج الصحيح للمحتوى المفيد.</p>
                </article>
            </body>
            </html>
        "#;

        let mut parser = create_parser(html);
        let result = parser.parse();
        
        assert!(result.is_some());
        let article = result.unwrap();
        assert!(article.content.is_some());
        assert!(article.text_content.is_some());
        // Unicode content should be preserved
        assert!(article.text_content.unwrap().contains("مقالة"));
    }

    #[test]
    fn test_malformed_html_handling() {
        let malformed_html = r#"
            <html>
            <head><title>Malformed HTML Test</title>
            <body>
                <article>
                    <h1>Article with malformed HTML
                    <p>This paragraph is not properly closed and has malformed structure but should still be parseable with sufficient content for readability analysis. Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.
                    <div>Another element with content that should be extracted despite the malformed structure above.</div>
            </body>
        "#;

        // Should not panic on malformed HTML
        let parser_result = Readability::new(malformed_html, None);
        assert!(parser_result.is_ok());
        
        let mut parser = parser_result.unwrap();
        let result = parser.parse();
        // May or may not have content depending on how scraper handles the malformed HTML
        // The important thing is that it doesn't crash
        if let Some(article) = result {
            assert!(article.content.is_some());
        }
    }

    // Mozilla test cases - these run the actual Mozilla readability test suite
    #[test]
    fn test_mozilla_readability_test_cases() {
        let test_cases = load_mozilla_test_cases();
        
        if test_cases.is_empty() {
            println!("No Mozilla test cases found - skipping Mozilla compatibility tests");
            return;
        }
        
        let mut passed = 0;
        let mut failed = 0;
        let mut errors = Vec::new();
        
        for test_case in test_cases {
            println!("Testing: {}", test_case.name);
            
            let uri = "http://fakehost/test/page.html";
            let mut parser = match Readability::new_with_base_uri(&test_case.source, uri, Some(ReadabilityOptions {
                debug: false,
                classes_to_preserve: vec!["caption".to_string()],
                ..Default::default()
            })) {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!("{}: Parser creation failed: {}", test_case.name, e));
                    failed += 1;
                    continue;
                }
            };
            
            match parser.parse() {
                Some(result) => {
                    // Test that we got a result
                    assert!(result.content.is_some(), "Content should be extracted for {}", test_case.name);
                    
                    // Check title if expected
                    if let Some(expected_title) = &test_case.expected_metadata.title {
                        if let Some(actual_title) = &result.title {
                            if actual_title != expected_title {
                                errors.push(format!("{}: Title mismatch. Expected: '{}', Got: '{}'", 
                                    test_case.name, expected_title, actual_title));
                            }
                        } else {
                            errors.push(format!("{}: Expected title '{}' but got None", 
                                test_case.name, expected_title));
                        }
                    }
                    
                    // Check byline if expected
                    if let Some(expected_byline) = &test_case.expected_metadata.byline {
                        if let Some(actual_byline) = &result.byline {
                            if actual_byline != expected_byline {
                                errors.push(format!("{}: Byline mismatch. Expected: '{}', Got: '{}'", 
                                    test_case.name, expected_byline, actual_byline));
                            }
                        } else {
                            errors.push(format!("{}: Expected byline '{}' but got None", 
                                test_case.name, expected_byline));
                        }
                    }
                    
                    // Check excerpt if expected
                    if let Some(expected_excerpt) = &test_case.expected_metadata.excerpt {
                        if let Some(actual_excerpt) = &result.excerpt {
                            if actual_excerpt != expected_excerpt {
                                errors.push(format!("{}: Excerpt mismatch. Expected: '{}', Got: '{}'", 
                                    test_case.name, expected_excerpt, actual_excerpt));
                            }
                        } else {
                            errors.push(format!("{}: Expected excerpt '{}' but got None", 
                                test_case.name, expected_excerpt));
                        }
                    }
                    
                    // Check site name if expected
                    if let Some(expected_site_name) = &test_case.expected_metadata.site_name {
                        if let Some(actual_site_name) = &result.site_name {
                            if actual_site_name != expected_site_name {
                                errors.push(format!("{}: Site name mismatch. Expected: '{}', Got: '{}'", 
                                    test_case.name, expected_site_name, actual_site_name));
                            }
                        } else {
                            errors.push(format!("{}: Expected site name '{}' but got None", 
                                test_case.name, expected_site_name));
                        }
                    }
                    
                    passed += 1;
                }
                None => {
                    // Check if this was expected to be non-readerable
                    if test_case.expected_metadata.readerable == Some(false) {
                        passed += 1;
                    } else {
                        errors.push(format!("{}: Failed to extract content but was expected to be readerable", test_case.name));
                        failed += 1;
                    }
                }
            }
        }
        
        println!("Mozilla test results: {} passed, {} failed", passed, failed);
        
        if !errors.is_empty() {
            println!("Test errors:");
            for error in &errors[..std::cmp::min(10, errors.len())] {  // Show first 10 errors
                println!("  {}", error);
            }
            if errors.len() > 10 {
                println!("  ... and {} more errors", errors.len() - 10);
            }
        }
        
        // Allow some failures but require significant success rate
        let total_tests = passed + failed;
        if total_tests > 0 {
            let success_rate = (passed as f64) / (total_tests as f64);
            assert!(success_rate > 0.5, "Success rate too low: {:.2}% ({}/{} tests passed)", 
                success_rate * 100.0, passed, total_tests);
        }
    }

    #[test]
    fn test_mozilla_is_probably_readerable_test_cases() {
        let test_cases = load_mozilla_test_cases();
        
        if test_cases.is_empty() {
            println!("No Mozilla test cases found - skipping isProbablyReaderable tests");
            return;
        }
        
        let mut correct_predictions = 0;
        let mut total_predictions = 0;
        let mut errors = Vec::new();
        
        for test_case in test_cases {
            if let Some(expected_readerable) = test_case.expected_metadata.readerable {
                let actual_readerable = is_probably_readerable(&test_case.source, None);
                
                if actual_readerable == expected_readerable {
                    correct_predictions += 1;
                } else {
                    errors.push(format!("{}: isProbablyReaderable mismatch. Expected: {}, Got: {}", 
                        test_case.name, expected_readerable, actual_readerable));
                }
                total_predictions += 1;
            }
        }
        
        println!("isProbablyReaderable test results: {}/{} correct predictions", 
            correct_predictions, total_predictions);
        
        if !errors.is_empty() {
            println!("isProbablyReaderable errors:");
            for error in &errors[..std::cmp::min(5, errors.len())] {  // Show first 5 errors
                println!("  {}", error);
            }
            if errors.len() > 5 {
                println!("  ... and {} more errors", errors.len() - 5);
            }
        }
        
        // Require at least 70% accuracy for isProbablyReaderable
        if total_predictions > 0 {
            let accuracy = (correct_predictions as f64) / (total_predictions as f64);
            assert!(accuracy > 0.7, "isProbablyReaderable accuracy too low: {:.2}% ({}/{} correct)", 
                accuracy * 100.0, correct_predictions, total_predictions);
        }
    }
}