//! Scripted UI automation for the client.
//!
//! When `MAG_AUTOMATION_SCRIPT` points at a script file, [`AutomationDriver`]
//! replays it against the running client by synthesising SDL input events
//! and feeding them straight into the [`SceneManager`], exactly where real
//! input arrives after HiDPI adjustment. Everything downstream (widget hit
//! testing, focus, text input, form actions, network) runs unchanged, so a
//! scripted run exercises the same code paths as a human.
//!
//! # Script format
//!
//! One command per line, `#` starts a comment, arguments are whitespace
//! separated and may be double-quoted. `${NAME}` is replaced with the value
//! of the environment variable `NAME` (unset variables are a load error).
//! Coordinates are in the logical 960x540 space.
//!
//! | Command | Effect |
//! |---------|--------|
//! | `wait <secs>` | Pause the script for `secs` (fractional allowed). |
//! | `wait_scene <scene> [timeout_secs]` | Block until `scene` is active and any fade has finished (default timeout 60 s). |
//! | `click <x> <y> \| @<target> [left\|right\|middle]` | Move the mouse there, then press and release. |
//! | `move <x> <y> \| @<target>` | Move the mouse. |
//! | `wheel <delta> [<x> <y> \| @<target>]` | Scroll (positive = up) at the target or the last mouse position. |
//! | `type <text...>` | Deliver text input one character at a time. |
//! | `key <name> [ctrl] [shift] [alt]` | Press and release a key by SDL name (`Return`, `Escape`, `a`, `F1`, ...). |
//! | `profile [secs]` | Start the game scene's render profiler. |
//! | `action <name> [args...]` | Invoke a scene-specific [`Scene::automation_action`]. |
//! | `screenshot <path>` | Save the next presented frame as PNG. |
//! | `log <text...>` | Write a line to the client log. |
//! | `quit` | Exit the client. |
//!
//! Targets are `@name` or `@<scene>.name`; each scene lists its own names via
//! [`Scene::automation_targets`]. Scene names are those returned by
//! [`SceneType::automation_name`]: `login`, `new_account`, `char_select`,
//! `char_create`, `game`, ...
//!
//! One command executes per frame while no fade transition is running. A
//! failing command (unknown target, `wait_scene` timeout, ...) aborts the
//! script with an error that names the script line; when the script ends
//! without `quit` the client simply stays interactive.
//!
//! [`Scene::automation_action`]: crate::scenes::scene::Scene::automation_action
//! [`Scene::automation_targets`]: crate::scenes::scene::Scene::automation_targets

use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use sdl2::{
    event::Event,
    keyboard::{Keycode, Mod},
    mouse::{MouseButton, MouseState, MouseWheelDirection},
};

use crate::{
    scenes::scene::{SceneManager, SceneType},
    state::AppState,
    ui::widget::Bounds,
};

/// Environment variable holding the path of the script to run.
pub const SCRIPT_ENV_VAR: &str = "MAG_AUTOMATION_SCRIPT";

/// Default `wait_scene` timeout.
const DEFAULT_SCENE_TIMEOUT: Duration = Duration::from_secs(60);

/// Where a mouse command should land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// Absolute logical coordinates.
    Point { x: i32, y: i32 },
    /// A named widget, optionally qualified with the scene name.
    Named { scene: Option<String>, name: String },
}

/// Modifier keys held while a `key` command is delivered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    /// Left control held.
    pub ctrl: bool,
    /// Left shift held.
    pub shift: bool,
    /// Left alt held.
    pub alt: bool,
}

/// A single parsed script instruction.
#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Pause for the given duration.
    Wait(Duration),
    /// Block until the scene is active (with timeout).
    WaitScene { scene: SceneType, timeout: Duration },
    /// Move, press and release a mouse button.
    Click { target: Target, button: MouseButton },
    /// Move the mouse.
    Move(Target),
    /// Scroll the wheel.
    Wheel { delta: i32, target: Option<Target> },
    /// Deliver text input.
    Type(String),
    /// Press and release a key.
    Key {
        keycode: Keycode,
        modifiers: Modifiers,
    },
    /// Scene-specific action with arguments.
    Action { name: String, args: Vec<String> },
    /// Capture the next presented frame to this path.
    Screenshot(PathBuf),
    /// Emit a log line.
    Log(String),
    /// Exit the client.
    Quit,
}

/// A command together with its 1-based source line for error reporting.
#[derive(Clone, Debug, PartialEq)]
pub struct Instruction {
    /// Source line number.
    pub line: usize,
    /// Parsed command.
    pub command: Command,
}

/// Result of a single [`AutomationDriver::step`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    /// More commands remain.
    Running,
    /// The script has finished (with or without `quit`).
    Finished,
}

/// Replays a parsed script one command per frame.
pub struct AutomationDriver {
    script_path: PathBuf,
    instructions: Vec<Instruction>,
    pc: usize,
    /// Deadline for the current `wait` command.
    wait_until: Option<Instant>,
    /// Deadline for the current `wait_scene` command.
    scene_deadline: Option<Instant>,
    /// Screenshot requested by the last executed command.
    pending_screenshot: Option<PathBuf>,
    mouse_x: i32,
    mouse_y: i32,
    finished: bool,
}

impl AutomationDriver {
    /// Builds a driver from `MAG_AUTOMATION_SCRIPT`, if that variable is set.
    ///
    /// # Returns
    ///
    /// * `Ok(None)` when no script is configured.
    /// * `Ok(Some(driver))` when the script loaded and parsed.
    /// * `Err` describing the parse/load failure (with file and line).
    pub fn from_env() -> Result<Option<Self>, String> {
        match std::env::var(SCRIPT_ENV_VAR) {
            Ok(path) if !path.trim().is_empty() => Self::load(Path::new(path.trim())).map(Some),
            _ => Ok(None),
        }
    }

    /// Loads and parses a script file.
    ///
    /// # Arguments
    ///
    /// * `path` - Script file location.
    ///
    /// # Returns
    ///
    /// * The ready-to-run driver, or a load/parse error.
    pub fn load(path: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read automation script {}: {e}", path.display()))?;
        let instructions = parse_script(&source, |name| std::env::var(name).ok())
            .map_err(|e| format!("{}:{e}", path.display()))?;
        log::info!(
            "Automation script {} loaded ({} commands)",
            path.display(),
            instructions.len()
        );
        Ok(Self::from_instructions(path.to_path_buf(), instructions))
    }

    /// Builds a driver from already-parsed instructions.
    ///
    /// # Arguments
    ///
    /// * `script_path` - Used only in log and error messages.
    /// * `instructions` - Commands to execute in order.
    ///
    /// # Returns
    ///
    /// * A driver positioned at the first instruction.
    pub fn from_instructions(script_path: PathBuf, instructions: Vec<Instruction>) -> Self {
        Self {
            script_path,
            instructions,
            pc: 0,
            wait_until: None,
            scene_deadline: None,
            pending_screenshot: None,
            mouse_x: 0,
            mouse_y: 0,
            finished: false,
        }
    }

    /// Whether the script has run to completion.
    ///
    /// # Returns
    ///
    /// * `true` once every instruction has executed or the script aborted.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Returns the screenshot path requested this frame, if any.
    ///
    /// # Returns
    ///
    /// * The destination path; `None` when no capture is pending.
    pub fn take_pending_screenshot(&mut self) -> Option<PathBuf> {
        self.pending_screenshot.take()
    }

    /// Advances the script by at most one command.
    ///
    /// Call once per frame after real SDL events have been dispatched.
    ///
    /// # Arguments
    ///
    /// * `scenes` - Scene manager receiving synthesised events.
    /// * `app_state` - Shared application state.
    ///
    /// # Returns
    ///
    /// * `Ok(StepOutcome)` on success; `Err` (with script path and line) when
    ///   a command fails, after which the driver is finished.
    pub fn step(
        &mut self,
        scenes: &mut SceneManager,
        app_state: &mut AppState<'_>,
    ) -> Result<StepOutcome, String> {
        if self.finished {
            return Ok(StepOutcome::Finished);
        }
        let Some(instruction) = self.instructions.get(self.pc).cloned() else {
            log::info!("Automation script {} finished", self.script_path.display());
            self.finished = true;
            return Ok(StepOutcome::Finished);
        };

        // Input is dropped during fades, so hold the script there too.
        if scenes.is_transitioning() {
            return Ok(StepOutcome::Running);
        }

        match self.execute(&instruction.command, scenes, app_state) {
            Ok(true) => {
                self.pc += 1;
                if instruction.command == Command::Quit {
                    self.finished = true;
                    return Ok(StepOutcome::Finished);
                }
                Ok(StepOutcome::Running)
            }
            Ok(false) => Ok(StepOutcome::Running),
            Err(err) => {
                self.finished = true;
                Err(format!(
                    "{}:{}: {err}",
                    self.script_path.display(),
                    instruction.line
                ))
            }
        }
    }

    /// Runs one command. Returns `Ok(true)` when it completed, `Ok(false)`
    /// when it is still waiting and must be retried next frame.
    fn execute(
        &mut self,
        command: &Command,
        scenes: &mut SceneManager,
        app_state: &mut AppState<'_>,
    ) -> Result<bool, String> {
        match command {
            Command::Wait(duration) => {
                let deadline = *self
                    .wait_until
                    .get_or_insert_with(|| Instant::now() + *duration);
                if Instant::now() < deadline {
                    return Ok(false);
                }
                self.wait_until = None;
                Ok(true)
            }
            Command::WaitScene { scene, timeout } => {
                if scenes.get_scene() == *scene {
                    self.scene_deadline = None;
                    log::info!("Automation: scene `{}` reached", scene.automation_name());
                    return Ok(true);
                }
                let deadline = *self
                    .scene_deadline
                    .get_or_insert_with(|| Instant::now() + *timeout);
                if Instant::now() >= deadline {
                    self.scene_deadline = None;
                    return Err(format!(
                        "timed out after {:.1}s waiting for scene `{}` (active: `{}`)",
                        timeout.as_secs_f64(),
                        scene.automation_name(),
                        scenes.get_scene().automation_name()
                    ));
                }
                Ok(false)
            }
            Command::Click { target, button } => {
                let (x, y) = self.resolve(target, scenes)?;
                self.dispatch(scenes, app_state, mouse_motion(x, y));
                self.dispatch(scenes, app_state, mouse_button(true, *button, x, y));
                self.dispatch(scenes, app_state, mouse_button(false, *button, x, y));
                Ok(true)
            }
            Command::Move(target) => {
                let (x, y) = self.resolve(target, scenes)?;
                self.dispatch(scenes, app_state, mouse_motion(x, y));
                Ok(true)
            }
            Command::Wheel { delta, target } => {
                let (x, y) = match target {
                    Some(target) => self.resolve(target, scenes)?,
                    None => (self.mouse_x, self.mouse_y),
                };
                self.dispatch(scenes, app_state, mouse_motion(x, y));
                self.dispatch(scenes, app_state, mouse_wheel(*delta, x, y));
                Ok(true)
            }
            Command::Type(text) => {
                for ch in text.chars() {
                    self.dispatch(scenes, app_state, text_input(ch.to_string()));
                }
                Ok(true)
            }
            Command::Key { keycode, modifiers } => {
                for event in key_press_events(*keycode, *modifiers) {
                    self.dispatch(scenes, app_state, event);
                }
                Ok(true)
            }
            Command::Action { name, args } => {
                scenes.automation_action(app_state, name, args)?;
                Ok(true)
            }
            Command::Screenshot(path) => {
                self.pending_screenshot = Some(path.clone());
                Ok(true)
            }
            Command::Log(text) => {
                log::info!("Automation: {text}");
                Ok(true)
            }
            Command::Quit => {
                log::info!("Automation: quit requested");
                scenes.request_scene_change(SceneType::Exit, app_state);
                Ok(true)
            }
        }
    }

    /// Resolves a target to logical coordinates against the active scene.
    fn resolve(&self, target: &Target, scenes: &SceneManager) -> Result<(i32, i32), String> {
        match target {
            Target::Point { x, y } => Ok((*x, *y)),
            Target::Named { scene, name } => {
                let active = scenes.get_scene();
                if let Some(scene) = scene
                    && scene != active.automation_name()
                {
                    return Err(format!(
                        "target @{scene}.{name} belongs to scene `{scene}` but `{}` is active",
                        active.automation_name()
                    ));
                }
                let targets = scenes.automation_targets();
                let bounds = find_target(&targets, name).ok_or_else(|| {
                    let mut names: Vec<&str> = targets.iter().map(|(n, _)| *n).collect();
                    names.sort_unstable();
                    format!(
                        "unknown target @{name} in scene `{}` (available: {})",
                        active.automation_name(),
                        names.join(", ")
                    )
                })?;
                Ok(bounds_center(&bounds))
            }
        }
    }

    /// Sends one synthesised event to the scene manager and tracks the mouse.
    fn dispatch(&mut self, scenes: &mut SceneManager, app_state: &mut AppState<'_>, event: Event) {
        if let Event::MouseMotion { x, y, .. } = &event {
            self.mouse_x = *x;
            self.mouse_y = *y;
        }
        scenes.handle_event(app_state, &event);
    }
}

/// Looks up a named target in a scene's target list.
///
/// # Arguments
///
/// * `targets` - `(name, bounds)` pairs from the active scene.
/// * `name` - Unqualified target name.
///
/// # Returns
///
/// * The matching bounds, if any.
pub fn find_target(targets: &[(&'static str, Bounds)], name: &str) -> Option<Bounds> {
    targets
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, bounds)| *bounds)
}

/// Center point of a rectangle.
///
/// # Arguments
///
/// * `bounds` - Rectangle in logical coordinates.
///
/// # Returns
///
/// * `(x, y)` of the center.
pub fn bounds_center(bounds: &Bounds) -> (i32, i32) {
    (
        bounds.x + bounds.width as i32 / 2,
        bounds.y + bounds.height as i32 / 2,
    )
}

// ---------------------------------------------------------------------------
// Event synthesis
// ---------------------------------------------------------------------------

fn mouse_motion(x: i32, y: i32) -> Event {
    Event::MouseMotion {
        timestamp: 0,
        window_id: 0,
        which: 0,
        mousestate: MouseState::from_sdl_state(0),
        x,
        y,
        xrel: 0,
        yrel: 0,
    }
}

fn mouse_button(down: bool, mouse_btn: MouseButton, x: i32, y: i32) -> Event {
    if down {
        Event::MouseButtonDown {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn,
            clicks: 1,
            x,
            y,
        }
    } else {
        Event::MouseButtonUp {
            timestamp: 0,
            window_id: 0,
            which: 0,
            mouse_btn,
            clicks: 1,
            x,
            y,
        }
    }
}

fn mouse_wheel(delta: i32, mouse_x: i32, mouse_y: i32) -> Event {
    Event::MouseWheel {
        timestamp: 0,
        window_id: 0,
        which: 0,
        x: 0,
        y: delta,
        direction: MouseWheelDirection::Normal,
        precise_x: 0.0,
        precise_y: delta as f32,
        mouse_x,
        mouse_y,
    }
}

fn text_input(text: String) -> Event {
    Event::TextInput {
        timestamp: 0,
        window_id: 0,
        text,
    }
}

fn key_event(down: bool, keycode: Keycode, keymod: Mod) -> Event {
    if down {
        Event::KeyDown {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod,
            repeat: false,
        }
    } else {
        Event::KeyUp {
            timestamp: 0,
            window_id: 0,
            keycode: Some(keycode),
            scancode: None,
            keymod,
            repeat: false,
        }
    }
}

/// Builds the event sequence for a key press: modifier downs, key down/up,
/// modifier ups — mirroring what SDL delivers for a real chord.
///
/// # Arguments
///
/// * `keycode` - Key to press.
/// * `modifiers` - Modifier keys held around the press.
///
/// # Returns
///
/// * Events in delivery order.
pub fn key_press_events(keycode: Keycode, modifiers: Modifiers) -> Vec<Event> {
    let mut held: Vec<(Keycode, Mod)> = Vec::new();
    if modifiers.ctrl {
        held.push((Keycode::LCtrl, Mod::LCTRLMOD));
    }
    if modifiers.shift {
        held.push((Keycode::LShift, Mod::LSHIFTMOD));
    }
    if modifiers.alt {
        held.push((Keycode::LAlt, Mod::LALTMOD));
    }

    let mut keymod = Mod::NOMOD;
    let mut events = Vec::with_capacity(held.len() * 2 + 2);
    for (kc, m) in &held {
        keymod |= *m;
        events.push(key_event(true, *kc, keymod));
    }
    events.push(key_event(true, keycode, keymod));
    events.push(key_event(false, keycode, keymod));
    for (kc, m) in held.iter().rev() {
        events.push(key_event(false, *kc, keymod));
        keymod.remove(*m);
    }
    events
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Parses a whole script.
///
/// # Arguments
///
/// * `source` - Script text.
/// * `env` - Resolver for `${NAME}` references (normally `std::env::var`).
///
/// # Returns
///
/// * Instructions in file order, or `"<line>: <message>"` for the first error.
pub fn parse_script(
    source: &str,
    env: impl Fn(&str) -> Option<String>,
) -> Result<Vec<Instruction>, String> {
    let mut instructions = Vec::new();
    for (idx, raw) in source.lines().enumerate() {
        let line = idx + 1;
        let tokens = tokenize(raw).map_err(|e| format!("{line}: {e}"))?;
        if tokens.is_empty() {
            continue;
        }
        let tokens = tokens
            .into_iter()
            .map(|t| expand_env(&t, &env))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("{line}: {e}"))?;
        let command = parse_command(&tokens).map_err(|e| format!("{line}: {e}"))?;
        instructions.push(Instruction { line, command });
    }
    Ok(instructions)
}

/// Splits one line into tokens, honouring double quotes and `#` comments.
///
/// # Arguments
///
/// * `line` - Raw script line.
///
/// # Returns
///
/// * Tokens with quotes removed; empty for blank/comment lines.
pub fn tokenize(line: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_token = false;
    let mut chars = line.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '#' if !in_token => break,
            '"' => {
                in_token = true;
                loop {
                    match chars.next() {
                        Some('\\') => match chars.next() {
                            Some(escaped) => current.push(escaped),
                            None => return Err("dangling escape in quoted string".to_owned()),
                        },
                        Some('"') => break,
                        Some(c) => current.push(c),
                        None => return Err("unterminated quoted string".to_owned()),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_token {
                    tokens.push(std::mem::take(&mut current));
                    in_token = false;
                }
            }
            c => {
                in_token = true;
                current.push(c);
            }
        }
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Replaces every `${NAME}` in `token` using `env`.
///
/// # Arguments
///
/// * `token` - Token possibly containing references.
/// * `env` - Variable resolver.
///
/// # Returns
///
/// * The expanded token, or an error naming the first unset variable.
pub fn expand_env(token: &str, env: &impl Fn(&str) -> Option<String>) -> Result<String, String> {
    let mut out = String::with_capacity(token.len());
    let mut rest = token;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(format!("unterminated variable reference in `{token}`"));
        };
        let name = &after[..end];
        let value = env(name).ok_or_else(|| format!("environment variable `{name}` is not set"))?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Parses one tokenized command line.
fn parse_command(tokens: &[String]) -> Result<Command, String> {
    let (head, args) = tokens.split_first().expect("non-empty tokens");
    match head.to_ascii_lowercase().as_str() {
        "wait" => {
            let secs = parse_secs(args.first(), "wait")?;
            Ok(Command::Wait(Duration::from_secs_f64(secs)))
        }
        "wait_scene" => {
            let name = args.first().ok_or("wait_scene: missing scene name")?;
            let scene = SceneType::from_automation_name(name)
                .ok_or_else(|| format!("wait_scene: unknown scene `{name}`"))?;
            let timeout = match args.get(1) {
                Some(_) => Duration::from_secs_f64(parse_secs(args.get(1), "wait_scene")?),
                None => DEFAULT_SCENE_TIMEOUT,
            };
            Ok(Command::WaitScene { scene, timeout })
        }
        "click" => {
            let (target, rest) = parse_target(args, "click")?;
            let button = match rest.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                None | Some("left") => MouseButton::Left,
                Some("right") => MouseButton::Right,
                Some("middle") => MouseButton::Middle,
                Some(b) => return Err(format!("click: unknown mouse button `{b}`")),
            };
            Ok(Command::Click { target, button })
        }
        "move" => {
            let (target, _) = parse_target(args, "move")?;
            Ok(Command::Move(target))
        }
        "wheel" => {
            let delta = args
                .first()
                .ok_or("wheel: missing delta")?
                .parse::<i32>()
                .map_err(|_| format!("wheel: invalid delta `{}`", args[0]))?;
            let target = if args.len() > 1 {
                Some(parse_target(&args[1..], "wheel")?.0)
            } else {
                None
            };
            Ok(Command::Wheel { delta, target })
        }
        "type" => {
            if args.is_empty() {
                return Err("type: missing text".to_owned());
            }
            Ok(Command::Type(args.join(" ")))
        }
        "key" => {
            let name = args.first().ok_or("key: missing key name")?;
            let keycode = Keycode::from_name(name)
                .ok_or_else(|| format!("key: unknown key name `{name}`"))?;
            let mut modifiers = Modifiers::default();
            for m in &args[1..] {
                match m.to_ascii_lowercase().as_str() {
                    "ctrl" => modifiers.ctrl = true,
                    "shift" => modifiers.shift = true,
                    "alt" => modifiers.alt = true,
                    other => return Err(format!("key: unknown modifier `{other}`")),
                }
            }
            Ok(Command::Key { keycode, modifiers })
        }
        "profile" => Ok(Command::Action {
            name: "profile".to_owned(),
            args: args.to_vec(),
        }),
        "action" => {
            let name = args.first().ok_or("action: missing action name")?;
            Ok(Command::Action {
                name: name.clone(),
                args: args[1..].to_vec(),
            })
        }
        "screenshot" => {
            let path = args.first().ok_or("screenshot: missing path")?;
            Ok(Command::Screenshot(PathBuf::from(path)))
        }
        "log" => Ok(Command::Log(args.join(" "))),
        "quit" => Ok(Command::Quit),
        other => Err(format!("unknown command `{other}`")),
    }
}

fn parse_secs(arg: Option<&String>, cmd: &str) -> Result<f64, String> {
    let raw = arg.ok_or_else(|| format!("{cmd}: missing seconds"))?;
    let secs = raw
        .parse::<f64>()
        .map_err(|_| format!("{cmd}: invalid seconds `{raw}`"))?;
    if !secs.is_finite() || secs < 0.0 {
        return Err(format!("{cmd}: seconds must be non-negative, got `{raw}`"));
    }
    Ok(secs)
}

/// Parses `@target` or `<x> <y>` from the front of `args`, returning the
/// target and the unconsumed arguments.
fn parse_target<'a>(args: &'a [String], cmd: &str) -> Result<(Target, &'a [String]), String> {
    let first = args
        .first()
        .ok_or_else(|| format!("{cmd}: missing target (@name or x y)"))?;
    if let Some(name) = first.strip_prefix('@') {
        if name.is_empty() {
            return Err(format!("{cmd}: empty target name"));
        }
        let target = match name.split_once('.') {
            Some((scene, name)) => Target::Named {
                scene: Some(scene.to_ascii_lowercase()),
                name: name.to_owned(),
            },
            None => Target::Named {
                scene: None,
                name: name.to_owned(),
            },
        };
        return Ok((target, &args[1..]));
    }
    let y_raw = args
        .get(1)
        .ok_or_else(|| format!("{cmd}: missing y coordinate"))?;
    let x = first
        .parse::<i32>()
        .map_err(|_| format!("{cmd}: invalid x coordinate `{first}`"))?;
    let y = y_raw
        .parse::<i32>()
        .map_err(|_| format!("{cmd}: invalid y coordinate `{y_raw}`"))?;
    Ok((Target::Point { x, y }, &args[2..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn tokenize_handles_quotes_comments_and_escapes() {
        assert_eq!(
            tokenize(r#"type "hello world" plain # trailing comment"#).unwrap(),
            vec!["type", "hello world", "plain"]
        );
        assert_eq!(
            tokenize("   # only a comment").unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(tokenize("").unwrap(), Vec::<String>::new());
        assert_eq!(
            tokenize(r#"log "say \"hi\"" a#b"#).unwrap(),
            vec!["log", "say \"hi\"", "a#b"]
        );
        assert!(tokenize(r#"type "unterminated"#).is_err());
    }

    #[test]
    fn expand_env_replaces_references_and_rejects_unset() {
        let env = |name: &str| (name == "USER_NAME").then(|| "alice".to_owned());
        assert_eq!(expand_env("${USER_NAME}-1", &env).unwrap(), "alice-1");
        assert_eq!(expand_env("plain", &env).unwrap(), "plain");
        assert!(expand_env("${MISSING}", &env).is_err());
        assert!(expand_env("${OPEN", &env).is_err());
    }

    #[test]
    fn parse_script_produces_expected_commands() {
        let src = "\
# comment
wait 1.5
wait_scene char_select 30
click @login.username
click 10 20 right
move @create
wheel -3 @row0
type hello world
key Return ctrl shift
profile 15
action custom a b
screenshot /tmp/shot.png
log done
quit
";
        let parsed = parse_script(src, no_env).unwrap();
        let commands: Vec<Command> = parsed.iter().map(|i| i.command.clone()).collect();
        assert_eq!(parsed[0].line, 2);
        assert_eq!(
            commands,
            vec![
                Command::Wait(Duration::from_secs_f64(1.5)),
                Command::WaitScene {
                    scene: SceneType::CharacterSelection,
                    timeout: Duration::from_secs(30),
                },
                Command::Click {
                    target: Target::Named {
                        scene: Some("login".to_owned()),
                        name: "username".to_owned(),
                    },
                    button: MouseButton::Left,
                },
                Command::Click {
                    target: Target::Point { x: 10, y: 20 },
                    button: MouseButton::Right,
                },
                Command::Move(Target::Named {
                    scene: None,
                    name: "create".to_owned(),
                }),
                Command::Wheel {
                    delta: -3,
                    target: Some(Target::Named {
                        scene: None,
                        name: "row0".to_owned(),
                    }),
                },
                Command::Type("hello world".to_owned()),
                Command::Key {
                    keycode: Keycode::Return,
                    modifiers: Modifiers {
                        ctrl: true,
                        shift: true,
                        alt: false,
                    },
                },
                Command::Action {
                    name: "profile".to_owned(),
                    args: vec!["15".to_owned()],
                },
                Command::Action {
                    name: "custom".to_owned(),
                    args: vec!["a".to_owned(), "b".to_owned()],
                },
                Command::Screenshot(PathBuf::from("/tmp/shot.png")),
                Command::Log("done".to_owned()),
                Command::Quit,
            ]
        );
    }

    #[test]
    fn parse_script_reports_line_numbers_on_errors() {
        let err = parse_script("wait 1\nclick @\n", no_env).unwrap_err();
        assert!(err.starts_with("2:"), "{err}");
        let err = parse_script("bogus\n", no_env).unwrap_err();
        assert!(err.contains("unknown command `bogus`"), "{err}");
        let err = parse_script("wait -1\n", no_env).unwrap_err();
        assert!(err.contains("non-negative"), "{err}");
        let err = parse_script("wait_scene nowhere\n", no_env).unwrap_err();
        assert!(err.contains("unknown scene"), "{err}");
        let err = parse_script("key NotAKey\n", no_env).unwrap_err();
        assert!(err.contains("unknown key name"), "{err}");
        let err = parse_script("click 5\n", no_env).unwrap_err();
        assert!(err.contains("missing y"), "{err}");
        let err = parse_script("click @x sideways\n", no_env).unwrap_err();
        assert!(err.contains("unknown mouse button"), "{err}");
    }

    #[test]
    fn find_target_and_center() {
        let targets = [
            ("a", Bounds::new(10, 20, 30, 40)),
            ("b", Bounds::new(0, 0, 1, 1)),
        ];
        assert_eq!(
            find_target(&targets, "a"),
            Some(Bounds::new(10, 20, 30, 40))
        );
        assert_eq!(find_target(&targets, "zzz"), None);
        assert_eq!(bounds_center(&Bounds::new(10, 20, 30, 40)), (25, 40));
    }

    #[test]
    fn key_press_events_wrap_key_in_modifiers() {
        let events = key_press_events(
            Keycode::A,
            Modifiers {
                ctrl: true,
                shift: false,
                alt: false,
            },
        );
        assert_eq!(events.len(), 4);
        assert!(matches!(
            events[0],
            Event::KeyDown {
                keycode: Some(Keycode::LCtrl),
                ..
            }
        ));
        assert!(matches!(
            events[1],
            Event::KeyDown {
                keycode: Some(Keycode::A),
                keymod,
                ..
            } if keymod.contains(Mod::LCTRLMOD)
        ));
        assert!(matches!(
            events[2],
            Event::KeyUp {
                keycode: Some(Keycode::A),
                ..
            }
        ));
        assert!(matches!(
            events[3],
            Event::KeyUp {
                keycode: Some(Keycode::LCtrl),
                ..
            }
        ));

        let plain = key_press_events(Keycode::Return, Modifiers::default());
        assert_eq!(plain.len(), 2);
    }

    #[test]
    fn scene_names_roundtrip() {
        for scene in [
            SceneType::Login,
            SceneType::NewAccount,
            SceneType::CharacterSelection,
            SceneType::CharacterCreation,
            SceneType::Game,
            SceneType::Exit,
        ] {
            assert_eq!(
                SceneType::from_automation_name(scene.automation_name()),
                Some(scene)
            );
        }
        assert_eq!(
            SceneType::from_automation_name("GAME"),
            Some(SceneType::Game)
        );
        assert_eq!(SceneType::from_automation_name("nope"), None);
    }
}
