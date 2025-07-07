# Readability for Rust

A Rust port of [Mozilla's Readability library](https://github.com/mozilla/readability) for extracting article content from web pages. This library removes clutter like ads, navigation, and sidebars to extract the main content, making it perfect for reader mode implementations, content archiving, and text analysis.

## Features

- **Content Extraction**: Automatically identifies and extracts the main article content from web pages
- **Cleanup**: Removes ads, navigation menus, sidebars, and other non-content elements
- **Metadata Extraction**: Extracts title, author, published date, and other article metadata
- **Multiple Output Formats**: Support for JSON, plain text, and clean HTML output
- **CLI Tool**: Command-line interface for batch processing and shell scripting
- **Readability Check**: Quick assessment of whether a document is suitable for content extraction
- **Rust Performance**: Native Rust implementation for speed and memory safety

## Installation

### As a Library

Add this to your `Cargo.toml`:

```toml
[dependencies]
readability = "0.1.0"
```

### CLI Tool

Install the command-line tool:

```bash
cargo install readability
```

Or build from source:

```bash
git clone https://github.com/mozilla/readability-rust
cd readability-rust
cargo build --release
./target/release/readability --help
```

## Library Usage

### Basic Example

```rust
use readability::{Readability, ReadabilityOptions};

let html = r#"
<html>
  <head><title>Example Article</title></head>
  <body>
    <nav>Navigation menu...</nav>
    <article>
      <h1>Main Article Title</h1>
      <p>This is the main content of the article that should be extracted.</p>
      <p>More content here...</p>
    </article>
    <aside>Sidebar content that should be removed.</aside>
  </body>
</html>
"#;

let mut readability = Readability::new(html, None).unwrap();
if let Some(article) = readability.parse() {
    println!("Title: {}", article.title.unwrap_or_default());
    println!("Content: {}", article.content.unwrap_or_default());
    println!("Text: {}", article.text_content.unwrap_or_default());
    println!("Length: {} characters", article.length.unwrap_or(0));
}
```

### With Custom Options

```rust
use readability::{Readability, ReadabilityOptions};

let options = ReadabilityOptions {
    debug: true,
    char_threshold: 300,
    keep_classes: true,
    ..Default::default()
};

let mut readability = Readability::new(html, Some(options)).unwrap();
let article = readability.parse().expect("Failed to parse article");
```

### With Base URI

```rust
use readability::Readability;

let base_uri = "https://example.com/articles/";
let mut readability = Readability::new_with_base_uri(html, base_uri, None).unwrap();
let article = readability.parse().unwrap();
```

### Check Readability

```rust
use readability::is_probably_readerable;

let html = "<html><body><p>Short content</p></body></html>";
if is_probably_readerable(html, None) {
    println!("Document appears to be readerable");
} else {
    println!("Document may not have enough content");
}
```

## CLI Usage

### Basic Usage

```bash
# Read from file and output JSON
readability -i article.html

# Read from stdin
curl https://example.com/article | readability

# Output as plain text
readability -i article.html -f text

# Output as clean HTML
readability -i article.html -f html -o clean.html
```

### CLI Options

```bash
readability [OPTIONS]

Options:
  -i, --input <FILE>              Input HTML file (use '-' for stdin)
  -o, --output <FILE>             Output file (default: stdout)
  -f, --format <FORMAT>           Output format: json, text, html [default: json]
  -b, --base-uri <URI>            Base URI for resolving relative URLs
  -d, --debug                     Enable debug output
  -c, --check                     Only check if document is readable (exit code 0=readable, 1=not readable)
      --min-content-length <LENGTH> Minimum content length for readability check [default: 140]
      --char-threshold <CHARS>    Minimum character threshold for article content [default: 500]
      --keep-classes              Keep CSS classes in output
      --disable-json-ld           Disable JSON-LD parsing for metadata
  -h, --help                      Print help
  -V, --version                   Print version
```

### Examples

```bash
# Check if a document is readerable
readability -c -i questionable.html
echo $?  # 0 if readable, 1 if not

# Extract with debugging enabled
readability -d -i article.html -f text

# Process with base URI for relative links
readability -b "https://example.com/" -i article.html -f html

# Keep CSS classes in output
readability --keep-classes -i styled-article.html -f html

# Batch processing
find articles/ -name "*.html" -exec readability -i {} -f text \;
```

## API Reference

### `Readability`

The main parser struct for extracting content.

#### Methods

- `new(html: &str, options: Option<ReadabilityOptions>) -> Result<Self, ReadabilityError>`
- `new_with_base_uri(html: &str, base_uri: &str, options: Option<ReadabilityOptions>) -> Result<Self, ReadabilityError>`
- `parse(&mut self) -> Option<Article>`

### `ReadabilityOptions`

Configuration options for the parser.

```rust
pub struct ReadabilityOptions {
    pub debug: bool,                        // Enable debug logging
    pub max_elems_to_parse: usize,         // Maximum elements to parse (0 = no limit)
    pub nb_top_candidates: usize,          // Number of top candidates to consider
    pub char_threshold: usize,             // Minimum character threshold
    pub classes_to_preserve: Vec<String>,  // CSS classes to preserve
    pub keep_classes: bool,                // Whether to keep CSS classes
    pub disable_json_ld: bool,             // Disable JSON-LD parsing
    pub allowed_video_regex: Option<Regex>, // Custom video URL regex
    pub link_density_modifier: f64,        // Link density modifier
}
```

### `Article`

The extracted article content and metadata.

```rust
pub struct Article {
    pub title: Option<String>,           // Article title
    pub content: Option<String>,         // HTML content
    pub text_content: Option<String>,    // Plain text content
    pub length: Option<usize>,           // Content length in characters
    pub excerpt: Option<String>,         // Article excerpt/description
    pub byline: Option<String>,          // Author information
    pub dir: Option<String>,             // Content direction (ltr/rtl)
    pub site_name: Option<String>,       // Site name
    pub lang: Option<String>,            // Content language
    pub published_time: Option<String>,  // Published time
}
```

### Functions

- `is_probably_readerable(html: &str, options: Option<ReadabilityOptions>) -> bool`

## Performance

This Rust implementation offers several performance advantages:

- **Memory Safety**: No risk of memory leaks or buffer overflows
- **Zero-Cost Abstractions**: Rust's design ensures minimal runtime overhead
- **Parallel Processing**: Safe concurrency for batch processing
- **Native Speed**: Compiled to native machine code

## Comparison with Original

This implementation aims to provide the same functionality as Mozilla's JavaScript Readability library while offering:

- Better performance through native compilation
- Memory safety guarantees
- Strong typing
- Easy integration with Rust projects
- Command-line interface for scripting

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development

```bash
# Clone the repository
git clone https://github.com/mozilla/readability-rust
cd readability-rust

# Run tests
cargo test

# Run with debug logging
RUST_LOG=debug cargo run -- -d -i example.html

# Build release version
cargo build --release
```

### Testing

The project includes comprehensive tests:

```bash
# Run all tests
cargo test

# Run with coverage
cargo test --coverage

# Run specific test module
cargo test regexps
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

This is a port of Mozilla's Readability library, which is also licensed under Apache 2.0.

## Acknowledgments

- [Mozilla Readability](https://github.com/mozilla/readability) - The original JavaScript implementation
- [Arc90 Readability](http://code.google.com/p/arc90labs-readability) - The original algorithm
- The Rust community for excellent HTML parsing libraries

## Related Projects

- [readability-cli](https://www.npmjs.com/package/readability-cli) - Node.js CLI for the original library
- [python-readability](https://github.com/buriy/python-readability) - Python port
- [go-readability](https://github.com/go-shiori/go-readability) - Go port
