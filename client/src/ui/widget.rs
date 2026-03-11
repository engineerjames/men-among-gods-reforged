//! Core widget trait, geometry types, and event definitions for the UI framework.

use std::time::Duration;

use sdl2::keyboard::Keycode;

use super::style::Padding;
use super::RenderContext;
use crate::preferences::DisplayMode;

// ---------------------------------------------------------------------------
// Geometry
// ---------------------------------------------------------------------------

/// Axis-aligned bounding rectangle for widget layout and hit-testing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    /// Left edge (pixels).
    pub x: i32,
    /// Top edge (pixels).
    pub y: i32,
    /// Width (pixels).
    pub width: u32,
    /// Height (pixels).
    pub height: u32,
}

impl Bounds {
    /// Create a new `Bounds` rectangle.
    ///
    /// # Arguments
    ///
    /// * `x` - Left edge in pixels.
    /// * `y` - Top edge in pixels.
    /// * `width` - Width in pixels.
    /// * `height` - Height in pixels.
    ///
    /// # Returns
    ///
    /// A new `Bounds` value.
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns `true` if the point `(px, py)` lies inside (or on the edge of)
    /// this rectangle.
    ///
    /// # Arguments
    ///
    /// * `px` - X coordinate of the point.
    /// * `py` - Y coordinate of the point.
    ///
    /// # Returns
    ///
    /// `true` if the point is inside or on the boundary.
    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x
            && py >= self.y
            && px < self.x + self.width as i32
            && py < self.y + self.height as i32
    }

    /// Returns a new `Bounds` shrunk inward by the given `padding`.
    ///
    /// If the padding exceeds the available space, width/height are clamped to
    /// zero.
    ///
    /// # Arguments
    ///
    /// * `padding` - The padding to subtract from each edge.
    ///
    /// # Returns
    ///
    /// A new, smaller `Bounds`.
    pub fn inner(&self, padding: &Padding) -> Bounds {
        let left = padding.left as i32;
        let top = padding.top as i32;
        let h_pad = padding.left + padding.right;
        let v_pad = padding.top + padding.bottom;
        Bounds {
            x: self.x + left,
            y: self.y + top,
            width: self.width.saturating_sub(h_pad),
            height: self.height.saturating_sub(v_pad),
        }
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Which mouse button was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Primary / left button.
    Left,
    /// Secondary / right button.
    Right,
    /// Middle / wheel button.
    Middle,
}

/// Modifier key state at the time of a key event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// `true` if Ctrl (or Cmd on macOS) is held.
    pub ctrl: bool,
    /// `true` if Shift is held.
    pub shift: bool,
    /// `true` if Alt (or Option on macOS) is held.
    pub alt: bool,
}

impl KeyModifiers {
    /// Build `KeyModifiers` from SDL2 modifier flags.
    ///
    /// # Arguments
    ///
    /// * `m` - SDL2 key-modifier bitfield.
    ///
    /// # Returns
    ///
    /// A `KeyModifiers` with the corresponding flags set.
    pub fn from_sdl2(m: sdl2::keyboard::Mod) -> Self {
        Self {
            ctrl: m.intersects(sdl2::keyboard::Mod::LCTRLMOD | sdl2::keyboard::Mod::RCTRLMOD),
            shift: m.intersects(sdl2::keyboard::Mod::LSHIFTMOD | sdl2::keyboard::Mod::RSHIFTMOD),
            alt: m.intersects(sdl2::keyboard::Mod::LALTMOD | sdl2::keyboard::Mod::RALTMOD),
        }
    }
}

/// An input event translated from SDL2 into widget-local terms.
#[derive(Clone, Debug)]
pub enum UiEvent {
    /// A mouse button was pressed.
    MouseClick {
        /// X in logical viewport coordinates.
        x: i32,
        /// Y in logical viewport coordinates.
        y: i32,
        /// Which button.
        button: MouseButton,
        /// Modifier key state at the time of the click.
        modifiers: KeyModifiers,
    },
    /// The scroll wheel moved.
    MouseWheel {
        /// X position of the mouse when the wheel moved.
        x: i32,
        /// Y position of the mouse when the wheel moved.
        y: i32,
        /// Positive = scroll up (toward newer), negative = scroll down.
        delta: i32,
    },
    /// The mouse moved to a new position.
    MouseMove {
        /// X in logical viewport coordinates.
        x: i32,
        /// Y in logical viewport coordinates.
        y: i32,
    },
    /// Text was typed (one or more UTF-8 characters).
    TextInput {
        /// The typed characters.
        text: String,
    },
    /// A physical key was pressed.
    KeyDown {
        /// Which key.
        keycode: Keycode,
        /// Modifier state at the time of press.
        modifiers: KeyModifiers,
    },
}

/// Whether a widget consumed an event or ignored it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventResponse {
    /// The widget handled this event; do not propagate further.
    Consumed,
    /// The widget did not handle this event; propagate to the next handler.
    Ignored,
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// Identifies one of the togglable HUD panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudPanel {
    /// Skills / character / talent tree.
    Skills,
    /// Money / inventory / equipment.
    Inventory,
    /// Settings / options.
    Settings,
    /// World minimap overlay.
    Minimap,
}

/// A side-effect that a widget wants the owning scene to perform.
///
/// Widgets cannot access the network or global game state directly. Instead
/// they produce actions that the scene drains each frame.
#[derive(Clone, Debug)]
pub enum WidgetAction {
    /// Send a chat message through the network.
    SendChat(String),
    /// Toggle visibility of a HUD panel.
    TogglePanel(HudPanel),
    /// Commit pending stat/skill raises to the server.
    ///
    /// Each tuple is `(stat_index, raise_count)` where `stat_index` is the
    /// protocol stat number (0-7 = attribs/pools, 8+ = skill_nr + 8).
    CommitStats {
        /// The raises to commit.
        raises: Vec<(i16, i32)>,
    },
    /// Cast/fire a skill by its protocol skill number.
    CastSkill {
        /// The skill number to cast.
        skill_nr: u32,
    },
    /// Begin a spell-bar assignment for the given skilltab index.
    BeginSkillAssign {
        /// The skilltab index of the skill to assign.
        skill_id: usize,
    },
    /// Bind a skill to a CTRL+key slot (1-9).
    BindSkillKey {
        /// The protocol skill number to bind.
        skill_nr: u32,
        /// Key slot index (0 = key "1", 8 = key "9").
        key_slot: u8,
    },
    /// Inventory interaction (pick up, equip, shift-equip, etc.).
    ///
    /// Mapped to `ClientCommand::new_inv(a, b, selected_char)` by the scene.
    InvAction {
        /// Action code (0=shift-pick, 1=shift-equip, 5=equip, 6=pick, 7=right-click worn).
        a: u32,
        /// Item slot index or wear-slot number.
        b: u32,
        /// Target character (0 = self).
        selected_char: u32,
    },
    /// Inspect an inventory/worn item.
    ///
    /// Mapped to `ClientCommand::new_inv_look(a, b, c)` by the scene.
    InvLookAction {
        /// Item slot index.
        a: u32,
        /// Reserved (usually 0).
        b: u32,
        /// Target character.
        c: u32,
    },
    /// Change the player's speed mode.
    ///
    /// Mapped to `ClientCommand::new_mode(mode)` by the scene.
    ChangeMode(i32),
    /// Shop interaction (buy/sell/take from depot or grave).
    ///
    /// Mapped to `ClientCommand::new_shop(shop_nr, action)` by the scene.
    ShopAction {
        /// The NPC/shop number.
        shop_nr: i16,
        /// Slot index (0-61 = buy/take, 62-123 = sell/look).
        action: i32,
    },
    /// Close the shop/depot/grave overlay.
    CloseShop,
    /// Disconnect from the game server and return to character selection.
    Disconnect,
    /// Quit the application entirely.
    Quit,
    /// Open the log directory in the platform file manager.
    OpenLogDir,
    /// Start the wall-clock performance profiler.
    StartProfiler,
    /// Toggle shadow rendering.
    SetShadows(bool),
    /// Toggle spell/visual effects.
    SetSpellEffects(bool),
    /// Toggle overhead player name display.
    SetShowNames(bool),
    /// Toggle overhead health percentage display.
    SetShowHealthPct(bool),
    /// Toggle wall hiding.
    SetHideWalls(bool),
    /// Change the master volume (0.0 = muted, 1.0 = full).
    SetMasterVolume(f32),
    /// Change the display mode (windowed, fullscreen, borderless).
    SetDisplayMode(DisplayMode),
    /// Toggle pixel-perfect (integer-only) scaling.
    SetPixelPerfectScaling(bool),
    /// Toggle vertical sync.
    SetVSync(bool),
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------

/// A renderable, interactive UI element.
///
/// Implementors should be object-safe so they can be stored as
/// `Box<dyn Widget>` inside container widgets.
pub trait Widget {
    /// Returns the bounding rectangle of this widget.
    fn bounds(&self) -> &Bounds;

    /// Moves the widget's top-left corner to `(x, y)`.
    ///
    /// # Arguments
    ///
    /// * `x` - New left edge.
    /// * `y` - New top edge.
    fn set_position(&mut self, x: i32, y: i32);

    /// Process an input event.
    ///
    /// # Arguments
    ///
    /// * `event` - The translated UI event.
    ///
    /// # Returns
    ///
    /// `Consumed` if this widget handled the event, `Ignored` otherwise.
    fn handle_event(&mut self, event: &UiEvent) -> EventResponse;

    /// Advance any time-driven widget state by `dt`.
    ///
    /// Called once per frame before `render`. The default implementation is a
    /// no-op; override it when a widget needs to animate or react to elapsed
    /// time (e.g. idle-fade, cooldown timers).
    ///
    /// # Arguments
    ///
    /// * `_dt` - Elapsed time since the last frame.
    fn update(&mut self, _dt: Duration) {}

    /// Draw this widget onto the canvas.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Mutable render context (canvas + graphics cache).
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an SDL2 error string.
    fn render(&mut self, ctx: &mut RenderContext) -> Result<(), String>;

    /// Drain any pending actions that this widget produced since the last call.
    ///
    /// # Returns
    ///
    /// A vector of actions. Empty if there are none.
    fn take_actions(&mut self) -> Vec<WidgetAction> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Bounds::contains_point --

    #[test]
    fn contains_point_inside() {
        let b = Bounds::new(10, 20, 100, 50);
        assert!(b.contains_point(50, 40));
    }

    #[test]
    fn contains_point_top_left_edge() {
        let b = Bounds::new(10, 20, 100, 50);
        assert!(b.contains_point(10, 20));
    }

    #[test]
    fn contains_point_just_outside_right() {
        let b = Bounds::new(10, 20, 100, 50);
        // right edge = 10 + 100 = 110, so 110 is outside
        assert!(!b.contains_point(110, 40));
    }

    #[test]
    fn contains_point_just_outside_bottom() {
        let b = Bounds::new(10, 20, 100, 50);
        // bottom edge = 20 + 50 = 70, so 70 is outside
        assert!(!b.contains_point(50, 70));
    }

    #[test]
    fn contains_point_above() {
        let b = Bounds::new(10, 20, 100, 50);
        assert!(!b.contains_point(50, 19));
    }

    #[test]
    fn contains_point_left_of() {
        let b = Bounds::new(10, 20, 100, 50);
        assert!(!b.contains_point(9, 40));
    }

    #[test]
    fn contains_point_bottom_right_inclusive() {
        let b = Bounds::new(10, 20, 100, 50);
        // Last pixel inside: (109, 69)
        assert!(b.contains_point(109, 69));
    }

    // -- Bounds::inner --

    #[test]
    fn inner_with_uniform_padding() {
        let b = Bounds::new(10, 20, 100, 80);
        let p = Padding::uniform(5);
        let inner = b.inner(&p);
        assert_eq!(inner, Bounds::new(15, 25, 90, 70));
    }

    #[test]
    fn inner_with_asymmetric_padding() {
        let b = Bounds::new(0, 0, 200, 100);
        let p = Padding {
            top: 10,
            right: 20,
            bottom: 30,
            left: 40,
        };
        let inner = b.inner(&p);
        assert_eq!(inner, Bounds::new(40, 10, 140, 60));
    }

    #[test]
    fn inner_clamped_to_zero() {
        let b = Bounds::new(0, 0, 10, 10);
        let p = Padding::uniform(20);
        let inner = b.inner(&p);
        assert_eq!(inner.width, 0);
        assert_eq!(inner.height, 0);
    }

    // -- KeyModifiers --

    #[test]
    fn key_modifiers_default_is_none() {
        let m = KeyModifiers::default();
        assert!(!m.ctrl);
        assert!(!m.shift);
        assert!(!m.alt);
    }
}
