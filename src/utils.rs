//! Utility functions for the Readability parser

use scraper::{ElementRef, Element};
use url::Url;
use std::collections::HashSet;

/// HTML elements that are considered phrasing content
pub const PHRASING_ELEMS: &[&str] = &[
    "ABBR", "AUDIO", "B", "BDO", "BR", "BUTTON", "CITE", "CODE", "DATA",
    "DATALIST", "DFN", "EM", "EMBED", "I", "IMG", "INPUT", "KBD", "LABEL",
    "MARK", "MATH", "METER", "NOSCRIPT", "OBJECT", "OUTPUT", "PROGRESS",
    "Q", "RUBY", "SAMP", "SCRIPT", "SELECT", "SMALL", "SPAN", "STRONG",
    "SUB", "SUP", "TEXTAREA", "TIME", "VAR", "WBR"
];

/// Elements that can be converted from DIV to P
pub const DIV_TO_P_ELEMS: &[&str] = &[
    "BLOCKQUOTE", "DL", "DIV", "IMG", "OL", "P", "PRE", "TABLE", "UL"
];

/// Elements with unlikely roles for main content
pub const UNLIKELY_ROLES: &[&str] = &[
    "menu", "menubar", "complementary", "navigation", "alert",
    "alertdialog", "dialog"
];

/// Presentational attributes that should be removed
pub const PRESENTATIONAL_ATTRIBUTES: &[&str] = &[
    "align", "background", "bgcolor", "border", "cellpadding", "cellspacing",
    "frame", "hspace", "rules", "style", "valign", "vspace"
];

/// Convert relative URLs to absolute URLs
pub fn to_absolute_uri(uri: &str, base_uri: &str) -> String {
    // Handle hash links - keep them as-is if base matches document
    if uri.starts_with('#') {
        return uri.to_string();
    }

    // Try to resolve against base URI
    match Url::parse(base_uri) {
        Ok(base) => {
            match base.join(uri) {
                Ok(absolute_url) => absolute_url.to_string(),
                Err(_) => uri.to_string(), // Return original if join fails
            }
        }
        Err(_) => uri.to_string(), // Return original if base URL is invalid
    }
}

/// Check if a string is a valid URL
pub fn is_url(text: &str) -> bool {
    Url::parse(text).is_ok()
}

/// Get the inner text content of an element
pub fn get_inner_text(element: &ElementRef, normalize_spaces: bool) -> String {
    let text = element.text().collect::<Vec<_>>().join(" ");
    if normalize_spaces {
        normalize_whitespace(&text)
    } else {
        text
    }
}

/// Normalize whitespace in text
pub fn normalize_whitespace(text: &str) -> String {
    // Replace multiple whitespace characters with single space
    let mut result = String::new();
    let mut prev_was_space = false;
    
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_was_space {
                result.push(' ');
                prev_was_space = true;
            }
        } else {
            result.push(ch);
            prev_was_space = false;
        }
    }
    
    result.trim().to_string()
}

/// Get the character count of text
pub fn get_char_count(text: &str, separator: Option<char>) -> usize {
    if let Some(sep) = separator {
        text.matches(sep).count()
    } else {
        text.chars().count()
    }
}

/// Check if an element is phrasing content
pub fn is_phrasing_content(tag_name: &str) -> bool {
    PHRASING_ELEMS.contains(&tag_name.to_uppercase().as_str())
}

/// Check if an element is a single image
pub fn is_single_image(element: &ElementRef) -> bool {
    let tag_name = element.value().name().to_uppercase();
    if tag_name == "IMG" {
        return true;
    }

    // Check if element contains only one img child
    let children: Vec<_> = element.children().collect();
    if children.len() == 1 {
        if let Some(child_element) = children[0].value().as_element() {
            return child_element.name().to_uppercase() == "IMG";
        }
    }

    false
}

/// Check if an element is probably visible
pub fn is_node_visible(element: &ElementRef) -> bool {
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
            } else {
                return false;
            }
        }
    }

    true
}

/// Check if element has ancestor with specific tag
pub fn has_ancestor_tag(
    element: &ElementRef,
    tag_name: &str,
    max_depth: Option<usize>,
    filter_fn: Option<fn(&ElementRef) -> bool>
) -> bool {
    let target_tag = tag_name.to_uppercase();
    let mut current = element.parent_element();
    let mut depth = 0;
    
    while let Some(parent) = current {
        if let Some(max) = max_depth {
            if depth >= max {
                break;
            }
        }
        
        if parent.value().name().to_uppercase() == target_tag {
            if let Some(filter) = filter_fn {
                if filter(&parent) {
                    return true;
                }
            } else {
                return true;
            }
        }
        
        current = parent.parent_element();
        depth += 1;
    }
    
    false
}

/// Get node ancestors up to a certain depth
pub fn get_node_ancestors<'a>(element: &'a ElementRef<'a>, max_depth: Option<usize>) -> Vec<ElementRef<'a>> {
    let mut ancestors = Vec::new();
    let mut current = element.parent_element();
    let mut depth = 0;
    let max_depth = max_depth.unwrap_or(100); // Default max depth

    while let Some(ancestor) = current {
        ancestors.push(ancestor);
        depth += 1;
        if depth >= max_depth {
            break;
        }
        current = ancestor.parent_element();
    }

    ancestors
}

/// Check if an element is without content
pub fn is_element_without_content(element: &ElementRef) -> bool {
    let tag_name = element.value().name().to_uppercase();
    
    match tag_name.as_str() {
        "IMG" | "VIDEO" | "AUDIO" | "EMBED" | "OBJECT" | "IFRAME" => false,
        _ => {
            let text_content = get_inner_text(element, true);
            text_content.is_empty()
        }
    }
}

/// Check if an element has a single tag inside
pub fn has_single_tag_inside_element(element: &ElementRef, tag: &str) -> bool {
    let children: Vec<_> = element.children()
        .filter_map(|child| child.value().as_element())
        .collect();
    
    children.len() == 1 && 
    children[0].name().eq_ignore_ascii_case(tag)
}

/// Check if an element has child block elements
pub fn has_child_block_element(element: &ElementRef) -> bool {
    for child in element.children() {
        if let Some(child_element) = child.value().as_element() {
            let tag_name = child_element.name().to_uppercase();
            if !is_phrasing_content(&tag_name) {
                return true;
            }
        }
    }
    false
}

/// Clean attributes from an element (conceptual - actual implementation would modify DOM)
pub fn should_clean_attribute(attr_name: &str) -> bool {
    PRESENTATIONAL_ATTRIBUTES.contains(&attr_name.to_lowercase().as_str())
}

/// Extract text content and handle encoding
pub fn extract_text_content(element: &ElementRef) -> String {
    element.text().collect::<Vec<_>>().join(" ")
}

/// Word count for text
pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Check if text looks like a title
pub fn is_title_candidate(text: &str, current_title: Option<&str>) -> bool {
    let word_count = word_count(text);
    
    // Should be reasonable length
    if word_count < 2 || word_count > 15 {
        return false;
    }
    
    // If we have a current title, check similarity
    if let Some(title) = current_title {
        let similarity = text_similarity(text, title);
        similarity > 0.3 // At least 30% similar
    } else {
        true
    }
}

/// Calculate text similarity (Jaccard similarity)
pub fn text_similarity(text_a: &str, text_b: &str) -> f64 {
    let words_a: HashSet<&str> = text_a.split_whitespace().collect();
    let words_b: HashSet<&str> = text_b.split_whitespace().collect();
    
    if words_a.is_empty() && words_b.is_empty() {
        return 1.0;
    }
    
    if words_a.is_empty() || words_b.is_empty() {
        return 0.0;
    }
    
    let intersection = words_a.intersection(&words_b).count();
    let union = words_a.union(&words_b).count();
    
    intersection as f64 / union as f64
}

/// Unescape HTML entities
pub fn unescape_html_entities(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}

/// Remove extra whitespace and normalize text
pub fn clean_text(text: &str) -> String {
    let unescaped = unescape_html_entities(text);
    normalize_whitespace(&unescaped)
}

/// Get link density for an element
pub fn get_link_density(element: &ElementRef) -> f64 {
    let total_text_length = get_inner_text(element, false).len();
    if total_text_length == 0 {
        return 0.0;
    }
    
    // Count text inside link elements
    let mut link_text_length = 0;
    for descendant in element.descendants() {
        if let Some(descendant_element) = descendant.value().as_element() {
            if descendant_element.name().eq_ignore_ascii_case("a") {
                let link_element = ElementRef::wrap(descendant).unwrap();
                link_text_length += get_inner_text(&link_element, false).len();
            }
        }
    }
    
    link_text_length as f64 / total_text_length as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use scraper::{Html, Selector};

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("hello    world\n\ntest"), "hello world test");
        assert_eq!(normalize_whitespace("  \n\t  "), "");
        assert_eq!(normalize_whitespace("single"), "single");
    }

    #[test]
    fn test_word_count() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("  hello   world  test  "), 3);
        assert_eq!(word_count(""), 0);
    }

    #[test]
    fn test_text_similarity() {
        assert_eq!(text_similarity("hello world", "hello world"), 1.0);
        assert!(text_similarity("hello world", "hello there") > 0.0);
        assert!(text_similarity("hello world", "hello there") < 1.0);
        assert_eq!(text_similarity("hello", "world"), 0.0);
        assert_eq!(text_similarity("", ""), 1.0);
    }

    #[test]
    fn test_is_url() {
        assert!(is_url("https://example.com"));
        assert!(is_url("http://example.com"));
        assert!(!is_url("not a url"));
        assert!(!is_url(""));
    }

    #[test]
    fn test_to_absolute_uri() {
        let base = "https://example.com/path/";
        assert_eq!(to_absolute_uri("#anchor", base), "#anchor");
        assert_eq!(to_absolute_uri("/absolute", base), "https://example.com/absolute");
        assert_eq!(to_absolute_uri("relative", base), "https://example.com/path/relative");
    }

    #[test]
    fn test_is_phrasing_content() {
        assert!(is_phrasing_content("span"));
        assert!(is_phrasing_content("STRONG"));
        assert!(!is_phrasing_content("div"));
        assert!(!is_phrasing_content("section"));
    }

    #[test]
    fn test_unescape_html_entities() {
        assert_eq!(unescape_html_entities("&lt;div&gt;"), "<div>");
        assert_eq!(unescape_html_entities("&quot;hello&quot;"), "\"hello\"");
        assert_eq!(unescape_html_entities("&amp;nbsp;"), "&nbsp;");
    }

    #[test]
    fn test_is_title_candidate() {
        assert!(is_title_candidate("A Great Article Title", None));
        assert!(!is_title_candidate("A", None)); // Too short
        assert!(!is_title_candidate("This is way too long to be a reasonable title for an article", None)); // Too long
    }

    #[test]
    fn test_get_char_count() {
        assert_eq!(get_char_count("hello,world,test", Some(',')), 2);
        assert_eq!(get_char_count("hello world", None), 11);
    }
}