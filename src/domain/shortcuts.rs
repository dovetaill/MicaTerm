use std::collections::BTreeMap;
use std::sync::LazyLock;

use gpui_kit::{Keystroke, Modifiers, component::kbd::Kbd};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum DoubleClickAction {
    #[default]
    SelectWord,
    Paste,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutScope {
    Terminal,
    Workspace,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutCategory {
    Terminal,
    Session,
    Navigation,
}

impl ShortcutCategory {
    pub const ALL: [Self; 3] = [Self::Terminal, Self::Session, Self::Navigation];

    pub fn label(self) -> &'static str {
        match self {
            Self::Terminal => "终端交互",
            Self::Session => "会话与分屏",
            Self::Navigation => "标签与面板导航",
        }
    }
}

macro_rules! shortcut_actions {
    ($($category:ident { $($action:ident => ($label:literal, $pc:literal, $mac:literal $(, $tab:literal)?)),+ $(,)? })+) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum ShortcutAction { $($($action,)+)+ }

        impl ShortcutAction {
            pub const ALL: &'static [Self] = &[$($(Self::$action,)+)+];

            pub fn category(self) -> ShortcutCategory {
                match self { $($(Self::$action)|+ => ShortcutCategory::$category,)+ }
            }

            pub fn label(self) -> &'static str {
                match self { $($(Self::$action => $label,)+)+ }
            }

            pub fn tab_index(self) -> Option<usize> {
                match self { $($($(Self::$action => Some($tab),)?)+)+ _ => None }
            }

            pub fn default_chord(self) -> &'static ShortcutChord {
                match self { $($(Self::$action => {
                    static DEFAULT: LazyLock<ShortcutChord> = LazyLock::new(|| {
                        let keys = if cfg!(target_os = "macos") { $mac } else { $pc };
                        let stroke = Keystroke::parse(keys).expect("内置快捷键格式无效");
                        ShortcutChord::try_record(&stroke).expect("内置快捷键不可录制")
                    });
                    &DEFAULT
                },)+)+ }
            }
        }
    };
}

shortcut_actions! {
    Terminal {
        Copy => ("复制选中内容", "ctrl-shift-c", "cmd-c"),
        Paste => ("粘贴剪贴板", "ctrl-shift-v", "cmd-v"),
        SelectAll => ("全选终端内容", "ctrl-shift-a", "cmd-a"),
        FindInTerminal => ("终端内搜索", "ctrl-shift-f", "cmd-f"),
        ClearTerminal => ("清屏", "ctrl-shift-k", "cmd-k"),
        JumpPrevBlock => ("跳至上一条命令块", "ctrl-shift-up", "cmd-up"),
        JumpNextBlock => ("跳至下一条命令块", "ctrl-shift-down", "cmd-down"),
        ZoomIn => ("放大终端字号", "ctrl-=", "cmd-="),
        ZoomOut => ("缩小终端字号", "ctrl--", "cmd--"),
        ZoomReset => ("重置默认字号", "ctrl-0", "cmd-0"),
    }
    Session {
        NewLocalTab => ("新建本地终端", "ctrl-shift-t", "cmd-t"),
        NewSshHost => ("新建 SSH 主机连接", "ctrl-shift-n", "cmd-n"),
        ClosePaneOrTab => ("关闭当前分屏或标签", "ctrl-shift-w", "cmd-w"),
        SplitHorizontal => ("向右水平分屏", "ctrl-shift-d", "cmd-d"),
        SplitVertical => ("向下垂直分屏", "ctrl-shift-e", "cmd-shift-d"),
    }
    Navigation {
        NextTab => ("切换到下一个标签页", "ctrl-tab", "ctrl-tab"),
        PrevTab => ("切换到上一个标签页", "ctrl-shift-tab", "ctrl-shift-tab"),
        SelectTab1 => ("切换到第 1 个标签页", "alt-1", "cmd-1", 0),
        SelectTab2 => ("切换到第 2 个标签页", "alt-2", "cmd-2", 1),
        SelectTab3 => ("切换到第 3 个标签页", "alt-3", "cmd-3", 2),
        SelectTab4 => ("切换到第 4 个标签页", "alt-4", "cmd-4", 3),
        SelectTab5 => ("切换到第 5 个标签页", "alt-5", "cmd-5", 4),
        SelectTab6 => ("切换到第 6 个标签页", "alt-6", "cmd-6", 5),
        SelectTab7 => ("切换到第 7 个标签页", "alt-7", "cmd-7", 6),
        SelectTab8 => ("切换到第 8 个标签页", "alt-8", "cmd-8", 7),
        SelectTab9 => ("切换到第 9 个标签页", "alt-9", "cmd-9", 8),
        ToggleLeftSidebar => ("显隐左侧资产栏", "ctrl-shift-b", "cmd-b"),
        ToggleSftpPanel => ("显隐右侧 SFTP 面板", "ctrl-shift-s", "cmd-shift-s"),
        OpenSettings => ("打开偏好设置", "ctrl-,", "cmd-,"),
    }
}

impl ShortcutAction {
    pub fn scope(self) -> ShortcutScope {
        match self.category() {
            ShortcutCategory::Terminal => ShortcutScope::Terminal,
            ShortcutCategory::Session | ShortcutCategory::Navigation => ShortcutScope::Workspace,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(try_from = "Keystroke")]
pub struct ShortcutChord {
    modifiers: Modifiers,
    key: String,
}

impl ShortcutChord {
    pub fn from_keystroke(ks: &Keystroke) -> Option<Self> {
        let key = ks.key.to_ascii_lowercase();
        let mut modifiers = Modifiers {
            function: false,
            ..ks.modifiers
        };
        // Windows can report shifted punctuation as the symbol with the Shift bit cleared.
        let base = match key.as_str() {
            "" | "shift" | "control" | "ctrl" | "alt" | "platform" | "cmd" | "super" | "win"
            | "meta" | "function" | "capslock" => return None,
            "~" => "`",
            "!" => "1",
            "@" => "2",
            "#" => "3",
            "$" => "4",
            "%" => "5",
            "^" => "6",
            "&" => "7",
            "*" => "8",
            "(" => "9",
            ")" => "0",
            "_" => "-",
            "+" => "=",
            "{" => "[",
            "}" => "]",
            "|" => "\\",
            ":" => ";",
            "\"" => "'",
            "<" => ",",
            ">" => ".",
            "?" => "/",
            _ => return Some(Self { modifiers, key }),
        };
        modifiers.shift = true;
        Some(Self {
            modifiers,
            key: base.into(),
        })
    }

    pub fn try_record(ks: &Keystroke) -> Result<Self, &'static str> {
        let chord = Self::from_keystroke(ks).ok_or("请在修饰键之外再按一个键")?;
        let key = chord.key.as_str();
        if key == "escape" {
            return Err("Esc 用于取消录制");
        }
        let mods = chord.modifiers;
        if mods.control
            || mods.alt
            || mods.platform
            || matches!(
                key,
                "f1" | "f2"
                    | "f3"
                    | "f4"
                    | "f5"
                    | "f6"
                    | "f7"
                    | "f8"
                    | "f9"
                    | "f10"
                    | "f11"
                    | "f12"
            )
            || (mods.shift && matches!(key, "insert" | "delete"))
        {
            Ok(chord)
        } else {
            Err("快捷键需包含 Ctrl / Alt / Cmd 修饰键，或使用 F1~F12、Shift+Insert、Shift+Delete")
        }
    }
}

impl TryFrom<Keystroke> for ShortcutChord {
    type Error = &'static str;

    fn try_from(ks: Keystroke) -> Result<Self, Self::Error> {
        Self::try_record(&ks)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShortcutConfig {
    overrides: BTreeMap<ShortcutAction, Option<ShortcutChord>>,
}

impl ShortcutConfig {
    fn binding(&self, action: ShortcutAction) -> Option<&ShortcutChord> {
        match self.overrides.get(&action) {
            Some(value) => value.as_ref(),
            None => Some(action.default_chord()),
        }
    }

    pub fn get(&self, action: ShortcutAction) -> Option<ShortcutChord> {
        self.binding(action).cloned()
    }

    pub fn is_customized(&self, action: ShortcutAction) -> bool {
        self.overrides.contains_key(&action)
    }

    fn bound_action(&self, chord: &ShortcutChord) -> Option<ShortcutAction> {
        ShortcutAction::ALL
            .iter()
            .copied()
            .find(|&action| self.binding(action) == Some(chord))
    }

    pub fn set(
        &mut self,
        action: ShortcutAction,
        chord: Option<ShortcutChord>,
    ) -> Option<ShortcutAction> {
        let conflict = chord
            .as_ref()
            .and_then(|c| self.bound_action(c))
            .filter(|other| *other != action);
        if let Some(other) = conflict {
            self.overrides.insert(other, None);
        }
        if chord.as_ref() == Some(action.default_chord()) {
            self.overrides.remove(&action);
        } else {
            self.overrides.insert(action, chord);
        }
        conflict
    }

    pub fn reset_one(&mut self, action: ShortcutAction) {
        self.set(action, Some(action.default_chord().clone()));
    }

    pub fn reset_all(&mut self) {
        self.overrides.clear();
    }

    pub fn match_action(&self, ks: &Keystroke, scope: ShortcutScope) -> Option<ShortcutAction> {
        use ShortcutAction::{Copy, Paste, ZoomIn};
        let mut chord = ShortcutChord::from_keystroke(ks)?;
        if let Some(action) = self.bound_action(&chord) {
            return (action.scope() == scope).then_some(action);
        }
        if scope == ShortcutScope::Terminal {
            if chord.key == "=" && chord.modifiers.shift {
                chord.modifiers.shift = false;
                return (self.binding(ZoomIn) == Some(&chord)).then_some(ZoomIn);
            }
            if !cfg!(target_os = "macos") && chord.key == "insert" {
                let action = if chord.modifiers == Modifiers::control() {
                    Copy
                } else if chord.modifiers == Modifiers::shift() {
                    Paste
                } else {
                    return None;
                };
                return self.binding(action).map(|_| action);
            }
        }
        None
    }
}

pub fn format_chord(chord: &ShortcutChord) -> String {
    Kbd::format(&Keystroke {
        modifiers: chord.modifiers,
        key: chord.key.clone(),
        key_char: None,
    })
}

pub fn tooltip(label: &str, chord: Option<&ShortcutChord>) -> String {
    match chord {
        Some(chord) => format!("{label} ({})", format_chord(chord)),
        None => label.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ShortcutAction::*;
    use std::collections::HashSet;

    #[test]
    fn every_default_is_unique_and_matches_its_scope() {
        let config = ShortcutConfig::default();
        let mut seen = HashSet::new();
        assert_eq!(ShortcutAction::ALL.len(), 29);
        for &action in ShortcutAction::ALL {
            let chord = config.get(action).unwrap();
            assert!(seen.insert(chord.clone()), "重复的默认快捷键 {action:?}");
            assert!(!config.is_customized(action));
            let ks = Keystroke {
                key: chord.key,
                modifiers: chord.modifiers,
                key_char: None,
            };
            assert_eq!(config.match_action(&ks, action.scope()), Some(action));
            let other_scope = match action.scope() {
                ShortcutScope::Terminal => ShortcutScope::Workspace,
                ShortcutScope::Workspace => ShortcutScope::Terminal,
            };
            assert_eq!(config.match_action(&ks, other_scope), None);
        }
        assert_eq!(
            ShortcutAction::ALL
                .iter()
                .filter_map(|a| a.tab_index())
                .collect::<Vec<_>>(),
            (0..9).collect::<Vec<_>>()
        );
    }

    #[test]
    fn recording_and_json_share_canonical_keys() {
        for (symbol, base) in "~!@#$%^&*()_+{}|:\"<>?"
            .chars()
            .zip("`1234567890-=[]\\;',./".chars())
        {
            let win = Keystroke {
                key: symbol.to_string(),
                modifiers: Modifiers {
                    control: true,
                    function: true,
                    ..Modifiers::none()
                },
                key_char: Some(symbol.to_string()),
            };
            let other = Keystroke {
                key: base.to_string(),
                modifiers: Modifiers::control_shift(),
                key_char: None,
            };
            let chord = ShortcutChord::try_record(&win).unwrap();
            assert_eq!(chord, ShortcutChord::try_record(&other).unwrap());
            let json = serde_json::to_value(&chord).unwrap();
            assert_eq!(json["key"], base.to_string());
            assert_eq!(
                serde_json::from_value::<ShortcutChord>(json).unwrap(),
                chord
            );
        }
        let shifted =
            ShortcutChord::try_record(&Keystroke::parse("ctrl-shift-=").unwrap()).unwrap();
        let from_disk = serde_json::json!({"key": "+", "modifiers": {"control": true}});
        assert_eq!(
            serde_json::from_value::<ShortcutChord>(from_disk).unwrap(),
            shifted
        );
        let invalid = serde_json::json!({"key": "a", "modifiers": {}});
        assert!(serde_json::from_value::<ShortcutChord>(invalid).is_err());
    }

    #[test]
    fn recording_reserves_text_navigation_and_escape() {
        for key in [
            "a",
            "tab",
            "shift-tab",
            "enter",
            "shift-space",
            "shift-a",
            "shift-1",
            "shift-/",
            "shift-up",
            "escape",
            "ctrl-escape",
            "f13",
            "shift",
            "capslock",
        ] {
            assert!(
                ShortcutChord::try_record(&Keystroke::parse(key).unwrap()).is_err(),
                "{key}"
            );
        }
        for key in [
            "ctrl-shift-n",
            "alt-1",
            "cmd-a",
            "f1",
            "f5",
            "shift-f12",
            "shift-insert",
            "shift-delete",
        ] {
            assert!(
                ShortcutChord::try_record(&Keystroke::parse(key).unwrap()).is_ok(),
                "{key}"
            );
        }
        for key in [
            "shift", "control", "ctrl", "alt", "platform", "cmd", "super", "win", "meta",
            "function", "capslock",
        ] {
            assert!(
                ShortcutChord::from_keystroke(&Keystroke {
                    key: key.into(),
                    ..Default::default()
                })
                .is_none()
            );
        }
        assert_eq!(
            ShortcutChord::from_keystroke(&Keystroke {
                key: "C".into(),
                modifiers: Modifiers::control_shift(),
                key_char: None
            }),
            ShortcutChord::from_keystroke(&Keystroke::parse("ctrl-shift-c").unwrap())
        );
    }

    #[test]
    fn conflicts_and_resets_preserve_one_owner_across_scopes() {
        let mut config = ShortcutConfig::default();
        let original = config.get(Copy);
        assert_eq!(config.set(NewLocalTab, original.clone()), Some(Copy));
        assert_eq!(config.get(Copy), None);
        assert!(config.is_customized(Copy));
        assert_eq!(config.get(NewLocalTab), original);
        assert_eq!(
            tooltip(Copy.label(), config.get(Copy).as_ref()),
            Copy.label()
        );
        config.reset_one(Copy);
        assert_eq!(config.get(Copy), original);
        assert!(!config.is_customized(Copy));
        assert_eq!(config.get(NewLocalTab), None);
        config.reset_all();
        assert_eq!(config, ShortcutConfig::default());
        let expected = if cfg!(target_os = "macos") {
            "新建本地终端 (⌘T)"
        } else {
            "新建本地终端 (Ctrl+Shift+T)"
        };
        assert_eq!(
            tooltip(NewLocalTab.label(), config.get(NewLocalTab).as_ref()),
            expected
        );
    }

    #[test]
    fn shifted_equals_obeys_explicit_bindings_and_clearing() {
        let mut config = ShortcutConfig::default();
        let plus = Keystroke {
            key: "+".into(),
            modifiers: ZoomIn.default_chord().modifiers,
            key_char: None,
        };
        assert_eq!(
            config.match_action(&plus, ShortcutScope::Terminal),
            Some(ZoomIn)
        );
        config.set(NewSshHost, Some(ShortcutChord::try_record(&plus).unwrap()));
        assert_eq!(config.match_action(&plus, ShortcutScope::Terminal), None);
        assert_eq!(
            config.match_action(&plus, ShortcutScope::Workspace),
            Some(NewSshHost)
        );
        config.reset_one(NewSshHost);
        config.set(ZoomIn, None);
        assert_eq!(config.match_action(&plus, ShortcutScope::Terminal), None);
        config.set(
            ZoomIn,
            Some(ShortcutChord::try_record(&Keystroke::parse("alt-=").unwrap()).unwrap()),
        );
        assert_eq!(config.match_action(&plus, ShortcutScope::Terminal), None);
        assert_eq!(
            config.match_action(
                &Keystroke::parse("alt-shift-=").unwrap(),
                ShortcutScope::Terminal
            ),
            Some(ZoomIn)
        );
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn insert_aliases_yield_to_explicit_bindings_and_clearing() {
        for (key, action) in [("ctrl-insert", Copy), ("shift-insert", Paste)] {
            let ks = Keystroke::parse(key).unwrap();
            let mut config = ShortcutConfig::default();
            assert_eq!(
                config.match_action(&ks, ShortcutScope::Terminal),
                Some(action)
            );
            config.set(NewLocalTab, Some(ShortcutChord::try_record(&ks).unwrap()));
            assert_eq!(config.match_action(&ks, ShortcutScope::Terminal), None);
            assert_eq!(
                config.match_action(&ks, ShortcutScope::Workspace),
                Some(NewLocalTab)
            );
            config.reset_one(NewLocalTab);
            config.set(action, None);
            assert_eq!(config.match_action(&ks, ShortcutScope::Terminal), None);
        }
    }
}
