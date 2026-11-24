//! Keyboard, paste, and SGR mouse input encoding for VT / xterm / Alacritty modes.

use alacritty_terminal::term::TermMode;
use gpui_kit::{Keystroke, Modifiers, MouseButton};

/// Encodes a GPUI `Keystroke` into raw terminal input bytes according to `TermMode`.
/// Returns `None` if the keystroke should not be sent to the PTY (e.g., pure platform shortcuts).
pub fn encode_keystroke(ks: &Keystroke, mode: TermMode) -> Option<Vec<u8>> {
    let key = ks.key.as_str();
    let mods = ks.modifiers;

    // Platform command key (macOS Cmd) is reserved for UI shortcuts, not terminal VT input.
    if mods.platform {
        return None;
    }

    let mod_param = xterm_modifier_param(mods);
    let app_cursor = mode.contains(TermMode::APP_CURSOR);

    match key {
        "enter" => {
            if mods.alt {
                return Some(b"\x1b\r".to_vec());
            }
            return Some(b"\r".to_vec());
        }
        "tab" => {
            if mods.control || mods.alt {
                return None;
            }
            if mods.shift {
                return Some(b"\x1b[Z".to_vec());
            }
            return Some(b"\t".to_vec());
        }
        "backspace" => {
            if mods.control {
                return Some(vec![0x08]);
            }
            if mods.alt {
                return Some(vec![0x1b, 0x7f]);
            }
            return Some(vec![0x7f]);
        }
        "escape" => return Some(vec![0x1b]),
        "space" => {
            if mods.control {
                return Some(vec![0x00]);
            }
            if mods.alt {
                return Some(b"\x1b ".to_vec());
            }
            return Some(b" ".to_vec());
        }
        "up" => return Some(encode_cursor_key(b'A', mod_param, app_cursor)),
        "down" => return Some(encode_cursor_key(b'B', mod_param, app_cursor)),
        "right" => return Some(encode_cursor_key(b'C', mod_param, app_cursor)),
        "left" => return Some(encode_cursor_key(b'D', mod_param, app_cursor)),
        "home" => return Some(encode_cursor_key(b'H', mod_param, app_cursor)),
        "end" => return Some(encode_cursor_key(b'F', mod_param, app_cursor)),
        "insert" => return Some(encode_tilde_key(2, mod_param)),
        "delete" => return Some(encode_tilde_key(3, mod_param)),
        "pageup" => return Some(encode_tilde_key(5, mod_param)),
        "pagedown" => return Some(encode_tilde_key(6, mod_param)),
        "f1" => return Some(encode_ss3_or_csi_fn(b'P', 11, mod_param)),
        "f2" => return Some(encode_ss3_or_csi_fn(b'Q', 12, mod_param)),
        "f3" => return Some(encode_ss3_or_csi_fn(b'R', 13, mod_param)),
        "f4" => return Some(encode_ss3_or_csi_fn(b'S', 14, mod_param)),
        "f5" => return Some(encode_tilde_key(15, mod_param)),
        "f6" => return Some(encode_tilde_key(17, mod_param)),
        "f7" => return Some(encode_tilde_key(18, mod_param)),
        "f8" => return Some(encode_tilde_key(19, mod_param)),
        "f9" => return Some(encode_tilde_key(20, mod_param)),
        "f10" => return Some(encode_tilde_key(21, mod_param)),
        "f11" => return Some(encode_tilde_key(23, mod_param)),
        "f12" => return Some(encode_tilde_key(24, mod_param)),
        _ => {}
    }

    // Control + ASCII key combinations (including shifted punctuation like Ctrl+^ / Ctrl+_ / Ctrl+@)
    if mods.control
        && (!mods.shift || !key.as_bytes().first().is_some_and(u8::is_ascii_alphabetic))
        && let Some(ctrl_byte) = control_byte_for_key(key)
    {
        if mods.alt {
            return Some(vec![0x1b, ctrl_byte]);
        }
        return Some(vec![ctrl_byte]);
    }
    if mods.control {
        return None;
    }

    // Printable text from `key_char` or single-character `key`
    let text = ks
        .key_char
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| (key.chars().count() == 1).then_some(key))?;

    let mut out = Vec::with_capacity(text.len() + 1);
    if mods.alt {
        out.push(0x1b);
    }
    if mods.shift && ks.key_char.is_none() && text.len() == 1 {
        out.extend_from_slice(text.to_ascii_uppercase().as_bytes());
    } else {
        out.extend_from_slice(text.as_bytes());
    }
    Some(out)
}

pub fn encode_paste(text: &str, mode: TermMode) -> Vec<u8> {
    if mode.contains(TermMode::BRACKETED_PASTE) {
        let sanitized = text.replace("\x1b[201~", "");
        let mut out = Vec::with_capacity(sanitized.len() + 12);
        out.extend_from_slice(b"\x1b[200~");
        out.extend_from_slice(sanitized.as_bytes());
        out.extend_from_slice(b"\x1b[201~");
        out
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseReportKind {
    Press(MouseButton),
    Release(MouseButton),
    Drag(MouseButton),
    Move,
    WheelUp,
    WheelDown,
}

/// Encodes mouse events when the running TUI application has requested mouse tracking.
pub fn encode_mouse_report(
    kind: MouseReportKind,
    col_zero_based: usize,
    row_zero_based: usize,
    mods: Modifiers,
    mode: TermMode,
) -> Option<Vec<u8>> {
    if !mode.intersects(TermMode::MOUSE_MODE) {
        return None;
    }

    match kind {
        MouseReportKind::Move if !mode.contains(TermMode::MOUSE_MOTION) => return None,
        MouseReportKind::Drag(_)
            if !mode.intersects(TermMode::MOUSE_DRAG | TermMode::MOUSE_MOTION) =>
        {
            return None;
        }
        _ => {}
    }

    let mut cb: u8 = match kind {
        MouseReportKind::Press(btn) | MouseReportKind::Release(btn) => match btn {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            _ => return None,
        },
        MouseReportKind::Drag(btn) => match btn {
            MouseButton::Left => 32,
            MouseButton::Middle => 33,
            MouseButton::Right => 34,
            _ => return None,
        },
        MouseReportKind::Move => 35,
        MouseReportKind::WheelUp => 64,
        MouseReportKind::WheelDown => 65,
    };

    if mods.shift {
        cb |= 4;
    }
    if mods.alt {
        cb |= 8;
    }
    if mods.control {
        cb |= 16;
    }

    let cx = col_zero_based.saturating_add(1);
    let cy = row_zero_based.saturating_add(1);

    if mode.contains(TermMode::SGR_MOUSE) {
        let suffix = match kind {
            MouseReportKind::Release(_) => 'm',
            _ => 'M',
        };
        Some(format!("\x1b[<{cb};{cx};{cy}{suffix}").into_bytes())
    } else if cx <= 223 && cy <= 223 {
        let btn_code = match kind {
            MouseReportKind::Release(_) => (3 | (cb & 0b0001_1100)).wrapping_add(32),
            _ => cb.wrapping_add(32),
        };
        Some(vec![
            0x1b,
            b'[',
            b'M',
            btn_code,
            (cx as u8).wrapping_add(32),
            (cy as u8).wrapping_add(32),
        ])
    } else {
        None
    }
}

fn xterm_modifier_param(mods: Modifiers) -> u8 {
    let mut param = 1u8;
    if mods.shift {
        param += 1;
    }
    if mods.alt {
        param += 2;
    }
    if mods.control {
        param += 4;
    }
    param
}

fn encode_cursor_key(final_byte: u8, mod_param: u8, app_cursor: bool) -> Vec<u8> {
    if mod_param > 1 {
        format!("\x1b[1;{mod_param}{}", final_byte as char).into_bytes()
    } else if app_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

fn encode_tilde_key(code: u8, mod_param: u8) -> Vec<u8> {
    if mod_param > 1 {
        format!("\x1b[{code};{mod_param}~").into_bytes()
    } else {
        format!("\x1b[{code}~").into_bytes()
    }
}

fn encode_ss3_or_csi_fn(ss3_byte: u8, tilde_code: u8, mod_param: u8) -> Vec<u8> {
    if mod_param > 1 {
        let _ = tilde_code;
        format!("\x1b[1;{mod_param}{}", ss3_byte as char).into_bytes()
    } else {
        vec![0x1b, b'O', ss3_byte]
    }
}

fn control_byte_for_key(key: &str) -> Option<u8> {
    let b = key.as_bytes();
    if b.len() != 1 {
        return None;
    }
    match b[0] {
        b'a'..=b'z' => Some(b[0] - b'a' + 1),
        b'A'..=b'Z' => Some(b[0] - b'A' + 1),
        b'@' | b'2' => Some(0x00),
        b'[' | b'3' => Some(0x1b),
        b'\\' | b'4' => Some(0x1c),
        b']' | b'5' => Some(0x1d),
        b'^' | b'6' => Some(0x1e),
        b'_' | b'-' | b'/' | b'7' => Some(0x1f),
        b'?' | b'8' => Some(0x7f),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinguishes_terminal_input_from_workspace_shortcuts() {
        for (shortcut, expected) in [
            ("tab", Some(&b"\t"[..])),
            ("shift-tab", Some(&b"\x1b[Z"[..])),
            ("ctrl-tab", None),
            ("ctrl-shift-tab", None),
            ("alt-tab", None),
            ("alt-1", Some(&b"\x1b1"[..])),
            ("alt-n", Some(&b"\x1bn"[..])),
            ("ctrl-b", Some(&b"\x02"[..])),
            ("ctrl-_", Some(&b"\x1f"[..])),
            ("ctrl-shift--", Some(&b"\x1f"[..])),
            ("ctrl-shift-t", None),
            ("ctrl-shift-t->T", None),
        ] {
            let key = Keystroke::parse(shortcut).unwrap();
            assert_eq!(
                encode_keystroke(&key, TermMode::NONE).as_deref(),
                expected,
                "{shortcut}"
            );
        }
    }

    #[test]
    fn encodes_arrows_and_ctrl_keys_and_bracketed_paste() {
        let ks_up = Keystroke {
            modifiers: Modifiers::default(),
            key: "up".into(),
            key_char: None,
        };
        assert_eq!(encode_keystroke(&ks_up, TermMode::NONE).unwrap(), b"\x1b[A");
        assert_eq!(
            encode_keystroke(&ks_up, TermMode::APP_CURSOR).unwrap(),
            b"\x1bOA"
        );

        let ks_ctrl_c = Keystroke {
            modifiers: Modifiers {
                control: true,
                ..Default::default()
            },
            key: "c".into(),
            key_char: None,
        };
        assert_eq!(
            encode_keystroke(&ks_ctrl_c, TermMode::NONE).unwrap(),
            vec![0x03]
        );

        let pasted = encode_paste("echo hi", TermMode::BRACKETED_PASTE);
        assert_eq!(pasted, b"\x1b[200~echo hi\x1b[201~");
    }
}
