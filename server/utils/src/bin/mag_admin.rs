use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use serde::Deserialize;
use serde::Serialize;
use server_utils::admin_client::{
    AdminClient, BadwordEntryResponse, BadwordsListResponse, BadwordsMutationResponse,
    TextReloadResponse, TextReloadStatusResponse,
};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

const DEFAULT_API_URL: &str = "https://127.0.0.1:5554";
const DEFAULT_WAIT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Parser)]
#[command(
    name = "mag-admin",
    version,
    about = "Scriptable admin CLI for Men Among Gods Reforged",
    arg_required_else_help = true
)]
struct Cli {
    #[arg(
        long = "api",
        visible_alias = "admin-api",
        env = "MAG_API_BASE_URL",
        default_value = DEFAULT_API_URL,
        global = true,
        help = "Base URL for the API service"
    )]
    api: String,

    #[arg(
        long = "admin-token",
        visible_alias = "api-token",
        env = "MAG_ADMIN_API_TOKEN",
        global = true,
        help = "Admin bearer token"
    )]
    admin_token: Option<String>,

    #[arg(
        long,
        value_enum,
        default_value_t = OutputFormat::Table,
        global = true,
        help = "Output format"
    )]
    format: OutputFormat,

    #[arg(long, global = true, help = "Suppress non-data status messages")]
    quiet: bool,

    #[arg(
        long,
        global = true,
        help = "Open an interactive menu instead of running one command"
    )]
    menu: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Table,
    Json,
    Plain,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Query and mutate the server badwords list.
    Badwords {
        #[command(subcommand)]
        command: BadwordsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum BadwordsCommand {
    /// List all badwords.
    List,
    /// Check whether a badword exists.
    Get { word: String },
    /// Add one or more badwords idempotently.
    Add {
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
        #[arg(long, help = "Request a running server refresh after persistence")]
        refresh: bool,
        #[arg(long, help = "Wait until the running server reports refresh applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Remove one or more badwords idempotently.
    Remove {
        #[arg(required = true, num_args = 1..)]
        words: Vec<String>,
        #[arg(long, help = "Request a running server refresh after persistence")]
        refresh: bool,
        #[arg(long, help = "Wait until the running server reports refresh applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Replace the complete badwords list from JSON or newline-delimited input.
    Replace {
        #[arg(long, help = "Input file path, or '-' for stdin")]
        input: String,
        #[arg(long, help = "Request a running server refresh after persistence")]
        refresh: bool,
        #[arg(long, help = "Wait until the running server reports refresh applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Export the badwords list to stdout or a file.
    Export {
        #[arg(
            long,
            default_value = "-",
            help = "Output file path, or '-' for stdout"
        )]
        output: String,
    },
    /// Request a running server refresh of badwords.
    Refresh {
        #[arg(long, help = "Wait until the running server reports refresh applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
}

#[derive(Debug)]
enum CliError {
    Runtime(String),
    NotFound(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuAction {
    List,
    Get,
    Add,
    Remove,
    Replace,
    Export,
    Refresh,
    Quit,
}

#[derive(Debug, Deserialize)]
struct WordsInput {
    words: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Runtime(message)) => {
            eprintln!("error: {message}");
            ExitCode::from(1)
        }
        Err(CliError::NotFound(message)) => {
            eprintln!("not found: {message}");
            ExitCode::from(3)
        }
    }
}

fn run(cli: Cli) -> Result<(), CliError> {
    let Some(admin_token) = cli.admin_token.as_deref().map(str::trim) else {
        return Err(CliError::Runtime(
            "admin token is empty; pass --admin-token or set MAG_ADMIN_API_TOKEN".to_string(),
        ));
    };
    if admin_token.is_empty() {
        return Err(CliError::Runtime(
            "admin token is empty; pass --admin-token or set MAG_ADMIN_API_TOKEN".to_string(),
        ));
    }

    let client = AdminClient::new(cli.api.trim(), admin_token).map_err(CliError::Runtime)?;

    if cli.menu {
        return run_menu(&cli, &client);
    }

    let Some(command) = &cli.command else {
        return Err(CliError::Runtime(
            "no command provided; pass --menu or run `mag-admin help`".to_string(),
        ));
    };

    match command {
        Commands::Badwords { command } => run_badwords(&cli, &client, command),
    }
}

fn run_menu(cli: &Cli, client: &AdminClient) -> Result<(), CliError> {
    let theme = ColorfulTheme::default();
    print_menu_header(&cli.api);

    loop {
        let action = choose_menu_action(&theme)?;
        match action {
            MenuAction::List => menu_list_badwords(client)?,
            MenuAction::Get => menu_get_badword(client, &theme)?,
            MenuAction::Add => menu_add_badwords(client, &theme)?,
            MenuAction::Remove => menu_remove_badwords(client, &theme)?,
            MenuAction::Replace => menu_replace_badwords(client, &theme)?,
            MenuAction::Export => menu_export_badwords(client, &theme)?,
            MenuAction::Refresh => menu_refresh_badwords(client, &theme)?,
            MenuAction::Quit => break,
        }

        if !Confirm::with_theme(&theme)
            .with_prompt("Return to the admin menu?")
            .default(true)
            .interact()
            .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?
        {
            break;
        }
    }

    Ok(())
}

fn print_menu_header(api: &str) {
    println!("+--------------------------------------------+");
    println!("| Men Among Gods Reforged Admin              |");
    println!("| Interactive operator menu                  |");
    println!("+--------------------------------------------+");
    println!("API: {api}");
    println!();
}

fn choose_menu_action(theme: &ColorfulTheme) -> Result<MenuAction, CliError> {
    let items = [
        "List badwords",
        "Check one badword",
        "Add badwords",
        "Remove badwords",
        "Replace badwords list",
        "Export badwords",
        "Refresh running server cache",
        "Quit",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("Choose an action")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => MenuAction::List,
        1 => MenuAction::Get,
        2 => MenuAction::Add,
        3 => MenuAction::Remove,
        4 => MenuAction::Replace,
        5 => MenuAction::Export,
        6 => MenuAction::Refresh,
        _ => MenuAction::Quit,
    })
}

fn menu_list_badwords(client: &AdminClient) -> Result<(), CliError> {
    let response = client.fetch_badwords().map_err(CliError::Runtime)?;
    print_badwords_list(&response, OutputFormat::Table)
}

fn menu_get_badword(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let word = prompt_text(theme, "Word to check", None)?;
    let response = client.get_badword(&word).map_err(CliError::Runtime)?;
    print_badword_entry(&response, OutputFormat::Table)
}

fn menu_add_badwords(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let words = prompt_words(theme, "Words to add (comma or newline separated)")?;
    if words.is_empty() {
        println!("No words entered; nothing changed.");
        return Ok(());
    }
    let response = client.add_badwords(&words).map_err(CliError::Runtime)?;
    print_mutation_response(&response, OutputFormat::Table, false)?;
    menu_refresh_after_mutation(client, theme)
}

fn menu_remove_badwords(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let current = client.fetch_badwords().map_err(CliError::Runtime)?;
    let selected = if current.words.is_empty() {
        prompt_words(theme, "Words to remove (comma or newline separated)")?
    } else {
        let indexes = MultiSelect::with_theme(theme)
            .with_prompt("Select words to remove, or press Enter to type manually")
            .items(&current.words)
            .interact()
            .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
        if indexes.is_empty() {
            prompt_words(theme, "Words to remove (comma or newline separated)")?
        } else {
            indexes
                .into_iter()
                .filter_map(|index| current.words.get(index).cloned())
                .collect()
        }
    };

    if selected.is_empty() {
        println!("No words selected; nothing changed.");
        return Ok(());
    }
    let response = client
        .remove_badwords(&selected)
        .map_err(CliError::Runtime)?;
    print_mutation_response(&response, OutputFormat::Table, false)?;
    menu_refresh_after_mutation(client, theme)
}

fn menu_replace_badwords(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let mode_items = ["Paste/type entries", "Read from file", "Cancel"];
    let selected = Select::with_theme(theme)
        .with_prompt("How would you like to provide the replacement list?")
        .items(&mode_items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    let words = match selected {
        0 => prompt_words(theme, "Replacement words (comma or newline separated)")?,
        1 => {
            let path = prompt_text(theme, "Input file", None)?;
            read_words_input(&path)?
        }
        _ => return Ok(()),
    };

    println!("Replacement list contains {} entries.", words.len());
    if !Confirm::with_theme(theme)
        .with_prompt("Replace the complete badwords list?")
        .default(false)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?
    {
        println!("Replacement cancelled.");
        return Ok(());
    }

    let response = client.replace_badwords(&words).map_err(CliError::Runtime)?;
    print_mutation_response(&response, OutputFormat::Table, false)?;
    menu_refresh_after_mutation(client, theme)
}

fn menu_export_badwords(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let response = client.fetch_badwords().map_err(CliError::Runtime)?;
    let output = prompt_text(theme, "Output file (`-` for stdout)", Some("-"))?;
    write_badwords_export(&response, OutputFormat::Plain, &output)
}

fn menu_refresh_badwords(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let wait = Confirm::with_theme(theme)
        .with_prompt("Wait for the running server to apply the refresh?")
        .default(true)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
    let timeout_seconds = if wait {
        prompt_timeout_seconds(theme)?
    } else {
        DEFAULT_WAIT_TIMEOUT_SECS
    };
    let response = client
        .request_text_reload(true)
        .map_err(CliError::Runtime)?;
    if wait {
        let status = wait_for_text_reload(client, &response.request_id, timeout_seconds)?;
        print_reload_status(&status, OutputFormat::Table, false)
    } else {
        print_reload_response(&response, OutputFormat::Table, false)
    }
}

fn menu_refresh_after_mutation(
    client: &AdminClient,
    theme: &ColorfulTheme,
) -> Result<(), CliError> {
    if Confirm::with_theme(theme)
        .with_prompt("Refresh the running server cache now?")
        .default(true)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?
    {
        menu_refresh_badwords(client, theme)?;
    }
    Ok(())
}

fn prompt_text(
    theme: &ColorfulTheme,
    prompt: &str,
    default: Option<&str>,
) -> Result<String, CliError> {
    let mut input = Input::<String>::with_theme(theme).with_prompt(prompt.to_string());
    if let Some(default) = default {
        input = input.default(default.to_string());
    }
    input
        .interact_text()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))
}

fn prompt_words(theme: &ColorfulTheme, prompt: &str) -> Result<Vec<String>, CliError> {
    let raw = prompt_text(theme, prompt, None)?;
    Ok(split_words(&raw))
}

fn prompt_timeout_seconds(theme: &ColorfulTheme) -> Result<u64, CliError> {
    Input::<u64>::with_theme(theme)
        .with_prompt("Refresh wait timeout in seconds")
        .default(DEFAULT_WAIT_TIMEOUT_SECS)
        .interact_text()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))
}

fn run_badwords(
    cli: &Cli,
    client: &AdminClient,
    command: &BadwordsCommand,
) -> Result<(), CliError> {
    match command {
        BadwordsCommand::List => {
            let response = client.fetch_badwords().map_err(CliError::Runtime)?;
            print_badwords_list(&response, cli.format)
        }
        BadwordsCommand::Get { word } => {
            let response = client.get_badword(word).map_err(CliError::Runtime)?;
            if !response.exists {
                return Err(CliError::NotFound(response.word));
            }
            print_badword_entry(&response, cli.format)
        }
        BadwordsCommand::Add {
            words,
            refresh,
            wait,
            timeout_seconds,
        } => {
            let response = client.add_badwords(words).map_err(CliError::Runtime)?;
            print_mutation_response(&response, cli.format, cli.quiet)?;
            maybe_refresh(
                client,
                *refresh || *wait,
                *wait,
                *timeout_seconds,
                cli.quiet,
            )
        }
        BadwordsCommand::Remove {
            words,
            refresh,
            wait,
            timeout_seconds,
        } => {
            let response = client.remove_badwords(words).map_err(CliError::Runtime)?;
            print_mutation_response(&response, cli.format, cli.quiet)?;
            maybe_refresh(
                client,
                *refresh || *wait,
                *wait,
                *timeout_seconds,
                cli.quiet,
            )
        }
        BadwordsCommand::Replace {
            input,
            refresh,
            wait,
            timeout_seconds,
        } => {
            let words = read_words_input(input)?;
            let response = client.replace_badwords(&words).map_err(CliError::Runtime)?;
            print_mutation_response(&response, cli.format, cli.quiet)?;
            maybe_refresh(
                client,
                *refresh || *wait,
                *wait,
                *timeout_seconds,
                cli.quiet,
            )
        }
        BadwordsCommand::Export { output } => {
            let response = client.fetch_badwords().map_err(CliError::Runtime)?;
            write_badwords_export(&response, cli.format, output)
        }
        BadwordsCommand::Refresh {
            wait,
            timeout_seconds,
        } => {
            let response = client
                .request_text_reload(true)
                .map_err(CliError::Runtime)?;
            if *wait {
                let status = wait_for_text_reload(client, &response.request_id, *timeout_seconds)?;
                print_reload_status(&status, cli.format, cli.quiet)
            } else {
                print_reload_response(&response, cli.format, cli.quiet)
            }
        }
    }
}

fn maybe_refresh(
    client: &AdminClient,
    refresh: bool,
    wait: bool,
    timeout_seconds: u64,
    quiet: bool,
) -> Result<(), CliError> {
    if !refresh {
        return Ok(());
    }
    let response = client
        .request_text_reload(true)
        .map_err(CliError::Runtime)?;
    if !quiet {
        eprintln!("refresh requested: {}", response.request_id);
    }
    if wait {
        let status = wait_for_text_reload(client, &response.request_id, timeout_seconds)?;
        if !quiet {
            eprintln!("refresh status: {}", status.status);
        }
    }
    Ok(())
}

fn wait_for_text_reload(
    client: &AdminClient,
    request_id: &str,
    timeout_seconds: u64,
) -> Result<TextReloadStatusResponse, CliError> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let status = client
            .text_reload_status(request_id)
            .map_err(CliError::Runtime)?;
        if status.status == "applied" {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(CliError::Runtime(format!(
                "timed out waiting for text reload {}",
                request_id
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn read_words_input(path: &str) -> Result<Vec<String>, CliError> {
    let mut raw = String::new();
    if path == "-" {
        io::stdin()
            .read_to_string(&mut raw)
            .map_err(|error| CliError::Runtime(format!("read stdin: {error}")))?;
    } else {
        raw = fs::read_to_string(path)
            .map_err(|error| CliError::Runtime(format!("read {}: {error}", path)))?;
    }
    parse_words_input(&raw)
}

fn parse_words_input(raw: &str) -> Result<Vec<String>, CliError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    if trimmed.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(trimmed)
            .map_err(|error| CliError::Runtime(format!("parse JSON array: {error}")));
    }
    if trimmed.starts_with('{') {
        return serde_json::from_str::<WordsInput>(trimmed)
            .map(|input| input.words)
            .map_err(|error| CliError::Runtime(format!("parse JSON object: {error}")));
    }

    Ok(split_words(raw))
}

fn split_words(raw: &str) -> Vec<String> {
    raw.split(|character: char| character == ',' || character == '\n' || character == '\r')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn write_badwords_export(
    response: &BadwordsListResponse,
    format: OutputFormat,
    output: &str,
) -> Result<(), CliError> {
    let payload = match format {
        OutputFormat::Json => json_string(response)?,
        OutputFormat::Table | OutputFormat::Plain => response.words.join("\n") + "\n",
    };

    if output == "-" {
        print!("{payload}");
        return Ok(());
    }

    fs::write(Path::new(output), payload)
        .map_err(|error| CliError::Runtime(format!("write {}: {error}", output)))
}

fn print_badwords_list(
    response: &BadwordsListResponse,
    format: OutputFormat,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => {
            for word in &response.words {
                println!("{word}");
            }
        }
        OutputFormat::Table => {
            println!("VERSION  COUNT");
            println!("{}  {}", response.version, response.count);
            println!();
            println!("WORD");
            for word in &response.words {
                println!("{word}");
            }
        }
    }
    Ok(())
}

fn print_badword_entry(
    response: &BadwordEntryResponse,
    format: OutputFormat,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => println!("{}", response.word),
        OutputFormat::Table => {
            println!("WORD  EXISTS");
            println!("{}  {}", response.word, response.exists);
        }
    }
    Ok(())
}

fn print_mutation_response(
    response: &BadwordsMutationResponse,
    format: OutputFormat,
    quiet: bool,
) -> Result<(), CliError> {
    if quiet {
        return Ok(());
    }
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => {
            println!("version={}", response.version);
            println!("count={}", response.count);
            println!("added={}", response.added.len());
            println!("removed={}", response.removed.len());
            println!("unchanged={}", response.unchanged.len());
        }
        OutputFormat::Table => {
            println!("VERSION  COUNT  ADDED  REMOVED  UNCHANGED");
            println!(
                "{}  {}  {}  {}  {}",
                response.version,
                response.count,
                response.added.len(),
                response.removed.len(),
                response.unchanged.len()
            );
        }
    }
    Ok(())
}

fn print_reload_response(
    response: &TextReloadResponse,
    format: OutputFormat,
    quiet: bool,
) -> Result<(), CliError> {
    if quiet {
        return Ok(());
    }
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => println!("{}", response.request_id),
        OutputFormat::Table => {
            println!("REQUEST_ID  KINDS");
            println!("{}  {}", response.request_id, response.kinds.join(","));
        }
    }
    Ok(())
}

fn print_reload_status(
    response: &TextReloadStatusResponse,
    format: OutputFormat,
    quiet: bool,
) -> Result<(), CliError> {
    if quiet {
        return Ok(());
    }
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => println!("{}", response.status),
        OutputFormat::Table => {
            println!("REQUEST_ID  STATUS");
            println!("{}  {}", response.request_id, response.status);
        }
    }
    Ok(())
}

fn json_string<T: Serialize>(value: &T) -> Result<String, CliError> {
    serde_json::to_string_pretty(value)
        .map_err(|error| CliError::Runtime(format!("encode JSON: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_words_input_accepts_json_array() {
        let words = parse_words_input(r#"["alpha","bravo"]"#).unwrap();
        assert_eq!(words, vec!["alpha".to_string(), "bravo".to_string()]);
    }

    #[test]
    fn parse_words_input_accepts_json_object() {
        let words = parse_words_input(r#"{"words":["alpha"]}"#).unwrap();
        assert_eq!(words, vec!["alpha".to_string()]);
    }

    #[test]
    fn parse_words_input_accepts_newline_list() {
        let words = parse_words_input("alpha\n\nbravo\n").unwrap();
        assert_eq!(words, vec!["alpha".to_string(), "bravo".to_string()]);
    }

    #[test]
    fn parse_words_input_accepts_comma_list() {
        let words = parse_words_input("alpha, bravo,,charlie").unwrap();
        assert_eq!(
            words,
            vec![
                "alpha".to_string(),
                "bravo".to_string(),
                "charlie".to_string()
            ]
        );
    }
}
