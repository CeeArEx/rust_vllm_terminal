use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use futures::StreamExt;
use std::io::{stdout, Write};
use clap::Parser; // For using the CLI as interface
use termimad::print_text; // For displaying markdown instead of plain text
use dialoguer::{theme::ColorfulTheme, Input, Select, Confirm, Editor};
use std::path::{Path, PathBuf};
use chrono::Local; // For generating timestamps for chat filenames.

/// A CLI tool to interact with a local vLLM server
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
// An ArgGroup makes `--prompt` and `--chat` mutually exclusive.
// The user can run the tool with a prompt, in chat mode, or with the configure flag.
#[clap(group(
    clap::ArgGroup::new("mode")
        .required(false) // No mode is required, so the help message is shown by default.
        .args(&["prompt", "chat", "configure", "manage_chats"]),
))]
struct Cli {
    /// The prompt to send to the language model (single-shot mode)
    #[arg(short, long)]
    prompt: Option<String>,

    /// Starts an interactive, persistent chat session
    #[arg(short = 'c', long)] // The flag to enter chat mode.
    chat: bool,

    /// Runs an interactive wizard to create or update the config file
    #[arg(long, default_value_t = false)]
    configure: bool,

    /// Opens a menu to manage saved chat sessions
    #[arg(long, default_value_t = false)]
    manage_chats: bool,
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
    bad_words: Vec<String>,
    #[serde(default)] // Defaults to false if not in the file
    stream: bool,
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
# List of words that are not allowed to be generated. More precisely, only the last token of a corresponding token sequence is not allowed when the next generated token can complete the sequence.
bad_words = []

# --- Application Behavior ---
# Set to true to receive the response token-by-token, false to wait for the full response.
stream = true
"#;

// This tells Rust to automatically implement the `Deserialize` trait for these structs.
// It allows them to be created directly from incoming JSON.
#[derive(Deserialize, Debug)]
struct ApiResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: Message,
}

// This struct is now used for both API responses and for storing chat history.
// We derive `Serialize` to write it to JSON files and `Clone` for easier handling.
#[derive(Deserialize, Serialize, Debug, Clone)]
struct Message {
    role: String,
    content: String,
}

// This struct represents a full chat session, which is essentially a list of messages.
// It will be serialized to/from a JSON file for each chat.
#[derive(Serialize, Deserialize, Debug, Clone)]
struct ChatSession {
    messages: Vec<Message>,
}

// Structs for parsing streaming API responses 
#[derive(Deserialize, Debug)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize, Debug)]
struct StreamChoice {
    delta: StreamDelta,
}

#[derive(Deserialize, Debug)]
struct StreamDelta {
    // The content field is optional because the first/last chunks might not have it.
    content: Option<String>,
}

// This function handles the entire streaming process.
async fn send_api_request_stream(config: &Config, messages: &[Message]) -> Result<Message, Box<dyn std::error::Error>> {
    let payload = json!({
        "model": &config.model_name,
        "messages": messages,
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "top_p": config.top_p,
        "min_p": config.min_p,
        "top_k": config.top_k,
        "presence_penalty": config.presence_penalty,
        "bad_words": &config.bad_words,
        "stream": true, // The crucial flag to enable streaming
    });

    let api_endpoint = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let client = reqwest::Client::new();

    let res = client.post(&api_endpoint).json(&payload).send().await?;

    // Check for failure status before trying to stream.
    if !res.status().is_success() {
        let status = res.status();
        let error_body = res.text().await?;
        return Err(format!("❌ Request failed with status {}: {}", status, error_body).into());
    }

    let mut stream = res.bytes_stream();
    let mut full_response_content = String::new();

    print!("\nAssistant: "); // Print the initial prompt for the assistant's response
    stdout().flush()?; // MUST flush to make "Assistant: " appear immediately.

    // Loop over the stream of byte chunks
    while let Some(item) = stream.next().await {
        let chunk = item?;
        // SSE streams can send multiple events in one chunk, so we split by newline.
        for line in chunk.split(|&b| b == b'\n') {
            if line.starts_with(b"data: ") {
                // Strip the "data: " prefix
                let json_data = &line[6..];
                // Skip empty data lines
                if json_data.is_empty() { continue; }

                // Check for the end-of-stream signal
                if json_data == b"[DONE]" {
                    break;
                }
                
                // Try to parse the chunk into our streaming structs
                match serde_json::from_slice::<StreamChunk>(json_data) {
                    Ok(stream_chunk) => {
                        if let Some(choice) = stream_chunk.choices.first() {
                            if let Some(content) = &choice.delta.content {
                                // Print the token to the screen immediately
                                print!("{}", content);
                                stdout().flush()?; // Crucial for token-by-token display
                                // And add it to our full response string for saving later
                                full_response_content.push_str(content);
                            }
                        }
                    }
                    Err(_) => {
                        // Silently ignore chunks that aren't valid JSON, as they might be metadata
                    }
                }
            }
        }
    }
    
    println!(); // Add a final newline for clean formatting
    
    // Return a complete Message struct, just like the non-streaming function does.
    // This allows us to save the chat history correctly.
    Ok(Message {
        role: "assistant".to_string(),
        content: full_response_content,
    })
}

// Filesystem and Configuration Functions

/// Gets the path to the configuration file.
fn get_config_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(dirs::config_dir()
        .ok_or("Could not find a config directory")?
        .join("vllm-cli/config.toml"))
}

// Gets the path to the chat history directory.
// Best practice is to store user data (like chats) in a different location than user configuration.
fn get_chat_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = dirs::data_dir()
        .ok_or("Could not find a user data directory")?
        .join("vllm-cli/chats");
    // This ensures the directory is created if it doesn't exist yet.
    fs::create_dir_all(&path)?;
    Ok(path)
}

// A helper function to load all chat session filenames from the chat directory.
fn load_chat_sessions() -> Result<Vec<(PathBuf, String)>, Box<dyn std::error::Error>> {
    let chat_dir = get_chat_dir()?;
    let mut sessions = Vec::new();

    for entry in fs::read_dir(chat_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let filename = path.file_name().unwrap().to_string_lossy().to_string();
            sessions.push((path, filename));
        }
    }
    // Sort sessions chronologically based on their filenames.
    sessions.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(sessions)
}

// A function for handling all chat management tasks.
fn handle_chat_management() -> Result<(), Box<dyn std::error::Error>> {
    let theme = ColorfulTheme::default();

    // The main loop for the management menu.
    'management_loop: loop {
        // We load the sessions every time the loop runs, so the list is always fresh.
        let mut sessions = load_chat_sessions()?;
        
        let menu_items = &["Rename a chat session", "Delete a chat session", "Exit"];
        let selection = Select::with_theme(&theme)
            .with_prompt("Chat Management Menu")
            .items(menu_items)
            .default(0)
            .interact()?;

        match selection {
            0 => { // RENAME A CHAT
                if sessions.is_empty() {
                    println!("No chats to rename.");
                    continue; // Go back to the start of the loop
                }
                
                let session_names: Vec<String> = sessions.iter().map(|(_, name)| name.clone()).collect();
                let to_rename_index = Select::with_theme(&theme)
                    .with_prompt("Which session do you want to rename?")
                    .items(&session_names)
                    .interact()?;

                let (old_path, old_name) = &sessions[to_rename_index];

                let new_name: String = Input::with_theme(&theme)
                    .with_prompt("Enter the new name for the session")
                    .default(old_name.strip_suffix(".json").unwrap_or(old_name).to_string())
                    .interact_text()?;

                // Basic validation for the new name
                if new_name.trim().is_empty() || new_name.contains('/') || new_name.contains('\\') {
                    eprintln!("Error: Invalid filename. Please avoid empty names and slashes.");
                    continue;
                }

                // Ensure the new name ends with .json
                let final_name = if new_name.ends_with(".json") {
                    new_name
                } else {
                    format!("{}.json", new_name)
                };

                let new_path = old_path.with_file_name(final_name);
                
                // Perform the rename operation
                fs::rename(old_path, &new_path)?;
                println!("✅ Renamed '{}' to '{}'", old_name, new_path.file_name().unwrap().to_string_lossy());
            }
            1 => { // DELETE A CHAT
                if sessions.is_empty() {
                    println!("No chats to delete.");
                    continue;
                }
                
                let session_names: Vec<String> = sessions.iter().map(|(_, name)| name.clone()).collect();
                
                // Use dialoguer's `MultiSelect` to allow deleting multiple files at once.
                let to_delete_indices = dialoguer::MultiSelect::with_theme(&theme)
                    .with_prompt("Use SPACE to select chats to delete, ENTER to confirm")
                    .items(&session_names)
                    .interact()?;

                if to_delete_indices.is_empty() {
                    println!("No chats selected for deletion.");
                } else if Confirm::with_theme(&theme)
                    .with_prompt(format!("Are you sure you want to delete {} chat(s)? This cannot be undone.", to_delete_indices.len()))
                    .interact()?
                {
                    for index in to_delete_indices.iter().rev() { // Reverse to handle index shifts correctly
                        let (path, name) = &sessions[*index];
                        fs::remove_file(path)?;
                        println!("🗑️ Deleted chat: {}", name);
                    }
                } else {
                    println!("Deletion cancelled.");
                }
            }
            2 => { // EXIT
                break 'management_loop;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

// Saves a chat session struct to its corresponding JSON file.
fn save_chat_session(path: &Path, session: &ChatSession) -> Result<(), Box<dyn std::error::Error>> {
    let json_string = serde_json::to_string_pretty(session)?;
    fs::write(path, json_string)?;
    Ok(())
}

// A reusable function to manage a list of strings
fn manage_list(list: &mut Vec<String>, theme: &ColorfulTheme, prompt: &str) -> Result<(), Box<dyn std::error::Error>> {
    'list_menu: loop {
        // First, display the current list
        if list.is_empty() {
            println!("\nThe list is currently empty.");
        } else {
            println!("\nCurrent items in the list:");
            for item in list.iter() {
                println!("- {}", item);
            }
        }
        
        let menu_items = &["Add an item", "Remove an item", "Finish editing this list"];
        let selection = Select::with_theme(theme)
            .with_prompt(prompt)
            .items(menu_items)
            .default(0)
            .interact()?;

        match selection {
            0 => { // Add an item
                let new_item: String = Input::with_theme(theme)
                    .with_prompt("Enter the new item")
                    .interact_text()?;
                if !new_item.trim().is_empty() {
                    list.push(new_item.trim().to_string());
                }
            }
            1 => { // Remove an item
                if list.is_empty() {
                    println!("Nothing to remove.");
                    continue;
                }
                // We add a "Cancel" option to our list for the user
                let mut removable_items = list.clone();
                removable_items.push("Cancel".to_string());

                let to_remove = Select::with_theme(theme)
                    .with_prompt("Choose an item to remove")
                    .items(&removable_items)
                    .default(0)
                    .interact()?;
                
                // If the user didn't select "Cancel"
                if to_remove < list.len() {
                    list.remove(to_remove);
                }
            }
            2 => { // Finish editing
                break 'list_menu;
            }
            _ => unreachable!(),
        }
    }
    Ok(())
}

// The new and improved interactive wizard function
fn handle_configure() -> Result<bool, Box<dyn std::error::Error>> {
    let config_path = get_config_path()?;
    // Load existing config or create a new default one.
    // This is a bit different from load_config because we want to proceed with a default if it doesn't exist.
    let mut config = match fs::read_to_string(&config_path) {
        Ok(content) => toml::from_str(&content).unwrap_or_else(|e| {
        eprintln!("Warning: Could not parse config file (error: {}). Starting with defaults.", e);
        create_default_config_in_memory()
    }),
        Err(_) => {
            println!("No existing configuration found. Starting with defaults.");
            create_default_config_in_memory()
        }
    };

    let theme = ColorfulTheme::default();
    loop {

        let system_prompt_display = config.system_prompt.as_deref() // Get an Option<&str> from Option<String>
            .map(|prompt| { // If the prompt exists...
                const TRUNCATE_LIMIT: usize = 25; // Set our character limit
                if prompt.chars().count() > TRUNCATE_LIMIT {
                    // If it's too long, safely take the first 25 chars and add "..."
                    format!("{}...", prompt.chars().take(TRUNCATE_LIMIT).collect::<String>())
                } else {
                    // Otherwise, just use the original string
                    prompt.to_string()
                }
            })
            .unwrap_or_else(|| "Not set".to_string()); // If prompt is None, use "Not set"

        let items = &[
            format!("1. API URL      : {}", config.api_url),
            format!("2. Model Name   : {}", config.model_name),
            format!("3. Temperature  : {}", config.temperature),
            format!("4. Max Tokens   : {}", config.max_tokens),
            format!("5. System Prompt: {}", system_prompt_display),
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
                let initial_text = config.system_prompt.as_deref().unwrap_or("");

                println!("\nOpening your default editor to edit the prompt...");
                println!("(Save and quit the editor to accept, or close without saving to cancel)");
                
                // 2. Call the editor. It does not take a prompt itself.
                if let Some(new_prompt) = Editor::new().edit(initial_text)? {
                    // This logic remains the same.
                    config.system_prompt = if new_prompt.trim().is_empty() { None } else { Some(new_prompt) };
                    println!("System prompt updated.");
                } else {
                    println!("Edit cancelled.");
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
                        format!("5. Bad Words List    : [{}]", config.bad_words.join(", ")),
                        format!("6. Stream Mode : {}", config.stream),
                        "7. Return to Main Menu".to_string(),
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
                        4 => {
                            // Call our list management function
                            manage_list(&mut config.bad_words, &theme, "Manage Bad Words List")?;
                        },
                        5 => config.stream = Input::with_theme(&theme).with_prompt("Enable streaming?").default(config.stream).interact()?,
                        6 => break 'advanced_menu, 
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

// The handler for the entire chat mode experience.
async fn handle_chat_mode(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let theme = ColorfulTheme::default();
    let mut sessions = load_chat_sessions()?;

    // This block lets the user choose a session or start a new one.
    let (mut session, session_path) = loop {
        // Create a menu with "New Chat" and all existing chat filenames.
        let mut menu_items: Vec<String> = sessions.iter().map(|(_, name)| name.clone()).collect();
        menu_items.insert(0, "✨ Start a New Chat".to_string());

        let selection = Select::with_theme(&theme)
            .with_prompt("Select a chat session")
            .items(&menu_items)
            .default(0)
            .interact()?;

        match selection {
            0 => { // User chose "Start a New Chat"
                // Generate a descriptive filename for the new chat.
                let now = Local::now();
                let timestamp = now.format("%Y-%m-%d_%H-%M-%S");
                let filename = format!("{}_new-chat.json", timestamp);
                let session_path = get_chat_dir()?.join(filename);

                let mut messages = Vec::new();
                if let Some(sys_prompt) = &config.system_prompt {
                    if !sys_prompt.is_empty() {
                         messages.push(Message {
                            role: "system".to_string(),
                            content: sys_prompt.clone(),
                        });
                    }
                }
                
                let new_session = ChatSession { messages };
                println!("Starting a new chat. Your conversation will be saved to: {}", session_path.display());
                println!("Write !quit to exit.");
                break (new_session, session_path);
            }
            _ => { // User chose an existing chat
                let (path, _) = sessions.remove(selection - 1);
                let file_content = fs::read_to_string(&path)?;
                let session: ChatSession = serde_json::from_str(&file_content)?;
                println!("\n📜 Resuming chat from: {}", path.display());
                println!("Write !quit to exit.");
                // Display the previous messages to give the user context.
                for message in &session.messages {
                     if message.role == "assistant" {
                        println!("\nAssistant:");
                        print_text(message.content.trim());
                    } else if message.role == "user" {
                        println!("\n\n: {}", message.content.trim());
                    }
                }
                println!("\n"); // Add a newline for spacing
                break (session, path);
            }
        }
    };

    // Main interactive chat loop
    loop {
        let user_input: String = Input::with_theme(&theme)
            .with_prompt(":")
            .interact_text()?;

        if user_input.trim() == "!quit" || user_input.trim() == "!exit" {
            println!("Exiting chat. Your session has been saved at: {}", session_path.display());
            break;
        }

        // Add user's message to our session history.
        session.messages.push(Message {
            role: "user".to_string(),
            content: user_input,
        });

        // The API call is now made with the entire history.
        let response_message = if config.stream {
            send_api_request_stream(config, &session.messages).await?
        } else {
            println!("\nAssistant:"); // We need to print this ourselves in non-streaming mode
            send_api_request(config, &session.messages).await?
        };

        if !config.stream {
            print_text(response_message.content.trim());
            println!("\n");
        }

        // Add assistant's response to history and save the whole session to disk.
        session.messages.push(response_message);
        save_chat_session(&session_path, &session)?;
    }

    Ok(())
}

async fn send_api_request(config: &Config, messages: &[Message]) -> Result<Message, Box<dyn std::error::Error>> {
    let payload = json!({
        "model": &config.model_name,
        "messages": messages, 
        "temperature": config.temperature,
        "max_tokens": config.max_tokens,
        "top_p": config.top_p,
        "min_p": config.min_p,
        "top_k": config.top_k,
        "presence_penalty": config.presence_penalty,
        "bad_words": &config.bad_words
    });

    let api_endpoint = format!("{}/chat/completions", config.api_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let response = client.post(&api_endpoint).json(&payload).send().await?;

    if response.status().is_success() {
        let response_body: ApiResponse = response.json().await?;
        // We take the first choice from the API response.
        if let Some(first_choice) = response_body.choices.into_iter().next() {
            Ok(first_choice.message)
        } else {
            Err("API returned a successful response but with no choices.".into())
        }
    } else {
        let status = response.status();
        let error_body = response.text().await?;
        Err(format!("❌ Request failed with status {}: {}", status, error_body).into())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // First, handle the utility modes which exit immediately.
    if cli.configure {
        handle_configure()?;
        return Ok(());
    }
    if cli.manage_chats {
        handle_chat_management()?;
        return Ok(());
    }

    // Load the configuration. This will only run if no utility flags were used.
    let config = load_config()?;

    if cli.chat {
        // CHAT MODE
        handle_chat_mode(&config).await?;

    } else if let Some(prompt) = cli.prompt {
        println!("🚀 Sending request...");
        
        let mut messages = Vec::new();
        if let Some(sys_prompt) = &config.system_prompt {
            if !sys_prompt.is_empty() {
                messages.push(Message {
                    role: "system".to_string(),
                    content: sys_prompt.clone(),
                });
            }
        }
        messages.push(Message {
            role: "user".to_string(),
            content: prompt,
        });

        // Decide which function to call based on the config flag
        let result = if config.stream {
            send_api_request_stream(&config, &messages).await
        } else {
            send_api_request(&config, &messages).await
        };

        // Handle the result from whichever function was called
        match result {
            Ok(response_message) => {
                // If we were NOT streaming, we need to print the final response.
                // The streaming function already printed its output token-by-token.
                if !config.stream {
                    println!("\nAssistant: ");
                    print_text(response_message.content.trim());
                    println!();
                }
            }
            Err(e) => {
                eprintln!("{}", e); // Print errors to stderr.
            }
        }

    } else {
        // If no mode flag was given, print the help message.
        use clap::CommandFactory;
        Cli::command().print_help()?;
    }
        
    Ok(())
}