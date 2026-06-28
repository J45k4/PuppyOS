mod renderer;

use std::{
    num::NonZeroU32,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use compositor_core::{DesktopCompositor, Point, PointerInteraction, ResizeEdge, Size};
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, Version};
use glutin::display::GetGlDisplay;
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasRawWindowHandle;
use renderer::GlRenderer;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, KeyEvent, MouseButton, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, ModifiersState, NamedKey};
use winit::window::{CursorIcon, WindowBuilder};

fn main() -> Result<()> {
    let event_loop = EventLoop::new()?;
    let window_builder = Some(
        WindowBuilder::new()
            .with_title("PuppyOS Compositor")
            .with_inner_size(PhysicalSize::new(1024, 700)),
    );

    let template = ConfigTemplateBuilder::new().with_alpha_size(8);
    let display_builder = DisplayBuilder::new().with_window_builder(window_builder);
    let (window, gl_config) = display_builder
        .build(&event_loop, template, |configs| {
            configs
                .reduce(|best, config| {
                    if config.num_samples() > best.num_samples() {
                        config
                    } else {
                        best
                    }
                })
                .expect("at least one GL config")
        })
        .map_err(|err| anyhow!("failed to create window and GL config: {err}"))?;

    let window = window.context("display builder did not create a window")?;
    let raw_window_handle = window.raw_window_handle();
    let gl_display = gl_config.display();

    let context_attributes = ContextAttributesBuilder::new()
        .with_context_api(ContextApi::OpenGl(Some(Version::new(3, 3))))
        .build(Some(raw_window_handle));

    let not_current_gl_context = unsafe {
        gl_display
            .create_context(&gl_config, &context_attributes)
            .context("failed to create OpenGL context")?
    };

    let size = window.inner_size();
    let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
        raw_window_handle,
        non_zero(size.width),
        non_zero(size.height),
    );

    let gl_surface = unsafe {
        gl_display
            .create_window_surface(&gl_config, &attrs)
            .context("failed to create OpenGL surface")?
    };

    let gl_context = not_current_gl_context
        .make_current(&gl_surface)
        .context("failed to make OpenGL context current")?;

    let _ = gl_surface.set_swap_interval(&gl_context, SwapInterval::Wait(non_zero(1)));

    let gl = unsafe {
        glow::Context::from_loader_function(|symbol| {
            let symbol = std::ffi::CString::new(symbol).expect("GL symbol contained nul byte");
            gl_display.get_proc_address(&symbol) as *const _
        })
    };

    let mut renderer = unsafe { GlRenderer::new(gl).context("failed to initialize renderer")? };
    let mut compositor = DesktopCompositor::sample();
    let mut pointer_position = Point::new(0.0, 0.0);
    let mut last_click: Option<(Instant, Point)> = None;
    let mut modifiers = ModifiersState::empty();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => elwt.exit(),
                WindowEvent::Resized(size) => {
                    gl_surface.resize(&gl_context, non_zero(size.width), non_zero(size.height));
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    pointer_position = Point::new(position.x as f32, position.y as f32);
                    compositor.pointer_move(pointer_position);
                    let size = window.inner_size();
                    window.set_cursor_icon(cursor_icon(compositor.pointer_interaction(
                        pointer_position,
                        Size::new(size.width as f32, size.height as f32),
                    )));
                    window.request_redraw();
                }
                WindowEvent::MouseInput {
                    state,
                    button: MouseButton::Left,
                    ..
                } => {
                    match state {
                        ElementState::Pressed => {
                            let now = Instant::now();
                            let size = window.inner_size();
                            let desktop_size = Size::new(size.width as f32, size.height as f32);

                            if compositor
                                .activate_taskbar_at(pointer_position, desktop_size)
                                .is_some()
                            {
                                last_click = None;
                            } else if compositor.taskbar_contains(pointer_position, desktop_size) {
                                last_click = None;
                            } else if compositor
                                .activate_control_at(pointer_position, desktop_size)
                                .is_some()
                            {
                                last_click = None;
                            } else {
                                let is_double_click = last_click
                                    .map(|(last_time, last_position)| {
                                        now.duration_since(last_time) <= Duration::from_millis(450)
                                            && point_distance_squared(
                                                last_position,
                                                pointer_position,
                                            ) <= 25.0
                                    })
                                    .unwrap_or(false);

                                if is_double_click
                                    && compositor
                                        .toggle_full_size_at(pointer_position, desktop_size)
                                {
                                    last_click = None;
                                } else {
                                    compositor.pointer_down(pointer_position);
                                    last_click = Some((now, pointer_position));
                                }
                            }
                        }
                        ElementState::Released => compositor.pointer_up(),
                    }
                    window.request_redraw();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if handle_keyboard_input(&mut compositor, &event, modifiers) {
                        window.request_redraw();
                    }
                }
                WindowEvent::ModifiersChanged(new_modifiers) => {
                    modifiers = new_modifiers.state();
                }
                WindowEvent::RedrawRequested => {
                    let size = window.inner_size();
                    let scene = compositor.scene(Size::new(size.width as f32, size.height as f32));

                    unsafe {
                        renderer.render(&scene, size.width, size.height);
                    }

                    if let Err(err) = gl_surface.swap_buffers(&gl_context) {
                        eprintln!("swap buffers failed: {err}");
                        elwt.exit();
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {
                window.request_redraw();
            }
            _ => {}
        }
    })?;

    Ok(())
}

fn non_zero(value: u32) -> NonZeroU32 {
    NonZeroU32::new(value.max(1)).expect("value was clamped to non-zero")
}

fn cursor_icon(interaction: PointerInteraction) -> CursorIcon {
    match interaction {
        PointerInteraction::Default => CursorIcon::Default,
        PointerInteraction::Move => CursorIcon::Move,
        PointerInteraction::Resize(edge) => match edge {
            ResizeEdge::Left => CursorIcon::WResize,
            ResizeEdge::Right => CursorIcon::EResize,
            ResizeEdge::Top => CursorIcon::NResize,
            ResizeEdge::Bottom => CursorIcon::SResize,
            ResizeEdge::TopLeft => CursorIcon::NwResize,
            ResizeEdge::TopRight => CursorIcon::NeResize,
            ResizeEdge::BottomLeft => CursorIcon::SwResize,
            ResizeEdge::BottomRight => CursorIcon::SeResize,
        },
    }
}

fn point_distance_squared(a: Point, b: Point) -> f32 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    dx * dx + dy * dy
}

fn handle_keyboard_input(
    compositor: &mut DesktopCompositor,
    event: &KeyEvent,
    modifiers: ModifiersState,
) -> bool {
    if event.state != ElementState::Pressed {
        return false;
    }

    match &event.logical_key {
        Key::Named(NamedKey::F1) => {
            compositor.toggle_launcher();
            true
        }
        Key::Named(NamedKey::Space) if modifiers.control_key() => {
            compositor.toggle_launcher();
            true
        }
        Key::Named(NamedKey::Escape) if compositor.launcher_is_open() => {
            compositor.close_launcher();
            true
        }
        Key::Named(NamedKey::Enter) if compositor.launcher_is_open() => {
            compositor.launcher_launch_selected();
            true
        }
        Key::Named(NamedKey::Backspace) if compositor.launcher_is_open() => {
            compositor.launcher_backspace();
            true
        }
        Key::Named(NamedKey::ArrowDown) if compositor.launcher_is_open() => {
            compositor.launcher_select_next();
            true
        }
        Key::Named(NamedKey::ArrowUp) if compositor.launcher_is_open() => {
            compositor.launcher_select_previous();
            true
        }
        _ if compositor.launcher_is_open() => {
            if let Some(text) = event.text.as_deref() {
                compositor.launcher_insert_text(text);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}
