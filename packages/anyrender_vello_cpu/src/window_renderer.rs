use crate::VelloCpuAnyrenderScene;
use anyrender::{WindowHandle, WindowRenderer};
use peniko::color::PremulRgba8;
use softbuffer::{Context, Surface};
use std::{num::NonZero, sync::Arc, time::Instant};
use vello_cpu::{Pixmap, RenderContext};

// Simple struct to hold the state of the renderer
pub struct ActiveRenderState {
    _context: Context<Arc<dyn WindowHandle>>,
    surface: Surface<Arc<dyn WindowHandle>, Arc<dyn WindowHandle>>,
}

#[allow(clippy::large_enum_variant)]
pub enum RenderState {
    Active(ActiveRenderState),
    Suspended,
}

pub struct VelloCpuWindowRenderer {
    // The fields MUST be in this order, so that the surface is dropped before the window
    // Window is cached even when suspended so that it can be reused when the app is resumed after being suspended
    render_state: RenderState,
    window_handle: Arc<dyn WindowHandle>,
    render_context: VelloCpuAnyrenderScene,
}

impl WindowRenderer for VelloCpuWindowRenderer {
    type Scene = VelloCpuAnyrenderScene;

    fn new(window: Arc<dyn WindowHandle>) -> Self {
        Self {
            render_state: RenderState::Suspended,
            window_handle: window,
            render_context: VelloCpuAnyrenderScene(RenderContext::new(0, 0)),
        }
    }

    fn is_active(&self) -> bool {
        matches!(self.render_state, RenderState::Active(_))
    }

    fn resume(&mut self, width: u32, height: u32) {
        let context = Context::new(self.window_handle.clone()).unwrap();
        let surface = Surface::new(&context, self.window_handle.clone()).unwrap();
        self.render_state = RenderState::Active(ActiveRenderState {
            _context: context,
            surface,
        });

        self.set_size(width, height);
    }

    fn suspend(&mut self) {
        self.render_state = RenderState::Suspended;
    }

    fn set_size(&mut self, physical_width: u32, physical_height: u32) {
        if let RenderState::Active(state) = &mut self.render_state {
            state
                .surface
                .resize(
                    NonZero::new(physical_width.max(1)).unwrap(),
                    NonZero::new(physical_height.max(1)).unwrap(),
                )
                .unwrap();
            self.render_context = VelloCpuAnyrenderScene(RenderContext::new(
                physical_width as u16,
                physical_height as u16,
            ));
        };
    }

    fn render<F: FnOnce(&mut Self::Scene)>(&mut self, draw_fn: F) {
        let RenderState::Active(state) = &mut self.render_state else {
            return;
        };
        let Ok(mut surface_buffer) = state.surface.buffer_mut() else {
            return;
        };

        let start = Instant::now();

        // Paint
        let width = self.render_context.0.width();
        let height = self.render_context.0.height();
        let mut pixmap = Pixmap::new(width, height);
        draw_fn(&mut self.render_context);

        let command_time = start.elapsed().as_millis();
        let command_ms = command_time;

        self.render_context.0.render_to_pixmap(&mut pixmap);

        let render_time = start.elapsed().as_millis();
        let render_ms = render_time - command_time;

        let out = surface_buffer.as_mut();
        assert_eq!(pixmap.data().len(), out.len());
        for (src, dest) in pixmap.data().iter().zip(out.iter_mut()) {
            let PremulRgba8 { r, g, b, a } = *src;
            if a == 0 {
                *dest = u32::MAX;
            } else {
                *dest = (r as u32) << 16 | (g as u32) << 8 | b as u32;
            }
        }

        let swizel_time = start.elapsed().as_millis();
        let swizel_ms = swizel_time - render_time;

        surface_buffer.present().unwrap();

        let present_time = start.elapsed().as_millis();
        let present_ms = present_time - swizel_time;

        let overall_ms = present_time;

        // Empty the Vello render context (memory optimisation)
        self.render_context.0.reset();

        println!("Frame time: {overall_ms}ms (cmd: {command_ms}ms, render: {render_ms}ms, swizel: {swizel_ms}ms, present: {present_ms}ms)");
    }
}
