use std::fmt::Display;

use crate::GbGen;

impl GbGen {
    pub fn include_hardware(&mut self) {
        self.main_code.push(String::from("INCLUDE\"hardware.inc\""));
    }

    pub fn def<T: Display>(&mut self, label: &str, value: T) {
        // Convert the value to a string representation for the constant
        let value_str = format!("{}", value);
        // Parse the string to u16 for the constants map
        let value_u16: u16 = value_str.parse().expect("Failed to parse value as u16");
        self.constants.insert(label.to_string(), value_u16);
        self.main_code
            .push(format!("DEF {} EQU {}", label, value_str));
    }

    pub fn section(&mut self, name: &str, mem_sec: &str) {
        self.main_code
            .push(format!("SECTION \"{}\", {}", name, mem_sec));
    }

    pub fn label(&mut self, name: &str) {
        self.labels.push(name.to_string());
        self.main_code.push(format!("{}:", name));
    }

    pub fn raw(&mut self, line: &str) {
        self.main_code.push(line.to_string());
    }

    pub fn comment(&mut self, text: &str) {
        self.main_code.push(format!("; {}", text));
    }

    pub fn db(&mut self, values: &str) {
        self.main_code.push(format!("\tdb {}", values));
    }

    pub fn dw(&mut self, value: &str) {
        self.main_code.push(format!("\tdw {}", value));
    }

    pub fn output(&self) -> String {
        self.main_code.join("\n")
    }
}
