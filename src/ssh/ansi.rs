const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";

pub fn badge(text: &str) -> String {
    format!("\x1b[1;97;46m {text} {RESET}")
}

pub fn bold(text: &str) -> String {
    format!("{BOLD}{text}{RESET}")
}

pub fn cyan(text: &str) -> String {
    format!("\x1b[36m{text}{RESET}")
}

pub fn bold_underline(text: &str) -> String {
    format!("\x1b[1;4m{text}{RESET}")
}

pub fn yellow(text: &str) -> String {
    format!("\x1b[33m{text}{RESET}")
}

pub fn green_bold(text: &str) -> String {
    format!("\x1b[1;32m{text}{RESET}")
}

pub fn red_bold(text: &str) -> String {
    format!("\x1b[1;31m{text}{RESET}")
}
