#![recursion_limit = "512"]

use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Input, MultiSelect, Select};
use mag_core::constants::{self, CharacterFlags, ItemFlags};
use mag_core::string_operations::c_string_to_str;
use mag_core::types::{Character, Item};
use mag_core::{ranks, skills, traits};
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use server_utils::admin_client::{
    AdminClient, BadwordEntryResponse, BadwordsListResponse, BadwordsMutationResponse,
    BanActionStatusResponse, BanCreateRequest, BanCreateTargetRequest, BanListResponse,
    BanMutationResponse, CharacterSearchResult, GlobalsResponse, TemplateListResponse,
    TemplateSummary, TextReloadResponse, TextReloadStatusResponse, WorldActionKind,
    WorldActionResponse, WorldActionStatusResponse,
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
    arg_required_else_help = false
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
        help = "Run a subcommand non-interactively instead of opening the menu"
    )]
    auto: bool,

    #[arg(
        long,
        global = true,
        hide = true,
        help = "Deprecated no-op; the interactive menu is now the default"
    )]
    menu: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
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
    /// Query and mutate account, character, and IPv4 bans.
    Bans {
        #[command(subcommand)]
        command: BansCommand,
    },
    /// Search and inspect item and character templates.
    Templates {
        #[command(subcommand)]
        command: TemplatesCommand,
    },
    /// Inspect persisted global server state.
    Globals {
        #[command(subcommand)]
        command: GlobalsCommand,
    },
    /// Execute live world actions on the running server.
    World {
        #[command(subcommand)]
        command: WorldCommand,
    },
}

#[derive(Debug, Subcommand)]
enum WorldCommand {
    /// Enqueue a live world action.
    Action {
        #[command(subcommand)]
        command: WorldActionCommand,
    },
}

#[derive(Debug, Subcommand)]
enum WorldActionCommand {
    /// Spawn missing respawnable NPC templates.
    Populate {
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Recompute map lighting.
    RebuildLights {
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Synchronize player skill metadata from templates.
    SyncSkills {
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Reset one character template and its live instances.
    ResetChar {
        template_id: usize,
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Reset one item template and its live instances.
    ResetItem {
        template_id: usize,
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Reset all character and item templates.
    ResetAll {
        #[arg(long, help = "Wait until the running server reports action applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
}

#[derive(Debug, Subcommand)]
enum TemplatesCommand {
    /// Search or show item templates.
    Items {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    /// Search or show character templates.
    Characters {
        #[command(subcommand)]
        command: TemplateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TemplateCommand {
    /// Fuzzy-search template names and references.
    Search {
        query: String,
        #[arg(long, default_value_t = 20, help = "Maximum matches to print")]
        limit: usize,
        #[arg(long, help = "Include stored templates whose used flag is empty")]
        all: bool,
        #[arg(long, help = "Also show the full details for the top match")]
        details: bool,
    },
    /// Show the full details for one template slot.
    Show { id: usize },
}

#[derive(Debug, Subcommand)]
enum GlobalsCommand {
    /// Show persisted global server counters.
    Show,
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

#[derive(Debug, Subcommand)]
enum BansCommand {
    /// List active bans.
    List {
        #[arg(long, help = "Filter by account, character, or ipv4")]
        scope: Option<String>,
        #[arg(long, help = "Include expired records still present in storage")]
        include_expired: bool,
    },
    /// Ban an account by account id or username.
    AddAccount {
        #[arg(long)]
        account_id: Option<u64>,
        #[arg(long)]
        username: Option<String>,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        expires_at: Option<u64>,
        #[arg(long)]
        duration_seconds: Option<u64>,
        #[arg(long, help = "Do not request live kicks for matching online sessions")]
        no_kick: bool,
        #[arg(long, help = "Wait until live kick enforcement is applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Ban a character by API character id.
    AddCharacter {
        character_id: u64,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        expires_at: Option<u64>,
        #[arg(long)]
        duration_seconds: Option<u64>,
        #[arg(long, help = "Do not request live kicks for matching online sessions")]
        no_kick: bool,
        #[arg(long, help = "Wait until live kick enforcement is applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Ban an IPv4 address.
    AddIp {
        address: String,
        #[arg(long)]
        reason: Option<String>,
        #[arg(long)]
        expires_at: Option<u64>,
        #[arg(long)]
        duration_seconds: Option<u64>,
        #[arg(long, help = "Do not request live kicks for matching online sessions")]
        no_kick: bool,
        #[arg(long, help = "Wait until live kick enforcement is applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Remove an account ban.
    RemoveAccount {
        account_id: u64,
        #[arg(long, help = "Wait until live unban notification is applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Remove a character ban.
    RemoveCharacter {
        character_id: u64,
        #[arg(long, help = "Wait until live unban notification is applied")]
        wait: bool,
        #[arg(long, default_value_t = DEFAULT_WAIT_TIMEOUT_SECS)]
        timeout_seconds: u64,
    },
    /// Remove an IPv4 ban.
    RemoveIp {
        address: String,
        #[arg(long, help = "Wait until live unban notification is applied")]
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
    WorldEffects,
    BanManagement,
    BadwordManagement,
    TemplateManagement,
    ShowGlobals,
    Quit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldMenuAction {
    Populate,
    RebuildLights,
    SyncSkills,
    ResetChar,
    ResetItem,
    ResetAll,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BadwordsMenuAction {
    List,
    Get,
    Add,
    Remove,
    Export,
    Refresh,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BansMenuAction {
    List,
    AddAccount,
    AddCharacter,
    AddIp,
    Remove,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplatesMenuAction {
    SearchItemTemplates,
    SearchCharacterTemplates,
    ShowItemTemplate,
    ShowCharacterTemplate,
    Back,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TemplateKindArg {
    Items,
    Characters,
}

#[derive(Debug, Clone, Serialize)]
struct TemplateMatch {
    id: usize,
    score: i64,
    used: bool,
    name: String,
    reference: String,
    matched_field: String,
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
    if cli.command.is_some() && !cli.auto {
        return Err(CliError::Runtime(
            "subcommands are automation mode; pass --auto or run `mag-admin` for the menu"
                .to_string(),
        ));
    }
    if cli.auto && cli.command.is_none() {
        return Err(CliError::Runtime(
            "--auto requires a subcommand; run `mag-admin --help` for command help".to_string(),
        ));
    }

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

    if cli.menu && !cli.auto {
        return run_menu(&cli, &client);
    }

    if !cli.auto {
        return run_menu(&cli, &client);
    }

    let Some(command) = &cli.command else {
        return Err(CliError::Runtime(
            "no command provided; pass --menu or run `mag-admin help`".to_string(),
        ));
    };

    match command {
        Commands::Badwords { command } => run_badwords(&cli, &client, command),
        Commands::Bans { command } => run_bans(&cli, &client, command),
        Commands::Templates { command } => run_templates(&cli, &client, command),
        Commands::Globals { command } => run_globals(&cli, &client, command),
        Commands::World { command } => run_world(&cli, &client, command),
    }
}

fn run_menu(cli: &Cli, client: &AdminClient) -> Result<(), CliError> {
    let theme = ColorfulTheme::default();
    print_menu_header(&cli.api);

    loop {
        let action = choose_menu_action(&theme)?;
        match action {
            MenuAction::WorldEffects => run_world_effects_menu(client, &theme)?,
            MenuAction::BanManagement => run_bans_menu(client, &theme)?,
            MenuAction::BadwordManagement => run_badwords_menu(client, &theme)?,
            MenuAction::TemplateManagement => run_templates_menu(client, &theme)?,
            MenuAction::ShowGlobals => menu_show_globals(client)?,
            MenuAction::Quit => break,
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
        "World effects",
        "Ban management",
        "Badword management",
        "Template management",
        "View globals",
        "Quit",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("Choose an action")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => MenuAction::WorldEffects,
        1 => MenuAction::BanManagement,
        2 => MenuAction::BadwordManagement,
        3 => MenuAction::TemplateManagement,
        4 => MenuAction::ShowGlobals,
        _ => MenuAction::Quit,
    })
}

fn run_bans_menu(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    loop {
        match choose_bans_menu_action(theme)? {
            BansMenuAction::List => menu_list_bans(client)?,
            BansMenuAction::AddAccount => menu_add_account_ban(client, theme)?,
            BansMenuAction::AddCharacter => menu_add_character_ban(client, theme)?,
            BansMenuAction::AddIp => menu_add_ip_ban(client, theme)?,
            BansMenuAction::Remove => menu_remove_ban(client, theme)?,
            BansMenuAction::Back => break,
        }
    }
    Ok(())
}

fn choose_bans_menu_action(theme: &ColorfulTheme) -> Result<BansMenuAction, CliError> {
    let items = [
        "List bans",
        "Ban account",
        "Ban character",
        "Ban IPv4 address",
        "Remove ban",
        "Back",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("Ban management")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => BansMenuAction::List,
        1 => BansMenuAction::AddAccount,
        2 => BansMenuAction::AddCharacter,
        3 => BansMenuAction::AddIp,
        4 => BansMenuAction::Remove,
        _ => BansMenuAction::Back,
    })
}

fn run_world_effects_menu(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    loop {
        match choose_world_menu_action(theme)? {
            WorldMenuAction::Populate => {
                menu_request_world_action(client, theme, WorldActionKind::PopulateMissing, None)?
            }
            WorldMenuAction::RebuildLights => {
                menu_request_world_action(client, theme, WorldActionKind::RebuildLights, None)?
            }
            WorldMenuAction::SyncSkills => {
                menu_request_world_action(client, theme, WorldActionKind::SyncPlayerSkills, None)?
            }
            WorldMenuAction::ResetChar => menu_reset_char(client, theme)?,
            WorldMenuAction::ResetItem => menu_reset_item(client, theme)?,
            WorldMenuAction::ResetAll => menu_request_world_action(
                client,
                theme,
                WorldActionKind::ResetAll,
                Some("reset all character and item templates"),
            )?,
            WorldMenuAction::Back => break,
        }
    }
    Ok(())
}

fn choose_world_menu_action(theme: &ColorfulTheme) -> Result<WorldMenuAction, CliError> {
    let items = [
        "Populate missing NPCs",
        "Rebuild lights",
        "Sync player skills",
        "Reset character template",
        "Reset item template",
        "Reset all templates",
        "Back",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("World effects")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => WorldMenuAction::Populate,
        1 => WorldMenuAction::RebuildLights,
        2 => WorldMenuAction::SyncSkills,
        3 => WorldMenuAction::ResetChar,
        4 => WorldMenuAction::ResetItem,
        5 => WorldMenuAction::ResetAll,
        _ => WorldMenuAction::Back,
    })
}

fn run_badwords_menu(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    loop {
        match choose_badwords_menu_action(theme)? {
            BadwordsMenuAction::List => menu_list_badwords(client)?,
            BadwordsMenuAction::Get => menu_get_badword(client, theme)?,
            BadwordsMenuAction::Add => menu_add_badwords(client, theme)?,
            BadwordsMenuAction::Remove => menu_remove_badwords(client, theme)?,
            BadwordsMenuAction::Export => menu_export_badwords(client, theme)?,
            BadwordsMenuAction::Refresh => menu_refresh_badwords(client, theme)?,
            BadwordsMenuAction::Back => break,
        }
    }
    Ok(())
}

fn choose_badwords_menu_action(theme: &ColorfulTheme) -> Result<BadwordsMenuAction, CliError> {
    let items = [
        "List badwords",
        "Check one badword",
        "Add badwords",
        "Remove badwords",
        "Export badwords",
        "Refresh running server cache",
        "Back",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("Badword management")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => BadwordsMenuAction::List,
        1 => BadwordsMenuAction::Get,
        2 => BadwordsMenuAction::Add,
        3 => BadwordsMenuAction::Remove,
        4 => BadwordsMenuAction::Export,
        5 => BadwordsMenuAction::Refresh,
        _ => BadwordsMenuAction::Back,
    })
}

fn run_templates_menu(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    loop {
        match choose_templates_menu_action(theme)? {
            TemplatesMenuAction::SearchItemTemplates => {
                menu_search_templates(client, theme, TemplateKindArg::Items)?
            }
            TemplatesMenuAction::SearchCharacterTemplates => {
                menu_search_templates(client, theme, TemplateKindArg::Characters)?
            }
            TemplatesMenuAction::ShowItemTemplate => {
                menu_show_template(client, theme, TemplateKindArg::Items)?
            }
            TemplatesMenuAction::ShowCharacterTemplate => {
                menu_show_template(client, theme, TemplateKindArg::Characters)?
            }
            TemplatesMenuAction::Back => break,
        }
    }
    Ok(())
}

fn choose_templates_menu_action(theme: &ColorfulTheme) -> Result<TemplatesMenuAction, CliError> {
    let items = [
        "Search item templates",
        "Search character templates",
        "Show item template by id",
        "Show character template by id",
        "Back",
    ];
    let selected = Select::with_theme(theme)
        .with_prompt("Template management")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;

    Ok(match selected {
        0 => TemplatesMenuAction::SearchItemTemplates,
        1 => TemplatesMenuAction::SearchCharacterTemplates,
        2 => TemplatesMenuAction::ShowItemTemplate,
        3 => TemplatesMenuAction::ShowCharacterTemplate,
        _ => TemplatesMenuAction::Back,
    })
}

fn menu_search_templates(
    client: &AdminClient,
    theme: &ColorfulTheme,
    kind: TemplateKindArg,
) -> Result<(), CliError> {
    let query = prompt_text(theme, "Search query", None)?;
    let response = fetch_template_summaries(client, kind)?;
    let matches = rank_template_summaries(&response.items, &query, false, 20);
    if matches.is_empty() {
        return Err(CliError::NotFound(format!(
            "no {} templates matched {query:?}",
            template_kind_label(kind)
        )));
    }

    print_template_matches(&matches, OutputFormat::Table)?;
    let items: Vec<String> = matches
        .iter()
        .map(|template_match| {
            format!(
                "{}: {} ({})",
                template_match.id, template_match.name, template_match.score
            )
        })
        .collect();
    let selected = Select::with_theme(theme)
        .with_prompt("Show details for which match?")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
    if let Some(template_match) = matches.get(selected) {
        show_template(client, kind, template_match.id, OutputFormat::Table)?;
    }
    Ok(())
}

fn menu_show_template(
    client: &AdminClient,
    theme: &ColorfulTheme,
    kind: TemplateKindArg,
) -> Result<(), CliError> {
    let id = prompt_usize(theme, "Template id")?;
    show_template(client, kind, id, OutputFormat::Table)
}

fn menu_show_globals(client: &AdminClient) -> Result<(), CliError> {
    let globals = client.fetch_globals().map_err(CliError::Runtime)?;
    print_globals(&globals, OutputFormat::Table)
}

fn menu_reset_char(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let template_id = prompt_usize(theme, "Character template id")?;
    menu_request_world_action(
        client,
        theme,
        WorldActionKind::ResetChar { template_id },
        Some("reset this character template and its live instances"),
    )
}

fn menu_reset_item(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let template_id = prompt_usize(theme, "Item template id")?;
    menu_request_world_action(
        client,
        theme,
        WorldActionKind::ResetItem { template_id },
        Some("reset this item template and its live instances"),
    )
}

fn menu_request_world_action(
    client: &AdminClient,
    theme: &ColorfulTheme,
    action: WorldActionKind,
    confirmation: Option<&str>,
) -> Result<(), CliError> {
    if let Some(label) = confirmation {
        let confirmed = Confirm::with_theme(theme)
            .with_prompt(format!("Confirm world action: {label}?"))
            .default(false)
            .interact()
            .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
        if !confirmed {
            println!("Action cancelled.");
            return Ok(());
        }
    }

    let wait = Confirm::with_theme(theme)
        .with_prompt("Wait for the running server to apply the action?")
        .default(true)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
    let timeout_seconds = if wait {
        prompt_timeout_seconds(theme)?
    } else {
        DEFAULT_WAIT_TIMEOUT_SECS
    };

    let response = client
        .request_world_action(&action)
        .map_err(CliError::Runtime)?;
    if wait {
        let status = wait_for_world_action(client, &response.request_id, timeout_seconds)?;
        print_world_action_status(&status, OutputFormat::Table, false)
    } else {
        print_world_action_response(&response, OutputFormat::Table, false)
    }
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

fn menu_list_bans(client: &AdminClient) -> Result<(), CliError> {
    let response = client.list_bans(None, false).map_err(CliError::Runtime)?;
    print_ban_list(&response, OutputFormat::Table)
}

fn menu_add_account_ban(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let character = menu_select_character_for_ban(client, theme)?;
    let Some(account_id) = character.account_id else {
        return Err(CliError::Runtime(format!(
            "character '{}' does not have an account id",
            character.name
        )));
    };
    let reason = prompt_optional_text(theme, "Reason (optional)")?;
    let response = client
        .create_ban(&BanCreateRequest {
            target: BanCreateTargetRequest::Account {
                account_id: Some(account_id),
                username: character.account_username.clone(),
            },
            reason,
            expires_at: None,
            duration_seconds: None,
            created_by: Some("mag-admin".to_string()),
            kick_online: Some(true),
        })
        .map_err(CliError::Runtime)?;
    print_ban_mutation(&response, OutputFormat::Table, false)?;
    menu_wait_for_ban_action(client, theme, &response)
}

fn menu_add_character_ban(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let character = menu_select_character_for_ban(client, theme)?;
    let reason = prompt_optional_text(theme, "Reason (optional)")?;
    let response = client
        .create_ban(&BanCreateRequest {
            target: BanCreateTargetRequest::Character {
                character_id: character.id,
            },
            reason,
            expires_at: None,
            duration_seconds: None,
            created_by: Some("mag-admin".to_string()),
            kick_online: Some(true),
        })
        .map_err(CliError::Runtime)?;
    print_ban_mutation(&response, OutputFormat::Table, false)?;
    menu_wait_for_ban_action(client, theme, &response)
}

fn menu_select_character_for_ban(
    client: &AdminClient,
    theme: &ColorfulTheme,
) -> Result<CharacterSearchResult, CliError> {
    let query = prompt_text(theme, "Character name", None)?;
    let response = client
        .search_characters(&query, 20)
        .map_err(CliError::Runtime)?;
    if response.characters.is_empty() {
        return Err(CliError::Runtime(format!(
            "no characters found matching '{}'",
            response.query
        )));
    }

    let items: Vec<String> = response
        .characters
        .iter()
        .map(format_character_search_result)
        .collect();
    let selected = Select::with_theme(theme)
        .with_prompt("Choose character")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
    Ok(response.characters[selected].clone())
}

fn format_character_search_result(character: &CharacterSearchResult) -> String {
    let account = match (&character.account_username, character.account_id) {
        (Some(username), Some(account_id)) => format!("{} ({})", username, account_id),
        (Some(username), None) => username.clone(),
        (None, Some(account_id)) => account_id.to_string(),
        (None, None) => "unknown".to_string(),
    };
    let server = character
        .server_id
        .map(|value| value.to_string())
        .unwrap_or_else(|| "offline".to_string());
    format!(
        "{} (api id {}, /who id {}, account {})",
        character.name, character.id, server, account
    )
}

fn menu_add_ip_ban(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let address = prompt_text(theme, "IPv4 address", None)?;
    let reason = prompt_optional_text(theme, "Reason (optional)")?;
    let response = client
        .create_ban(&BanCreateRequest {
            target: BanCreateTargetRequest::Ipv4 { address },
            reason,
            expires_at: None,
            duration_seconds: None,
            created_by: Some("mag-admin".to_string()),
            kick_online: Some(true),
        })
        .map_err(CliError::Runtime)?;
    print_ban_mutation(&response, OutputFormat::Table, false)?;
    menu_wait_for_ban_action(client, theme, &response)
}

fn menu_remove_ban(client: &AdminClient, theme: &ColorfulTheme) -> Result<(), CliError> {
    let bans = client
        .list_bans(None, false)
        .map_err(CliError::Runtime)?
        .bans;
    if bans.is_empty() {
        println!("No active bans to remove.");
        return Ok(());
    }

    let items: Vec<String> = bans.iter().map(format_ban_choice).collect();
    let selected = Select::with_theme(theme)
        .with_prompt("Choose ban to remove")
        .items(&items)
        .default(0)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?;
    let ban = &bans[selected];
    let response = client
        .remove_ban(&ban.target.scope, &ban.target.value)
        .map_err(CliError::Runtime)?;
    print_ban_mutation(&response, OutputFormat::Table, false)?;
    menu_wait_for_ban_action(client, theme, &response)
}

fn format_ban_choice(ban: &server_utils::admin_client::BanRecordResponse) -> String {
    let expires_at = ban
        .expires_at
        .map(|value| value.to_string())
        .unwrap_or_else(|| "permanent".to_string());
    let reason = if ban.reason.trim().is_empty() {
        "no reason"
    } else {
        ban.reason.trim()
    };
    format!(
        "{} {} (expires {}, by {}, {})",
        ban.target.scope, ban.target.value, expires_at, ban.created_by, reason
    )
}

fn menu_wait_for_ban_action(
    client: &AdminClient,
    theme: &ColorfulTheme,
    response: &BanMutationResponse,
) -> Result<(), CliError> {
    let Some(request_id) = response.live_request_id.as_deref() else {
        return Ok(());
    };
    if Confirm::with_theme(theme)
        .with_prompt("Wait for live ban enforcement?")
        .default(true)
        .interact()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))?
    {
        let timeout_seconds = prompt_timeout_seconds(theme)?;
        let status = wait_for_ban_action(client, request_id, timeout_seconds)?;
        print_ban_action_status(&status, OutputFormat::Table, false)?;
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

fn prompt_optional_text(theme: &ColorfulTheme, prompt: &str) -> Result<Option<String>, CliError> {
    let value = prompt_text(theme, prompt, Some(""))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed.to_string()))
    }
}

fn prompt_timeout_seconds(theme: &ColorfulTheme) -> Result<u64, CliError> {
    Input::<u64>::with_theme(theme)
        .with_prompt("Refresh wait timeout in seconds")
        .default(DEFAULT_WAIT_TIMEOUT_SECS)
        .interact_text()
        .map_err(|error| CliError::Runtime(format!("menu prompt failed: {error}")))
}

fn prompt_usize(theme: &ColorfulTheme, prompt: &str) -> Result<usize, CliError> {
    Input::<usize>::with_theme(theme)
        .with_prompt(prompt.to_string())
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

fn run_bans(cli: &Cli, client: &AdminClient, command: &BansCommand) -> Result<(), CliError> {
    match command {
        BansCommand::List {
            scope,
            include_expired,
        } => {
            let response = client
                .list_bans(scope.as_deref(), *include_expired)
                .map_err(CliError::Runtime)?;
            print_ban_list(&response, cli.format)
        }
        BansCommand::AddAccount {
            account_id,
            username,
            reason,
            expires_at,
            duration_seconds,
            no_kick,
            wait,
            timeout_seconds,
        } => run_ban_create(
            cli,
            client,
            BanCreateRequest {
                target: BanCreateTargetRequest::Account {
                    account_id: *account_id,
                    username: username.clone(),
                },
                reason: reason.clone(),
                expires_at: *expires_at,
                duration_seconds: *duration_seconds,
                created_by: Some("mag-admin".to_string()),
                kick_online: Some(!*no_kick),
            },
            *wait,
            *timeout_seconds,
        ),
        BansCommand::AddCharacter {
            character_id,
            reason,
            expires_at,
            duration_seconds,
            no_kick,
            wait,
            timeout_seconds,
        } => run_ban_create(
            cli,
            client,
            BanCreateRequest {
                target: BanCreateTargetRequest::Character {
                    character_id: *character_id,
                },
                reason: reason.clone(),
                expires_at: *expires_at,
                duration_seconds: *duration_seconds,
                created_by: Some("mag-admin".to_string()),
                kick_online: Some(!*no_kick),
            },
            *wait,
            *timeout_seconds,
        ),
        BansCommand::AddIp {
            address,
            reason,
            expires_at,
            duration_seconds,
            no_kick,
            wait,
            timeout_seconds,
        } => run_ban_create(
            cli,
            client,
            BanCreateRequest {
                target: BanCreateTargetRequest::Ipv4 {
                    address: address.clone(),
                },
                reason: reason.clone(),
                expires_at: *expires_at,
                duration_seconds: *duration_seconds,
                created_by: Some("mag-admin".to_string()),
                kick_online: Some(!*no_kick),
            },
            *wait,
            *timeout_seconds,
        ),
        BansCommand::RemoveAccount {
            account_id,
            wait,
            timeout_seconds,
        } => run_ban_remove(
            cli,
            client,
            "account",
            &account_id.to_string(),
            *wait,
            *timeout_seconds,
        ),
        BansCommand::RemoveCharacter {
            character_id,
            wait,
            timeout_seconds,
        } => run_ban_remove(
            cli,
            client,
            "character",
            &character_id.to_string(),
            *wait,
            *timeout_seconds,
        ),
        BansCommand::RemoveIp {
            address,
            wait,
            timeout_seconds,
        } => run_ban_remove(cli, client, "ip", address, *wait, *timeout_seconds),
    }
}

fn run_ban_create(
    cli: &Cli,
    client: &AdminClient,
    request: BanCreateRequest,
    wait: bool,
    timeout_seconds: u64,
) -> Result<(), CliError> {
    let response = client.create_ban(&request).map_err(CliError::Runtime)?;
    print_ban_mutation(&response, cli.format, cli.quiet)?;
    maybe_wait_for_ban_action(cli, client, &response, wait, timeout_seconds)
}

fn run_ban_remove(
    cli: &Cli,
    client: &AdminClient,
    scope: &str,
    value: &str,
    wait: bool,
    timeout_seconds: u64,
) -> Result<(), CliError> {
    let response = client.remove_ban(scope, value).map_err(CliError::Runtime)?;
    print_ban_mutation(&response, cli.format, cli.quiet)?;
    maybe_wait_for_ban_action(cli, client, &response, wait, timeout_seconds)
}

fn maybe_wait_for_ban_action(
    cli: &Cli,
    client: &AdminClient,
    response: &BanMutationResponse,
    wait: bool,
    timeout_seconds: u64,
) -> Result<(), CliError> {
    if !wait {
        return Ok(());
    }
    let Some(request_id) = response.live_request_id.as_deref() else {
        return Ok(());
    };
    let status = wait_for_ban_action(client, request_id, timeout_seconds)?;
    print_ban_action_status(&status, cli.format, cli.quiet)
}

fn run_templates(
    cli: &Cli,
    client: &AdminClient,
    command: &TemplatesCommand,
) -> Result<(), CliError> {
    match command {
        TemplatesCommand::Items { command } => {
            run_template_command(cli, client, TemplateKindArg::Items, command)
        }
        TemplatesCommand::Characters { command } => {
            run_template_command(cli, client, TemplateKindArg::Characters, command)
        }
    }
}

fn run_template_command(
    cli: &Cli,
    client: &AdminClient,
    kind: TemplateKindArg,
    command: &TemplateCommand,
) -> Result<(), CliError> {
    match command {
        TemplateCommand::Search {
            query,
            limit,
            all,
            details,
        } => {
            let response = fetch_template_summaries(client, kind)?;
            let matches = rank_template_summaries(&response.items, query, *all, *limit);
            if matches.is_empty() {
                return Err(CliError::NotFound(format!(
                    "no {} templates matched {query:?}",
                    template_kind_label(kind)
                )));
            }
            print_template_matches(&matches, cli.format)?;
            if *details {
                if cli.format == OutputFormat::Table {
                    println!();
                }
                show_template(client, kind, matches[0].id, cli.format)?;
            }
            Ok(())
        }
        TemplateCommand::Show { id } => show_template(client, kind, *id, cli.format),
    }
}

fn run_globals(cli: &Cli, client: &AdminClient, command: &GlobalsCommand) -> Result<(), CliError> {
    match command {
        GlobalsCommand::Show => {
            let globals = client.fetch_globals().map_err(CliError::Runtime)?;
            print_globals(&globals, cli.format)
        }
    }
}

fn run_world(cli: &Cli, client: &AdminClient, command: &WorldCommand) -> Result<(), CliError> {
    match command {
        WorldCommand::Action { command } => run_world_action(cli, client, command),
    }
}

fn run_world_action(
    cli: &Cli,
    client: &AdminClient,
    command: &WorldActionCommand,
) -> Result<(), CliError> {
    let (action, wait, timeout_seconds) = match command {
        WorldActionCommand::Populate {
            wait,
            timeout_seconds,
        } => (WorldActionKind::PopulateMissing, *wait, *timeout_seconds),
        WorldActionCommand::RebuildLights {
            wait,
            timeout_seconds,
        } => (WorldActionKind::RebuildLights, *wait, *timeout_seconds),
        WorldActionCommand::SyncSkills {
            wait,
            timeout_seconds,
        } => (WorldActionKind::SyncPlayerSkills, *wait, *timeout_seconds),
        WorldActionCommand::ResetChar {
            template_id,
            wait,
            timeout_seconds,
        } => (
            WorldActionKind::ResetChar {
                template_id: *template_id,
            },
            *wait,
            *timeout_seconds,
        ),
        WorldActionCommand::ResetItem {
            template_id,
            wait,
            timeout_seconds,
        } => (
            WorldActionKind::ResetItem {
                template_id: *template_id,
            },
            *wait,
            *timeout_seconds,
        ),
        WorldActionCommand::ResetAll {
            wait,
            timeout_seconds,
        } => (WorldActionKind::ResetAll, *wait, *timeout_seconds),
    };

    request_and_maybe_wait_world_action(cli, client, action, wait, timeout_seconds)
}

fn request_and_maybe_wait_world_action(
    cli: &Cli,
    client: &AdminClient,
    action: WorldActionKind,
    wait: bool,
    timeout_seconds: u64,
) -> Result<(), CliError> {
    let response = client
        .request_world_action(&action)
        .map_err(CliError::Runtime)?;
    if wait {
        let status = wait_for_world_action(client, &response.request_id, timeout_seconds)?;
        print_world_action_status(&status, cli.format, cli.quiet)
    } else {
        print_world_action_response(&response, cli.format, cli.quiet)
    }
}

fn fetch_template_summaries(
    client: &AdminClient,
    kind: TemplateKindArg,
) -> Result<TemplateListResponse, CliError> {
    match kind {
        TemplateKindArg::Items => client
            .fetch_item_template_summaries()
            .map_err(CliError::Runtime),
        TemplateKindArg::Characters => client
            .fetch_character_template_summaries()
            .map_err(CliError::Runtime),
    }
}

fn show_template(
    client: &AdminClient,
    kind: TemplateKindArg,
    id: usize,
    format: OutputFormat,
) -> Result<(), CliError> {
    match kind {
        TemplateKindArg::Items => {
            let item = client
                .fetch_single_item_template(id)
                .map_err(CliError::Runtime)?;
            print_item_template_detail(id, &item, format)
        }
        TemplateKindArg::Characters => {
            let character = client
                .fetch_single_character_template(id)
                .map_err(CliError::Runtime)?;
            print_character_template_detail(id, &character, format)
        }
    }
}

fn template_kind_label(kind: TemplateKindArg) -> &'static str {
    match kind {
        TemplateKindArg::Items => "item",
        TemplateKindArg::Characters => "character",
    }
}

fn rank_template_summaries(
    summaries: &[TemplateSummary],
    query: &str,
    include_unused: bool,
    limit: usize,
) -> Vec<TemplateMatch> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() || limit == 0 {
        return Vec::new();
    }

    let mut matches: Vec<TemplateMatch> = summaries
        .iter()
        .filter(|summary| include_unused || summary.used)
        .filter_map(|summary| {
            let name_score = fuzzy_field_score(&summary.name, &normalized_query);
            let reference_score = fuzzy_field_score(&summary.reference, &normalized_query);
            let (score, matched_field) = match (name_score, reference_score) {
                (Some(name_score), Some(reference_score)) if reference_score > name_score => {
                    (reference_score, "reference")
                }
                (Some(name_score), _) => (name_score, "name"),
                (None, Some(reference_score)) => (reference_score, "reference"),
                (None, None) => return None,
            };
            Some(TemplateMatch {
                id: summary.id,
                score,
                used: summary.used,
                name: summary.name.clone(),
                reference: summary.reference.clone(),
                matched_field: matched_field.to_string(),
            })
        })
        .collect();

    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then(left.id.cmp(&right.id))
            .then(left.name.cmp(&right.name))
    });
    matches.truncate(limit);
    matches
}

fn fuzzy_field_score(field: &str, normalized_query: &str) -> Option<i64> {
    let normalized_field = field.trim().to_ascii_lowercase();
    if normalized_field.is_empty() {
        return None;
    }
    if normalized_field == normalized_query {
        return Some(10_000 - normalized_field.len() as i64);
    }
    if normalized_field.starts_with(normalized_query) {
        return Some(8_000 - normalized_field.len() as i64);
    }
    if let Some(position) = normalized_field.find(normalized_query) {
        return Some(6_000 - position as i64 - normalized_field.len() as i64);
    }
    ordered_subsequence_score(&normalized_field, normalized_query)
}

fn ordered_subsequence_score(normalized_field: &str, normalized_query: &str) -> Option<i64> {
    let mut query_chars = normalized_query.chars();
    let mut wanted = query_chars.next()?;
    let mut first_match = None;

    for (position, field_char) in normalized_field.chars().enumerate() {
        if field_char != wanted {
            continue;
        }
        first_match.get_or_insert(position);
        match query_chars.next() {
            Some(next) => wanted = next,
            None => {
                let span = position.saturating_sub(first_match.unwrap_or(0)) + 1;
                return Some(4_000 - span as i64 - normalized_field.len() as i64);
            }
        }
    }

    None
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

fn wait_for_world_action(
    client: &AdminClient,
    request_id: &str,
    timeout_seconds: u64,
) -> Result<WorldActionStatusResponse, CliError> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let status = client
            .world_action_status(request_id)
            .map_err(CliError::Runtime)?;
        if status.status == "applied" {
            return Ok(status);
        }
        if status.status == "failed" {
            return Err(CliError::Runtime(format!(
                "world action {} failed: {}",
                request_id, status.message
            )));
        }
        if Instant::now() >= deadline {
            return Err(CliError::Runtime(format!(
                "timed out waiting for world action {}",
                request_id
            )));
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn wait_for_ban_action(
    client: &AdminClient,
    request_id: &str,
    timeout_seconds: u64,
) -> Result<BanActionStatusResponse, CliError> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        let status = client
            .ban_action_status(request_id)
            .map_err(CliError::Runtime)?;
        if status.status == "applied" {
            return Ok(status);
        }
        if status.status == "failed" {
            return Err(CliError::Runtime(format!(
                "ban action {} failed: {}",
                request_id, status.message
            )));
        }
        if Instant::now() >= deadline {
            return Err(CliError::Runtime(format!(
                "timed out waiting for ban action {}",
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

fn print_world_action_response(
    response: &WorldActionResponse,
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
            println!("REQUEST_ID  ACTION");
            println!("{}  {}", response.request_id, response.action);
        }
    }
    Ok(())
}

fn print_world_action_status(
    response: &WorldActionStatusResponse,
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
            println!("REQUEST_ID  ACTION  STATUS  UPDATED_AT  MESSAGE");
            println!(
                "{}  {}  {}  {}  {}",
                response.request_id,
                response.action,
                response.status,
                response.updated_at,
                response.message
            );
        }
    }
    Ok(())
}

fn print_ban_list(response: &BanListResponse, format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => {
            for ban in &response.bans {
                println!(
                    "{}\t{}\t{}\t{}",
                    ban.target.scope, ban.target.value, ban.active, ban.reason
                );
            }
        }
        OutputFormat::Table => {
            println!("SCOPE  TARGET  ACTIVE  EXPIRES_AT  CREATED_BY  REASON");
            for ban in &response.bans {
                let expires_at = ban
                    .expires_at
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "permanent".to_string());
                println!(
                    "{}  {}  {}  {}  {}  {}",
                    ban.target.scope,
                    ban.target.value,
                    ban.active,
                    expires_at,
                    ban.created_by,
                    ban.reason
                );
            }
        }
    }
    Ok(())
}

fn print_ban_mutation(
    response: &BanMutationResponse,
    format: OutputFormat,
    quiet: bool,
) -> Result<(), CliError> {
    if quiet {
        return Ok(());
    }
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain => {
            if let Some(ban) = &response.ban {
                println!("{}\t{}", ban.target.scope, ban.target.value);
            } else {
                println!("unchanged");
            }
        }
        OutputFormat::Table => {
            println!("CHANGED  VERSION  LIVE_REQUEST_ID");
            println!(
                "{}  {}  {}",
                response.changed,
                response.version,
                response.live_request_id.as_deref().unwrap_or("")
            );
            if let Some(ban) = &response.ban {
                println!();
                println!("SCOPE  TARGET  ACTIVE  EXPIRES_AT  CREATED_BY  REASON");
                let expires_at = ban
                    .expires_at
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "permanent".to_string());
                println!(
                    "{}  {}  {}  {}  {}  {}",
                    ban.target.scope,
                    ban.target.value,
                    ban.active,
                    expires_at,
                    ban.created_by,
                    ban.reason
                );
            }
        }
    }
    Ok(())
}

fn print_ban_action_status(
    response: &BanActionStatusResponse,
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
            println!("REQUEST_ID  ACTION  STATUS  UPDATED_AT  MESSAGE");
            println!(
                "{}  {}  {}  {}  {}",
                response.request_id,
                response.action,
                response.status,
                response.updated_at,
                response.message
            );
        }
    }
    Ok(())
}

fn print_template_matches(matches: &[TemplateMatch], format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(matches)?),
        OutputFormat::Plain => {
            for template_match in matches {
                if template_match.reference.is_empty() {
                    println!(
                        "{}\t{}\t{}",
                        template_match.id, template_match.score, template_match.name
                    );
                } else {
                    println!(
                        "{}\t{}\t{}\t{}",
                        template_match.id,
                        template_match.score,
                        template_match.name,
                        template_match.reference
                    );
                }
            }
        }
        OutputFormat::Table => {
            println!("ID  SCORE  USED  FIELD      NAME  REFERENCE");
            for template_match in matches {
                println!(
                    "{}  {}  {}  {:9}  {}  {}",
                    template_match.id,
                    template_match.score,
                    template_match.used,
                    template_match.matched_field,
                    template_match.name,
                    template_match.reference
                );
            }
        }
    }
    Ok(())
}

fn print_item_template_detail(
    id: usize,
    item: &Item,
    format: OutputFormat,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(&item_template_value(id, item))?),
        OutputFormat::Plain | OutputFormat::Table => {
            println!("ITEM TEMPLATE {id}");
            print_detail_line("name", c_string_to_str(&item.name));
            print_detail_line("reference", c_string_to_str(&item.reference));
            print_detail_line("description", c_string_to_str(&item.description));
            print_detail_line("used", item.used);
            print_detail_line("temp", item.temp);
            print_detail_line(
                "flags",
                format_flags(item.flags, &active_item_flags(item.flags)),
            );
            print_detail_line("value", format_gold_silver(item.value));
            print_detail_line("placement", placement_label(item.placement));
            print_detail_line("damage_state", item.damage_state);
            print_detail_debug("max_age", item.max_age);
            print_detail_debug("current_age", item.current_age);
            print_detail_line("max_damage", item.max_damage);
            print_detail_line("current_damage", item.current_damage);
            print_detail_debug("attrib", item.attrib);
            print_detail_debug("hp", item.hp);
            print_detail_debug("end", item.end);
            print_detail_debug("mana", item.mana);
            print_detail_debug("skill", item.skill);
            print_detail_debug("armor", item.armor);
            print_detail_debug("weapon", item.weapon);
            print_detail_debug("light", item.light);
            print_detail_line("duration", item.duration);
            print_detail_line("cost", item.cost);
            print_detail_line("power", item.power);
            print_detail_line("active", item.active);
            print_detail_line("x", item.x);
            print_detail_line("y", item.y);
            print_detail_line("carried", item.carried);
            print_detail_line("sprite_override", item.sprite_override);
            print_detail_debug("sprite", item.sprite);
            print_detail_debug("status", item.status);
            print_detail_debug("gethit_dam", item.gethit_dam);
            print_detail_line("min_rank", rank_label(item.min_rank));
            print_detail_debug("future", item.future);
            print_detail_debug("future3", item.future3);
            print_detail_line("t_bought", item.t_bought);
            print_detail_line("t_sold", item.t_sold);
            print_detail_line("driver", item.driver);
            print_detail_debug("data", item.data);
            print_nonzero_item_skills(item);
        }
    }
    Ok(())
}

fn print_character_template_detail(
    id: usize,
    character: &Character,
    format: OutputFormat,
) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => {
            println!("{}", json_string(&character_template_value(id, character))?)
        }
        OutputFormat::Plain | OutputFormat::Table => {
            println!("CHARACTER TEMPLATE {id}");
            print_detail_line("name", c_string_to_str(&character.name));
            print_detail_line("reference", c_string_to_str(&character.reference));
            print_detail_line("description", c_string_to_str(&character.description));
            print_detail_line("used", character.used);
            print_detail_line("temp", character.temp);
            print_detail_line(
                "kindred",
                format_flags(
                    character.kindred as u64,
                    &active_kindred_flags(character.kindred),
                ),
            );
            print_detail_line("player", character.player);
            print_detail_line("pass1", character.pass1);
            print_detail_line("pass2", character.pass2);
            print_detail_line("sprite", character.sprite);
            print_detail_line("sound", character.sound);
            print_detail_line(
                "flags",
                format_flags(character.flags, &active_character_flags(character.flags)),
            );
            print_detail_line("alignment", character.alignment);
            print_detail_line(
                "temple",
                format!("{},{}", character.temple_x, character.temple_y),
            );
            print_detail_line(
                "tavern",
                format!("{},{}", character.tavern_x, character.tavern_y),
            );
            print_detail_debug("attrib", character.attrib);
            print_detail_debug("hp", character.hp);
            print_detail_debug("end", character.end);
            print_detail_debug("mana", character.mana);
            print_detail_debug("skill", character.skill);
            print_detail_line("weapon_bonus", character.weapon_bonus);
            print_detail_line("armor_bonus", character.armor_bonus);
            print_detail_line("a_hp", character.a_hp);
            print_detail_line("a_end", character.a_end);
            print_detail_line("a_mana", character.a_mana);
            print_detail_line("light", character.light);
            print_detail_line("mode", character.mode);
            print_detail_line("speed", character.speed);
            print_detail_line("points", character.points);
            print_detail_line(
                "points_tot",
                format!(
                    "{} ({})",
                    character.points_tot,
                    ranks::rank_name((character.points_tot as i64).max(0) as u32)
                ),
            );
            print_detail_line("armor", character.armor);
            print_detail_line("weapon", character.weapon);
            print_detail_line("position", format!("{},{}", character.x, character.y));
            print_detail_line("target", format!("{},{}", character.tox, character.toy));
            print_detail_line("from", format!("{},{}", character.frx, character.fry));
            print_detail_line("status", character.status);
            print_detail_line("status2", character.status2);
            print_detail_line("dir", character.dir);
            print_detail_line("gold", format_gold_silver_i32(character.gold));
            print_detail_debug("item", character.item);
            print_detail_debug("worn", character.worn);
            print_detail_debug("spell", character.spell);
            print_detail_line("citem", character.citem);
            print_detail_line("creation_date", character.creation_date);
            print_detail_line("login_date", character.login_date);
            print_detail_line("addr", character.addr);
            print_detail_line("current_online_time", character.current_online_time);
            print_detail_line("total_online_time", character.total_online_time);
            print_detail_line("comp_volume", character.comp_volume);
            print_detail_line("raw_volume", character.raw_volume);
            print_detail_line("idle", character.idle);
            print_detail_line("attack_cn", character.attack_cn);
            print_detail_line("skill_nr", character.skill_nr);
            print_detail_line("skill_target1", character.skill_target1);
            print_detail_line("skill_target2", character.skill_target2);
            print_detail_line("goto", format!("{},{}", character.goto_x, character.goto_y));
            print_detail_line("use_nr", character.use_nr);
            print_detail_line("misc_action", character.misc_action);
            print_detail_line("misc_target1", character.misc_target1);
            print_detail_line("misc_target2", character.misc_target2);
            print_detail_line("cerrno", character.cerrno);
            print_detail_line("escape_timer", character.escape_timer);
            print_detail_debug("enemy", character.enemy);
            print_detail_line("current_enemy", character.current_enemy);
            print_detail_line("retry", character.retry);
            print_detail_line("stunned", character.stunned);
            print_detail_line("speed_mod", character.speed_mod);
            print_detail_line("last_action", character.last_action);
            print_detail_line("unused", character.unused);
            print_detail_line("depot_sold", character.depot_sold);
            print_detail_line("gethit_dam", character.gethit_dam);
            print_detail_line("gethit_bonus", character.gethit_bonus);
            print_detail_line("light_bonus", character.light_bonus);
            print_detail_debug("passwd", character.passwd);
            print_detail_line("lastattack", character.lastattack);
            print_detail_debug("future1", character.future1);
            print_detail_line("sprite_override", character.sprite_override);
            print_detail_debug("future2", character.future2);
            print_detail_debug("depot", character.depot);
            print_detail_line("depot_cost", character.depot_cost);
            print_detail_line("luck", character.luck);
            print_detail_line("unreach", character.unreach);
            print_detail_line("unreachx", character.unreachx);
            print_detail_line("unreachy", character.unreachy);
            print_detail_line("monster_class", character.monster_class);
            print_detail_debug("future3", character.future3);
            print_detail_line("logout_date", character.logout_date);
            print_detail_debug("data", character.data);
            print_detail_debug("text", character_text(character));
            print_nonzero_character_skills(character);
        }
    }
    Ok(())
}

fn print_globals(response: &GlobalsResponse, format: OutputFormat) -> Result<(), CliError> {
    match format {
        OutputFormat::Json => println!("{}", json_string(response)?),
        OutputFormat::Plain | OutputFormat::Table => {
            println!("GLOBALS");
            print_detail_line(
                "date",
                format!(
                    "{} / {} / {}",
                    response.mdtime, response.mdday, response.mdyear
                ),
            );
            print_detail_line("dlight", response.dlight);
            print_detail_line("ticker", response.ticker);
            print_detail_line("flags", format_global_flags(response.flags));
            print_detail_line("dirty", response.dirty);
            print_detail_line("players_created", response.players_created);
            print_detail_line("npcs_created", response.npcs_created);
            print_detail_line("players_died", response.players_died);
            print_detail_line("npcs_died", response.npcs_died);
            print_detail_line("character_cnt", response.character_cnt);
            print_detail_line("item_cnt", response.item_cnt);
            print_detail_line("effect_cnt", response.effect_cnt);
            print_detail_line("players_online", response.players_online);
            print_detail_line("queuesize", response.queuesize);
            print_detail_line("max_online", response.max_online);
            print_detail_line("total_online_time", response.total_online_time);
            print_detail_debug("online_per_hour", response.online_per_hour);
            print_detail_line("uptime", response.uptime);
            print_detail_debug("uptime_per_hour", response.uptime_per_hour);
            print_detail_debug("max_online_per_hour", response.max_online_per_hour);
            print_detail_line("recv", response.recv);
            print_detail_line("send", response.send);
            print_detail_line("load_avg", response.load_avg);
            print_detail_line("load", response.load);
            print_detail_line(
                "expire",
                format!("cnt={} run={}", response.expire_cnt, response.expire_run),
            );
            print_detail_line(
                "gc",
                format!("cnt={} run={}", response.gc_cnt, response.gc_run),
            );
            print_detail_line(
                "lost",
                format!("cnt={} run={}", response.lost_cnt, response.lost_run),
            );
            print_detail_line(
                "reset",
                format!("char={} item={}", response.reset_char, response.reset_item),
            );
            print_detail_line("awake", response.awake);
            print_detail_line("body", response.body);
            print_detail_line("transfer_reset_time", response.transfer_reset_time);
            print_detail_line("fullmoon", response.fullmoon);
            print_detail_line("newmoon", response.newmoon);
            print_detail_line("unique", response.unique);
            print_detail_line("cap", response.cap);
        }
    }
    Ok(())
}

fn item_template_value(id: usize, item: &Item) -> Value {
    serde_json::json!({
        "id": id,
        "used": item.used,
        "name": c_string_to_str(&item.name),
        "reference": c_string_to_str(&item.reference),
        "description": c_string_to_str(&item.description),
        "flags": { "bits": item.flags, "names": active_item_flags(item.flags) },
        "value": item.value,
        "placement": { "bits": item.placement, "label": placement_label(item.placement) },
        "temp": item.temp,
        "damage_state": item.damage_state,
        "max_age": item.max_age,
        "current_age": item.current_age,
        "max_damage": item.max_damage,
        "current_damage": item.current_damage,
        "attrib": item.attrib,
        "hp": item.hp,
        "end": item.end,
        "mana": item.mana,
        "skill": item.skill.to_vec(),
        "armor": item.armor,
        "weapon": item.weapon,
        "light": item.light,
        "duration": item.duration,
        "cost": item.cost,
        "power": item.power,
        "active": item.active,
        "x": item.x,
        "y": item.y,
        "carried": item.carried,
        "sprite_override": item.sprite_override,
        "sprite": item.sprite,
        "status": item.status,
        "gethit_dam": item.gethit_dam,
        "min_rank": { "value": item.min_rank, "label": rank_label(item.min_rank) },
        "future": item.future,
        "future3": item.future3,
        "t_bought": item.t_bought,
        "t_sold": item.t_sold,
        "driver": item.driver,
        "data": item.data,
        "nonzero_skills": nonzero_item_skills(item),
    })
}

fn character_template_value(id: usize, character: &Character) -> Value {
    serde_json::json!({
        "id": id,
        "used": character.used,
        "name": c_string_to_str(&character.name),
        "reference": c_string_to_str(&character.reference),
        "description": c_string_to_str(&character.description),
        "kindred": { "bits": character.kindred, "names": active_kindred_flags(character.kindred) },
        "player": character.player,
        "pass1": character.pass1,
        "pass2": character.pass2,
        "sprite": character.sprite,
        "sound": character.sound,
        "flags": { "bits": character.flags, "names": active_character_flags(character.flags) },
        "alignment": character.alignment,
        "temple": { "x": character.temple_x, "y": character.temple_y },
        "tavern": { "x": character.tavern_x, "y": character.tavern_y },
        "temp": character.temp,
        "attrib": character.attrib,
        "hp": character.hp,
        "end": character.end,
        "mana": character.mana,
        "skill": character.skill.to_vec(),
        "weapon_bonus": character.weapon_bonus,
        "armor_bonus": character.armor_bonus,
        "a_hp": character.a_hp,
        "a_end": character.a_end,
        "a_mana": character.a_mana,
        "light": character.light,
        "mode": character.mode,
        "speed": character.speed,
        "points": character.points,
        "points_tot": character.points_tot,
        "rank": ranks::rank_name((character.points_tot as i64).max(0) as u32),
        "armor": character.armor,
        "weapon": character.weapon,
        "position": { "x": character.x, "y": character.y },
        "target": { "x": character.tox, "y": character.toy },
        "from": { "x": character.frx, "y": character.fry },
        "status": character.status,
        "status2": character.status2,
        "dir": character.dir,
        "gold": character.gold,
        "item": character.item.to_vec(),
        "worn": character.worn,
        "spell": character.spell,
        "citem": character.citem,
        "creation_date": character.creation_date,
        "login_date": character.login_date,
        "addr": character.addr,
        "current_online_time": character.current_online_time,
        "total_online_time": character.total_online_time,
        "comp_volume": character.comp_volume,
        "raw_volume": character.raw_volume,
        "idle": character.idle,
        "attack_cn": character.attack_cn,
        "skill_nr": character.skill_nr,
        "skill_target1": character.skill_target1,
        "skill_target2": character.skill_target2,
        "goto": { "x": character.goto_x, "y": character.goto_y },
        "use_nr": character.use_nr,
        "misc_action": character.misc_action,
        "misc_target1": character.misc_target1,
        "misc_target2": character.misc_target2,
        "cerrno": character.cerrno,
        "escape_timer": character.escape_timer,
        "enemy": character.enemy,
        "current_enemy": character.current_enemy,
        "retry": character.retry,
        "stunned": character.stunned,
        "speed_mod": character.speed_mod,
        "last_action": character.last_action,
        "unused": character.unused,
        "depot_sold": character.depot_sold,
        "gethit_dam": character.gethit_dam,
        "gethit_bonus": character.gethit_bonus,
        "light_bonus": character.light_bonus,
        "passwd": character.passwd,
        "lastattack": character.lastattack,
        "future1": character.future1,
        "sprite_override": character.sprite_override,
        "future2": character.future2.to_vec(),
        "depot": character.depot.to_vec(),
        "depot_cost": character.depot_cost,
        "luck": character.luck,
        "unreach": character.unreach,
        "unreachx": character.unreachx,
        "unreachy": character.unreachy,
        "monster_class": character.monster_class,
        "future3": character.future3,
        "logout_date": character.logout_date,
        "data": character.data.to_vec(),
        "text": character_text(character),
        "nonzero_skills": nonzero_character_skills(character),
    })
}

fn print_detail_line(label: &str, value: impl std::fmt::Display) {
    println!("{label}: {value}");
}

fn print_detail_debug(label: &str, value: impl std::fmt::Debug) {
    println!("{label}: {value:?}");
}

fn format_flags(bits: u64, names: &[&'static str]) -> String {
    if names.is_empty() {
        return format!("0x{bits:016X} []");
    }
    format!("0x{bits:016X} [{}]", names.join(", "))
}

fn format_global_flags(bits: i32) -> String {
    let labels = [
        (constants::GF_LOOTING, "Looting"),
        (constants::GF_MAYHEM, "Mayhem"),
        (constants::GF_CLOSEENEMY, "CloseEnemy"),
        (constants::GF_CAP, "Cap"),
        (constants::GF_SPEEDY, "Speedy"),
        (constants::GF_DIRTY, "Dirty"),
    ];
    let names: Vec<&'static str> = labels
        .iter()
        .filter_map(|(flag, label)| if bits & flag != 0 { Some(*label) } else { None })
        .collect();
    format_flags(bits as u64, &names)
}

fn format_gold_silver(value: u32) -> String {
    format_gold_silver_i32(value.min(i32::MAX as u32) as i32)
}

fn format_gold_silver_i32(value: i32) -> String {
    let gold = value / 1000;
    let silver = value % 1000;
    if gold > 0 && silver > 0 {
        format!("{value} ({gold} gold, {silver} silver)")
    } else if gold > 0 {
        format!("{value} ({gold} gold)")
    } else {
        format!("{value} ({silver} silver)")
    }
}

fn rank_label(min_rank: i8) -> String {
    if min_rank < 0 {
        return "-1: None".to_string();
    }
    let rank_index = min_rank as usize;
    format!("{}: {}", rank_index, ranks::rank_name_by_index(rank_index))
}

fn placement_label(placement: u16) -> String {
    let labels = [
        (0, "Unset"),
        (constants::PL_HEAD, "Head"),
        (constants::PL_NECK, "Neck"),
        (constants::PL_BODY, "Body"),
        (constants::PL_ARMS, "Arms"),
        (constants::PL_BELT, "Belt"),
        (constants::PL_LEGS, "Legs"),
        (constants::PL_FEET, "Feet"),
        (constants::PL_WEAPON, "Weapon"),
        (constants::PL_SHIELD, "Shield"),
        (constants::PL_CLOAK, "Cloak"),
        (constants::PL_TWOHAND, "Two-Hand"),
        (0x0900, "Two-Handed"),
        (constants::PL_RING, "Ring"),
    ];
    labels
        .iter()
        .find_map(|(value, label)| (*value == placement).then(|| (*label).to_string()))
        .unwrap_or_else(|| format!("Unknown (0x{placement:04X})"))
}

fn active_item_flags(bits: u64) -> Vec<&'static str> {
    let flags = ItemFlags::from_bits_truncate(bits);
    item_flag_info()
        .iter()
        .filter_map(|(flag, label)| flags.contains(*flag).then_some(*label))
        .collect()
}

fn item_flag_info() -> &'static [(ItemFlags, &'static str)] {
    &[
        (ItemFlags::IF_MOVEBLOCK, "MoveBlock"),
        (ItemFlags::IF_SIGHTBLOCK, "SightBlock"),
        (ItemFlags::IF_TAKE, "Take"),
        (ItemFlags::IF_MONEY, "Money"),
        (ItemFlags::IF_LOOK, "Look"),
        (ItemFlags::IF_LOOKSPECIAL, "LookSpecial"),
        (ItemFlags::IF_SPELL, "Spell"),
        (ItemFlags::IF_NOREPAIR, "NoRepair"),
        (ItemFlags::IF_ARMOR, "Armor"),
        (ItemFlags::IF_USE, "Use"),
        (ItemFlags::IF_USESPECIAL, "UseSpecial"),
        (ItemFlags::IF_SINGLEAGE, "SingleAge"),
        (ItemFlags::IF_SHOPDESTROY, "ShopDestroy"),
        (ItemFlags::IF_UPDATE, "Update"),
        (ItemFlags::IF_ALWAYSEXP1, "AlwaysExp1"),
        (ItemFlags::IF_ALWAYSEXP2, "AlwaysExp2"),
        (ItemFlags::IF_WP_SWORD, "WeaponSword"),
        (ItemFlags::IF_WP_DAGGER, "WeaponDagger"),
        (ItemFlags::IF_WP_AXE, "WeaponAxe"),
        (ItemFlags::IF_WP_STAFF, "WeaponStaff"),
        (ItemFlags::IF_WP_TWOHAND, "WeaponTwoHand"),
        (ItemFlags::IF_USEDESTROY, "UseDestroy"),
        (ItemFlags::IF_USEACTIVATE, "UseActivate"),
        (ItemFlags::IF_USEDEACTIVATE, "UseDeactivate"),
        (ItemFlags::IF_MAGIC, "Magic"),
        (ItemFlags::IF_MISC, "Misc"),
        (ItemFlags::IF_REACTIVATE, "Reactivate"),
        (ItemFlags::IF_PERMSPELL, "PermSpell"),
        (ItemFlags::IF_UNIQUE, "Unique"),
        (ItemFlags::IF_DONATE, "Donate"),
        (ItemFlags::IF_LABYDESTROY, "LabyDestroy"),
        (ItemFlags::IF_NOMARKET, "NoMarket"),
        (ItemFlags::IF_HIDDEN, "Hidden"),
        (ItemFlags::IF_STEPACTION, "StepAction"),
        (ItemFlags::IF_NODEPOT, "NoDepot"),
        (ItemFlags::IF_LIGHTAGE, "LightAge"),
        (ItemFlags::IF_EXPIREPROC, "ExpireProc"),
        (ItemFlags::IF_IDENTIFIED, "Identified"),
        (ItemFlags::IF_NOEXPIRE, "NoExpire"),
        (ItemFlags::IF_SOULSTONE, "Soulstone"),
    ]
}

fn active_character_flags(bits: u64) -> Vec<&'static str> {
    let flags = CharacterFlags::from_bits_truncate(bits);
    character_flag_info()
        .iter()
        .filter_map(|flag| {
            flags
                .contains(*flag)
                .then_some(constants::character_flags_name(*flag))
        })
        .collect()
}

fn character_flag_info() -> &'static [CharacterFlags] {
    &[
        CharacterFlags::Immortal,
        CharacterFlags::God,
        CharacterFlags::Creator,
        CharacterFlags::BuildMode,
        CharacterFlags::Respawn,
        CharacterFlags::Player,
        CharacterFlags::NewUser,
        CharacterFlags::NoTell,
        CharacterFlags::NoShout,
        CharacterFlags::Merchant,
        CharacterFlags::Staff,
        CharacterFlags::NoHpReg,
        CharacterFlags::NoEndReg,
        CharacterFlags::NoManaReg,
        CharacterFlags::Invisible,
        CharacterFlags::Infrared,
        CharacterFlags::Body,
        CharacterFlags::NoSleep,
        CharacterFlags::Undead,
        CharacterFlags::NoMagic,
        CharacterFlags::Stoned,
        CharacterFlags::Usurp,
        CharacterFlags::Imp,
        CharacterFlags::ShutUp,
        CharacterFlags::NoDesc,
        CharacterFlags::Profile,
        CharacterFlags::Simple,
        CharacterFlags::Kicked,
        CharacterFlags::NoList,
        CharacterFlags::NoWho,
        CharacterFlags::SpellIgnore,
        CharacterFlags::ComputerControlledPlayer,
        CharacterFlags::Safe,
        CharacterFlags::NoStaff,
        CharacterFlags::Poh,
        CharacterFlags::PohLeader,
        CharacterFlags::Thrall,
        CharacterFlags::LabKeeper,
        CharacterFlags::IsLooting,
        CharacterFlags::Golden,
        CharacterFlags::Black,
        CharacterFlags::Passwd,
        CharacterFlags::Update,
        CharacterFlags::SaveMe,
        CharacterFlags::GreaterGod,
        CharacterFlags::GreaterInv,
    ]
}

fn active_kindred_flags(kindred: i32) -> Vec<&'static str> {
    let bits = kindred as u32;
    kindred_flag_info()
        .iter()
        .filter_map(|(flag, label)| if bits & flag != 0 { Some(*label) } else { None })
        .collect()
}

fn kindred_flag_info() -> &'static [(u32, &'static str)] {
    &[
        (traits::KIN_MERCENARY, "Mercenary"),
        (traits::KIN_SEYAN_DU, "SeyanDu"),
        (traits::KIN_PURPLE, "Purple"),
        (traits::KIN_MONSTER, "Monster"),
        (traits::KIN_TEMPLAR, "Templar"),
        (traits::KIN_ARCHTEMPLAR, "ArchTemplar"),
        (traits::KIN_HARAKIM, "Harakim"),
        (traits::KIN_MALE, "Male"),
        (traits::KIN_FEMALE, "Female"),
        (traits::KIN_ARCHHARAKIM, "ArchHarakim"),
        (traits::KIN_WARRIOR, "Warrior"),
        (traits::KIN_SORCERER, "Sorcerer"),
    ]
}

fn nonzero_item_skills(item: &Item) -> Vec<Value> {
    item.skill
        .iter()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|value| *value != 0))
        .map(|(index, row)| {
            serde_json::json!({
                "index": index,
                "name": skills::get_skill_name(index),
                "values": row,
            })
        })
        .collect()
}

fn nonzero_character_skills(character: &Character) -> Vec<Value> {
    character
        .skill
        .iter()
        .enumerate()
        .filter(|(_, row)| row.iter().any(|value| *value != 0))
        .map(|(index, row)| {
            serde_json::json!({
                "index": index,
                "name": skills::get_skill_name(index),
                "values": row,
            })
        })
        .collect()
}

fn print_nonzero_item_skills(item: &Item) {
    let rows = nonzero_item_skills(item);
    if !rows.is_empty() {
        print_detail_debug("nonzero_skills", rows);
    }
}

fn print_nonzero_character_skills(character: &Character) {
    let rows = nonzero_character_skills(character);
    if !rows.is_empty() {
        print_detail_debug("nonzero_skills", rows);
    }
}

fn character_text(character: &Character) -> Vec<String> {
    character
        .text
        .iter()
        .map(|line| c_string_to_str(line).to_string())
        .collect()
}

fn json_string<T: Serialize + ?Sized>(value: &T) -> Result<String, CliError> {
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

    #[test]
    fn command_without_auto_is_rejected_before_token_check() {
        let cli = Cli {
            api: DEFAULT_API_URL.to_string(),
            admin_token: None,
            format: OutputFormat::Table,
            quiet: false,
            auto: false,
            menu: false,
            command: Some(Commands::Globals {
                command: GlobalsCommand::Show,
            }),
        };

        let error = run(cli).unwrap_err();

        match error {
            CliError::Runtime(message) => assert!(message.contains("pass --auto")),
            CliError::NotFound(message) => panic!("unexpected not found: {message}"),
        }
    }

    #[test]
    fn fuzzy_ranking_prefers_exact_then_prefix_then_subsequence() {
        let summaries = vec![
            TemplateSummary {
                id: 30,
                used: true,
                name: "Long Silver Sword".to_string(),
                reference: "".to_string(),
            },
            TemplateSummary {
                id: 10,
                used: true,
                name: "Sword".to_string(),
                reference: "blade".to_string(),
            },
            TemplateSummary {
                id: 20,
                used: true,
                name: "Sword of Dawn".to_string(),
                reference: "".to_string(),
            },
            TemplateSummary {
                id: 40,
                used: true,
                name: "Sturdy Ward".to_string(),
                reference: "".to_string(),
            },
        ];

        let matches = rank_template_summaries(&summaries, "sword", false, 10);

        assert_eq!(
            matches.iter().map(|entry| entry.id).collect::<Vec<_>>(),
            vec![10, 20, 30]
        );
    }

    #[test]
    fn fuzzy_ranking_skips_unused_unless_requested() {
        let summaries = vec![
            TemplateSummary {
                id: 1,
                used: false,
                name: "Unused Sword".to_string(),
                reference: "".to_string(),
            },
            TemplateSummary {
                id: 2,
                used: true,
                name: "Used Sword".to_string(),
                reference: "".to_string(),
            },
        ];

        let default_matches = rank_template_summaries(&summaries, "sword", false, 10);
        let all_matches = rank_template_summaries(&summaries, "sword", true, 10);

        assert_eq!(default_matches.len(), 1);
        assert_eq!(default_matches[0].id, 2);
        assert_eq!(all_matches.len(), 2);
    }
}
