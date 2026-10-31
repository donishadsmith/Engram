use crate::render::{Frame, fit_nearest, to_rgba};
use chrono::Local;
use std::{
    fs::{File, remove_file},
    io::{BufWriter, Error, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, SyncSender},
    thread::{JoinHandle, spawn},
};

// https://gist.github.com/carrotflakes/d6960c62d3de5dcbcc2b44562814d5a7
//https://github.com/NapoleonWils0n/ffmpeg-rust-scripts

struct Active {
    child: Child,
    sender: SyncSender<Vec<u8>>,
    writer: JoinHandle<()>,
    audio: BufWriter<File>,
    output: PathBuf,
    temp_video: PathBuf,
    temp_audio: PathBuf,
    recording_size: (u32, u32),
    sample_rate: u32,
}

impl Active {
    fn start(
        width: u32,
        height: u32,
        fps: f32,
        sample_rate: u32,
        video_dir: PathBuf,
    ) -> Result<Self, Error> {
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let output = video_dir.join(format!("video_{timestamp}.mp4"));
        let temp_video = video_dir.join(format!("video_{timestamp}.tmp.mkv"));
        let temp_audio = video_dir.join(format!("video_{timestamp}.tmp.pcm"));

        let mut child = Command::new("ffmpeg")
            .args(["-y", "-f", "rawvideo", "-pix_fmt", "rgba"])
            .args([
                "-s",
                &format!("{width}x{height}"),
                "-r",
                &fps.to_string(),
                "-i",
                "-",
            ])
            .args(["-vf", "scale=iw*4:ih*4:flags=neighbor"])
            .args([
                "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
            ])
            .arg(&temp_video)
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stdin = child.stdin.take().unwrap();
        let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(16);
        let writer = spawn(move || {
            while let Ok(frame) = receiver.recv() {
                if stdin.write_all(&frame).is_err() {
                    break;
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
        })
    }

    fn complete(mut self) -> Result<(), Error> {
        drop(self.sender);
        let _ = self.writer.join();
        self.child.wait()?;
        let _ = self.audio.flush();

        Command::new("ffmpeg")
            .args(["-y", "-i"])
            .arg(&self.temp_video)
            .args([
                "-f",
                "f32le",
                "-ar",
                &self.sample_rate.to_string(),
                "-ac",
                "2",
                "-i",
            ])
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

        match Active::start(
            recording_size.0,
            recording_size.1,
            fps,
            sample_rate,
            video_dir,
        ) {
            Ok(active) => self.active = Some(active),
            Err(_) => return Err("child process failed to start").map_err(Error::other),
        }

        Ok(())
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        let Some(active) = self.active.take() else {
            return Ok(());
        };

        active.complete()?;

        Ok(())
    }

    pub fn capture(&mut self, frame: &Frame, audio: Vec<u8>) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
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

        let _ = active.sender.send(rgba);
        let _ = active.audio.write_all(&audio);
    }
}

pub fn ffmpeg_is_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok()
}
