//! Determinism tests (E09_03): the same ROM gives the same framebuffer and audio hashes, and a
//! machine restored from a save state continues bit-exactly (decision A_05). The hash is FNV-1a
//! 64-bit over every framebuffer pixel and every drained audio sample of each frame.

use super::Dmg;
use crate::machine::{Machine, AUDIO_RING_SAMPLES};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Hashes of a run: framebuffers, audio samples, and whether any sample was non-zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Run {
    video: u64,
    audio: u64,
    audible: bool,
}

/// Program at $0150: sound channel 1 on, a stripe pattern in VRAM tile 0, then a loop copying LY
/// into SCX (a per-line scroll that makes the picture depend on the exact timing).
fn rom() -> Vec<u8> {
    let program: &[u8] = &[
        0x3E, 0x80, 0xE0, 0x26, // NR52 = $80
        0x3E, 0x77, 0xE0, 0x24, // NR50 = $77
        0x3E, 0xFF, 0xE0, 0x25, // NR51 = $FF
        0x3E, 0x80, 0xE0, 0x11, // NR11 = $80
        0x3E, 0xF0, 0xE0, 0x12, // NR12 = $F0
        0x3E, 0x00, 0xE0, 0x13, // NR13 = $00
        0x3E, 0x87, 0xE0, 0x14, // NR14 = $87 (trigger)
        0x21, 0x00, 0x80, // LD HL,$8000
        0x3E, 0xAA, // LD A,$AA
        0x0E, 0x20, // LD C,$20
        0x22, // LD (HL+),A
        0x0D, // DEC C
        0x20, 0xFC, // JR NZ,-4
        0xF0, 0x44, // LDH A,($44)
        0xE0, 0x43, // LDH ($43),A
        0x18, 0xFA, // JR -6
    ];
    let mut r = vec![0u8; 0x8000];
    r[0x100..0x103].copy_from_slice(&[0xC3, 0x50, 0x01]);
    r[0x150..0x150 + program.len()].copy_from_slice(program);
    r
}

fn fresh() -> Dmg {
    Dmg::new(&rom()).expect("valid")
}

/// Runs `frames` frames, draining the audio after each one.
fn run_frames(dmg: &mut Dmg, frames: usize) -> Run {
    let mut run = Run {
        video: FNV_OFFSET,
        audio: FNV_OFFSET,
        audible: false,
    };
    let mut samples = vec![0i16; AUDIO_RING_SAMPLES];
    for _ in 0..frames {
        dmg.run_frame();
        for pixel in dmg.framebuffer() {
            run.video = fnv(run.video, &pixel.to_le_bytes());
        }
        let count = dmg.drain_audio(&mut samples);
        for sample in &samples[..count] {
            run.audio = fnv(run.audio, &sample.to_le_bytes());
            if *sample != 0 {
                run.audible = true;
            }
        }
    }
    run
}

/// Runs 3 frames plus `extra_ticks` dots, saves, runs 3 frames (hash A), then restores the
/// state into the same machine and into a fresh one: both must give hash A again.
fn round_trip(extra_ticks: usize) {
    let mut a = fresh();
    run_frames(&mut a, 3);
    for _ in 0..extra_ticks {
        a.tick();
    }
    let bytes = a.save_state_vec().expect("room");
    let expected = run_frames(&mut a, 3);
    assert!(expected.audible);
    a.load_state(&bytes).expect("valid");
    assert_eq!(run_frames(&mut a, 3), expected, "same machine");
    let mut b = fresh();
    b.load_state(&bytes).expect("valid");
    assert_eq!(run_frames(&mut b, 3), expected, "fresh machine");
}

#[test]
fn e09_03_save_run_load_run_gives_the_same_hashes() {
    round_trip(0);
}

#[test]
fn e09_03_state_saved_mid_frame_resumes_identically() {
    round_trip(12_345);
}

#[test]
fn e09_03_two_fresh_runs_give_identical_hashes() {
    let first = run_frames(&mut fresh(), 5);
    let second = run_frames(&mut fresh(), 5);
    assert_eq!(first, second);
}

#[test]
fn e09_03_runs_are_not_vacuous() {
    let mut dmg = fresh();
    let run = run_frames(&mut dmg, 3);
    let frame = dmg.framebuffer();
    assert!(frame.iter().any(|&pixel| pixel != frame[0]), "flat picture");
    assert!(run.audible, "silent audio");
    assert_ne!(run.video, FNV_OFFSET);
    assert_ne!(run.audio, FNV_OFFSET);
}
