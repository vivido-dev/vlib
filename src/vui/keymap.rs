//! Keys: naming them, binding them, and deciding what an action means.
//!
//! A key press arrives as a USB HID usage and a modifier mask — protocol values, not platform
//! ones — so a binding written here means the same thing on every host. A binding names an
//! action, and an action is handled by whichever element has focus, or by the window when nothing
//! focused wants it.

use std::fmt;

use crate::vui::interactive::Modifiers;

/// Something a key or a menu item can ask for.
///
/// The `actions!` macro declares these; an action is a type because that is what a handler is
/// written against.
pub trait Action: 'static {
    /// The identity bindings are matched on.
    const ID: &'static str;
    /// The name a keystroke hint or a menu shows.
    const NAME: &'static str;
}

/// The identity of an action, for code that carries one without knowing its type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionId(pub &'static str);

impl ActionId {
    pub const fn of<A: Action>() -> Self {
        Self(A::ID)
    }

    /// The name of the action, for a hint on screen.
    pub fn name(self) -> &'static str {
        self.0.rsplit("::").next().unwrap_or(self.0)
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

/// Declare actions: `actions!(editor, [Save, Undo, SelectAll]);`
#[macro_export]
macro_rules! actions {
    ($namespace:ident, [$($action:ident),* $(,)?]) => {
        $(
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub struct $action;

            impl $crate::vui::keymap::Action for $action {
                const ID: &'static str = concat!(stringify!($namespace), "::", stringify!($action));
                const NAME: &'static str = stringify!($action);
            }
        )*
    };
}

/// A key and the modifiers that must be held with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Keystroke {
    /// A USB HID keyboard-page usage, in `0x04..=0xe7`.
    pub key: u32,
    pub modifiers: Modifiers,
}

impl Keystroke {
    pub const fn new(key: u32, modifiers: Modifiers) -> Self {
        Self { key, modifiers }
    }

    /// Parse a keystroke as it is written: `"cmd-shift-p"`, `"tab"`, `"ctrl-alt-delete"`.
    ///
    /// The modifiers are the ones a key must be held with, exactly: `"a"` does not fire while
    /// shift is down, and `"shift-a"` does not fire without it.
    pub fn parse(spec: &str) -> Option<Self> {
        let mut modifiers = Modifiers::NONE;
        let mut key = None;
        for part in spec.split('-') {
            let part = part.trim().to_ascii_lowercase();
            if part.is_empty() {
                return None;
            }
            match part.as_str() {
                "cmd" | "command" | "super" | "meta" => modifiers = modifiers.with_super(),
                "ctrl" | "control" => modifiers = modifiers.with_control(),
                "alt" | "opt" | "option" => modifiers = modifiers.with_alt(),
                "shift" => modifiers = modifiers.with_shift(),
                // A key name, which must be the last part: `"cmd--"` and `"shift-minus"` are
                // different keys, and only the name can say which.
                name => {
                    if key.is_some() {
                        return None;
                    }
                    key = usage_of(name);
                }
            }
        }
        key.map(|key| Self { key, modifiers })
    }

    /// Whether this keystroke is what the host just reported.
    pub fn matches(&self, key: u32, modifiers: Modifiers) -> bool {
        self.key == key && self.modifiers == modifiers
    }
}

impl fmt::Display for Keystroke {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<&str> = Vec::new();
        if self.modifiers.control() {
            parts.push("ctrl");
        }
        if self.modifiers.alt() {
            parts.push("alt");
        }
        if self.modifiers.shift() {
            parts.push("shift");
        }
        if self.modifiers.command() {
            parts.push("cmd");
        }
        let name = name_of(self.key);
        parts.push(&name);
        f.write_str(&parts.join("-"))
    }
}

/// A key press that means an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub keystroke: Keystroke,
    pub action: ActionId,
    /// Where the binding applies. `None` means everywhere in the window.
    pub context: Option<&'static str>,
}

impl KeyBinding {
    pub const fn new(
        keystroke: Keystroke,
        action: ActionId,
        context: Option<&'static str>,
    ) -> Self {
        Self {
            keystroke,
            action,
            context,
        }
    }

    /// A binding written the way a hint shows it: `KeyBinding::parse("cmd-s", Save)`.
    pub fn parse(spec: &str, action: ActionId) -> Option<Self> {
        Keystroke::parse(spec).map(|keystroke| Self::new(keystroke, action, None))
    }
}

/// The HID usage for a key name.
pub fn usage_of(name: &str) -> Option<u32> {
    KEY_TABLE
        .iter()
        .find(|(label, _)| *label == name)
        .map(|(_, usage)| *usage)
}

/// The name of a HID usage, as `Keystroke::parse` would have written it.
pub fn name_of(usage: u32) -> String {
    KEY_TABLE
        .iter()
        .find(|(_, known)| *known == usage)
        .map(|(label, _)| (*label).to_owned())
        .unwrap_or_else(|| format!("key{usage:02x}"))
}

/// The names every binding is written with, and the USB HID keyboard-page usage each one is.
///
/// The table is the protocol's: `desktop-surface-v1` and the overlay input lane both carry HID
/// usages, so a binding never depends on a platform's key naming.
pub const KEY_TABLE: &[(&str, u32)] = &[
    ("a", 0x04),
    ("b", 0x05),
    ("c", 0x06),
    ("d", 0x07),
    ("e", 0x08),
    ("f", 0x09),
    ("g", 0x0a),
    ("h", 0x0b),
    ("i", 0x0c),
    ("j", 0x0d),
    ("k", 0x0e),
    ("l", 0x0f),
    ("m", 0x10),
    ("n", 0x11),
    ("o", 0x12),
    ("p", 0x13),
    ("q", 0x14),
    ("r", 0x15),
    ("s", 0x16),
    ("t", 0x17),
    ("u", 0x18),
    ("v", 0x19),
    ("w", 0x1a),
    ("x", 0x1b),
    ("y", 0x1c),
    ("z", 0x1d),
    ("1", 0x1e),
    ("2", 0x1f),
    ("3", 0x20),
    ("4", 0x21),
    ("5", 0x22),
    ("6", 0x23),
    ("7", 0x24),
    ("8", 0x25),
    ("9", 0x26),
    ("0", 0x27),
    ("enter", 0x28),
    ("escape", 0x29),
    ("backspace", 0x2a),
    ("tab", 0x2b),
    ("space", 0x2c),
    ("minus", 0x2d),
    ("equal", 0x2e),
    ("bracket-left", 0x2f),
    ("bracket-right", 0x30),
    ("backslash", 0x31),
    ("semicolon", 0x33),
    ("quote", 0x34),
    ("grave", 0x35),
    ("comma", 0x36),
    ("period", 0x37),
    ("slash", 0x38),
    ("caps-lock", 0x39),
    ("f1", 0x3a),
    ("f2", 0x3b),
    ("f3", 0x3c),
    ("f4", 0x3d),
    ("f5", 0x3e),
    ("f6", 0x3f),
    ("f7", 0x40),
    ("f8", 0x41),
    ("f9", 0x42),
    ("f10", 0x43),
    ("f11", 0x44),
    ("f12", 0x45),
    ("insert", 0x49),
    ("home", 0x4a),
    ("page-up", 0x4b),
    ("delete", 0x4c),
    ("end", 0x4d),
    ("page-down", 0x4e),
    ("right", 0x4f),
    ("left", 0x50),
    ("down", 0x51),
    ("up", 0x52),
];

#[cfg(test)]
mod tests {
    use super::*;
    use vivid_protocol::overlay::modifiers;

    actions!(test_ns, [Save, Undo]);

    #[test]
    fn a_keystroke_parses_the_way_a_hint_writes_it() {
        let save = Keystroke::parse("cmd-s").unwrap();
        assert_eq!(save.key, usage_of("s").unwrap());
        assert_eq!(save.modifiers.bits(), modifiers::SUPER);

        let plain = Keystroke::parse("tab").unwrap();
        assert_eq!(plain.modifiers, Modifiers::NONE);
        assert_eq!(plain.key, usage_of("tab").unwrap());

        // Order does not matter, and the names are the ones the table spells.
        assert_eq!(
            Keystroke::parse("shift-cmd-p"),
            Keystroke::parse("cmd-shift-p")
        );
        assert_eq!(
            Keystroke::parse("ctrl-alt-delete").unwrap().key,
            usage_of("delete").unwrap()
        );
    }

    #[test]
    fn a_keystroke_that_names_nothing_is_refused() {
        assert!(Keystroke::parse("").is_none());
        assert!(Keystroke::parse("cmd-").is_none());
        assert!(Keystroke::parse("cmd-shift").is_none());
        assert!(Keystroke::parse("cmd-nosuchkey").is_none());
        assert!(Keystroke::parse("a-b").is_none());
    }

    #[test]
    fn a_keystroke_round_trips_through_its_own_writing() {
        for spec in [
            "cmd-s",
            "shift-tab",
            "ctrl-alt-delete",
            "page-up",
            "f5",
            "space",
        ] {
            let keystroke = Keystroke::parse(spec).unwrap();
            assert_eq!(
                Keystroke::parse(&keystroke.to_string()),
                Some(keystroke),
                "{spec}"
            );
        }
    }

    #[test]
    fn matching_is_exact_about_modifiers() {
        let save = Keystroke::parse("cmd-s").unwrap();
        assert!(save.matches(
            usage_of("s").unwrap(),
            Modifiers::from_bits(modifiers::SUPER)
        ));
        // The same key without the modifier is a different keystroke.
        assert!(!save.matches(usage_of("s").unwrap(), Modifiers::NONE));
        // And so is the same key with one more.
        assert!(!save.matches(
            usage_of("s").unwrap(),
            Modifiers::from_bits(modifiers::SUPER | modifiers::SHIFT)
        ));
    }

    #[test]
    fn a_binding_names_an_action_by_type() {
        let binding = KeyBinding::parse("cmd-s", ActionId::of::<Save>()).unwrap();
        assert_eq!(binding.action, ActionId("test_ns::Save"));
        assert_eq!(binding.action.name(), "Save");
        assert_eq!(binding.context, None);
        assert_ne!(ActionId::of::<Save>(), ActionId::of::<Undo>());
    }

    #[test]
    fn every_name_in_the_table_round_trips_and_stays_in_the_keyboard_page() {
        for (label, usage) in KEY_TABLE {
            assert!(
                (vivid_protocol::overlay::keys::FIRST_USAGE
                    ..=vivid_protocol::overlay::keys::LAST_USAGE)
                    .contains(usage),
                "{label} is outside the keyboard page"
            );
            assert_eq!(usage_of(label), Some(*usage));
            assert_eq!(name_of(*usage), *label);
        }
        // An unmapped usage still has a name a hint can show.
        assert_eq!(name_of(0x99), "key99");
    }
}
