//! A test driver: runs Duck Quill from a list of steps, as a person
//! would, and takes screenshots. Only in builds with the `demo` feature
//! (`cargo build -p quill_app --features demo`), never in the download.
//!
//! `QUILL_DEMO=steps.json DuckQuill` reads the steps; `QUILL_DEMO_OUT`
//! is where screenshots and exports go (default: the current folder).
//! Strokes go through the real pen input, with pressure, exactly as a
//! tablet's would; frame changes, exports and saving through the same
//! functions the timeline, the menus and File call.

use std::path::{Path, PathBuf};

use eframe::egui::{self, ViewportCommand};
use quill_engine::pen::{InputKind, PenSample};
use serde_json::Value;

use crate::DuckQuill;
use crate::paint_tool::Tool;
use crate::pen_input::InkEvent;
use crate::timeline::TimelineAction;

pub struct Demo {
    steps: Vec<Value>,
    next: usize,
    /// UI frames to wait before the next step (so the screen catches up).
    wait: u32,
    /// Waiting for a screenshot to arrive, to save under this name.
    shot: Option<(
        String,
        std::sync::mpsc::Receiver<std::sync::Arc<egui::ColorImage>>,
    )>,
    /// Seconds of playing left.
    playing: f32,
    out: PathBuf,
}

impl Demo {
    /// The driver, if `QUILL_DEMO` names a steps file.
    pub fn from_env() -> Option<Demo> {
        let path = std::env::var_os("QUILL_DEMO")?;
        let text = std::fs::read_to_string(&path).ok()?;
        let steps: Vec<Value> = serde_json::from_str(&text).ok()?;
        let out =
            std::env::var_os("QUILL_DEMO_OUT").map_or_else(|| PathBuf::from("."), PathBuf::from);
        let _ = std::fs::create_dir_all(&out);
        Some(Demo {
            steps,
            next: 0,
            wait: 10,
            shot: None,
            playing: 0.0,
            out,
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        let path = Path::new(name);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.out.join(path)
        }
    }
}

fn color(value: &Value) -> Option<[u8; 3]> {
    let text = value.as_str()?;
    quill_engine::script::parse_hex(text)
        .ok()
        .map(|c| [c[0], c[1], c[2]])
}

fn log(text: impl AsRef<str>) {
    eprintln!("demo: {}", text.as_ref());
}

impl DuckQuill {
    /// Runs the next step, if it's time. Returns pen events to add to
    /// this frame's.
    pub(crate) fn demo_step(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) -> Vec<InkEvent> {
        let Some(mut demo) = self.demo.take() else {
            return Vec::new();
        };
        let ctx = ui.ctx().clone();
        ctx.request_repaint();
        let mut ink = Vec::new();
        // A screenshot that has arrived.
        if let Some((name, receiver)) = &demo.shot {
            let name = name.clone();
            if let Ok(image) = receiver.try_recv() {
                let [w, h] = image.size;
                let mut picture =
                    quill_engine::image::Image::new(w as u32, h as u32, [0, 0, 0, 255]);
                for (i, pixel) in image.pixels.iter().enumerate() {
                    let [r, g, b, _] = pixel.to_array();
                    picture.set_pixel((i % w) as i32, (i / w) as i32, [r, g, b, 255]);
                }
                let path = demo.path(&format!("{name}.png"));
                match picture.save_png(&path, false) {
                    Ok(()) => log(format!("screenshot {}", path.display())),
                    Err(e) => log(format!("screenshot failed: {e}")),
                }
                demo.shot = None;
                demo.wait = 2;
            }
            self.demo = Some(demo);
            return ink;
        }
        if demo.playing > 0.0 {
            demo.playing -= ui.input(|i| i.stable_dt);
            if demo.playing <= 0.0 {
                self.timeline.playback.playing = false;
            }
            self.demo = Some(demo);
            return ink;
        }
        if demo.wait > 0 {
            demo.wait -= 1;
            self.demo = Some(demo);
            return ink;
        }
        let Some(step) = demo.steps.get(demo.next).cloned() else {
            log("done");
            self.autosave.finish();
            std::process::exit(0);
        };
        demo.next += 1;
        demo.wait = 3;
        let Some((name, value)) = step.as_object().and_then(|o| o.iter().next()) else {
            self.demo = Some(demo);
            return ink;
        };
        log(format!("{name} {value}"));
        let mut result: Result<(), String> = Ok(());
        match name.as_str() {
            "wait" => demo.wait = value.as_u64().unwrap_or(10) as u32,
            "tools" => self.animation_tools = value.as_bool().unwrap_or(true),
            "new_animation" => {
                let n = |i: usize, d: u64| value.get(i).and_then(Value::as_u64).unwrap_or(d);
                self.document = quill_engine::frames::new_animation(
                    n(0, 640) as u32,
                    n(1, 400) as u32,
                    n(2, 8) as usize,
                );
                self.history = quill_engine::history::History::default();
                self.canvas.reset();
                self.new_picture_shown();
                self.animation_tools = true;
                demo.wait = 10;
            }
            "layer" => {
                let wanted = value.as_str().unwrap_or_default();
                match self.document.layers().iter().find(|l| l.name() == wanted) {
                    Some(layer) => {
                        let id = layer.id();
                        self.document.set_active(id);
                    }
                    None => result = Err(format!("no layer called {wanted}")),
                }
            }
            "tool" => {
                let tool = match value.get("tool").and_then(Value::as_str).unwrap_or("brush") {
                    "fill" => Tool::Fill,
                    "eraser" => Tool::Eraser,
                    _ => Tool::Brush,
                };
                let size = value.get("size").and_then(Value::as_f64).map(|s| s as f32);
                self.canvas
                    .choose(tool, value.get("color").and_then(color), size);
            }
            "stroke" | "click" => {
                // Canvas points [x, y, pressure], drawn with the pen.
                let points: Vec<[f32; 3]> = if name == "click" {
                    let x = value.get(0).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    let y = value.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32;
                    vec![[x, y, 1.0], [x, y, 1.0]]
                } else {
                    serde_json::from_value(value.clone()).unwrap_or_default()
                };
                let ppp = ctx.pixels_per_point();
                let count = points.len();
                for (i, [x, y, pressure]) in points.into_iter().enumerate() {
                    let (sx, sy) = self.canvas.canvas_to_window(x, y);
                    let last = i + 1 == count;
                    ink.push(InkEvent {
                        sample: PenSample {
                            kind: InputKind::Pen,
                            x: sx * ppp,
                            y: sy * ppp,
                            pressure: if last { 0.0 } else { pressure },
                            ..PenSample::default()
                        },
                        in_contact: !last,
                        in_range: true,
                        barrel_button: false,
                        has_pressure: true,
                        has_tilt: false,
                        device_name: "test driver".into(),
                    });
                }
            }
            "go_to" => {
                self.timeline_action(TimelineAction::GoTo(value.as_u64().unwrap_or(0) as usize))
            }
            "next_frame" => {
                let to = crate::timeline::step(self.document.timeline(), self.document.frame(), 1);
                self.timeline_action(TimelineAction::GoTo(to));
            }
            "add_frame" => self.timeline_action(TimelineAction::Add),
            "duplicate_frame" => self.timeline_action(TimelineAction::Duplicate),
            "hold" => {
                let frame = value.get(0).and_then(Value::as_u64).unwrap_or(0) as usize;
                let hold = value.get(1).and_then(Value::as_u64).unwrap_or(2) as u32;
                self.timeline_action(TimelineAction::Hold(frame, hold));
            }
            "onion" => {
                self.timeline.onion.on = value.get(0).and_then(Value::as_bool).unwrap_or(true);
                self.timeline.onion.before =
                    value.get(1).and_then(Value::as_u64).unwrap_or(1) as usize;
                self.timeline.onion.after =
                    value.get(2).and_then(Value::as_u64).unwrap_or(1) as usize;
                demo.wait = 20;
            }
            "fps" => {
                let mut settings = self.document.timeline().settings();
                settings.fps = value.as_f64().unwrap_or(12.0) as f32;
                self.timeline_action(TimelineAction::Timing(settings));
            }
            "play" => {
                self.timeline.playback.playing = true;
                demo.playing = value.as_f64().unwrap_or(1.0) as f32;
            }
            "undo" => {
                self.history.undo(&mut self.document);
            }
            "screenshot" => {
                let (sender, receiver) = std::sync::mpsc::channel();
                ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::ScreenshotCallback::new(
                    move |image| {
                        let _ = sender.send(image);
                    },
                )));
                demo.shot = Some((value.as_str().unwrap_or("screen").to_owned(), receiver));
            }
            "export_gif" | "export_sheet" | "export_frames" => {
                let path = demo.path(value.as_str().unwrap_or("out"));
                let choice = match name.as_str() {
                    "export_gif" => crate::files::AnimationExport::Gif,
                    "export_sheet" => crate::files::AnimationExport::Sheet,
                    _ => crate::files::AnimationExport::Frames,
                };
                result = self
                    .export_animation_to(choice, &path)
                    .map(|what| log(format!("exported {what}")));
            }
            "save" => {
                let path = demo.path(value.as_str().unwrap_or("picture.quill"));
                result = quill_engine::file::save(&self.document, &path)
                    .map(|()| self.history.mark_saved())
                    .map_err(|e| e.to_string());
            }
            "open" => {
                let path = demo.path(value.as_str().unwrap_or("picture.quill"));
                match self.files.open_path(path) {
                    Some(document) => {
                        self.document = document;
                        self.history = quill_engine::history::History::default();
                        self.canvas.reset();
                        self.new_picture_shown();
                        demo.wait = 15;
                    }
                    None => result = Err("couldn't open".into()),
                }
            }
            "check_frames" => {
                // The frame count and the frame shown, as expected.
                let want = value.get(0).and_then(Value::as_u64).unwrap_or(0) as usize;
                if self.document.frame_count() != want {
                    result = Err(format!(
                        "{} frames, not {want}",
                        self.document.frame_count()
                    ));
                }
            }
            other => result = Err(format!("unknown step {other}")),
        }
        if let Err(problem) = result {
            log(format!("FAILED: {problem}"));
            std::process::exit(1);
        }
        let _ = frame;
        self.demo = Some(demo);
        ink
    }
}
