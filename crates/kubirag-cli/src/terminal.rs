//! `CommandTerminal`: dispatch di una riga, REPL, modalità `-c` e `--script`.

use std::io::{BufRead, Write};

use crate::bootstrap::Notebook;
use crate::commands::{Command, CommandError};
use crate::tokenizer::tokenize;

pub const TERMINAL_NAME: &str = "qbrag";

pub struct Terminal {
    commands: Vec<Box<dyn Command>>,
    /// Indice del comando di default per l'input che non è un comando.
    default_command: usize,
    chat_messages: usize,
}

impl Terminal {
    pub fn new(commands: Vec<Box<dyn Command>>, default_command: &str) -> Self {
        let default_command = commands
            .iter()
            .position(|c| c.name() == default_command)
            .expect("the default command is registered");
        Self {
            commands,
            default_command,
            chat_messages: 0,
        }
    }

    /// `executeLine`: `false` se il comando eseguito non è riuscito.
    pub fn execute_line(
        &mut self,
        line: &str,
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> bool {
        match self.dispatch(line.trim(), notebook, out) {
            Ok(success) => success,
            Err(message) => {
                let _ = writeln!(out, "Error: {message}");
                false
            }
        }
    }

    fn dispatch(
        &mut self,
        line: &str,
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> Result<bool, String> {
        if line.is_empty() {
            return Ok(true);
        }
        let tokens = tokenize(line)?;
        let Some(first) = tokens.first() else {
            return Ok(true);
        };
        if first == "/clear" {
            self.chat_messages = 0;
            return Ok(true);
        }
        if first == "/help" {
            self.print_help(out).map_err(|e| e.to_string())?;
            return Ok(true);
        }
        let Some(index) = self.commands.iter().position(|c| c.name() == first) else {
            if first.starts_with('/') {
                writeln!(out, "Unknown command: {first}").map_err(|e| e.to_string())?;
                return Ok(false);
            }
            let default = &self.commands[self.default_command];
            return finish(default.execute(&tokens, notebook, out), None, out);
        };
        let command = &self.commands[index];
        let help = match tokens.get(1).map(String::as_str) {
            Some("?") => Some(command.help_line()),
            Some("??") => Some(command.help_usage()),
            Some("???") => Some(command.help_example()),
            _ => None,
        };
        if let Some(help) = help {
            writeln!(out, "{help}").map_err(|e| e.to_string())?;
            return Ok(true);
        }
        let result = command.execute(&tokens, notebook, out);
        let printed = finish(result, Some(&mut self.chat_messages), out)?;
        Ok(printed)
    }

    /// Estensione rispetto a Kotlin: l'elenco dei comandi con la loro riga di aiuto.
    fn print_help(&self, out: &mut dyn Write) -> std::io::Result<()> {
        writeln!(
            out,
            "Built-in commands: /quit (exit), /clear (clear chat context)."
        )?;
        for command in &self.commands {
            writeln!(out, "{}  {}", command.name(), command.help_line())?;
        }
        writeln!(
            out,
            "Any other input is a question for /lucene. Append ? ?? ??? to a command for help."
        )
    }

    pub fn prompt(&self) -> String {
        format!("{TERMINAL_NAME}[{}/0]> ", self.chat_messages)
    }

    /// REPL su `input` fino a `/quit` o alla fine dell'input.
    pub fn run(&mut self, input: &mut dyn BufRead, notebook: &mut Notebook, out: &mut dyn Write) {
        loop {
            let _ = write!(out, "{}", self.prompt());
            let _ = out.flush();
            let mut line = String::new();
            match input.read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {}
                Err(error) => {
                    let _ = writeln!(out, "Error: {error}");
                    return;
                }
            }
            let line = line.trim();
            if line == "/quit" {
                break;
            }
            self.execute_line(line, notebook, out);
        }
        let _ = writeln!(out, "Exit.");
    }

    /// `runOnce`: codice di uscita 0 o 1.
    pub fn run_once(&mut self, line: &str, notebook: &mut Notebook, out: &mut dyn Write) -> i32 {
        if line.chars().all(char::is_whitespace) {
            let _ = writeln!(out, "Usage: -c \"/command [args]\"");
            return 1;
        }
        i32::from(!self.execute_line(line, notebook, out))
    }

    /// `runScript`: una riga per comando, righe vuote e `#` saltate, `/quit` termina; si
    /// ferma al primo comando fallito.
    pub fn run_script(
        &mut self,
        script: &str,
        notebook: &mut Notebook,
        out: &mut dyn Write,
    ) -> i32 {
        for raw in kubirag_domain::retrieval::text::kotlin_lines(script) {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line == "/quit" {
                break;
            }
            let _ = writeln!(out, "{TERMINAL_NAME}> {line}");
            if !self.execute_line(line, notebook, out) {
                let _ = writeln!(out, "Script aborted at: {line}");
                return 1;
            }
        }
        0
    }
}

/// `tryExecuteCommand`/`tryExecuteDefaultCommand`: un comando esplicito stampa il messaggio
/// di successo; il comando di default no.
fn finish(
    result: Result<String, CommandError>,
    chat_messages: Option<&mut usize>,
    out: &mut dyn Write,
) -> Result<bool, String> {
    match result {
        Ok(message) => {
            if let Some(count) = chat_messages {
                writeln!(out, "{message}").map_err(|e| e.to_string())?;
                *count += 1;
            }
            Ok(true)
        }
        Err(CommandError::Failure(message)) => {
            if chat_messages.is_some() {
                writeln!(out, "{message}").map_err(|e| e.to_string())?;
            }
            Ok(false)
        }
        Err(CommandError::Exception(message)) => Err(message),
    }
}
