use dotenvy::dotenv; // To load the .env file
use serde_json::json;
use std::env; // To read environment variables

#[tokio::main]
async fn main() -> Result<(), reqwest::Error> {
    // 1. Load environment variables from the .env file
    dotenv().ok(); // This line loads the .env file. .ok() ignores errors if the file doesn't exist.

    // 2. Read the API URL from the environment.
    // .expect() will cause the program to crash if the variable isn't set,
    // which is good for critical configuration like this.
    let api_url = env::var("VLLM_API_URL").expect("VLLM_API_URL must be set in .env file");
    let model_name = env::var("MODEL_NAME").expect("MODEL_NAME must be set in .env file"); 

    // 3. Define the NEW payload for the OpenAI Chat Completions endpoint.
    // Note the structure: it uses a "messages" array with "role" and "content".
    let payload = json!({
        "model": model_name,
        "messages": [
            {
                "role": "system",
                "content": "You are a helpful and concise assistant."
            },
            {
                "role": "user",
                "content": "What is the capital of France?"
            }
        ],
        "temperature": 0.3,
        "max_tokens": 100
    });

    println!("🚀 Sending request to OpenAI-compatible server at {}...", api_url);
    println!("   Using model: {}", env::var("MODEL_NAME").unwrap()); 
    println!("   Payload: {}", serde_json::to_string_pretty(&payload).unwrap());

    // 4. Create an HTTP client and send the POST request.
    let client = reqwest::Client::new();
    let response = client
        .post(&api_url) 
        .json(&payload) // This automatically serializes `payload` to JSON and sets the correct header
        .send()
        .await?; // The `.await` pauses execution until the response is received.
                 // The `?` will automatically handle any network errors for us.

    // 5. Check if the request was successful and print the response.
    if response.status().is_success() {
        // Parse the JSON response body into a generic JSON Value
        let response_body: serde_json::Value = response.json().await?;

        println!("\n✅ Success! Server responded:");
        // Use `to_string_pretty` to format the JSON output nicely
        println!("{}", serde_json::to_string_pretty(&response_body).unwrap());
    } else {
        println!("\n❌ Request failed with status code: {}", response.status());
        let error_body = response.text().await?;
        println!("Error details: {}", error_body);
    }

    Ok(())
}
