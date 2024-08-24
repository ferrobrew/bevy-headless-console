use std::{io::Write, sync::mpsc, thread::JoinHandle};

use bevy::prelude::*;

use crate::{ConsoleCommandRawEntered, ConsoleSet, PrintConsoleLine};

/// Very basic stdin/stdout terminal using another thread to process input.
/// No pretty formatting, history, or search: this is primarily for testing.
pub struct BasicTerminalPlugin;
impl Plugin for BasicTerminalPlugin {
    fn build(&self, app: &mut App) {
        let (tx, rx) = mpsc::channel();
        app.insert_non_send_resource(StdinLineReceiver(rx))
            .insert_non_send_resource(StdinThread {
                _handle: std::thread::spawn(move || {
                    let mut input = String::new();
                    loop {
                        input.clear();
                        std::io::stdin().read_line(&mut input).unwrap();
                        tx.send(input.trim().to_string()).unwrap();
                    }
                }),
            })
            .add_systems(Startup, print_caret)
            .add_systems(
                Update,
                convert_stdin_to_events.in_set(ConsoleSet::ConsoleInput),
            )
            .add_systems(Update, output_console_lines.after(ConsoleSet::PostCommands));
    }
}

struct StdinThread {
    _handle: JoinHandle<()>,
}
struct StdinLineReceiver(mpsc::Receiver<String>);

fn convert_stdin_to_events(
    receiver: NonSend<StdinLineReceiver>,
    mut command_raw_entered: EventWriter<ConsoleCommandRawEntered>,
) {
    for line in receiver.0.try_iter() {
        command_raw_entered.send(ConsoleCommandRawEntered(line));
    }
}

fn output_console_lines(mut reader: EventReader<PrintConsoleLine>) {
    let mut handled = false;
    for line in reader.read() {
        println!("{}", line.line);
        handled = true;
    }
    if handled {
        print_caret();
    }
}

fn print_caret() {
    print!("> ");
    std::io::stdout().flush().unwrap();
}
