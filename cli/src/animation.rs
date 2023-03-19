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

use image::imageops::resize;
use core::fmt::Display;
use std::{
    fmt::Debug
};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::io::{ Write, stdout};
use drawille::{Canvas, PixelColor};
use image::codecs::gif::{GifDecoder};
use image::{ImageDecoder, AnimationDecoder};
use std::{thread, time};
use crossterm::{QueueableCommand, cursor, ExecutableCommand };

const FRAME_HEIGHT: u16 = 27;

pub struct BuilderAnimation {
}

pub trait Animation<T, E> {
    fn do_with_animation(&self, f: Box<dyn FnMut() -> Result<T, E>>) -> Result<T, E>
        where E: Debug + Display;
}

impl BuilderAnimation {
    pub fn new() -> Self {
        BuilderAnimation {
        }
    }
}
impl<T, E> Animation<T, E> for BuilderAnimation
    where 
    E: Debug + Display,
{
    fn do_with_animation(&self, mut f: Box<dyn FnMut() -> Result<T, E>>) -> Result<T, E>
        where E: Debug + Display
    {
        let home_dir = dirs::home_dir().unwrap();
        let torb_path = home_dir.join(".torb");
        let repository_path = torb_path.join("repositories");
        let repo = "torb-artifacts";
        let gif_path = "torb_dwarf_animation.gif";

        let artifacts_path = repository_path.join(repo);
        let animation_path = artifacts_path.join(gif_path);

        let animation = std::fs::File::open(animation_path).unwrap();

        let decoder = GifDecoder::new(animation).unwrap();

        let (mut width, mut height) = decoder.dimensions();

        let scale = 0.5;

        width = f32::floor(width as f32 * scale) as u32;
        height = f32::floor(height as f32 * scale) as u32;

        let mut canvas = Canvas::new(width, height);
        let frames = decoder.into_frames();
        let frames = frames.collect_frames().expect("error decoding gif");
        let mut new_stdout = stdout();

        let kill_flag = Arc::new(AtomicBool::new(false));
        let kill_flag_clone = kill_flag.clone();

        new_stdout.execute(cursor::Hide).unwrap();

        let animation_thread_handle = thread::spawn(move || {
            let mut thread_stdout = stdout();

            loop {
                let kill_flag = kill_flag_clone.clone();

                if kill_flag.load(Ordering::SeqCst) == true {
                    thread_stdout.write("\r".as_bytes()).unwrap();
                    thread_stdout.flush().unwrap();
                    break;
                };


                for frame in frames.iter().cloned() {
                    let mut img = frame.into_buffer();
                    img = resize(&img, width, height, image::imageops::FilterType::Gaussian);
                    for x in 0..width {
                        for y in 0..height {
                            let pixel = img.get_pixel(x, y);
                            let color = PixelColor::TrueColor { r: pixel[0], g: pixel[1], b: pixel[2] };
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
                };
            };
        });

        let res = f();
        kill_flag.store(true, Ordering::SeqCst);

        animation_thread_handle.join().unwrap();
        new_stdout.execute(cursor::Show).unwrap();
        res
    }
}


