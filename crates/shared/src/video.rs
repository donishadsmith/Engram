#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::{
    fs::{File, remove_file},
    io::{BufWriter, Error, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, SyncSender},
    thread::{JoinHandle, spawn},
};

use chrono::Local;

use crate::render::{Frame, fit_nearest, to_rgba};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const BYTES_PER_AUDIO_FRAME: usize = 8;

// https://gist.github.com/carrotflakes/d6960c62d3de5dcbcc2b44562814d5a7
//https://github.com/NapoleonWils0n/ffmpeg-rust-scripts

struct Active {
    child: Child,
    sender: SyncSender<(Vec<u8>, u8)>,
    writer: JoinHandle<()>,
    audio: BufWriter<File>,
    output: PathBuf,
    temp_video: PathBuf,
    temp_audio: PathBuf,
    recording_size: (u32, u32),
    sample_rate: u32,
    fps: f64,
    audio_frames: u64,
    video_frames: u64,
}

impl Active {
    fn start(width: u32, height: u32, fps: f32, sample_rate: u32, video_dir: PathBuf) -> Result<Self, Error> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let output = video_dir.join(format!("video_{timestamp}.mp4"));
        let temp_video = video_dir.join(format!("video_{timestamp}.tmp.mkv"));
        let temp_audio = video_dir.join(format!("video_{timestamp}.tmp.pcm"));

        let mut child = ffmpeg()
            .args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args(["-s", &format!("{width}x{height}"), "-r", &fps.to_string(), "-i", "-"])
            .args(["-vf", "scale=iw*4:ih*4:flags=neighbor"])
            .args([
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            ])
            .arg(&temp_video)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let (sender, receiver) = mpsc::sync_channel::<(Vec<u8>, u8)>(16);
        let writer = spawn(move || {
            'outer: while let Ok((frame, repeat)) = receiver.recv() {
                for _ in 0..repeat {
                    if stdin.write_all(&frame).is_err() {
                        break 'outer;
                    }
                }
            }
        });

        let audio = BufWriter::new(File::create(&temp_audio)?);

        Ok(Self {
            child,
            sender,
            writer,
            audio,
            output,
            temp_video,
            temp_audio,
            recording_size: (width, height),
            sample_rate,
            fps: fps as f64,
            audio_frames: 0,
            video_frames: 0,
        })
    }

    fn video_frames_due(&self) -> u64 {
        (self.audio_frames as f64 * self.fps / self.sample_rate as f64).round() as u64
    }

    fn audio_frames_due(&self) -> u64 {
        (self.video_frames as f64 * self.sample_rate as f64 / self.fps).round() as u64
    }

    fn complete(mut self) -> Result<(), Error> {
        drop(self.sender);
        let _ = self.writer.join();
        self.child.wait()?;
        let _ = self.audio.flush();

        ffmpeg()
            .args(["-y", "-i"])
            .arg(&self.temp_video)
            .args(["-f", "f32le", "-ar", &self.sample_rate.to_string(), "-ac", "2", "-i"])
            .arg(&self.temp_audio)
            .args(["-c:v", "copy", "-c:a", "aac", "-shortest"])
            .arg(&self.output)
            .stderr(Stdio::null())
            .status()?;

        let _ = remove_file(self.temp_audio);
        let _ = remove_file(self.temp_video);

        Ok(())
    }
}

pub struct VideoRecorder {
    active: Option<Active>,
}

impl VideoRecorder {
    pub fn new() -> Self {
        Self { active: None }
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    pub fn start(
        &mut self,
        fps: f32,
        recording_size: (u32, u32),
        video_dir: PathBuf,
        sample_rate: u32,
    ) -> Result<(), Error> {
        if !ffmpeg_is_available() {
            return Err(Error::other("``ffmpeg is unavailable``"));
        }

        match Active::start(recording_size.0, recording_size.1, fps, sample_rate, video_dir) {
            Ok(active) => self.active = Some(active),
            Err(_) => return Err("child process failed to start").map_err(Error::other),
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        if let Some(active) = self.active.take() {
            spawn(move || {
                if let Err(err) = active.complete() {
                    eprintln!("video recording failed to finalize: {err}");
                }
            });
        }

        Ok(())
    }

    pub fn capture(&mut self, frame: &Frame, audio: Vec<u8>) {
        let Some(active) = self.active.as_mut() else {
            return;
        };

        let silent = audio.len() < BYTES_PER_AUDIO_FRAME;

        let repeat = if silent {
            1
        } else {
            active.audio_frames += (audio.len() / BYTES_PER_AUDIO_FRAME) as u64;
            let _ = active.audio.write_all(&audio);

            active
                .video_frames_due()
                .saturating_sub(active.video_frames)
                .min(u8::MAX as u64) as u8
        };

        if repeat == 0 {
            return;
        }

        let mut rgba = to_rgba(&frame);

        if (frame.width as u32, frame.height as u32) != active.recording_size {
            rgba = fit_nearest(
                &rgba,
                frame.width,
                frame.height,
                active.recording_size.0 as usize,
                active.recording_size.1 as usize,
            );
        }

        let _ = active.sender.send((rgba, repeat));
        active.video_frames += repeat as u64;

        if silent {
            let missing = active.audio_frames_due().saturating_sub(active.audio_frames);
            let _ = active
                .audio
                .write_all(&vec![0u8; missing as usize * BYTES_PER_AUDIO_FRAME]);

            active.audio_frames += missing;
        }
    }
}

impl Drop for VideoRecorder {
    fn drop(&mut self) {
        if let Some(active) = self.active.take() {
            let _ = active.complete();
        }
    }
}

fn ffmpeg() -> Command {
    let mut command = Command::new("ffmpeg");

    // https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
    // NOTE TO SELF: around nov 15 around minish cap debugging, i used cfg in main to hide console on release builds,
    // but didnt realise that the app handoff process to launch the gui for windows terminal was slow as hell and blocks
    // the app for ~two seconds when recording and stopping vids due to the startup for the gui, essentially this just
    // doesnt open up the gui and lets console run headless so any delay is imperceptible, NEVER REMOVE THIS
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    command
}

fn ffmpeg_is_available() -> bool {
    ffmpeg()
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}