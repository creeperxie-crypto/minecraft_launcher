use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
use log::Level;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

use winit::dpi::PhysicalSize;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[derive(Debug)]
struct State {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
}

impl State {
    async fn new(window: Arc<Window>) -> State {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ..Default::default()
        });
        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let size = window.inner_size();
assert!(size.width > 0 && size.height > 0, "inner size must not be 0 0 during wgpu initialization. a resize event is expected shortly, but initialization has already failed.");
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 3,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        Self {
            surface,
            device,
            queue,
            config,
        }
    }
}

impl State {
    pub fn resize(&mut self, size: winit::dpi::PhysicalSize<u32>) {
        self.config.width = size.width;
        self.config.height = size.height;
        self.surface.configure(&self.device, &self.config);
    }

    pub fn render(&self) {
        let frame = self.surface.get_current_texture().unwrap();
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();
    }
}

#[derive(Default)]
struct App {
    window: Option<Arc<Window>>,
    state: Option<State>,
    #[cfg(target_arch = "wasm32")]
    event_loop_proxy: Option<EventLoopProxy<State>>,
}

impl ApplicationHandler<State> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let mut attributes = Window::default_attributes()
            .with_title("Minecraft Launcher")
            .with_inner_size(PhysicalSize::new(400, 400));

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            
            let canvas = web_sys::window()
                .and_then(|win| win.document())
                .and_then(|doc| doc.get_element_by_id("canvas"))
                .and_then(|canvas| canvas.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            attributes = attributes.with_canvas(canvas);
        }

        let window = Arc::new(event_loop.create_window(attributes).unwrap());
        self.window = Some(window.clone());

        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                let event_loop_proxy = self.event_loop_proxy.clone().unwrap();
                wasm_bindgen_futures::spawn_local(async move {
                    let state = State::new(window).await;
                    event_loop_proxy.send_event(state);
                });
            } else {
                self.state = Some(pollster::block_on(State::new(window)));
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(state) = &mut self.state {
                    state.resize(size);
                }
            },
            WindowEvent::RedrawRequested => {
                if let Some(state) = &self.state {
                    state.render();
                }
            }
            _ => (),
        }
    }
    
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: State) {
        #[cfg(target_arch = "wasm32")]
        {
            self.state = Some(event);
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(Level::Debug);
        } else {
            env_logger::init();
        }
    }

    // desktop don't need user event
    let event_loop = EventLoop::<State>::with_user_event().build().unwrap();
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::default();
    #[cfg(target_arch = "wasm32")]
    {
        app.event_loop_proxy = Some(event_loop.create_proxy());
    }
    event_loop.run_app(&mut app);
}
