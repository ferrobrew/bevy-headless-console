#![doc = include_str ! ("../README.md")]
#![deny(missing_docs)]

use bevy::prelude::*;
pub use bevy_headless_console_derive::ConsoleCommand;

use crate::commands::exit::{exit_command, ExitCommand};
use crate::commands::help::{help_command, HelpCommand};
pub use crate::console::{
    AddConsoleCommand, Command, ConsoleCommand, ConsoleCommandEntered, ConsoleCommandRawEntered,
    ConsoleConfiguration, NamedCommand, PrintConsoleLine,
};
// pub use color::{Style, StyledStr};

use console::parse_raw_commands;

pub use clap;

// mod color;
mod commands;
mod console;
mod macros;

mod basic_terminal;
pub use basic_terminal::BasicTerminalPlugin;

/// Console plugin.
pub struct ConsolePlugin;

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
/// The SystemSet for console/command related systems
pub enum ConsoleSet {
    /// Systems operating the console input (the input layer)
    ConsoleInput,

    /// Systems executing console commands (the functionality layer).
    /// All command handler systems are added to this set
    Commands,

    /// Systems running after command systems, which depend on the fact commands have executed beforehand (the output layer).
    /// For example a system which makes use of [`PrintConsoleLine`] events should be placed in this set to be able to receive
    /// New lines to print in the same frame
    PostCommands,
}

/// Run condition which does not run any command systems if no command was entered
fn have_commands(commands: EventReader<ConsoleCommandEntered>) -> bool {
    !commands.is_empty()
}

impl Plugin for ConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ConsoleConfiguration>()
            .add_event::<ConsoleCommandRawEntered>()
            .add_event::<ConsoleCommandEntered>()
            .add_event::<PrintConsoleLine>()
            .add_console_command::<ExitCommand, _>(exit_command)
            .add_console_command::<HelpCommand, _>(help_command)
            .add_systems(Update, parse_raw_commands.in_set(ConsoleSet::ConsoleInput))
            .configure_sets(
                Update,
                (
                    ConsoleSet::Commands
                        .after(ConsoleSet::ConsoleInput)
                        .run_if(have_commands),
                    ConsoleSet::PostCommands.after(ConsoleSet::Commands),
                ),
            );
    }
}
