//! Colours and the four ways the CLI prints a line.


use super::*;

pub(crate) fn report_error(m: &str) {
    eprintln!("{}{}{}", RED, m, RESET);
}

pub(crate) fn print_ok(m: &str) {
    println!("{}{}{}", GREEN, m, RESET);
}

pub(crate) fn print_info(m: &str) {
    println!("{}{}{}", CYAN, m, RESET);
}

pub(crate) fn print_dim(m: &str) {
    println!("{}{}{}", DIM, m, RESET);
}

/// A "Did you mean ...?" line under an error.
pub(crate) fn print_hint(text: &str) {
    println!("{}  {}{}", YELLOW, text, RESET);
}
