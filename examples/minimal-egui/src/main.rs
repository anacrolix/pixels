#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use error_iter::ErrorIter as _;
use image::EncodableLayout;
use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::iter::zip;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

mod gui;

const X_BOXES: u32 = 15;
const Y_BOXES: u32 = 11;
const WIDTH: u32 = BOX_SIZE as u32 * X_BOXES;
const HEIGHT: u32 = BOX_SIZE as u32 * Y_BOXES;
const BOX_SIZE: i16 = 32;

/// Representation of the application state. In this example, a box will bounce around the screen.
struct World {
    box_x: i16,
    box_y: i16,
    velocity_x: i16,
    velocity_y: i16,
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let player_icon_file = COTW2ICONS.get_file("323.ico").unwrap();
    let player_icon = ::image::load_from_memory(player_icon_file.contents())?;
    let all_icon_images: Vec<_> = COTW2ICONS
        .entries()
        .iter()
        .map(|item| {
            let contents = item.as_file().unwrap().contents();
            ::image::load_from_memory(contents).unwrap().to_rgba8()
        })
        .collect();
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(640f64, 480f64);
        WindowBuilder::new()
            .with_title("Hello Pixels + egui")
            .with_inner_size(size)
            // .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &pixels,
        );

        (pixels, framework)
    };
    let mut world = World::new();
    let mut window_size = ::winit::dpi::PhysicalSize::<u32>::new(WIDTH, HEIGHT);
    let mut pixels_size = None;

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.close_requested() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                window_size = size;
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    log_error("pixels.resize_surface", err);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                framework.resize(size.width, size.height);
            }

            // Update internal state and request a redraw
            world.update();
            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Draw the world
                world.draw(pixels.frame_mut());
                let player_tile = ((X_BOXES / 2) as usize, (Y_BOXES / 2) as usize);
                let dst_tiles = (0..Y_BOXES as usize)
                    .into_iter()
                    .map(|y| (0..X_BOXES as usize).into_iter().map(move |x| (x, y)))
                    .flatten();
                let src_icon_tile = (0, 0);
                for (dst_tile, icon) in zip(dst_tiles, &all_icon_images) {
                    draw_cotw_icon(&mut pixels, dst_tile, src_icon_tile, &icon);
                }

                if true {
                    draw_cotw_icon(
                        &mut pixels,
                        player_tile,
                        src_icon_tile,
                        &player_icon.to_rgba8(),
                    )
                }

                // Prepare egui
                let world_top = framework.prepare(&window);

                let mut view_size = window_size;
                view_size.height -= world_top as u32;
                dbg!(world_top);
                if Some(view_size) != pixels_size {
                    pixels_size = Some(view_size);
                }

                pixels.set_render_target(window_size.width, window_size.height-world_top as u32, (0., world_top));

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

                    // Render egui
                    framework.render(encoder, render_target, context);

                    Ok(())
                });

                // Basic error handling
                if let Err(err) = render_result {
                    log_error("pixels.render", err);
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => (),
        }
    });
}

static COTW2ICONS: include_dir::Dir =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/src/cotw2icons");

fn draw_cotw_icon(
    pixels: &mut Pixels,
    dst_tile: (usize, usize),
    src_tile: (usize, usize),
    icons: &image::RgbaImage,
) {
    let pixels_width = pixels.texture().width() as usize;
    let icon_dim = BOX_SIZE as usize;
    let bytes_per_pixel = 4;
    for y in 0..icon_dim {
        let dst: &mut [u8] = &mut pixels.frame_mut()[bytes_per_pixel
            * ((dst_tile.1 * icon_dim + y) * pixels_width + dst_tile.0 * icon_dim)..];
        let src_index = bytes_per_pixel
            * ((src_tile.1 * icon_dim + y) * icons.width() as usize + icon_dim * src_tile.0);
        let src = &icons.as_bytes()[src_index..];
        let copy_len = bytes_per_pixel * icon_dim;
        for (dst, src) in zip(dst.chunks_exact_mut(4), src.chunks_exact(4)).take(icon_dim) {
            if src[3] == 0 {
                continue;
            }
            dst.copy_from_slice(src);
        }
        // dst[..copy_len].copy_from_slice(&src[..copy_len]);
    }
}

fn log_error<E: std::error::Error + 'static>(method_name: &str, err: E) {
    error!("{method_name}() failed: {err}");
    for source in err.sources().skip(1) {
        error!("  Caused by: {source}");
    }
}

impl World {
    /// Create a new `World` instance that can draw a moving box.
    fn new() -> Self {
        Self {
            box_x: 24,
            box_y: 16,
            velocity_x: 1,
            velocity_y: 1,
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {
        if self.box_x <= 0 || self.box_x + BOX_SIZE > WIDTH as i16 {
            self.velocity_x *= -1;
        }
        if self.box_y <= 0 || self.box_y + BOX_SIZE > HEIGHT as i16 {
            self.velocity_y *= -1;
        }

        self.box_x += self.velocity_x;
        self.box_y += self.velocity_y;
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            let inside_the_box = x >= self.box_x
                && x < self.box_x + BOX_SIZE
                && y >= self.box_y
                && y < self.box_y + BOX_SIZE;

            let rgba = if inside_the_box {
                [0x5e, 0x48, 0xe8, 0xff]
            } else {
                [0x48, 0xb2, 0xe8, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
    }
}
