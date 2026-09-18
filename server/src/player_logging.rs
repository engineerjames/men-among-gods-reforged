use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use chrono::Utc;

const MAX_LOG_BYTES: u64 = 100 * 1024 * 1024;
const MAX_ROTATED_LOGS: usize = 5;
const MAX_NAME_BYTES: usize = 96;
const MAX_MESSAGE_BYTES: usize = 4096;

static PLAYER_LOGGER: OnceLock<Mutex<PlayerLogManager>> = OnceLock::new();

/// Categorizes an event written to a player audit log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLogCategory {
    /// Login and authentication lifecycle event.
    Login,
    /// Logout and disconnect lifecycle event.
    Logout,
    /// Player speech or other communication event.
    Chat,
    /// Player command dispatch event.
    Command,
    /// Skill or spell event.
    Spell,
    /// Gameplay event emitted by the shared gameplay log funnel.
    Gameplay,
}

impl Display for PlayerLogCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Login => "LOGIN",
            Self::Logout => "LOGOUT",
            Self::Chat => "CHAT",
            Self::Command => "COMMAND",
            Self::Spell => "SPELL",
            Self::Gameplay => "GAMEPLAY",
        };
        formatter.write_str(value)
    }
}

/// Describes the result of a player audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerLogOutcome {
    /// The player action was received but has not completed.
    Attempt,
    /// The player action completed successfully.
    Success,
    /// The player action was refused or could not complete.
    Rejected,
}

impl Display for PlayerLogOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Attempt => "ATTEMPT",
            Self::Success => "SUCCESS",
            Self::Rejected => "REJECTED",
        };
        formatter.write_str(value)
    }
}

struct PlayerLogManager {
    directory: PathBuf,
    active_logs: HashMap<usize, ActivePlayerLog>,
}

struct ActivePlayerLog {
    api_character_id: u64,
    character_slot: usize,
    name: String,
    server_slot: usize,
    path: PathBuf,
    file: File,
}

/// Initializes the server's player-log directory.
///
/// The logger is process-global because gameplay event helpers do not all have
/// mutable access to `GameState`. Calling this more than once is harmless.
///
/// # Arguments
///
/// * `directory` - Directory containing active and rotated player logs.
///
/// # Returns
///
/// * An I/O error if the directory cannot be created or the global logger
///   cannot be installed.
pub fn initialize(directory: impl AsRef<Path>) -> io::Result<()> {
    let directory = directory.as_ref().to_path_buf();
    fs::create_dir_all(&directory)?;
    PLAYER_LOGGER
        .set(Mutex::new(PlayerLogManager {
            directory,
            active_logs: HashMap::new(),
        }))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                "player logger already initialized",
            )
        })
        .or_else(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(error)
            }
        })
}

/// Associates a runtime character with its stable API identity.
///
/// # Arguments
///
/// * `character_slot` - Runtime character slot used by gameplay code.
/// * `server_slot` - Runtime server connection slot included in event records.
/// * `api_character_id` - Stable API character identifier used in filenames.
/// * `name` - Current character name used in the filename and event records.
pub fn bind_player(character_slot: usize, server_slot: usize, api_character_id: u64, name: &str) {
    if api_character_id == 0 {
        return;
    }

    with_manager(|manager| {
        if let Err(error) = manager.bind_player(character_slot, server_slot, api_character_id, name)
        {
            eprintln!("Warning: could not bind player log: {}", error);
        }
    });
}

/// Removes a runtime character's active player-log binding.
///
/// # Arguments
///
/// * `character_slot` - Runtime character slot to unbind.
pub fn unbind_player(character_slot: usize) {
    with_manager(|manager| {
        manager.active_logs.remove(&character_slot);
    });
}

/// Appends a categorized event to a bound player's log.
///
/// Events for NPCs, disconnected characters, or uninitialized test state are
/// intentionally ignored; those records remain available through the main log.
///
/// # Arguments
///
/// * `character_slot` - Runtime character slot associated with the event.
/// * `category` - Typed audit category such as [`PlayerLogCategory::Chat`] or
///   [`PlayerLogCategory::Spell`].
/// * `outcome` - Typed outcome such as [`PlayerLogOutcome::Attempt`],
///   [`PlayerLogOutcome::Success`], or [`PlayerLogOutcome::Rejected`].
/// * `message` - Human-readable event detail.
#[track_caller]
pub fn log_event(
    character_slot: usize,
    category: PlayerLogCategory,
    outcome: PlayerLogOutcome,
    message: &str,
) {
    let location = std::panic::Location::caller();
    with_manager(|manager| {
        if let Some(active_log) = manager.active_logs.get_mut(&character_slot)
            && let Err(error) = active_log.write_event(category, outcome, message, location)
        {
            eprintln!("Warning: could not write player log: {}", error);
        }
    });
}

/// Records an event when the API identity is known but no runtime character
/// slot is available, such as a rejected version or character validation.
///
/// # Arguments
///
/// * `server_slot` - Runtime server connection slot.
/// * `api_character_id` - Stable API character identifier.
/// * `name` - Character name supplied by the API.
/// * `category` - Stable audit category.
/// * `outcome` - Event outcome.
/// * `message` - Human-readable event detail.
#[track_caller]
pub fn log_identity_event(
    server_slot: usize,
    api_character_id: u64,
    name: &str,
    category: PlayerLogCategory,
    outcome: PlayerLogOutcome,
    message: &str,
) {
    if api_character_id == 0 {
        return;
    }

    let location = std::panic::Location::caller();
    with_manager(|manager| {
        if let Err(error) = manager.write_identity_event(
            server_slot,
            api_character_id,
            name,
            category,
            outcome,
            message,
            location,
        ) {
            eprintln!("Warning: could not write player identity log: {}", error);
        }
    });
}

fn with_manager(callback: impl FnOnce(&mut PlayerLogManager)) {
    let Some(logger) = PLAYER_LOGGER.get() else {
        return;
    };

    let Ok(mut manager) = logger.lock() else {
        eprintln!("Warning: player logger lock is poisoned");
        return;
    };
    callback(&mut manager);
}

impl PlayerLogManager {
    fn bind_player(
        &mut self,
        character_slot: usize,
        server_slot: usize,
        api_character_id: u64,
        name: &str,
    ) -> io::Result<()> {
        self.active_logs.remove(&character_slot);

        let sanitized_name = sanitize_name(name);
        let path = self.log_path(api_character_id, &sanitized_name);
        reconcile_name_history(&self.directory, api_character_id, &path)?;
        let file = open_append(&path)?;

        self.active_logs.insert(
            character_slot,
            ActivePlayerLog {
                api_character_id,
                character_slot,
                name: sanitized_name,
                server_slot,
                path,
                file,
            },
        );
        Ok(())
    }

    fn write_identity_event(
        &self,
        server_slot: usize,
        api_character_id: u64,
        name: &str,
        category: PlayerLogCategory,
        outcome: PlayerLogOutcome,
        message: &str,
        location: &'static std::panic::Location<'static>,
    ) -> io::Result<()> {
        let sanitized_name = sanitize_name(name);
        let path = self.log_path(api_character_id, &sanitized_name);
        reconcile_name_history(&self.directory, api_character_id, &path)?;
        let mut file = open_append(&path)?;
        write_event(
            &mut file,
            &path,
            api_character_id,
            None,
            &sanitized_name,
            server_slot,
            category,
            outcome,
            message,
            location,
        )
    }

    fn log_path(&self, api_character_id: u64, name: &str) -> PathBuf {
        self.directory
            .join(format!("{}_{}.log", api_character_id, name))
    }
}

impl ActivePlayerLog {
    fn write_event(
        &mut self,
        category: PlayerLogCategory,
        outcome: PlayerLogOutcome,
        message: &str,
        location: &'static std::panic::Location<'static>,
    ) -> io::Result<()> {
        write_event(
            &mut self.file,
            &self.path,
            self.api_character_id,
            Some(self.character_slot),
            &self.name,
            self.server_slot,
            category,
            outcome,
            message,
            location,
        )
    }
}

fn write_event(
    file: &mut File,
    path: &Path,
    api_character_id: u64,
    character_slot: Option<usize>,
    name: &str,
    server_slot: usize,
    category: PlayerLogCategory,
    outcome: PlayerLogOutcome,
    message: &str,
    location: &'static std::panic::Location<'static>,
) -> io::Result<()> {
    let message = sanitize_message(message);
    let character_slot =
        character_slot.map_or_else(|| "unknown".to_owned(), |slot| slot.to_string());
    let source = source_location(location);
    let line = format!(
        "{} {} [{}][{}] - {} (api_character_id={} character_slot={} server_slot={} name=\"{}\")\n",
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.f"),
        source,
        category,
        outcome,
        escape_field(&message),
        api_character_id,
        character_slot,
        server_slot,
        escape_field(name),
    );

    if file.metadata()?.len().saturating_add(line.len() as u64) > MAX_LOG_BYTES {
        file.flush()?;
        rotate_file(path)?;
        *file = open_append(path)?;
    }

    file.write_all(line.as_bytes())?;
    file.flush()
}

fn source_location(location: &'static std::panic::Location<'static>) -> String {
    let file = location.file();
    let file = file
        .rsplit_once("/server/")
        .map_or_else(|| file.to_owned(), |(_, suffix)| format!("server/{suffix}"));
    format!("{}:{}", file, location.line())
}

fn open_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

fn rotate_file(path: &Path) -> io::Result<()> {
    for index in (0..MAX_ROTATED_LOGS.saturating_sub(1)).rev() {
        let source = rotated_path(path, index);
        let destination = rotated_path(path, index + 1);
        if source.exists() {
            replace_file(&source, &destination)?;
        }
    }

    if path.exists() {
        replace_file(path, &rotated_path(path, 0))?;
    }
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(source, destination)
}

fn rotated_path(path: &Path, index: usize) -> PathBuf {
    PathBuf::from(format!("{}.{}", path.display(), index))
}

fn reconcile_name_history(
    directory: &Path,
    api_character_id: u64,
    current_path: &Path,
) -> io::Result<()> {
    if current_path.exists() {
        return Ok(());
    }

    let prefix = format!("{}_", api_character_id);
    let mut candidates = fs::read_dir(directory)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|value| value.to_str())
                .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".log"))
        })
        .collect::<Vec<_>>();
    candidates.sort();

    if let Some(previous_path) = candidates.into_iter().next() {
        replace_file(&previous_path, current_path)?;
        for index in 0..MAX_ROTATED_LOGS {
            let previous_archive = rotated_path(&previous_path, index);
            if previous_archive.exists() {
                replace_file(&previous_archive, &rotated_path(current_path, index))?;
            }
        }
    }
    Ok(())
}

fn sanitize_name(name: &str) -> String {
    let mut sanitized = String::new();
    for character in name.chars() {
        if character.is_alphanumeric() || matches!(character, '-' | '_' | '.') {
            sanitized.push(character);
        } else {
            sanitized.push('_');
        }
    }

    let mut bytes = sanitized.len();
    while bytes > MAX_NAME_BYTES {
        sanitized.pop();
        bytes = sanitized.len();
    }

    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        "unknown".to_owned()
    } else {
        sanitized
    }
}

fn sanitize_message(message: &str) -> String {
    message.chars().take(MAX_MESSAGE_BYTES).collect()
}

fn escape_field(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            '\\' => "\\\\".to_owned(),
            '"' => "\\\"".to_owned(),
            '\n' => "\\n".to_owned(),
            '\r' => "\\r".to_owned(),
            character if character.is_control() => " ".to_owned(),
            character => character.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PlayerLogCategory, PlayerLogManager, PlayerLogOutcome, sanitize_name};

    #[test]
    fn sanitize_name_removes_path_and_control_characters() {
        assert_eq!(sanitize_name("../mage/name\n"), ".._mage_name_");
    }

    #[test]
    fn sanitize_name_truncates_without_panicking_on_unicode() {
        let name = "é".repeat(100);
        let sanitized = sanitize_name(&name);
        assert!(sanitized.len() <= super::MAX_NAME_BYTES);
        assert!(sanitized.is_char_boundary(sanitized.len()));
    }

    #[test]
    fn sanitize_name_uses_fallback_for_empty_names() {
        assert_eq!(sanitize_name(""), "unknown");
        assert_eq!(sanitize_name(".."), "unknown");
    }

    #[test]
    fn event_preserves_distinct_identity_fields_and_enum_values() {
        let unique_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after the Unix epoch")
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "mag-player-log-test-{}-{unique_id}",
            std::process::id()
        ));
        std::fs::create_dir(&directory).expect("test log directory should be creatable");
        let mut manager = PlayerLogManager {
            directory: directory.clone(),
            active_logs: HashMap::new(),
        };
        manager
            .bind_player(12, 1, 77, "Ashrune")
            .expect("player log should be bindable");

        manager
            .active_logs
            .get_mut(&12)
            .expect("bound player log should exist")
            .write_event(
                PlayerLogCategory::Login,
                PlayerLogOutcome::Success,
                "login completed",
                std::panic::Location::caller(),
            )
            .expect("test event should be writable");
        drop(manager);

        let path = directory.join("77_Ashrune.log");
        let line = std::fs::read_to_string(&path).expect("test log should be readable");
        assert!(line.contains("[LOGIN][SUCCESS] - login completed"));
        assert!(
            line.contains("(api_character_id=77 character_slot=12 server_slot=1 name=\"Ashrune\")")
        );
        assert!(line.contains("server/src/player_logging.rs:"));
        assert!(line.contains("T") && !line.contains("-06:00"));

        std::fs::remove_dir_all(directory).expect("test log directory should be removable");
    }
}
