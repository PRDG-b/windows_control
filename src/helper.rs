// 자주 쓰는 조합키(Modifier) 상수 정의 (키보드 패킷의 첫 번째 바이트)
pub const MOD_NONE: u8  = 0x00;
pub const MOD_CTRL: u8  = 0x01; // 왼쪽 Ctrl
pub const MOD_SHIFT: u8 = 0x02; // 왼쪽 Shift
pub const MOD_ALT: u8   = 0x04; // 왼쪽 Alt
pub const MOD_WIN: u8   = 0x08; // 왼쪽 Windows(GUI) 키

// 자주 쓰는 특수키 번호(Keycode) 상수 정의 (키보드 패킷의 세 번째 바이트)
pub const KEY_NONE: u8      = 0x00;
pub const KEY_ENTER: u8     = 40;
pub const KEY_ESC: u8       = 41;
pub const KEY_BACKSPACE: u8 = 42;
pub const KEY_TAB: u8       = 43;
pub const KEY_SPACE: u8     = 44;
pub const KEY_F4: u8        = 61;
pub const KEY_F5: u8        = 62;
pub const KEY_F11: u8       = 68;
pub const KEY_RIGHT: u8     = 79;
pub const KEY_LEFT: u8      = 80;
pub const KEY_DOWN: u8      = 81;
pub const KEY_UP: u8        = 82;

/// ASCII 문자를 (USB HID Keycode, Shift 여부)로 변환 (US QWERTY 기준)
pub fn char_to_hid(ch: char) -> Option<(u8, u8)> {
    match ch {
        // 1. 알파벳
        'a'..='z' => Some((MOD_NONE, ch as u8 - b'a' + 0x04)),
        'A'..='Z' => Some((MOD_SHIFT, ch as u8 - b'A' + 0x04)),

        // 2. 숫자 (0x1E ~ 0x27)
        '1'..='9' => Some((MOD_NONE, ch as u8 - b'1' + 0x1E)),
        '0' => Some((MOD_NONE, 0x27)),

        // 3. 숫자열 Shift 특수문자 (! @ # $ % ^ & * ( ))
        '!' => Some((MOD_SHIFT, 0x1E)), // Shift + 1
        '@' => Some((MOD_SHIFT, 0x1F)), // Shift + 2
        '#' => Some((MOD_SHIFT, 0x20)), // Shift + 3
        '$' => Some((MOD_SHIFT, 0x21)), // Shift + 4
        '%' => Some((MOD_SHIFT, 0x22)), // Shift + 5
        '^' => Some((MOD_SHIFT, 0x23)), // Shift + 6
        '&' => Some((MOD_SHIFT, 0x24)), // Shift + 7
        '*' => Some((0x25, MOD_SHIFT)), // Shift + 8
        '(' => Some((MOD_SHIFT, 0x26)), // Shift + 9
        ')' => Some((MOD_SHIFT, 0x27)), // Shift + 0

        // 4. 공백 및 제어 문자
        '\n' | '\r' => Some((MOD_NONE, 0x28)), // Enter
        '\x1b' => Some((MOD_NONE, 0x29)),      // Escape
        '\x08' => Some((MOD_NONE, 0x2A)),      // Backspace
        '\t' => Some((MOD_NONE, 0x2B)),        // Tab
        ' ' => Some((MOD_NONE, 0x2C)),         // Space

        // 5. 기호 및 문장부호 (기본 / Shift 조합 쌍)
        '-' => Some((MOD_NONE, 0x2D)),
        '_' => Some((MOD_SHIFT, 0x2D)),
        '=' => Some((MOD_NONE, 0x2E)),
        '+' => Some((MOD_SHIFT, 0x2E)),
        '[' => Some((MOD_NONE, 0x2F)),
        '{' => Some((MOD_SHIFT, 0x2F)),
        ']' => Some((MOD_NONE, 0x30)),
        '}' => Some((MOD_SHIFT, 0x30)),
        '\\' => Some((MOD_NONE, 0x31)),
        '|' => Some((MOD_SHIFT, 0x31)),
        ';' => Some((MOD_NONE, 0x33)),
        ':' => Some((MOD_SHIFT, 0x33)),
        '\'' => Some((MOD_NONE, 0x34)),
        '"' => Some((MOD_SHIFT, 0x34)),
        '`' => Some((MOD_NONE, 0x35)),
        '~' => Some((MOD_SHIFT, 0x35)),
        ',' => Some((MOD_NONE, 0x36)),
        '<' => Some((MOD_SHIFT, 0x36)),
        '.' => Some((MOD_NONE, 0x37)),
        '>' => Some((MOD_SHIFT, 0x37)),
        '/' => Some((MOD_NONE, 0x38)),
        '?' => Some((MOD_SHIFT, 0x38)),

        _ => None,
    }
}

pub fn key_char(ch: char) -> Option<u8> {
    match ch {
        // 1. 알파벳
        'a'..='z' => Some(ch as u8 - b'a' + 0x04),
        'A'..='Z' => Some(ch as u8 - b'A' + 0x04),

        // 2. 숫자 (0x1E ~ 0x27)
        '1'..='9' => Some(ch as u8 - b'1' + 0x1E),
        '0' => Some(0x27),
         _ => None,
    }
}