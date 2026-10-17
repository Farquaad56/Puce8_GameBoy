//! Emulation thread: owns the core and runs it at the exact speed with an integer accumulator (E07_02).
use crate::pacing::{clamp_owed, Pacer};
use puce8gb_core::machine::FRAME_DOTS;
use puce8gb_core::{Dmg, Input, Machine};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Commands sent from the UI to the emulation thread.
pub enum Command {
    Load(Vec<u8>),
    Reset,
    Pause(bool),
    FrameStep,
    Input(Input),
    Quit,
}

/// UI side of the emulation thread.
pub struct EmuHandle {
    commands: Sender<Command>,
    /// Finished frames (0xAARRGGBB, 160 x 144); the thread drops frames when this is full.
    pub frames: Receiver<Vec<u32>>,
    /// Human readable status messages from the thread.
    pub status: Receiver<String>,
    thread: Option<JoinHandle<()>>,
}

impl EmuHandle {
    pub fn spawn() -> Self {
        let (commands, command_rx) = channel();
        let (frame_tx, frames) = sync_channel(2);
        let (status_tx, status) = channel();
        let thread = std::thread::spawn(move || run(command_rx, frame_tx, status_tx));
        EmuHandle {
            commands,
            frames,
            status,
            thread: Some(thread),
        }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

impl Drop for EmuHandle {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn run(commands: Receiver<Command>, frames: SyncSender<Vec<u32>>, status: Sender<String>) {
    let mut machine: Option<Dmg> = None;
    let mut paused = false;
    let mut steps: u32 = 0;
    let mut pacer = Pacer::default();
    let mut owed: u64 = 0;
    let mut last = Instant::now();
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Load(bytes)) => match Dmg::new(&bytes) {
                    Ok(dmg) => {
                        machine = Some(dmg);
                        owed = 0;
                        let _ = status.send(String::from("ROM loaded"));
                    }
                    Err(err) => {
                        let _ = status.send(format!("ROM rejected: {err:?}"));
                    }
                },
                Ok(Command::Reset) => {
                    if let Some(m) = machine.as_mut() {
                        m.reset();
                        owed = 0;
                    }
                }
                Ok(Command::Pause(p)) => {
                    paused = p;
                    owed = 0;
                    steps = 0;
                }
                Ok(Command::FrameStep) => steps += 1,
                Ok(Command::Input(input)) => {
                    if let Some(m) = machine.as_mut() {
                        m.set_input(input);
                    }
                }
                Ok(Command::Quit) | Err(TryRecvError::Disconnected) => return,
                Err(TryRecvError::Empty) => break,
            }
        }
        let now = Instant::now();
        let elapsed_ns = u64::try_from(now.duration_since(last).as_nanos()).unwrap_or(u64::MAX);
        last = now;
        if let Some(m) = machine.as_mut() {
            let mut produced = false;
            if paused {
                while steps > 0 {
                    m.run_frame();
                    steps -= 1;
                    produced = true;
                }
            } else {
                steps = 0;
                owed = clamp_owed(owed + pacer.cycles_to_run(elapsed_ns));
                while owed >= u64::from(FRAME_DOTS) {
                    m.run_frame();
                    owed -= u64::from(FRAME_DOTS);
                    produced = true;
                }
            }
            if produced {
                let _ = frames.try_send(m.framebuffer().to_vec()); // full channel: drop the frame
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
