//! Regular expressions used throughout the Readability parser

use regex::Regex;
use std::sync::OnceLock;

/// Regular expressions for identifying content patterns
pub struct ReadabilityRegexps {
    pub unlikely_candidates: Regex,
    pub ok_maybe_its_candidate: Regex,
    pub positive: Regex,
    pub negative: Regex,
    pub extraneous: Regex,
    pub byline: Regex,
    pub replace_fonts: Regex,
    pub normalize: Regex,
    pub videos: Regex,
    pub share_elements: Regex,
    pub next_link: Regex,
    pub prev_link: Regex,
    pub tokenize: Regex,
    pub whitespace: Regex,
    pub has_content: Regex,
    pub hash_url: Regex,
    pub srcset_url: Regex,
    pub b64_data_url: Regex,
    pub commas: Regex,
    pub json_ld_article_types: Regex,
    pub ad_words: Regex,
    pub loading_words: Regex,
}

impl ReadabilityRegexps {
    pub fn new() -> Self {
        Self {
            unlikely_candidates: Regex::new(
                r"(?i)-ad-|ai2html|banner|breadcrumbs|combx|comment|community|cover-wrap|disqus|extra|footer|gdpr|header|legends|menu|related|remark|replies|rss|shoutbox|sidebar|skyscraper|social|sponsor|supplemental|ad-break|agegate|pagination|pager|popup|yom-remote"
            ).unwrap(),
            
            ok_maybe_its_candidate: Regex::new(
                r"(?i)and|article|body|column|content|main|mathjax|shadow"
            ).unwrap(),
            
            positive: Regex::new(
                r"(?i)article|body|content|entry|hentry|h-entry|main|page|pagination|post|text|blog|story"
            ).unwrap(),
            
            negative: Regex::new(
                r"(?i)-ad-|hidden|^hid$| hid$| hid |^hid |banner|combx|comment|com-|contact|footer|gdpr|masthead|media|meta|outbrain|promo|related|scroll|share|shoutbox|sidebar|skyscraper|sponsor|shopping|tags|widget"
            ).unwrap(),
            
            extraneous: Regex::new(
                r"(?i)print|archive|comment|discuss|e[\-]?mail|share|reply|all|login|sign|single|utility"
            ).unwrap(),
            
            byline: Regex::new(
                r"(?i)byline|author|dateline|written\s*by|p-author|by\s+\w+"
            ).unwrap(),
            
            replace_fonts: Regex::new(
                r"<(\/?)font[^>]*>"
            ).unwrap(),
            
            normalize: Regex::new(
                r"\s{2,}"
            ).unwrap(),
            
            videos: Regex::new(
                r"\/\/(www\.)?((dailymotion|youtube|youtube-nocookie|player\.vimeo|v\.qq|bilibili|live.bilibili)\.com|(archive|upload\.wikimedia)\.org|player\.twitch\.tv)"
            ).unwrap(),
            
            share_elements: Regex::new(
                r"(\b|_)(share|sharedaddy)(\b|_)"
            ).unwrap(),
            
            next_link: Regex::new(
                r"(?i)(next|weiter|continue|>([^\|]|$)|»([^\|]|$))"
            ).unwrap(),
            
            prev_link: Regex::new(
                r"(?i)(prev|earl|old|new|<|«)"
            ).unwrap(),
            
            tokenize: Regex::new(
                r"\W+"
            ).unwrap(),
            
            whitespace: Regex::new(
                r"^\s*$"
            ).unwrap(),
            
            has_content: Regex::new(
                r"\S"
            ).unwrap(),
            
            hash_url: Regex::new(
                r"^#.+"
            ).unwrap(),
            
            srcset_url: Regex::new(
                r"(\S+)(\s+[\d.]+[xw])?(\s*(?:,|$))"
            ).unwrap(),
            
            b64_data_url: Regex::new(
                r"(?i)^data:\s*([^\s;,]+)\s*;\s*base64\s*,"
            ).unwrap(),
            
            // Commas as used in Latin, Sindhi, Chinese and various other scripts
            commas: Regex::new(
                r"\u{002C}|\u{060C}|\u{FE50}|\u{FE10}|\u{FE11}|\u{2E41}|\u{2E34}|\u{2E32}|\u{FF0C}"
            ).unwrap(),
            
            // Schema.org Article types
            json_ld_article_types: Regex::new(
                r"^Article|AdvertiserContentArticle|NewsArticle|AnalysisNewsArticle|AskPublicNewsArticle|BackgroundNewsArticle|OpinionNewsArticle|ReportageNewsArticle|ReviewNewsArticle|Report|SatiricalArticle|ScholarlyArticle|MedicalScholarlyArticle|SocialMediaPosting|BlogPosting|LiveBlogPosting|DiscussionForumPosting|TechArticle|APIReference$"
            ).unwrap(),
            
            // Words commonly used for ad blocks or loading indicators
            ad_words: Regex::new(
                r"(?i)^(ad(vertising|vertisement)?|pub(licité)?|werb(ung)?|广告|Реклама|Anuncio)$"
            ).unwrap(),
            
            loading_words: Regex::new(
                r"(?i)^((loading|正在加载|Загрузка|chargement|cargando)(…|\.\.\.)?)$"
            ).unwrap(),
        }
    }
}

/// Global instance of readability regexps
static REGEXPS: OnceLock<ReadabilityRegexps> = OnceLock::new();

/// Get the global regexps instance
pub fn get_regexps() -> &'static ReadabilityRegexps {
    REGEXPS.get_or_init(ReadabilityRegexps::new)
}

/// Check if a string matches the unlikely candidates pattern
pub fn is_unlikely_candidate(text: &str) -> bool {
    let regexps = get_regexps();
    regexps.unlikely_candidates.is_match(text) && !regexps.ok_maybe_its_candidate.is_match(text)
}

/// Check if a string has positive content indicators
pub fn has_positive_indicators(text: &str) -> bool {
    get_regexps().positive.is_match(text)
}

/// Check if a string has negative content indicators
pub fn has_negative_indicators(text: &str) -> bool {
    get_regexps().negative.is_match(text)
}

/// Check if a string contains byline indicators
pub fn is_byline(text: &str) -> bool {
    get_regexps().byline.is_match(text)
}

/// Check if a URL is a video URL
pub fn is_video_url(url: &str) -> bool {
    get_regexps().videos.is_match(url)
}

/// Normalize whitespace in text
pub fn normalize_whitespace(text: &str) -> String {
    get_regexps().normalize.replace_all(text, " ").to_string()
}

/// Check if text is only whitespace
pub fn is_whitespace(text: &str) -> bool {
    get_regexps().whitespace.is_match(text)
}

/// Check if text has content (non-whitespace)
pub fn has_content(text: &str) -> bool {
    get_regexps().has_content.is_match(text)
}

/// Check if a string contains ad-related words
pub fn contains_ad_words(text: &str) -> bool {
    get_regexps().ad_words.is_match(text)
}

/// Check if a string contains loading-related words
pub fn contains_loading_words(text: &str) -> bool {
    get_regexps().loading_words.is_match(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unlikely_candidates() {
        assert!(is_unlikely_candidate("sidebar-ad navigation"));
        assert!(is_unlikely_candidate("comment-section"));
        assert!(!is_unlikely_candidate("main-content"));
        assert!(!is_unlikely_candidate("article-body"));
    }

    #[test]
    fn test_positive_indicators() {
        assert!(has_positive_indicators("article-content"));
        assert!(has_positive_indicators("main-body"));
        assert!(!has_positive_indicators("sidebar"));
    }

    #[test]
    fn test_video_urls() {
        assert!(is_video_url("https://www.youtube.com/watch?v=test"));
        assert!(is_video_url("https://player.vimeo.com/video/123"));
        assert!(!is_video_url("https://example.com/image.jpg"));
    }

    #[test]
    fn test_whitespace() {
        assert!(is_whitespace("   \n\t  "));
        assert!(!is_whitespace("some text"));
        
        assert!(has_content("some text"));
        assert!(!has_content("   \n\t  "));
    }

    #[test]
    fn test_normalize_whitespace() {
        assert_eq!(normalize_whitespace("hello    world\n\ntest"), "hello world test");
    }

    #[test]
    fn test_byline() {
        assert!(is_byline("by author"));
        assert!(is_byline("written by John Doe"));
        assert!(!is_byline("random text"));
    }
}