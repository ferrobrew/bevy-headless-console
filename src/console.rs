use bevy::ecs::{
    component::Tick,
    resource::Resource,
    schedule::IntoScheduleConfigs,
    system::{ScheduleSystem, SystemMeta, SystemParam},
    world::unsafe_world_cell::UnsafeWorldCell,
};
use bevy::prelude::*;
use clap::{builder::StyledStr, CommandFactory, FromArgMatches};
use shlex::Shlex;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::mem;

use crate::ConsoleSet;

type ConsoleCommandEnteredReaderSystemParam =
    MessageReader<'static, 'static, ConsoleCommandEntered>;

type PrintConsoleLineWriterSystemParam = MessageWriter<'static, PrintConsoleLine>;

/// A super-trait for command like structures
pub trait Command: NamedCommand + CommandFactory + FromArgMatches + Sized + Resource {}
impl<T: NamedCommand + CommandFactory + FromArgMatches + Sized + Resource> Command for T {}

/// Trait used to allow uniquely identifying commands at compile time
pub trait NamedCommand {
    /// Return the unique command identifier (same as the command "executable")
    fn name() -> &'static str;
}

/// Executed parsed console command.
///
/// Used to capture console commands which implement [`CommandName`], [`CommandArgs`] & [`CommandHelp`].
/// These can be easily implemented with the [`ConsoleCommand`](bevy_headless_console_derive::ConsoleCommand) derive macro.
///
/// # Example
///
/// ```
/// # use bevy_headless_console::ConsoleCommand;
/// # use clap::Parser;
/// /// Prints given arguments to the console.
/// #[derive(Parser, ConsoleCommand)]
/// #[command(name = "log")]
/// struct LogCommand {
///     /// Message to print
///     msg: String,
///     /// Number of times to print message
///     num: Option<i64>,
/// }
///
/// fn log_command(mut log: ConsoleCommand<LogCommand>) {
///     if let Some(Ok(LogCommand { msg, num })) = log.take() {
///         log.ok();
///     }
/// }
/// ```
pub struct ConsoleCommand<'w, T> {
    command: Option<Result<T, clap::Error>>,
    console_line: MessageWriter<'w, PrintConsoleLine>,
}

impl<'w, T> ConsoleCommand<'w, T> {
    /// Returns Some(T) if the command was executed and arguments were valid.
    ///
    /// This method should only be called once.
    /// Consecutive calls will return None regardless if the command occurred.
    pub fn take(&mut self) -> Option<Result<T, clap::Error>> {
        mem::take(&mut self.command)
    }

    /// Print `[ok]` in the console.
    pub fn ok(&mut self) {
        self.console_line
            .write(PrintConsoleLine::new("[ok]".into()));
    }

    /// Print `[failed]` in the console.
    pub fn failed(&mut self) {
        self.console_line
            .write(PrintConsoleLine::new("[failed]".into()));
    }

    /// Print a reply in the console.
    ///
    /// See [`reply!`](crate::reply) for usage with the [`format!`] syntax.
    pub fn reply(&mut self, msg: impl Into<StyledStr>) {
        self.console_line.write(PrintConsoleLine::new(msg.into()));
    }

    /// Print a reply in the console followed by `[ok]`.
    ///
    /// See [`reply_ok!`](crate::reply_ok) for usage with the [`format!`] syntax.
    pub fn reply_ok(&mut self, msg: impl Into<StyledStr>) {
        self.console_line.write(PrintConsoleLine::new(msg.into()));
        self.ok();
    }

    /// Print a reply in the console followed by `[failed]`.
    ///
    /// See [`reply_failed!`](crate::reply_failed) for usage with the [`format!`] syntax.
    pub fn reply_failed(&mut self, msg: impl Into<StyledStr>) {
        self.console_line.write(PrintConsoleLine::new(msg.into()));
        self.failed();
    }
}

pub struct ConsoleCommandState<T> {
    #[allow(clippy::type_complexity)]
    event_reader: <ConsoleCommandEnteredReaderSystemParam as SystemParam>::State,
    console_line: <PrintConsoleLineWriterSystemParam as SystemParam>::State,
    marker: PhantomData<T>,
}

unsafe impl<T: Command> SystemParam for ConsoleCommand<'_, T> {
    type State = ConsoleCommandState<T>;
    type Item<'w, 's> = ConsoleCommand<'w, T>;

    fn init_state(world: &mut World) -> Self::State {
        let event_reader = ConsoleCommandEnteredReaderSystemParam::init_state(world);
        let console_line = PrintConsoleLineWriterSystemParam::init_state(world);
        ConsoleCommandState {
            event_reader,
            console_line,
            marker: PhantomData,
        }
    }

    fn init_access(
        state: &Self::State,
        system_meta: &mut SystemMeta,
        component_access_set: &mut bevy::ecs::query::FilteredAccessSet,
        world: &mut World,
    ) {
        ConsoleCommandEnteredReaderSystemParam::init_access(
            &state.event_reader,
            system_meta,
            component_access_set,
            world,
        );
        PrintConsoleLineWriterSystemParam::init_access(
            &state.console_line,
            system_meta,
            component_access_set,
            world,
        );
    }

    #[inline]
    unsafe fn get_param<'w, 's>(
        state: &'s mut Self::State,
        system_meta: &SystemMeta,
        world: UnsafeWorldCell<'w>,
        change_tick: Tick,
    ) -> Self::Item<'w, 's> {
        let mut event_reader = ConsoleCommandEnteredReaderSystemParam::get_param(
            &mut state.event_reader,
            system_meta,
            world,
            change_tick,
        );
        let mut console_line = PrintConsoleLineWriterSystemParam::get_param(
            &mut state.console_line,
            system_meta,
            world,
            change_tick,
        );

        let command = event_reader.read().find_map(|command| {
            if T::name() == command.command_name {
                let clap_command = T::command().no_binary_name(true);
                // .color(clap::ColorChoice::Always);
                let arg_matches = clap_command.try_get_matches_from(command.args.iter());

                match arg_matches {
                    Ok(matches) => {
                        return Some(T::from_arg_matches(&matches));
                    }
                    Err(err) => {
                        console_line.write(PrintConsoleLine::new(err.render()));
                        return Some(Err(err));
                    }
                }
            }
            None
        });

        ConsoleCommand {
            command,
            console_line,
        }
    }
}
/// Raw console command entered; will be parsed into `ConsoleCommandEntered`.
#[derive(Clone, Debug, Message)]
pub struct ConsoleCommandRawEntered(pub String);

/// Parsed raw console command into `command` and `args`.
#[derive(Clone, Debug, Message)]
pub struct ConsoleCommandEntered {
    /// the command definition
    pub command_name: String,
    /// Raw parsed arguments
    pub args: Vec<String>,
}

/// Messages to print to the console.
#[derive(Clone, Debug, Eq, Message, PartialEq)]
pub struct PrintConsoleLine {
    /// Console line
    pub line: StyledStr,
}

impl PrintConsoleLine {
    /// Creates a new console line to print.
    pub const fn new(line: StyledStr) -> Self {
        Self { line }
    }
}

/// Console configuration
#[derive(Clone, Resource)]
pub struct ConsoleConfiguration {
    /// Registered console commands
    pub commands: BTreeMap<&'static str, clap::Command>,
    /// Number of commands to store in history
    pub history_size: usize,
}

impl Default for ConsoleConfiguration {
    fn default() -> Self {
        Self {
            commands: BTreeMap::new(),
            history_size: 20,
        }
    }
}

/// Add a console commands to Bevy app.
pub trait AddConsoleCommand {
    /// Add a console command with a given system.
    ///
    /// This registers the console command so it will print with the built-in `help` console command.
    ///
    /// # Example
    ///
    /// ```
    /// # use bevy::prelude::*;
    /// # use bevy_headless_console::{AddConsoleCommand, ConsoleCommand};
    /// # use clap::Parser;
    /// App::new()
    ///     .add_console_command::<LogCommand, _>(log_command);
    /// #
    /// # /// Prints given arguments to the console.
    /// # #[derive(Parser, ConsoleCommand)]
    /// # #[command(name = "log")]
    /// # struct LogCommand;
    /// #
    /// # fn log_command(mut log: ConsoleCommand<LogCommand>) {}
    /// ```
    fn add_console_command<T: Command, M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self;
}

impl AddConsoleCommand for App {
    fn add_console_command<T: Command, M>(
        &mut self,
        system: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        let sys = move |mut config: ResMut<ConsoleConfiguration>| {
            let command = T::command().no_binary_name(true);
            // .color(clap::ColorChoice::Always);
            let name = T::name();
            if config.commands.contains_key(name) {
                eprintln!(
                    "warning: console command '{}' already registered and was overwritten",
                    name
                );
            }
            config.commands.insert(name, command);
        };

        self.add_systems(Startup, sys)
            .add_systems(Update, system.in_set(ConsoleSet::Commands))
    }
}

pub(crate) fn parse_raw_commands(
    config: Res<ConsoleConfiguration>,
    mut raw_commands_entered: MessageReader<ConsoleCommandRawEntered>,
    mut command_entered: MessageWriter<ConsoleCommandEntered>,
    mut output_console_lines: MessageWriter<PrintConsoleLine>,
) {
    for raw_command in raw_commands_entered.read() {
        let mut args = Shlex::new(&raw_command.0).collect::<Vec<_>>();

        if !args.is_empty() {
            let command_name = args.remove(0);

            let command = config.commands.get(command_name.as_str());

            if command.is_some() {
                command_entered.write(ConsoleCommandEntered { command_name, args });
            } else {
                output_console_lines.write(PrintConsoleLine::new(
                    format!("Command not recognized: `{command_name}`").into(),
                ));
            }
        }
    }
}
