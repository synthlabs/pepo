#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use std::process;

use tauri::Manager;
use tauri::Window;
use tauri::WindowEvent;
use twitchchat::{connector, messages, runner::AsyncRunner, Status, UserConfig};

async fn connect(user_config: &UserConfig, channels: &[String]) -> anyhow::Result<AsyncRunner> {
    // create a connector using ``smol``, this connects to Twitch.
    // you can provide a different address with `custom`
    // this can fail if DNS resolution cannot happen
    let connector = connector::smol::Connector::twitch()?;

    println!("we're connecting!");
    // create a new runner. this is a provided async 'main loop'
    // this method will block until you're ready
    let mut runner = AsyncRunner::connect(connector, user_config).await?;
    println!("..and we're connected");

    // and the identity Twitch gave you
    println!("our identity: {:#?}", runner.identity);

    for channel in channels {
        // the runner itself has 'blocking' join/part to ensure you join/leave a channel.
        // these two methods return whether the connection was closed early.
        // we'll ignore it for this demo
        println!("attempting to join '{}'", channel);
        let _ = runner.join(channel).await?;
        println!("joined '{}'!", channel);
    }

    Ok(runner)
}

// the payload type must implement `Serialize` and `Clone`.
#[derive(Clone, serde::Serialize)]
struct Payload {
    message: String,
}

// you can generally ignore the lifetime for these types.
async fn handle_message(msg: messages::Commands<'_>) {
    use messages::Commands::*;
    // All sorts of messages
    match msg {
        // This is the one users send to channels
        Privmsg(msg) => println!("[{}] {}: {}", msg.channel(), msg.name(), msg.data()),

        // This one is special, if twitch adds any new message
        // types, this will catch it until future releases of
        // this crate add them.
        Raw(_) => {}

        // These happen when you initially connect
        IrcReady(_) => {}
        Ready(_) => {}
        Cap(_) => {}

        // and a bunch of other messages you may be interested in
        ClearChat(_) => {}
        ClearMsg(_) => {}
        GlobalUserState(_) => {}
        HostTarget(_) => {}
        Join(_) => {}
        Notice(_) => {}
        Part(_) => {}
        Ping(_) => {}
        Pong(_) => {}
        Reconnect(_) => {}
        RoomState(_) => {}
        UserNotice(_) => {}
        UserState(_) => {}
        Whisper(_) => {}

        _ => {}
    }
}

// a 'main loop'
pub async fn main_loop(mut runner: AsyncRunner) -> anyhow::Result<()> {
    loop {
        match runner.next_message().await? {
            // this is the parsed message -- across all channels (and notifications from Twitch)
            Status::Message(msg) => {
                handle_message(msg).await;
            }

            // you signaled a quit
            Status::Quit => {
                println!("we signaled we wanted to quit");
                break;
            }
            // the connection closed normally
            Status::Eof => {
                println!("we got a 'normal' eof");
                break;
            }
        }
    }

    Ok(())
}

fn main() {
    pretty_env_logger::formatted_builder()
        .parse_filters(
            std::env::var("RUST_LOG")
                .unwrap_or("DEBUG".to_string())
                .as_str(),
        )
        .init();

    smol::block_on(async {
        let config = UserConfig::builder().anonymous().build().unwrap();

        let channels = vec![String::from("#carter")];

        // connect and join the provided channels
        let runner = match connect(&config, &channels).await {
            Ok(runner) => runner,
            Err(err) => {
                error!("failed to connect: {}", err);
                process::exit(-1)
            }
        };

        // you can get a handle to shutdown the runner
        let _quit_handle = runner.quit_handle();

        println!("starting main loop");
        // your 'main loop'. you'll just call next_message() until you're done
        smol::spawn(main_loop(runner)).detach();

        tauri::Builder::default()
            .invoke_handler(tauri::generate_handler![])
            .on_window_event(|event| match event.event() {
                WindowEvent::CloseRequested { api, .. } => println!("we be closing"),
                _ => println!("{:?}", event.event()),
            })
            .run(tauri::generate_context!())
            .expect("error while running tauri application");
    })
}
