// Business Source License 1.1
// Licensor:  Torb Foundry
// Licensed Work:  Torb v0.3.5-03.13
// The Licensed Work is © 2023-Present Torb Foundry
//
// Change License: GNU Affero General Public License Version 3
// Additional Use Grant: None
// Change Date: Feb 22, 2023
//
// See LICENSE file at https://github.com/TorbFoundry/torb/blob/main/LICENSE for details.

use core::fmt::Display;
use image::imageops::resize;
use std::fmt::Debug;

use crossterm::{cursor, terminal, ExecutableCommand, QueueableCommand};
use drawille::{Canvas, PixelColor};
use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, ImageDecoder};
use std::io::{stdout, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{thread, time};

use crate::utils::{PrettyContext, PrettyExit};

const FRAME_HEIGHT: u16 = 16;

pub struct BuilderAnimation {}

pub trait Animation<T, E> {
    fn do_with_animation(&self, f: Box<dyn FnMut() -> Result<T, E>>) -> Result<T, E>
    where
        E: Debug + Display;

    fn start_animation(
        &self,
        animation: std::fs::File,
        kill_flag: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()>;
}

impl BuilderAnimation {
    pub fn new() -> Self {
        BuilderAnimation {}
    }
}
impl<T, E> Animation<T, E> for BuilderAnimation
where
    E: Debug + Display,
{
    fn start_animation(
        &self,
        animation: std::fs::File,
        kill_flag: Arc<AtomicBool>,
    ) -> std::thread::JoinHandle<()> {
        let decoder = GifDecoder::new(animation).unwrap();

        let (mut width, mut height) = decoder.dimensions();

        let scale = 0.3;

        width = f32::floor(width as f32 * scale) as u32;
        height = f32::floor(height as f32 * scale) as u32;

        let mut canvas = Canvas::new(width, height);
        let frames = decoder.into_frames();
        let frames_opt = frames.collect_frames().use_or_pretty_warn(
            PrettyContext::default()
                .warn("Warning! Unable to decode frames for animation GIF.")
                .pretty(),
        );

        let kill_flag_clone = kill_flag.clone();

        thread::spawn(move || {
            let mut thread_stdout = stdout();

            loop {
                let kill_flag = kill_flag_clone.clone();

                if kill_flag.load(Ordering::SeqCst) == true {
                    thread_stdout.write("\r".as_bytes()).unwrap();
                    thread_stdout.flush().unwrap();
                    break;
                };

                let frames = frames_opt.clone().unwrap_or(vec![]);

                for frame in frames.iter().cloned() {
                    let mut img = frame.into_buffer();
                    img = resize(&img, width, height, image::imageops::FilterType::Gaussian);
                    for x in 0..width {
                        for y in 0..height {
                            let pixel = img.get_pixel(x, y);
                            let color = PixelColor::TrueColor {
                                r: pixel[0],
                                g: pixel[1],
                                b: pixel[2],
                            };
                            canvas.set_colored(x, y, color);
                        }
                    }

                    let frame = canvas.frame();

                    thread_stdout.write_all(frame.as_bytes()).unwrap();
                    thread_stdout.flush().unwrap();
                    thread::sleep(time::Duration::from_millis(60));
                    canvas.clear();

                    // Move up the height of the frame in the terminal
                    thread_stdout.queue(cursor::MoveUp(FRAME_HEIGHT)).unwrap();
                }
            }
        })
    }

    fn do_with_animation(&self, mut f: Box<dyn FnMut() -> Result<T, E>>) -> Result<T, E>
    where
        E: Debug + Display,
    {
        let home_dir = dirs::home_dir().unwrap();
        let torb_path = home_dir.join(".torb");
        let repository_path = torb_path.join("repositories");
        let repo = "torb-artifacts";
        let gif_path = "torb_dwarf_animation.gif";

        let artifacts_path = repository_path.join(repo);
        let animation_path = artifacts_path.join(gif_path);

        let animation_opt = std::fs::File::open(animation_path).use_or_pretty_warn(
            PrettyContext::default()
                .warn("Warning! Unable to open animation GIF.")
                .pretty(),
        );

        let mut new_stdout = stdout();
        new_stdout.execute(cursor::Hide).use_or_pretty_warn_send(
            PrettyContext::default()
                .warn("Warning! Unable to hide cursor for animation.")
                .pretty(),
        );

        let kill_flag = Arc::new(AtomicBool::new(false));
        let animation_thread_handle_opt = if animation_opt.is_some() {
            let animation = animation_opt.unwrap();

            Some(<BuilderAnimation as Animation<T, E>>::start_animation(
                self,
                animation,
                kill_flag.clone(),
            ))
        } else {
            None
        };

        let res = f();
        kill_flag.store(true, Ordering::SeqCst);

        if animation_thread_handle_opt.is_some() {
            let handle = animation_thread_handle_opt.unwrap();
            handle.join().use_or_pretty_warn_send(
                PrettyContext::default()
                    .warn("Warning! Animation thread in an errored state when joining.")
                    .pretty(),
            );
        };

        new_stdout.execute(cursor::Show).unwrap();
        new_stdout
            .execute(terminal::Clear(terminal::ClearType::FromCursorDown))
            .unwrap();
        res
    }
}
