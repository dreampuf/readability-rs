//! Content scoring algorithms for the Readability parser

use scraper::{ElementRef, Element};
use std::collections::HashMap;
use crate::regexps::*;

/// Represents the score and metadata for a DOM element
#[derive(Debug, Clone)]
pub struct ContentScore {
    pub score: f64,
    pub content_score: f64,
}

impl ContentScore {
    pub fn new() -> Self {
        Self {
            score: 0.0,
            content_score: 0.0,
        }
    }

    pub fn with_score(score: f64) -> Self {
        Self {
            score,
            content_score: score,
        }
    }
}

/// Content scorer for evaluating DOM elements
pub struct ContentScorer {
    scores: HashMap<String, ContentScore>,
}

impl ContentScorer {
    pub fn new() -> Self {
        Self {
            scores: HashMap::new(),
        }
    }

    /// Initialize a node with a score based on its tag name
    pub fn initialize_node(&mut self, element: &ElementRef) -> f64 {
        let tag_name = element.value().name();
        let content_score = match tag_name {
            "div" => 5.0,
            "pre" | "td" | "blockquote" => 3.0,
            "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" => -3.0,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" => -5.0,
            _ => 0.0,
        };

        // Adjust score based on class and id
        let final_score = content_score + self.get_class_weight(element);

        let element_id = self.get_element_id(element);
        let score = ContentScore::with_score(final_score);
        self.scores.insert(element_id, score);

        final_score
    }

    /// Get the weight of an element based on its class and id attributes
    pub fn get_class_weight(&self, element: &ElementRef) -> f64 {
        let mut weight = 0.0;

        // Look at class attribute
        if let Some(class_attr) = element.value().attr("class") {
            if has_negative_indicators(class_attr) {
                weight -= 25.0;
            }
            if has_positive_indicators(class_attr) {
                weight += 25.0;
            }
        }

        // Look at id attribute
        if let Some(id_attr) = element.value().attr("id") {
            if has_negative_indicators(id_attr) {
                weight -= 25.0;
            }
            if has_positive_indicators(id_attr) {
                weight += 25.0;
            }
        }

        weight
    }

    /// Get the score for an element
    pub fn get_score(&self, element: &ElementRef) -> f64 {
        let element_id = self.get_element_id(element);
        self.scores.get(&element_id)
            .map(|score| score.content_score)
            .unwrap_or(0.0)
    }

    /// Set the score for an element
    pub fn set_score(&mut self, element: &ElementRef, score: f64) {
        let element_id = self.get_element_id(element);
        let content_score = ContentScore::with_score(score);
        self.scores.insert(element_id, content_score);
    }

    /// Add to the score of an element
    pub fn add_score(&mut self, element: &ElementRef, score_to_add: f64) {
        let element_id = self.get_element_id(element);
        let current_score = self.scores.get(&element_id)
            .map(|s| s.content_score)
            .unwrap_or(0.0);
        
        let new_score = ContentScore::with_score(current_score + score_to_add);
        self.scores.insert(element_id, new_score);
    }

    /// Calculate the link density of an element
    pub fn get_link_density(&self, element: &ElementRef) -> f64 {
        let text_length = self.get_inner_text_length(element);
        if text_length == 0 {
            return 0.0;
        }

        let link_length = self.get_link_text_length(element);
        link_length as f64 / text_length as f64
    }

    /// Get the text density for specific tags within an element
    pub fn get_text_density(&self, element: &ElementRef, tags: &[&str]) -> f64 {
        let text_length = self.get_inner_text_length(element);
        if text_length == 0 {
            return 0.0;
        }

        let mut tag_text_length = 0;
        for &_tag in tags {
            // This would need proper implementation with DOM traversal
            // For now, simplified approach
            tag_text_length += text_length / 10; // Placeholder
        }

        tag_text_length as f64 / text_length as f64
    }

    /// Check if an element is probably visible
    pub fn is_probably_visible(&self, element: &ElementRef) -> bool {
        // Check for hidden styles
        if let Some(style) = element.value().attr("style") {
            if style.contains("display:none") || style.contains("display: none") {
                return false;
            }
        }

        // Check for hidden attribute
        if element.value().attr("hidden").is_some() {
            return false;
        }

        // Check for aria-hidden
        if let Some(aria_hidden) = element.value().attr("aria-hidden") {
            if aria_hidden == "true" {
                // Exception for fallback images
                if let Some(class) = element.value().attr("class") {
                    if !class.contains("fallback-image") {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get the character count of an element
    pub fn get_char_count(&self, element: &ElementRef, separator: Option<&str>) -> usize {
        let text = element.text().collect::<String>();
        if let Some(sep) = separator {
            text.matches(sep).count()
        } else {
            text.len()
        }
    }

    /// Score paragraphs and other content elements
    pub fn score_paragraphs<'a>(&mut self, elements: &[ElementRef<'a>]) -> Vec<(ElementRef<'a>, f64)> {
        let mut candidates = Vec::new();

        for element in elements {
            let parent_element = element.parent_element();
            let grand_parent_element = parent_element.and_then(|p| p.parent_element());

            let inner_text = element.text().collect::<String>();
            let inner_text_len = inner_text.len();

            // Skip if too short
            if inner_text_len < 25 {
                continue;
            }

            // Initialize parent and grandparent if needed
            if let Some(parent) = parent_element {
                if !self.has_score(&parent) {
                    self.initialize_node(&parent);
                }
            }

            if let Some(grandparent) = grand_parent_element {
                if !self.has_score(&grandparent) {
                    self.initialize_node(&grandparent);
                }
            }

            let mut content_score = 1.0;

            // Add points for any commas within this paragraph
            content_score += inner_text.matches(',').count() as f64;

            // For every 100 characters in this paragraph, add another point
            content_score += f64::min(inner_text_len as f64 / 100.0, 3.0);

            // Add the score to the parent
            if let Some(parent) = parent_element {
                self.add_score(&parent, content_score);
            }

            // Add half the score to the grandparent
            if let Some(grandparent) = grand_parent_element {
                self.add_score(&grandparent, content_score / 2.0);
            }
        }

        // Find all parent elements that have scores
        for element in elements {
            if let Some(parent) = element.parent_element() {
                let score = self.get_score(&parent);
                if score > 0.0 {
                    candidates.push((parent, score));
                }
            }
        }

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        candidates
    }

    fn get_element_id(&self, element: &ElementRef) -> String {
        // Generate a unique ID for the element based on its position in the DOM
        // This is a simplified approach - in a real implementation you'd want
        // a more robust way to identify elements
        format!("{:p}", element.value() as *const _)
    }

    fn has_score(&self, element: &ElementRef) -> bool {
        let element_id = self.get_element_id(element);
        self.scores.contains_key(&element_id)
    }

    fn get_inner_text_length(&self, element: &ElementRef) -> usize {
        element.text().collect::<String>().len()
    }

    fn get_link_text_length(&self, element: &ElementRef) -> usize {
        // This would need proper implementation to find all link elements
        // and sum their text lengths. For now, simplified approach.
        let text = element.text().collect::<String>();
        // Estimate based on common link patterns
        text.matches("http").count() * 20 // Rough estimate
    }
}

/// Calculate the text similarity between two strings
pub fn text_similarity(text_a: &str, text_b: &str) -> f64 {
    let tokens_a: Vec<&str> = text_a.split_whitespace().collect();
    let tokens_b: Vec<&str> = text_b.split_whitespace().collect();

    if tokens_a.is_empty() || tokens_b.is_empty() {
        return 0.0;
    }

    let mut intersections = 0;
    for token_a in &tokens_a {
        if tokens_b.contains(token_a) {
            intersections += 1;
        }
    }

    let union_length = tokens_a.len() + tokens_b.len() - intersections;
    if union_length == 0 {
        return 0.0;
    }

    intersections as f64 / union_length as f64
}

/// Check if an element should be removed based on its characteristics
pub fn should_remove_element(element: &ElementRef, tag_name: &str) -> bool {
    let class_and_id = format!("{} {}", 
        element.value().attr("class").unwrap_or(""),
        element.value().attr("id").unwrap_or("")
    );

    // Check for unlikely candidates
    if is_unlikely_candidate(&class_and_id) {
        return true;
    }

    // Additional checks based on tag name
    match tag_name.to_lowercase().as_str() {
        "script" | "style" | "link" | "meta" => true,
        "div" | "section" | "header" | "footer" | "aside" | "nav" => {
            // Check if it has very little content
            let text_content = element.text().collect::<String>();
            text_content.trim().len() < 25
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn test_content_score() {
        let score = ContentScore::new();
        assert_eq!(score.score, 0.0);
        assert_eq!(score.content_score, 0.0);

        let score_with_value = ContentScore::with_score(10.0);
        assert_eq!(score_with_value.score, 10.0);
        assert_eq!(score_with_value.content_score, 10.0);
    }

    #[test]
    fn test_text_similarity() {
        assert_eq!(text_similarity("hello world", "hello world"), 1.0);
        assert!(text_similarity("hello world", "hello there") > 0.0);
        assert!(text_similarity("hello world", "hello there") < 1.0);
        assert_eq!(text_similarity("hello", "world"), 0.0);
    }

    #[test]
    fn test_class_weight() {
        let html = r#"<div class="content main-article" id="article-body">Test</div>"#;
        let document = Html::parse_fragment(html);
        let selector = Selector::parse("div").unwrap();
        let element = document.select(&selector).next().unwrap();

        let scorer = ContentScorer::new();
        let weight = scorer.get_class_weight(&element);
        
        // Should have positive weight due to "content" and "main" indicators
        assert!(weight > 0.0);
    }

    #[test]
    fn test_initialize_node() {
        let html = r#"<div class="content">Test content</div>"#;
        let document = Html::parse_fragment(html);
        let selector = Selector::parse("div").unwrap();
        let element = document.select(&selector).next().unwrap();

        let mut scorer = ContentScorer::new();
        let score = scorer.initialize_node(&element);
        
        // Div gets 5 points, plus class weight
        assert!(score >= 5.0);
    }
}