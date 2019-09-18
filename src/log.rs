use std::io;

use colored::*;

#[derive(Copy, Clone)]
pub enum Level {
    DEBUG,
    INFO,
    WARNING,
    ERROR,
}

pub struct Logger {
    level: Level,
    color: bool,
    handle: Box<dyn io::Write>,
}

impl Logger {
    pub fn default(verbosity: usize, color: bool) -> Self {
        let level = match verbosity {
            0 => Level::WARNING,
            1 => Level::INFO,
            _ => Level::DEBUG,
        };
        let handle = Box::new(io::stdout());
        Self {
            level,
            color,
            handle,
        }
    }

    fn log(
        &mut self,
        level: Level,
        prelude: &str,
        msg: &str,
        color: &str,
    ) -> Result<(), io::Error> {
        if (level as i32) >= (self.level as i32) {
            if self.color {
                write!(
                    self.handle,
                    "{}{}\n",
                    prelude.color(color).bold(),
                    msg.color(color)
                )?;
            } else {
                write!(self.handle, "{}{}\n", prelude, msg)?;
            }
        }
        Ok(())
    }

    pub fn debug(&mut self, msg: &str) -> Result<(), io::Error> {
        self.log(Level::DEBUG, "DEBU: ", msg, "cyan")
    }

    pub fn info(&mut self, msg: &str) -> Result<(), io::Error> {
        self.log(Level::INFO, "INFO: ", msg, "green")
    }

    pub fn warn(&mut self, msg: &str) -> Result<(), io::Error> {
        self.log(Level::WARNING, "WARN: ", msg, "yellow")
    }

    pub fn error(&mut self, msg: &str) -> Result<(), io::Error> {
        self.log(Level::ERROR, "ERRO: ", msg, "red")
    }
}
