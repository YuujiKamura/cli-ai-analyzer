use cli_ai_analyzer::{prompt, AnalyzeOptions, Backend};

fn main() {
    let options = AnalyzeOptions::with_model("gpt-5.3-codex")
        .with_backend(Backend::Codex);

    match prompt("Return exactly: CODEX_OK", options) {
        Ok(result) => println!("OK:\n{}", result),
        Err(e) => {
            eprintln!("ERR:\n{}", e);
            std::process::exit(1);
        }
    }
}
