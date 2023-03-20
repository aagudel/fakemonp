// Hide console window on Windows in release
//#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;

fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(800.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Simulator",
        options,
        Box::new(|_cc| Box::new(MyApp::default())),
    );
}

use egui::{vec2, Color32, ColorImage, TextureHandle, TextureOptions};
use nalgebra::{SMatrix, Vector2};
use std::net::UdpSocket;

const N: usize = 30;
const WIDTH: usize = 200;

pub struct Painting {
    /// in 0-1 normalized coordinates
    lines: Vec<Vec<egui::Pos2>>,
    stroke: egui::Stroke,
}

impl Default for Painting {
    fn default() -> Self {
        Self {
            lines: Default::default(),
            stroke: egui::Stroke::new(1.0, Color32::from_rgb(25, 200, 100)),
        }
    }
}

impl Painting {
    pub fn ui_control(&mut self, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            egui::stroke_ui(ui, &mut self.stroke, "Stroke");
            ui.separator();
            if ui.button("Clear Painting").clicked() {
                self.lines.clear();
            }
        })
        .response
    }

    pub fn ui_content(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let (mut response, painter) =
            ui.allocate_painter(ui.available_size_before_wrap(), egui::Sense::drag());

        let to_screen = emath::RectTransform::from_to(
            egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.square_proportions()),
            response.rect,
        );
        let from_screen = to_screen.inverse();

        if self.lines.is_empty() {
            self.lines.push(vec![]);
        }

        let current_line = self.lines.last_mut().unwrap();

        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let canvas_pos = from_screen * pointer_pos;
            if current_line.last() != Some(&canvas_pos) {
                current_line.push(canvas_pos);
                response.mark_changed();
            }
        } else if !current_line.is_empty() {
            self.lines.push(vec![]);
            response.mark_changed();
        }

        let shapes = self
            .lines
            .iter()
            .filter(|line| line.len() >= 2)
            .map(|line| {
                let points: Vec<egui::Pos2> = line.iter().map(|p| to_screen * *p).collect();
                egui::Shape::line(points, self.stroke)
            });

        painter.extend(shapes);

        response
    }
}

#[allow(non_snake_case)]
struct MyApp {
    name: String,
    port: u32,
    row: usize,
    texture: Option<egui::TextureHandle>,
    img: ColorImage,
    time0: f64,
    W: SMatrix::<f32, N, 2>,
    periods: [f64; WIDTH],
    socket0: UdpSocket,
    socket1: UdpSocket,
    paint: Box<Painting>,
}

impl Default for MyApp {
    fn default() -> Self {
        let socket0 = UdpSocket::bind("0.0.0.0:0").expect("bind() failed");
        socket0.set_broadcast(true).expect("set_broadcast() failed");        
        socket0.connect("127.0.0.1:4600").expect("connect() failed");
        let socket1 = UdpSocket::bind("0.0.0.0:0").expect("bind() failed");        
        socket1.set_broadcast(true).expect("set_broadcast() failed");
        socket1.connect("127.0.0.1:4300").expect("connect() failed");
        Self {
            name: "192.168.1.255".to_owned(),
            port: 4300,
            row: 0,
            texture: None,
            img: ColorImage::new([WIDTH, N], Color32::BLACK),
            time0: 0.0,
            W: 20.0*SMatrix::<f32, N, 2>::new_random(),
            periods: [0.0; WIDTH],
            socket0,
            socket1,
            paint: Box::new(Painting::default()),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut x = Vector2::new(0.0, 0.0);
        egui::Window::new("Window").show(ctx, |ui| {
            ui.heading("Subject simulator");
            ui.horizontal(|ui| {
                let name_label = ui.label("Test: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.port, 0..=255).text("param"));
            if ui.button("Connect").clicked() {
                let s0 = format!("{}:4600",self.name);
                let s1 = format!("{}:4300",self.name);
                self.socket0.connect(s0).expect("connect() failed");
                self.socket1.connect(s1).expect("connect() failed");
            }
            //ui.label(format!("Test '{}', param {}", self.name, self.port));
            // Rust is hard, don't know how to do this better
            match self.paint.lines.last() {
                Some(t) =>  {
                    //ui.label(format!("{:?}",t.last()));
                    if t.len() != 0 {
                        let v = t.last().unwrap();
                        x[0] = v[0]/2.0; //Normalize to (0.0,1.0)
                        x[1] = 1.0-v[1]; //Already (1.0,0.0), invert
                    }
                }
                None => {
                    ui.label(format!("Error"));
                }
            }
            // Rust is hard, don't know how to do this better
            //if !self.paint.lines.last().is_none() {
            //    ui.label(format!("{:?}", self.paint.lines.last()));
            //     let l = self.paint.lines.last().last();
            //     ui.label(format!("{:?}", l));
            // }
            ui.label(format!("W.min {:.3} W.max {:.3}",self.W.min(),self.W.max()));

        });

        egui::Window::new("Trajectory")
            .default_size(vec2(300.0, 200.0))
            .vscroll(false)
            .show(ctx, |ui| {                
                self.paint.ui_control(ui);
                ui.label("Paint with your mouse/touch!");
                egui::Frame::canvas(ui.style()).show(ui, |ui| {
                    self.paint.ui_content(ui);
                });
            });
        
        egui::Window::new("Plot").show(ctx, |ui| {

            // Math is always first            
            let e = SMatrix::<f32, N, 1>::new_random();
            let z = self.W * x + 5.0*e;

            // Comms are second
            // Rust doesn't want you to be copying buffers al'round
            let mut kins0 = x[0].to_be_bytes().to_vec();
            kins0.extend(x[1].to_be_bytes().to_vec());
            let kins: [u8; 8] = std::array::from_fn(|i| kins0[i]);
            self.socket0.send(&kins).expect("Couldn't send message");

            // TODO: Rounding?
            let counts: [u8; N] = std::array::from_fn(|i| z[i] as u8);
            self.socket1.send(&counts).expect("Couldn't send message");

            ui.label(format!("z.min {:.3} z.max {:.3}",z.min(),z.max()));

            // Draw on pixels based on:
            // https://github.com/emilk/egui/discussions/343#discussioncomment-2700452            
            let mut img = ColorImage::new([WIDTH, N], Color32::BLACK);            
            for i in 0..=N-1 {
                self.img.pixels[i*WIDTH + self.row] = Color32::from_rgb(0, (10.0*z[i]) as u8, 0);
                img.pixels[i*WIDTH + self.row] = Color32::from_rgb(0, (10.0*z[i]) as u8, 0);
            }

            if self.row < WIDTH-1 {                
                self.row = self.row + 1;
                // Wrong ways of doing this (see wgui::Context docs)
                //let t: f64 = ctx.input(|i| i.time);
                let time1 = ctx.input().time;
                // This gives similar times
                //use std::time::SystemTime;
                //let time1 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("!").as_secs_f64();                
                self.periods[self.row] = time1 - self.time0;
                self.time0 = time1;
            } else {
                self.row = 0;
            }

            let texture_ref: &mut TextureHandle = self.texture.get_or_insert_with(|| {
                ui.ctx().load_texture("plot_demo", ColorImage::example(), Default::default())
                // I think this doesn't work because load_texture wants a copy and
                // takes ownership of img
                //ui.ctx().load_texture("plot_demo", img, Default::default())
            });
            
            // TextureHandle.set() takes ownership of the image. Clone is probably
            // not the most efficient way of doing this but I do not kow yet how
            // to do this better.
            //self.texture.set(img, TextureOptions::default());
            //texture_ref.set(img, TextureOptions::default());
            texture_ref.set(self.img.clone(), TextureOptions::default());
            
            //let size = texture_ref.size_vec2();
            let size = vec2(2.0*WIDTH as f32, 2.0*N as f32);
            
            ui.image(texture_ref, size);
            
            // There is likely better ways to do this ...
            let line_points: PlotPoints = (0..=WIDTH-1)
            .map(|i| {
                [i as f64, self.periods[i]]
            })
            .collect();
            
            let line = Line::new(line_points);

            use egui::plot::{Line, PlotPoints};
            //use egui::plot::{PlotPoint, PlotImage};
            
            // // TODO: I do not yet understand why the dereferencing here (&*) is
            // // needed or what does it do
            // let plot_image = PlotImage::new(
            //     &*texture_ref,
            //     PlotPoint::new(0.0, 0.0),
            //     //5.0 * vec2(texture_ref.aspect_ratio(), 1.0),
            //     5.0 * vec2(1.0, 1.0),
            // );

            egui::plot::Plot::new("example_plot")
                .height(100.0)
                .data_aspect(500.0)
                .show(ui, |plot_ui| {
                    plot_ui.line(line);
                    //plot_ui.image(plot_image.name("Image"));
                });

            ui.ctx().request_repaint();

            std::thread::sleep(std::time::Duration::from_millis(40));
        });
    }
}
