use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use clap::Parser; // For using the CLI as interface
use termimad::print_text; // For displaying markdown instead of plain text
use dialoguer::{theme::ColorfulTheme, Input, Select, Confirm};
use std::path::PathBuf;

/// A CLI tool to interact with a local vLLM server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// The prompt to send to the language model
    #[arg(short, long)]
    prompt: Option<String>,

    /// Runs an interactive wizard to create or update the config file
    #[arg(short, long, default_value_t = false)]
    configure: bool,
}

// Blueprint for our config.toml file
#[derive(Deserialize, Serialize, Clone)] // Added Clone
struct Config {
    api_url: String,
    model_name: String,
    temperature: f32,
    max_tokens: u16,
    #[serde(default)] // This makes the field optional. If it's missing, it will use the default value (None).
    system_prompt: Option<String>,
    top_p: f32,
    min_p: f32,
    top_k: i32,
    presence_penalty: f32,
}

// This is the template for our config file
const DEFAULT_CONFIG: &str = r#"
# Configuration for the vLLM CLI Tool

# IMPORTANT: Please change this to your server's base URL (e.g., http://localhost:8000/v1)
api_url = "http://localhost:8000/v1"
model_name = "your-model-name-here" # IMPORTANT: Please change this!

# --- LLM Parameters ---
temperature = 0.7
max_tokens = 512

# --- Prompts ---
# The system prompt is optional. You can comment it out with a '#' or delete the line.
system_prompt = "You are a helpful and concise assistant."

# --- Advanced Parameters ---
# top_p - Float that controls the cumulative probability of the top tokens to consider. Must be in (0, 1]. Set to 1 to consider all tokens.
top_p = 0.80
# min_p - Float that represents the minimum probability for a token to be considered, relative to the probability of the most likely token. Must be in [0, 1]. Set to 0 to disable this.
min_p = 0.00 
# top_k - Integer that controls the number of top tokens to consider. Set to -1 to consider all tokens.
top_k = 20 
# presence_penalty - Float that penalizes new tokens based on whether they appear in the prompt and the generated text so far. Values > 1 encourage the model to use new tokens, while values < 1 encourage the model to repeat tokens.
presence_penalty = 2.0 
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

/// Gets the path to the configuration file.
fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(dirs::config_dir()
        .ok_or("Could not find a config directory")?
        .join("vllm-cli/config.toml"))
}


// The new and improved interactive wizard function
fn handle_configure() -> Result<bool, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    // Load existing config or create a new default one.
    // This is a bit different from load_config because we want to proceed with a default if it doesn't exist.
    let mut config = match fs::read_to_string(&config_path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|_| create_default_config_in_memory()),
        Err(_) => {
            println!("No existing configuration found. Starting with defaults.");
            create_default_config_in_memory()
        }
    };

    let theme = ColorfulTheme::default();
    loop {
        let items = &[
            format!("1. API URL      : {}", config.api_url),
            format!("2. Model Name   : {}", config.model_name),
            format!("3. Temperature  : {}", config.temperature),
            format!("4. Max Tokens   : {}", config.max_tokens),
            format!("5. System Prompt: {}", config.system_prompt.as_deref().unwrap_or("Not set")),
            "6. Advanced Options...".to_string(),
            "7. Save and Exit".to_string(),    
            "8. Exit Without Saving".to_string(),
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt("Choose an option to edit (use arrow keys)")
            .items(items)
            .default(0)
            .interact()?;

        match selection {
            0 => {
                config.api_url = Input::with_theme(&theme)
                    .with_prompt("Enter your OpenAI-compatible base URL (e.g., http://localhost:8000/v1)")
                    .default(config.api_url)
                    .interact_text()?;
            }
            1 => {
                config.model_name = Input::with_theme(&theme)
                    .with_prompt("Enter the name of the model you are serving")
                    .default(config.model_name)
                    .interact_text()?;
            }
            2 => {
                config.temperature = Input::with_theme(&theme)
                    .with_prompt("Enter the temperature of the model (e.g., 0.8)")
                    .default(config.temperature)
                    .interact()?;
            }
            3 => {
                config.max_tokens = Input::with_theme(&theme)
                    .with_prompt("Enter the maximum number of tokens the model is allowed to generate")
                    .default(config.max_tokens)
                    .interact()?;
            }
            4 => {
                let current_prompt = config.system_prompt.clone().unwrap_or_default();
                if let Ok(new_prompt) = Input::with_theme(&theme)
                    .with_prompt("Enter the system prompt (leave empty for none)")
                    .default(current_prompt)
                    .interact_text() {
                    config.system_prompt = if new_prompt.is_empty() { None } else { Some(new_prompt) };
                }
            }
            // Handle the Advanced Options selection
            5 => { 
                'advanced_menu: loop { 
                    let advanced_items = &[
                        format!("1. Top P             : {}", config.top_p),
                        format!("2. Min P             : {}", config.min_p),
                        format!("3. Top K             : {}", config.top_k),
                        format!("4. Presence Penalty  : {}", config.presence_penalty),
                        "5. Return to Main Menu".to_string(),
                    ];

                    let advanced_selection = Select::with_theme(&theme)
                        .with_prompt("Advanced Options")
                        .items(advanced_items)
                        .default(0)
                        .interact()?;

                    match advanced_selection {
                        0 => config.top_p = Input::with_theme(&theme).with_prompt("Enter Top P").default(config.top_p).interact()?,
                        1 => config.min_p = Input::with_theme(&theme).with_prompt("Enter Min P").default(config.min_p).interact()?,
                        2 => config.top_k = Input::with_theme(&theme).with_prompt("Enter Top K (-1 to disable)").default(config.top_k).interact()?,
                        3 => config.presence_penalty = Input::with_theme(&theme).with_prompt("Enter Presence Penalty").default(config.presence_penalty).interact()?,
                        4 => break 'advanced_menu, 
                        _ => unreachable!(),
                    }
                }
            }
            6 => {
                // Save and exit
                let toml_string = toml::to_string(&config)?;
                if let Some(parent_dir) = config_path.parent() {
                    fs::create_dir_all(parent_dir)?;
                }
                fs::write(&config_path, toml_string)?;
                println!("\n✅ Configuration saved successfully at: {}", config_path.display());
                return Ok(true);
            }
            7 => {
                // Exit without saving
                if Confirm::with_theme(&theme).with_prompt("Are you sure you want to exit without saving?").interact()? {
                    println!("Configuration changes discarded.");
                    return Ok(false);
                }
            }
            _ => unreachable!(),
        }
    }
}


// Helper function to create a default config in memory
fn create_default_config_in_memory() -> Config {
    toml::from_str(DEFAULT_CONFIG).expect("Failed to parse default config template.")
}

// Function to load our configuration 
fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        println!("Configuration file not found. Creating a default one for you...");

        // Ensure the parent directory exists (e.g., ~/.config/vllm-cli/)
        if let Some(parent_dir) = config_path.parent() {
            fs::create_dir_all(parent_dir)?;
        }

        // Write the default content to the file
        fs::write(&config_path, DEFAULT_CONFIG)?;

        // Return a user-friendly error telling them what to do next
        println!("Please configure your settings..");
        let saved = handle_configure()?;

        if !saved {
            return Err("Configuration aborted. Please run `--configure` to set up the tool.".into());
        }
    }

    let config_content = fs::read_to_string(config_path)?;
    let config: Config = toml::from_str(&config_content)?;
    Ok(config)
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Parse command-line arguments FIRST
    let cli = Cli::parse();

    // Check if the configure flag is present
    if cli.configure {
        handle_configure()?;
    } 
    // Check if a prompt was provided
    else if let Some(prompt) = cli.prompt {
        // 2. Load configuration from the file
        let config = load_config()?;

        // 3. Build the messages array for the payload dynamically
        let mut messages = Vec::new();

        // Conditionally add the system prompt if it exists in the config
        if let Some(sys_prompt) = &config.system_prompt {
            if !sys_prompt.is_empty() {
                messages.push(json!({
                    "role": "system",
                    "content": sys_prompt
                }));
            }
        }

        // Add the user's prompt
        messages.push(json!({
            "role": "user",
            "content": prompt
        }));
        
        // 4. Build the final payload using data from the config
        let payload = json!({
            "model": &config.model_name,
            "messages": messages,
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "top_p": config.top_p,
            "min_p": config.min_p,
            "top_k": config.top_k,
            "presence_penalty": config.presence_penalty
        });

        println!("🚀 Sending request...");

        // Construct the full API URL for the chat completions endpoint
        let api_endpoint = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));


        // 5. Create an HTTP client and send the POST request.
        let client = reqwest::Client::new();
        let response = client
            .post(&api_endpoint) 
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
    // If no arguments were given, print the help message
    else {
        // This trick prints the help message for the command.
        use clap::CommandFactory;
        Cli::command().print_help()?;
    }
        
    Ok(())
}
