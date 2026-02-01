//! Basic usage example
//!
//! Run with: cargo run --example basic -- document.pdf

use gemini_analyzer::{analyze, prompt, AnalysisBuilder, AnalyzeOptions};
use std::env;
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        // Text-only prompt example
        println!("=== Text-only prompt example ===");
        match prompt(
            "日本の建設業界で使われる主な書類を5つ挙げてください。",
            AnalyzeOptions::default(),
        ) {
            Ok(result) => println!("{}", result),
            Err(e) => eprintln!("Error: {}", e),
        }
        return;
    }

    let file = PathBuf::from(&args[1]);
    println!("=== File analysis example ===");
    println!("Analyzing: {}", file.display());

    // Method 1: Simple analyze function
    match analyze(
        "この書類の概要を日本語で説明してください。",
        &[file.clone()],
        AnalyzeOptions::default(),
    ) {
        Ok(result) => {
            println!("\n--- Result (simple) ---");
            println!("{}", result);
        }
        Err(e) => eprintln!("Error: {}", e),
    }

    // Method 2: Using builder pattern
    match AnalysisBuilder::new("この書類の重要なポイントを3つ挙げてください。")
        .file(&file)
        .model("gemini-2.5-flash")
        .run()
    {
        Ok(result) => {
            println!("\n--- Result (builder) ---");
            println!("{}", result);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
