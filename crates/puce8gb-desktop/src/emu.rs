//! Emulation thread: owns the core, paced by the audio ring when a device is open, else by the clock (E07_02, E07_03).
use crate::audio::{pacing_decision, push_budget, target_fill, AudioOut, Pace};
use crate::pacing::{clamp_owed, Pacer, MAX_BACKLOG_FRAMES};
use crate::resample::Resampler;
use puce8gb_core::machine::{AUDIO_RING_SAMPLES, FRAME_DOTS};
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

/// Audio side of the thread: core samples, resampler, lock-free ring, cpal callback.
struct Sound {
    out: AudioOut,
    resampler: Resampler,
    target: usize,
    drain: Vec<i16>,
    mixed: Vec<[f32; 2]>,
}

impl Sound {
    fn open() -> Option<Sound> {
        let out = AudioOut::open()?;
        let rate = out.rate;
        Some(Sound {
            out,
            resampler: Resampler::new(rate),
            target: target_fill(rate),
            drain: vec![0; AUDIO_RING_SAMPLES],
            mixed: Vec::new(),
        })
    }

    fn wants_frame(&self) -> bool {
        pacing_decision(self.out.fill(), self.target) == Pace::RunFrame
    }

    /// Moves the samples of the frame just run into the ring; overflow drops the excess.
    fn collect(&mut self, machine: &mut Dmg) {
        let count = machine.drain_audio(&mut self.drain).min(self.drain.len());
        self.mixed.clear();
        for pair in self.drain[..count].as_chunks::<2>().0 {
            self.resampler.push(*pair, &mut self.mixed);
        }
        let budget = push_budget(self.out.free(), self.mixed.len());
        for pair in self.mixed.iter().take(budget) {
            let _ = self.out.producer.push(*pair);
        }
    }
}

fn run(commands: Receiver<Command>, frames: SyncSender<Vec<u32>>, status: Sender<String>) {
    let mut machine: Option<Dmg> = None;
    let mut sound = Sound::open();
    let mode = match &sound {
        Some(s) => format!("audio {} Hz, audio-driven pacing", s.out.rate),
        None => String::from("no audio device, clock pacing"),
    };
    let _ = status.send(mode.clone());
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
                        let _ = status.send(format!("ROM loaded - {mode}"));
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
                    if let Some(s) = sound.as_mut() {
                        s.collect(m);
                    }
                    steps -= 1;
                    produced = true;
                }
            } else if let Some(s) = sound.as_mut() {
                steps = 0;
                let mut ran: u64 = 0;
                while ran < MAX_BACKLOG_FRAMES && s.wants_frame() {
                    m.run_frame();
                    s.collect(m);
                    ran += 1;
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
