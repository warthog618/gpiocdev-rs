// SPDX-FileCopyrightText: 2021 Kent Gibson <warthog618@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::unquoted;
use anyhow::anyhow;
use anyhow::Result;
use rustyline::completion::{Completer, Pair};
use rustyline::config::CompletionType;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};

pub(super) struct Editor {
    rl: rustyline::Editor<InteractiveHelper, FileHistory>,
    prompt: String,
}
impl Editor {
    pub(super) fn new(line_names: Vec<String>, prompt: &str) -> Result<Editor> {
        let helper = InteractiveHelper { line_names };
        let config = rustyline::Config::builder()
            .completion_type(CompletionType::List)
            .auto_add_history(true)
            .max_history_size(20)?
            .history_ignore_space(true)
            .build();
        let mut rl = rustyline::Editor::with_config(config)?;
        rl.set_helper(Some(helper));
        Ok(Editor { rl, prompt: prompt.to_string() })
    }

    pub(super) fn next_line(&mut self) -> Result<String> {
        use std::io::Write;
        let mut stdout = std::io::stdout();
        /*
         * manually print the prompt, as rustyline doesn't if stdout
         * is not a tty? And flush to ensure the prompt and any
         * output buffered from the previous command is sent.
         */
        _ = stdout.write(self.prompt.as_bytes());
        _ = stdout.flush();
        match self.rl.readline(self.prompt.as_str()) {
            Ok(line) => Ok(line),
            Err(ReadlineError::Interrupted) => Ok("exit".to_string()),
            Err(ReadlineError::Eof) => Ok("".to_string()),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[derive(Helper, Validator, Hinter, Highlighter)]
pub(super) struct InteractiveHelper {
    pub(super) line_names: Vec<String>,
}

impl InteractiveHelper {
    fn complete_set(&self, mut pos: usize, mut words: CommandWords) -> (usize, Vec<Pair>) {
        let mut candidates = Vec::new();
        let mut line_values = Vec::new();
        while let Some(word) = &words.next() {
            line_values.push(*word);
        }
        let selected: Vec<&'_ str> = line_values
            .iter()
            .filter(|lv| lv.contains('='))
            .map(|lv| unquoted(&lv[..lv.find('=').unwrap()]))
            .collect();
        let unselected = self
            .line_names
            .iter()
            .filter(|l| !selected.contains(&l.as_str()));
        if !words.partial {
            for line in unselected {
                candidates.push(line_value_pair(line));
            }
            return (pos, candidates);
        }
        let mut part_word = *line_values.last().unwrap();
        match part_word.split_once('=') {
            Some((_, part_value)) => {
                const VALUES: [&str; 8] =
                    ["active", "inactive", "on", "off", "true", "false", "1", "0"];
                pos -= part_value.len();
                for value in VALUES.iter().filter(|v| v.starts_with(part_value)) {
                    candidates.push(base_pair(value))
                }
            }
            None => {
                pos -= part_word.len();
                part_word = unquoted(part_word);
                if part_word.starts_with('"') {
                    part_word = &part_word[1..];
                }
                for line in unselected.filter(|l| l.starts_with(part_word)) {
                    candidates.push(line_value_pair(line))
                }
            }
        }
        (pos, candidates)
    }

    fn complete_sleep(&self, mut pos: usize, mut words: CommandWords) -> (usize, Vec<Pair>) {
        const UNITS: [&str; 4] = ["s", "ms", "us", "ns"];
        let mut candidates = Vec::new();
        let mut times = Vec::new();
        while let Some(word) = &words.next() {
            times.push(*word);
        }
        if words.partial && times.len() == 1 {
            let t = &times[0];
            match t.find(|c: char| !c.is_ascii_digit()) {
                Some(n) => {
                    let (_num, units) = t.split_at(n);
                    pos -= units.len();
                    for display in UNITS.iter().filter(|u| u.starts_with(units)) {
                        candidates.push(base_pair(display))
                    }
                }
                None => {
                    for display in UNITS.iter() {
                        candidates.push(base_pair(display))
                    }
                }
            }
        }
        (pos, candidates)
    }

    fn complete_lines(&self, pos: usize, mut words: CommandWords) -> (usize, Vec<Pair>) {
        let mut selected = Vec::new();
        while let Some(word) = &words.next() {
            selected.push(unquoted(word));
        }
        let unselected = self
            .line_names
            .iter()
            .filter(|l| !selected.contains(&l.as_str()));
        if !words.partial {
            let candidates = unselected.map(|l| line_pair(l)).collect();
            return (pos, candidates);
        }
        let mut part_word = *selected.last().unwrap();
        let lpos = pos - part_word.len();
        if part_word.starts_with('"') {
            part_word = &part_word[1..];
        }
        let candidates = unselected
            .filter(|l| l.starts_with(part_word))
            .map(|l| line_pair(l))
            .collect();
        (lpos, candidates)
    }
}

impl Completer for InteractiveHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> Result<(usize, Vec<Pair>), ReadlineError> {
        const CMD_SET: [&str; 6] = ["exit", "get", "help", "set", "sleep", "toggle"];
        let cmd_pos = line.len() - line.trim_start().len();
        let mut words = CommandWords::new(&line[cmd_pos..pos]);
        Ok(match words.next() {
            Some(cmd) => {
                if words.partial {
                    let mut candidates = Vec::new();
                    for display in CMD_SET.iter().filter(|x| x.starts_with(cmd)) {
                        candidates.push(base_pair(display))
                    }
                    (cmd_pos, candidates)
                } else {
                    match cmd {
                        "get" => self.complete_lines(pos, words),
                        "set" => self.complete_set(pos, words),
                        "sleep" => self.complete_sleep(pos, words),
                        "toggle" => self.complete_lines(pos, words),
                        _ => (cmd_pos, vec![]),
                    }
                }
            }
            None => {
                let mut candidates = Vec::new();
                for display in CMD_SET.iter() {
                    candidates.push(base_pair(display))
                }
                (0, candidates)
            }
        })
    }
}

// a pair that ends a command word
fn base_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = display.clone();
    replacement.push(' ');
    Pair {
        display,
        replacement,
    }
}

fn quotable(line: &str) -> String {
    // force quotes iff necessary
    if line.contains(' ') {
        let mut quoted = "\"".to_string();
        quoted.push_str(line);
        quoted.push('"');
        quoted
    } else {
        line.to_string()
    }
}

// a pair that contains a line name - that may need to be quoted.
fn line_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = quotable(candidate);
    replacement.push(' ');
    Pair {
        display,
        replacement,
    }
}

// a pair that contains a line name - that may need to be quoted,
// and will be followed by a value.
fn line_value_pair(candidate: &str) -> Pair {
    let display = String::from(candidate);
    let mut replacement = quotable(candidate);
    replacement.push('=');
    Pair {
        display,
        replacement,
    }
}

pub(super) struct CommandWords<'a> {
    line: &'a str,
    liter: std::str::CharIndices<'a>,
    pub(super) inquote: bool,
    partial: bool,
}

impl CommandWords<'_> {
    pub fn new(line: &str) -> CommandWords {
        CommandWords {
            line,
            liter: line.char_indices(),
            inquote: false,
            partial: false,
        }
    }
}

impl<'a> Iterator for CommandWords<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let start;
        loop {
            match self.liter.next() {
                Some((_, ' ')) => {}
                Some((pos, c)) => {
                    start = pos;
                    if c == '"' {
                        self.inquote = true;
                    }
                    break;
                }
                None => return None,
            }
        }
        loop {
            match self.liter.next() {
                Some((_, '"')) if self.inquote => self.inquote = false,
                Some((_, '"')) if !self.inquote => self.inquote = true,
                Some((pos, ' ')) if !self.inquote => {
                    return Some(&self.line[start..pos]);
                }
                Some((_, _)) => {}
                None => {
                    self.partial = true;
                    return Some(&self.line[start..]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod command_words {
        use super::CommandWords;
        #[test]
        fn whole_words() {
            let mut words = CommandWords::new("basic command line");
            let mut word = words.next().unwrap();
            assert_eq!(word, "basic");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "command");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "line");
            assert!(words.partial);
            assert!(!words.inquote);
            assert_eq!(words.next(), None);
            assert!(words.partial);
            assert!(!words.inquote);
        }

        #[test]
        fn quoted_words() {
            let mut words = CommandWords::new("quoted \"command lines\" \"are awful");
            let mut word = words.next().unwrap();
            assert_eq!(word, "quoted");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "\"command lines\"");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "\"are awful");
            assert!(words.partial);
            assert!(words.inquote);
            assert_eq!(words.next(), None);
            assert!(words.partial);
            assert!(words.inquote);
        }

        #[test]
        fn quotes_mid_words() {
            let mut words = CommandWords::new("quoted \"comm\"and\" lines\" \"are awful");
            let mut word = words.next().unwrap();
            assert_eq!(word, "quoted");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "\"comm\"and\" lines\"");
            assert!(!words.partial);
            assert!(!words.inquote);
            word = words.next().unwrap();
            assert_eq!(word, "\"are awful");
            assert!(words.partial);
            assert!(words.inquote);
            assert_eq!(words.next(), None);
            assert!(words.partial);
            assert!(words.inquote);
        }
    }
}
