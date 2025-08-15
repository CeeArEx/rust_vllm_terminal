<div align="center">
  <pre>
╭────────────────╮
│ vLLM-CLI   >_  │
╰────────────────╯
  </pre>
  <h1>vLLM-CLI</h1>
  <p><b>Your Terminal Companion for Local Language Models</b></p>
  <p>
    A blazingly fast, feature-rich, and user-friendly command-line interface for interacting with <a href="https://github.com/vllm-project/vllm">vLLM</a> servers, right from your terminal.
  </p>
  <p>

<img src="https://img.shields.io/github/last-commit/CeeArEx/rust_vllm_terminal?style=for-the-badge" alt="Last Commit">
<img src="https://img.shields.io/badge/platform-linux%20%7C%20windows-blue?style=for-the-badge" alt="Platform">
    <a href="https://opensource.org/licenses/MIT"><img src="https://img.shields.io/badge/License-MIT-informational?style=for-the-badge" alt="License: MIT"></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Made%20with-Rust-orange?style=for-the-badge&logo=rust" alt="Made with Rust"></a>
    <img src="https://img.shields.io/github/languages/code-size/CeeArEx/rust_vllm_terminal?style=for-the-badge" alt="Code Size">
  </p>
</div>

---

`vllm-cli` is a tool designed to make interacting with your local, OpenAI-compatible language model servers (like vLLM) a seamless and enjoyable experience. It supports both quick, single-shot prompts and persistent, interactive chat sessions with history, all without leaving the comfort of your command line.

### 🛡️ A Note on Privacy

Your privacy is paramount. All configurations and chat histories are stored locally on your machine. This tool communicates **only** with the server URL you provide in your configuration. No data is ever sent to external servers or third parties. Everything stays on your metal.

### ❗ A Legal Disclaimer

This is an independent, community-driven project. It is not officially affiliated with, sponsored by, or endorsed by the official vLLM project team. This tool is simply a personal project that prefers to use the excellent vLLM server as its backend.

## ✨ Features

*   **🚀 Two Modes of Operation**: Use it for a quick question with `--prompt` or dive into a full conversation with `--chat`.
*   **💾 Persistent Chat History**: Your chat sessions are automatically saved, allowing you to resume conversations right where you left off.
*   **✍️ Chat Management**: Easily rename and delete saved chat sessions with an interactive menu via `--manage-chats`.
*   **⚙️ Interactive Configuration**: A first-run (or on-demand with `--configure`) wizard to help you set up your server URL, model parameters, and more. No manual config file editing required!
*   **📝 Markdown Rendering**: Model responses are rendered as markdown in the terminal, improving readability for code blocks, lists, and more.
*   **💨 Full Streaming Support**: Get token-by-token responses from the model in real-time for a more responsive feel.
*   **🛠️ Full Parameter Control**: Tweak advanced model parameters like `temperature`, `top_p`, `top_k`, and more through the configuration wizard.

## 🏁 Getting Started

### 1. Prerequisite

You must have a running instance of a vLLM server that is compatible with the OpenAI API. This tool is a client that connects to that server. You can find instructions on how to set one up in the [vLLM Documentation](https://docs.vllm.ai/en/latest/getting_started/quickstart.html).

### 2. Installation

You can install `vllm-cli` easily by downloading a pre-compiled binary from the releases page.

#### Recommended: Via Releases (Windows & Linux)

1.  Navigate to the **[Releases Page](https://github.com/CeeArEx/rust_vllm_terminal/releases)**.
2.  Download the correct file for your operating system:
    *   **Windows**: Download the `.exe` file. Place it in a folder of your choice and add that folder to your system's `PATH` environment variable so you can run it from anywhere.
    *   **Debian/Ubuntu**: Download the `.deb` file and install it using your package manager.
        ```bash
        sudo dpkg -i /path/to/your/downloaded/vllm-cli.deb
        ```

#### Alternative: Building from Source

If you are on a different OS (like macOS) or prefer to compile it yourself, you can build it from source.

1.  **Install Rust**: You will need the Rust toolchain. If you don't have it, install it from [rustup.rs](https://rustup.rs/).
2.  **Clone and Install**:
    ```bash
    # Clone the repository
    git clone https://github.com/CeeArEx/rust_vllm_terminal.git

    # Navigate into the project directory
    cd rust_vllm_terminal

    # Build and install the binary
    cargo install --path .
    ```

### 3. Configuration

The first time you run `vllm-cli`, an interactive configuration wizard will launch. You can also trigger it manually at any time.

```bash
vllm-cli --configure
```

You will be guided through setting up the API URL of your local server and your preferred model parameters.

## 🚀 Usage

#### Single-Shot Prompt

For a single question and answer, use the `-p` or `--prompt` flag.

```bash
vllm-cli --prompt "What is the capital of Germany?"
```

#### Interactive Chat Mode

For a continuous conversation, use the `-c` or `--chat` flag. This will present a menu to start a new chat or resume a previous one.

```bash
vllm-cli --chat
```

Inside the chat, type your message and press Enter. To exit, type `!quit` or `!exit`.

#### Manage Chat History

To rename or delete saved chat sessions, use the `--manage-chats` flag.

```bash
vllm-cli --manage-chats
```

## 🔧 Configuration File

The configuration is stored in `config.toml` in your system's standard config location. While it's recommended to use the `--configure` wizard, you can also edit it manually.

**Example `config.toml`:**

```toml
# Configuration for the vLLM CLI Tool
api_url = "http://localhost:8000/v1"
model_name = "mistralai/Mistral-7B-Instruct-v0.1"
temperature = 0.7
max_tokens = 1024
system_prompt = "You are a helpful and concise assistant."
# ... and other parameters
```

## 🤝 Contributing

Contributions, issues, and feature requests are welcome! Feel free to check the [issues page](https://github.com/CeeArEx/rust_vllm_terminal/issues).

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
