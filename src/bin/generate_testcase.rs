//! Generate and verify Mozilla readability test cases
//! 
//! This is the Rust equivalent of mozilla-readability/test/generate-testcase.js
//! 
//! Usage:
//!   cargo run --bin generate_testcase -- <test-case-name> [url]
//!   cargo run --bin generate_testcase -- all
//!   cargo run --bin generate_testcase -- verify

use clap::{Arg, Command};
use readability::{Readability, ReadabilityOptions, is_probably_readerable};
use serde::{Deserialize, Serialize};
use serde_json;
use std::{fs, path::Path, io::Write};

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedMetadata {
    title: Option<String>,
    byline: Option<String>,
    dir: Option<String>,
    excerpt: Option<String>,
    #[serde(rename = "siteName")]
    site_name: Option<String>,
    #[serde(rename = "publishedTime")]
    published_time: Option<String>,
    readerable: bool,
    lang: Option<String>,
}

#[derive(Debug)]
struct TestResult {
    name: String,
    success: bool,
    errors: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("generate_testcase")
        .about("Generate and verify Mozilla readability test cases")
        .arg(Arg::new("command")
            .help("The command to run: test case name, 'all', or 'verify'")
            .required(true)
            .index(1))
        .arg(Arg::new("url")
            .help("URL to fetch content from (only needed for new test cases)")
            .index(2))
        .get_matches();

    let command = matches.get_one::<String>("command").unwrap();
    let url = matches.get_one::<String>("url");

    let test_pages_dir = Path::new("mozilla-readability/test/test-pages");
    
    if !test_pages_dir.exists() {
        eprintln!("Error: Mozilla test pages directory not found at {:?}", test_pages_dir);
        eprintln!("Make sure you've initialized the git submodules with:");
        eprintln!("  git submodule update --init --recursive");
        std::process::exit(1);
    }

    match command.as_str() {
        "all" => {
            println!("Running all test cases...");
            let results = run_all_test_cases(test_pages_dir)?;
            print_summary(&results);
        }
        "verify" => {
            println!("Verifying all test cases...");
            let results = verify_all_test_cases(test_pages_dir)?;
            print_summary(&results);
        }
        test_name => {
            let test_dir = test_pages_dir.join(test_name);
            
            if test_dir.exists() {
                println!("Regenerating test case: {}", test_name);
                regenerate_test_case(&test_dir, url)?;
            } else {
                if url.is_none() {
                    eprintln!("Error: URL required for new test case '{}'", test_name);
                    std::process::exit(1);
                }
                println!("Creating new test case: {}", test_name);
                create_new_test_case(&test_dir, url.unwrap())?;
            }
        }
    }

    Ok(())
}

fn run_all_test_cases(test_pages_dir: &Path) -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    
    for entry in fs::read_dir(test_pages_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let result = run_single_test_case(&path)?;
            results.push(result);
        }
    }
    
    Ok(results)
}

fn verify_all_test_cases(test_pages_dir: &Path) -> Result<Vec<TestResult>, Box<dyn std::error::Error>> {
    let mut results = Vec::new();
    
    for entry in fs::read_dir(test_pages_dir)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            let result = verify_single_test_case(&path)?;
            results.push(result);
        }
    }
    
    Ok(results)
}

fn run_single_test_case(test_dir: &Path) -> Result<TestResult, Box<dyn std::error::Error>> {
    let name = test_dir.file_name().unwrap().to_str().unwrap().to_string();
    let mut errors = Vec::new();
    
    let source_path = test_dir.join("source.html");
    
    if !source_path.exists() {
        return Ok(TestResult {
            name,
            success: false,
            errors: vec!["source.html not found".to_string()],
        });
    }
    
    let source = fs::read_to_string(&source_path)?;
    
    // Run readability on the source
    match run_readability_on_source(&source) {
        Ok((content, metadata)) => {
            // Write expected content
            let expected_content_path = test_dir.join("expected.html");
            if let Err(e) = fs::write(&expected_content_path, &content) {
                errors.push(format!("Failed to write expected.html: {}", e));
            }
            
            // Write expected metadata
            let expected_metadata_path = test_dir.join("expected-metadata.json");
            let metadata_json = serde_json::to_string_pretty(&metadata)?;
            if let Err(e) = fs::write(&expected_metadata_path, &metadata_json) {
                errors.push(format!("Failed to write expected-metadata.json: {}", e));
            }
            
            println!("✓ Generated test case: {}", name);
        }
        Err(e) => {
            errors.push(format!("Readability parsing failed: {}", e));
        }
    }
    
    Ok(TestResult {
        name,
        success: errors.is_empty(),
        errors,
    })
}

fn verify_single_test_case(test_dir: &Path) -> Result<TestResult, Box<dyn std::error::Error>> {
    let name = test_dir.file_name().unwrap().to_str().unwrap().to_string();
    let mut errors = Vec::new();
    
    let source_path = test_dir.join("source.html");
    let expected_content_path = test_dir.join("expected.html");
    let expected_metadata_path = test_dir.join("expected-metadata.json");
    
    // Check if all required files exist
    if !source_path.exists() {
        errors.push("source.html not found".to_string());
    }
    if !expected_content_path.exists() {
        errors.push("expected.html not found".to_string());
    }
    if !expected_metadata_path.exists() {
        errors.push("expected-metadata.json not found".to_string());
    }
    
    if !errors.is_empty() {
        return Ok(TestResult { name, success: false, errors });
    }
    
    // Load files
    let source = fs::read_to_string(&source_path)?;
    let expected_content = fs::read_to_string(&expected_content_path)?;
    let expected_metadata_json = fs::read_to_string(&expected_metadata_path)?;
    let expected_metadata: ExpectedMetadata = serde_json::from_str(&expected_metadata_json)?;
    
    // Run readability and compare
    match run_readability_on_source(&source) {
        Ok((actual_content, actual_metadata)) => {
            // Compare content (normalize whitespace for comparison)
            let expected_normalized = normalize_html(&expected_content);
            let actual_normalized = normalize_html(&actual_content);
            
            if expected_normalized != actual_normalized {
                errors.push("Content mismatch".to_string());
            }
            
            // Compare metadata
            if actual_metadata.title != expected_metadata.title {
                errors.push(format!("Title mismatch. Expected: {:?}, Got: {:?}", 
                    expected_metadata.title, actual_metadata.title));
            }
            if actual_metadata.byline != expected_metadata.byline {
                errors.push(format!("Byline mismatch. Expected: {:?}, Got: {:?}", 
                    expected_metadata.byline, actual_metadata.byline));
            }
            if actual_metadata.excerpt != expected_metadata.excerpt {
                errors.push(format!("Excerpt mismatch. Expected: {:?}, Got: {:?}", 
                    expected_metadata.excerpt, actual_metadata.excerpt));
            }
            if actual_metadata.site_name != expected_metadata.site_name {
                errors.push(format!("Site name mismatch. Expected: {:?}, Got: {:?}", 
                    expected_metadata.site_name, actual_metadata.site_name));
            }
            if actual_metadata.readerable != expected_metadata.readerable {
                errors.push(format!("Readerable mismatch. Expected: {}, Got: {}", 
                    expected_metadata.readerable, actual_metadata.readerable));
            }
            
            if errors.is_empty() {
                println!("✓ Verified test case: {}", name);
            } else {
                println!("✗ Failed test case: {}", name);
                for error in &errors {
                    println!("  - {}", error);
                }
            }
        }
        Err(e) => {
            errors.push(format!("Readability parsing failed: {}", e));
        }
    }
    
    Ok(TestResult {
        name,
        success: errors.is_empty(),
        errors,
    })
}

fn regenerate_test_case(test_dir: &Path, _url: Option<&String>) -> Result<(), Box<dyn std::error::Error>> {
    let name = test_dir.file_name().unwrap().to_str().unwrap().to_string();
    
    let source_path = test_dir.join("source.html");
    if !source_path.exists() {
        eprintln!("Error: source.html not found in {}", test_dir.display());
        return Ok(());
    }
    
    let source = fs::read_to_string(&source_path)?;
    
    match run_readability_on_source(&source) {
        Ok((content, metadata)) => {
            // Write expected content
            let expected_content_path = test_dir.join("expected.html");
            fs::write(&expected_content_path, &content)?;
            
            // Write expected metadata
            let expected_metadata_path = test_dir.join("expected-metadata.json");
            let metadata_json = serde_json::to_string_pretty(&metadata)?;
            fs::write(&expected_metadata_path, &metadata_json)?;
            
            println!("✓ Regenerated test case: {}", name);
        }
        Err(e) => {
            eprintln!("✗ Failed to regenerate test case {}: {}", name, e);
        }
    }
    
    Ok(())
}

fn create_new_test_case(test_dir: &Path, url: &str) -> Result<(), Box<dyn std::error::Error>> {
    // For now, just print a message about creating new test cases
    // In a real implementation, you'd fetch the URL content
    println!("Note: Creating new test cases from URLs is not yet implemented.");
    println!("To create a new test case:");
    println!("1. Create directory: {}", test_dir.display());
    println!("2. Save the HTML source as source.html");
    println!("3. Run: cargo run --bin generate_testcase -- {}", 
             test_dir.file_name().unwrap().to_str().unwrap());
    println!("URL provided: {}", url);
    
    Ok(())
}

fn run_readability_on_source(source: &str) -> Result<(String, ExpectedMetadata), Box<dyn std::error::Error>> {
    let uri = "http://fakehost/test/page.html";
    
    // Run isProbablyReaderable
    let readerable = is_probably_readerable(source, None);
    
    // Run readability
    let mut parser = Readability::new_with_base_uri(source, uri, Some(ReadabilityOptions {
        classes_to_preserve: vec!["caption".to_string()],
        ..Default::default()
    }))?;
    
    match parser.parse() {
        Some(article) => {
            let content = article.content.unwrap_or_else(|| "<div></div>".to_string());
            let metadata = ExpectedMetadata {
                title: article.title,
                byline: article.byline,
                dir: article.dir,
                excerpt: article.excerpt,
                site_name: article.site_name,
                published_time: article.published_time,
                readerable,
                lang: article.lang,
            };
            
            Ok((pretty_print_html(&content), metadata))
        }
        None => {
            // If parsing failed but readerable is true, this is an issue
            if readerable {
                return Err("Readability parsing failed but isProbablyReaderable returned true".into());
            }
            
            // Return empty content with readerable = false
            let metadata = ExpectedMetadata {
                title: None,
                byline: None,
                dir: None,
                excerpt: None,
                site_name: None,
                published_time: None,
                readerable,
                lang: None,
            };
            
            Ok(("<div></div>".to_string(), metadata))
        }
    }
}

fn pretty_print_html(html: &str) -> String {
    // Basic HTML pretty printing - in a real implementation you might want to use a proper formatter
    html.to_string()
}

fn normalize_html(html: &str) -> String {
    // Normalize whitespace for comparison
    html.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn print_summary(results: &[TestResult]) {
    let total = results.len();
    let passed = results.iter().filter(|r| r.success).count();
    let failed = total - passed;
    
    println!("\n=== Test Summary ===");
    println!("Total:  {}", total);
    println!("Passed: {}", passed);
    println!("Failed: {}", failed);
    
    if failed > 0 {
        println!("\nFailed tests:");
        for result in results.iter().filter(|r| !r.success) {
            println!("  {}", result.name);
            for error in &result.errors {
                println!("    - {}", error);
            }
        }
    }
    
    let success_rate = if total > 0 { 
        (passed as f64 / total as f64) * 100.0 
    } else { 
        0.0 
    };
    println!("Success rate: {:.1}%", success_rate);
}