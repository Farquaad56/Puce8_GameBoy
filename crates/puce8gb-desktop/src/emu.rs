//! Emulation thread: owns the core, paced by the audio ring when a device is open, else by the clock (E07_02, E07_03).
use crate::audio::{pacing_decision, push_budget, target_fill, AudioOut, Pace};
use crate::battery::BatterySaver;
use crate::pacing::{clamp_owed, Pacer, MAX_BACKLOG_FRAMES};
use crate::resample::Resampler;
use crate::slots::{read_slot, slot_path, state_error_message, write_slot};
use crate::viewer::{capture, Snapshot};
use crate::volume::gain_for_percent;
use puce8gb_core::machine::{AUDIO_RING_SAMPLES, FRAME_DOTS};
use puce8gb_core::{Dmg, Input, Machine};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Frames between two viewer snapshots while the machine runs.
const SNAPSHOT_EVERY: u32 = 4;

/// Commands sent from the UI to the emulation thread.
pub enum Command {
    Load(Vec<u8>),
    SavePath(Option<PathBuf>),
    Reset,
    Pause(bool),
    FrameStep,
    /// Serialize the machine into slot file N (1..=4) next to the ROM.
    SaveSlot(u32),
    /// Restore the machine from slot file N (1..=4); battery RAM is untouched.
    LoadSlot(u32),
    Input(Input),
    Viewer(bool),
    Trace(bool),
    Quit,
}

/// UI side of the emulation thread.
pub struct EmuHandle {
    commands: Sender<Command>,
    /// Finished frames (0xAARRGGBB, 160 x 144); the thread drops frames when this is full.
    pub frames: Receiver<Vec<u32>>,
    /// Human readable status messages from the thread.
    pub status: Receiver<String>,
    /// Viewer snapshots, produced only while the viewer is open.
    pub snapshots: Receiver<Snapshot>,
    volume: Arc<AtomicU32>,
    thread: Option<JoinHandle<()>>,
}

impl EmuHandle {
    pub fn spawn(initial_volume: u8) -> Self {
        let volume = Arc::new(AtomicU32::new(gain_for_percent(initial_volume).to_bits()));
        let thread_volume = Arc::clone(&volume);
        let (commands, command_rx) = channel();
        let (frame_tx, frames) = sync_channel(2);
        let (status_tx, status) = channel();
        let (snapshot_tx, snapshots) = sync_channel(1);
        let thread = std::thread::spawn(move || {
            run(command_rx, frame_tx, status_tx, thread_volume, snapshot_tx)
        });
        EmuHandle {
            commands,
            frames,
            status,
            snapshots,
            volume,
            thread: Some(thread),
        }
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Sets the output volume (0..=100); the audio callback reads it without locking.
    pub fn set_volume(&self, percent: u8) {
        self.volume
            .store(gain_for_percent(percent).to_bits(), Ordering::Relaxed);
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
    fn open(volume: Arc<AtomicU32>) -> Option<Sound> {
        let out = AudioOut::open(volume)?;
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

fn run(
    commands: Receiver<Command>,
    frames: SyncSender<Vec<u32>>,
    status: Sender<String>,
    volume: Arc<AtomicU32>,
    snapshots: SyncSender<Snapshot>,
) {
    let mut machine: Option<Dmg> = None;
    let mut sound = Sound::open(volume);
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
    let mut battery = BatterySaver::new();
    let mut save_path: Option<PathBuf> = None;
    let mut viewer_on = false;
    let mut since_snapshot: u32 = 0;
    let mut trace_on = false;
    loop {
        loop {
            match commands.try_recv() {
                Ok(Command::Load(bytes)) => match Dmg::new(&bytes) {
                    Ok(mut new_dmg) => {
                        if let Some(old_dmg) = machine.as_ref() {
                            battery.flush(old_dmg);
                        }
                        battery.attach(&mut new_dmg, save_path.take());
                        machine = Some(new_dmg);
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
                Ok(Command::SaveSlot(slot)) => {
                    let (Some(m), Some(base)) = (machine.as_ref(), save_path.as_ref()) else {
                        let _ = status.send(String::from("Load a ROM first"));
                        continue;
                    };
                    let file = slot_path(base, slot);
                    match m.save_state_vec() {
                        Ok(bytes) => match write_slot(&file, &bytes) {
                            Ok(()) => {
                                let _ =
                                    status.send(format!("Slot {slot} saved to {}", file.display()));
                            }
                            Err(err) => {
                                let _ = status.send(format!("Cannot save slot {slot}: {err}"));
                            }
                        },
                        Err(err) => {
                            let _ = status.send(state_error_message(slot, &err));
                        }
                    }
                }
                Ok(Command::LoadSlot(slot)) => {
                    let (Some(m), Some(base)) = (machine.as_mut(), save_path.as_ref()) else {
                        let _ = status.send(String::from("Load a ROM first"));
                        continue;
                    };
                    let file = slot_path(base, slot);
                    match read_slot(&file) {
                        None => {
                            let _ =
                                status.send(format!("No save in slot {slot} ({})", file.display()));
                        }
                        Some(bytes) => match m.load_state(&bytes) {
                            Ok(()) => {
                                owed = 0;
                                let _ = status
                                    .send(format!("Slot {slot} loaded from {}", file.display()));
                            }
                            Err(err) => {
                                let _ = status.send(state_error_message(slot, &err));
                            }
                        },
                    }
                }
                Ok(Command::Input(input)) => {
                    if let Some(m) = machine.as_mut() {
                        m.set_input(input);
                    }
                }
                Ok(Command::SavePath(path)) => save_path = path,
                Ok(Command::Viewer(on)) => {
                    viewer_on = on;
                    since_snapshot = SNAPSHOT_EVERY;
                }
                Ok(Command::Trace(on)) => trace_on = on,
                Ok(Command::Quit) | Err(TryRecvError::Disconnected) => {
                    if let Some(m) = machine.as_ref() {
                        battery.flush(m);
                    }
                    return;
                }
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
            if m.trace_enabled() != trace_on {
                m.set_trace_enabled(trace_on);
            }
            if viewer_on {
                if produced {
                    since_snapshot += 1;
                }
                if since_snapshot >= SNAPSHOT_EVERY || (produced && paused) {
                    let _ = snapshots.try_send(capture(m));
                    since_snapshot = 0;
                }
            }
            battery.poll(m, Instant::now());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
