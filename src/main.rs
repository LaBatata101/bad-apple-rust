use std::{
    fs,
    io::Write,
    sync::mpsc,
    thread::{self, JoinHandle},
    time::Duration,
};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer_player as gst_player;

#[derive(Debug)]
enum Command {
    Play(String),
}

#[derive(Clone)]
struct PlayerController {
    player: gst_player::Player,
    mainloop: gst::glib::MainLoop,
    is_paused: bool,
    is_stoped: bool,
}

impl PlayerController {
    fn new() -> Self {
        let (mainloop, player) = Self::init_player();

        let mainloop_clone = mainloop.clone();

        // Connect to the player's "end-of-stream" signal, which will tell us when the
        // currently played media stream reached its end.
        player.connect_end_of_stream(move |player| {
            player.stop();
            mainloop_clone.quit();
        });

        player.connect_error(|player, error| {
            dbg!(error);
            player.stop();
        });

        Self {
            player,
            mainloop,
            is_paused: false,
            is_stoped: true,
        }
    }

    fn play(&mut self, uri: &str) -> Option<JoinHandle<()>> {
        let mut play_thread = None;
        let current_uri = self
            .player
            .uri()
            .unwrap_or_else(|| gst::glib::GString::from(""))
            .to_string();

        if self.is_paused && uri == current_uri {
            self.resume();
        } else if self.is_stoped || current_uri != uri {
            self.is_paused = false;
            self.is_stoped = false;

            self.player.set_uri(uri);

            let player = self.player.clone();
            let mainloop = self.mainloop.clone();

            play_thread = Some(thread::spawn(move || {
                player.play();
                mainloop.run();
            }));
        }

        play_thread
    }

    fn resume(&mut self) {
        if self.is_paused {
            self.is_paused = false;

            self.player.play()
        }
    }

    fn init_player() -> (gst::glib::MainLoop, gst_player::Player) {
        gst::init().expect("Failed gstreamer init!");

        let mainloop = gst::glib::MainLoop::new(None, false);

        let dispatcher = gst_player::PlayerGMainContextSignalDispatcher::new(None);
        let player = gst_player::Player::new(None, Some(&dispatcher.upcast::<gst_player::PlayerSignalDispatcher>()));

        (mainloop, player)
    }
}

struct PlayerDaemon {
    sender: mpsc::Sender<Command>,
}

impl PlayerDaemon {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();

        Self::init_daemon(receiver);

        Self { sender }
    }

    fn init_daemon(receiver: mpsc::Receiver<Command>) {
        let mut player = PlayerController::new();

        thread::spawn(move || loop {
            let command = receiver.recv().unwrap();

            match command {
                Command::Play(uri) => {
                    player.play(&uri);
                }
            }
        });
    }

    fn play(&self, uri: String) {
        self.sender.send(Command::Play(uri)).unwrap();
    }
}

fn main() {
    let src = fs::read_to_string("bad-apple.txt").unwrap().replace(".", " ");
    let frames: Vec<&str> = src.split("SPLIT").collect();

    let abs_path = std::fs::canonicalize("BadApple.m4a").expect("File not found!");
    let player_daemon = PlayerDaemon::new();
    player_daemon.play(format!("file://{}", abs_path.display()));

    for frame in frames {
        print!("{}\r", frame);
        std::io::stdout().flush().unwrap();
        thread::sleep(Duration::from_millis(41));
    }
}
