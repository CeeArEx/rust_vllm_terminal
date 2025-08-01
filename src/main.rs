use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use clap::{Parser, Subcommand}; // For using the CLI as interface
use termimad::print_text; // For displaying markdown instead of plain text
use half::f16;
use dialoguer::{theme::ColorfulTheme, Input};

/// A CLI tool to interact with a local vLLM server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

// The new enum that defines our subcommands
#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs an interactive wizard to create the config file
    Configure,
    /// Asks a question to the LLM (default command)
    Query(QueryArgs),
}

// The arguments for the `query` command
#[derive(Parser, Debug)]
struct QueryArgs {
    /// The prompt to send to the language model
    #[arg(short, long)]
    prompt: String,
}

// Blueprint for our config.toml file
#[derive(Deserialize, Serialize)]
struct Config {
    api_url: String,
    model_name: String,
    temperature: f16,
    max_tokens: u16,
    #[serde(default)] // This makes the field optional. If it's missing, it will use the default value (None).
    system_prompt: Option<String>,
}

// This is the template for our config file
const DEFAULT_CONFIG: &str = r#"
# Configuration for the vLLM CLI Tool

api_url = "http://localhost:8000/v1/chat/completions"
model_name = "your-model-name-here" # IMPORTANT: Please change this!

# --- LLM Parameters ---
temperature = 0.2
max_tokens = 1024

# --- Prompts ---
# The system prompt is optional. You can comment it out with a '#' or delete the line.
system_prompt = "You are a helpful and concise assistant."
"#;

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

// The interactive wizard function
fn handle_configure() -> Result<(), Box<dyn std::error::Error>> {
    println!("Welcome to the vLLM-CLI configuration wizard!");

    let theme = ColorfulTheme::default();
    let api_url: String = Input::with_theme(&theme)
        .with_prompt("Enter your vLLM API URL")
        .default("http://localhost:8000/v1/chat/completions".into())
        .interact_text()?;

    let model_name: String = Input::with_theme(&theme)
        .with_prompt("Enter the name of the model you are serving")
        .interact_text()?;

    let temperature: f16 = Input::with_theme(&theme)
        .with_prompt("Enter the temperature of the model (e.g., 0.8)")
        .interact()?;

    let max_tokens: u16 = Input::with_theme(&theme)
        .with_prompt("Enter the maximum of the token the model is allowed to answer (e.g., 1025)")
        .interact()?;

    // Create a config struct from the user's answers
    let new_config = Config {
        api_url,
        model_name,
        temperature,
        max_tokens,
        system_prompt: Some("You are a helpful and concise assistant.".into()),
    };

    // Convert the struct into a TOML string
    let toml_string = toml::to_string(&new_config)?;

    // Find the config path and write the file
    let config_path = dirs::config_dir()
        .ok_or("Could not find a config directory")?
        .join("vllm-cli/config.toml");

    if let Some(parent_dir) = config_path.parent() {
        fs::create_dir_all(parent_dir)?;
    }
    fs::write(&config_path, toml_string)?;

    println!("\n✅ Configuration saved successfully at: {}", config_path.display());
    Ok(())
}

// Function to load our configuration 
fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = dirs::config_dir()
        .ok_or("Could not find a config directory")?
        .join("vllm-cli/config.toml");

    if !config_path.exists() {
        println!("Configuration file not found. Creating a default one for you...");

        // Ensure the parent directory exists (e.g., ~/.config/vllm-cli/)
        if let Some(parent_dir) = config_path.parent() {
            fs::create_dir_all(parent_dir)?;
        }

        // Write the default content to the file
        fs::write(&config_path, DEFAULT_CONFIG)?;

        // Return a user-friendly error telling them what to do next
        let error_message = format!(
            "A default configuration file has been created at:\n{}\nPlease edit it with your model name and API URL.",
            config_path.display()
        );
        return Err(error_message.into());
    }

    // This part stays the same
    let config_content = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_content)?;
    Ok(config)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse command-line arguments FIRST
    let cli = Cli::parse();

    // Use a match statement to decide which command to run
    match cli.command {
        Some(Commands::Configure) => {
            handle_configure()?;
        }
        Some(Commands::Query(args)) => {
            // 2. Load configuration from the file
            let config = load_config()?;

            // 3. Build the messages array for the payload dynamically
            let mut messages = Vec::new();

            // Conditionally add the system prompt if it exists in the config
            if let Some(prompt) = config.system_prompt {
                messages.push(json!({
                    "role": "system",
                    "content": prompt
                }));
            }

            // Add the user's prompt
            messages.push(json!({
                "role": "user",
                "content": args.prompt
            }));

            // 4. Build the final payload using data from the config
            let payload = json!({
                "model": config.model_name,
                "messages": messages,
                "temperature": config.temperature,
                "max_tokens": config.max_tokens
            });

            println!("🚀 Sending request...");

            // 5. Create an HTTP client and send the POST request.
            let client = reqwest::Client::new();
            let response = client
                .post(&config.api_url) 
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
        }
        None => {
        // This is the default case.
        // You could print help or run a default query.
        println!("No command given. Use 'configure' or 'query'. Try --help for more info.");
    }
    }
        
    Ok(())
}
