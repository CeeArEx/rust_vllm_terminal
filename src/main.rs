use serde::Deserialize;
use dotenvy::dotenv; // To load the .env file
use serde_json::json;
use std::env; // To read environment variables
use clap::Parser; // For using the CLI as interface
use termimad::print_text; // For displaying markdown instead of plain text

/// A CLI tool to interact with a local vLLM server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The prompt to send to the language model
    #[arg(short, long)]
    prompt: String,
}

// This tells Rust to automatically implement the `Deserialize` trait for these structs.
// It allows them to be created directly from incoming JSON.
#[derive(Deserialize, Debug)]
struct ApiResponse {
    choices: Vec<Choice>,
    usage: Usage,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

#[derive(Deserialize, Debug)]
struct Message {
    content: String,
}

#[derive(Deserialize, Debug)] 
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // 1. Parse command-line arguments FIRST
    let cli = Cli::parse();

    // 2. Load environment variables from the .env file
    dotenv().ok(); // This line loads the .env file. .ok() ignores errors if the file doesn't exist.

    // 3. Read the API URL from the environment.
    // .expect() will cause the program to crash if the variable isn't set,
    // which is good for critical configuration like this.
    let api_url = env::var("VLLM_API_URL").expect("VLLM_API_URL must be set in .env file");
    let model_name = env::var("MODEL_NAME").expect("MODEL_NAME must be set in .env file"); 

    // 4. Define the NEW payload for the OpenAI Chat Completions endpoint.
    // Note the structure: it uses a "messages" array with "role" and "content".
    let payload = json!({
        "model": model_name,
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful and concise assistant. You can use markdown for formatting your answer."
            },
            {
                "role": "user",
                "content": cli.prompt
            }
        ],
        "temperature": 0.7,
        "max_tokens": 512
    });

    println!("🚀 Sending request...");

    // 5. Create an HTTP client and send the POST request.
    let client = reqwest::Client::new();
    let response = client
        .post(&api_url) 
        .json(&payload) // This automatically serializes `payload` to JSON and sets the correct header
        .send()
        .await?; // The `.await` pauses execution until the response is received.
                 // The `?` will automatically handle any network errors for us.

    // 6. Check if the request was successful and print the response.
    if response.status().is_success() {

        // Use the new struct for incoming messages
        let response_body: ApiResponse = response.json().await?;

        println!("\n✅ Assistant: ");
        
        // Instead of printing the whole blob, we navigate our struct
        if let Some(first_choice) = response_body.choices.get(0) {
            print_text(first_choice.message.content.trim());
        } else {
            println!("No choices found in the response.");
        }

        println!("\n------------------------------------");
        println!(
            "📊 Tokens: {} (prompt) + {} (completion) = {} (total)",
            response_body.usage.prompt_tokens,
            response_body.usage.completion_tokens,
            response_body.usage.total_tokens
        );

    } else {
        println!("\n❌ Request failed with status code: {}", response.status());
        let error_body = response.text().await?;
        println!("Error details: {}", error_body);
    }

    Ok(())
}
