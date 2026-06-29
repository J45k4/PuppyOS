#[path = "../../desktop/src/renderer.rs"]
mod renderer;
#[cfg(feature = "smithay-backend")]
mod smithay_backend;

use std::io::Write;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    num::NonZeroU32,
    os::fd::{AsFd, AsRawFd},
    os::unix::{fs::FileExt, fs::PermissionsExt, process::CommandExt},
    path::Path,
    process::{Child, Command},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use compositor_core::{
    DesktopCompositor, ExternalApp, LauncherLaunch, Point, PointerInteraction, ResizeEdge,
    ResourceId, Size, SurfaceHit, WindowId,
};
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, Version};
use glutin::display::GetGlDisplay;
use glutin::display::{AsRawDisplay, RawDisplay};
use glutin::prelude::*;
use glutin::surface::{SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasRawWindowHandle;
use renderer::{
    DmabufImport, DmabufPlane as RendererDmabufPlane, EglDmabufImporter, GlRenderer,
    SurfacePixelFormat,
};
use wayland_protocols::wp::linux_dmabuf::zv1::server::{
    zwp_linux_buffer_params_v1, zwp_linux_dmabuf_v1,
};
use wayland_protocols::xdg::decoration::zv1::server::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::server::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_server::{
    BindError, Client, DataInit, Dispatch, Display, DisplayHandle, GlobalDispatch, ListeningSocket,
    New, Resource, WEnum,
    backend::{ClientData, ClientId, DisconnectReason},
    protocol::{
        wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager,
        wl_data_source, wl_keyboard, wl_output, wl_pointer, wl_region, wl_seat, wl_shm,
        wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
    },
};
use winit::dpi::PhysicalSize;
use winit::error::EventLoopError;
use winit::event::{ElementState, Event, KeyEvent, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use winit::window::{CursorIcon, WindowBuilder};

static TERMINATION_REQUESTED: AtomicBool = AtomicBool::new(false);

fn main() -> Result<()> {
    install_termination_signal_handlers()
        .context("failed to install termination signal handlers")?;

    #[cfg(feature = "smithay-backend")]
    if std::env::args().any(|arg| arg == "--smithay") {
        return smithay_backend::run();
    }

    if std::env::args().any(|arg| arg == "--headless") {
        let mut wayland = NestedWaylandServer::new(false)?;
        print_startup(&wayland);
        return wayland.run_headless();
    }

    let event_loop = EventLoop::new()?;
    let window_builder = Some(
        WindowBuilder::new()
            .with_title("PuppyOS Nested Compositor")
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
    if let RawDisplay::Egl(display) = gl_display.raw_display() {
        if let Some(importer) =
            unsafe { EglDmabufImporter::load(display, |name| gl_display.get_proc_address(name)) }
        {
            renderer.set_egl_dmabuf_importer(importer);
        }
    }
    let enable_dmabuf = renderer.supports_dmabuf_import() && dmabuf_requested();
    let mut wayland = NestedWaylandServer::new(enable_dmabuf)?;
    print_startup(&wayland);
    let mut pointer_position = Point::new(0.0, 0.0);
    let mut last_click: Option<(Instant, Point)> = None;
    let mut modifiers = ModifiersState::empty();
    let start_time = Instant::now();

    let run_result = event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(8),
        ));

        if termination_requested() {
            elwt.exit();
            return;
        }

        if let Err(err) = wayland.pump() {
            eprintln!("Wayland server error: {err:#}");
            elwt.exit();
            return;
        }
        let uploads = wayland.drain_surface_uploads();
        let has_uploads = !uploads.is_empty();
        for upload in uploads {
            match upload {
                SurfaceUpload::Pixels {
                    id,
                    width,
                    height,
                    format,
                    pixels,
                } => unsafe {
                    renderer.set_surface_pixels(id, width, height, format, &pixels);
                },
                SurfaceUpload::Dmabuf { id, buffer } => {
                    if let Some(import) = dmabuf_import_for_renderer(&buffer) {
                        unsafe {
                            renderer.set_surface_dmabuf(id, &import);
                        }
                    }
                }
            }
        }
        if has_uploads {
            window.request_redraw();
        }
        if wayland.has_pending_frame_callbacks() {
            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    eprintln!("host window close requested; shutting down nested compositor");
                    elwt.exit();
                }
                WindowEvent::Destroyed => {
                    eprintln!("host window destroyed by the outer compositor; shutting down");
                    elwt.exit();
                }
                WindowEvent::Resized(size) => {
                    if size.width == 0 || size.height == 0 {
                        eprintln!(
                            "host window resized to {}x{}; rendering will resume when it is visible again",
                            size.width, size.height
                        );
                    }
                    gl_surface.resize(&gl_context, non_zero(size.width), non_zero(size.height));
                    wayland.set_desktop_size(Size::new(size.width as f32, size.height as f32));
                    wayland.configure_changed_surface_sizes();
                    window.request_redraw();
                }
                WindowEvent::Focused(focused) => {
                    if !focused {
                        eprintln!("host window lost focus");
                    }
                }
                WindowEvent::Occluded(occluded) => {
                    eprintln!("host window occluded={occluded}");
                }
                WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                    eprintln!("host window scale factor changed to {scale_factor}");
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    pointer_position = Point::new(position.x as f32, position.y as f32);
                    let size = window.inner_size();
                    let desktop_size = Size::new(size.width as f32, size.height as f32);
                    wayland.set_desktop_size(desktop_size);
                    wayland.compositor_mut().pointer_move(pointer_position);
                    wayland.configure_changed_surface_sizes();
                    if !wayland.compositor().pointer_grab_active() {
                        wayland.send_pointer_motion(
                            pointer_position,
                            desktop_size,
                            event_time_ms(start_time),
                        );
                    }
                    window.set_cursor_icon(cursor_icon(
                        wayland
                            .compositor()
                            .pointer_interaction(pointer_position, desktop_size),
                    ));
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

                            if matches!(
                                wayland
                                    .compositor()
                                    .pointer_interaction(pointer_position, desktop_size),
                                PointerInteraction::Resize(_)
                            ) {
                                wayland.compositor_mut().pointer_down(pointer_position);
                                last_click = None;
                            } else if wayland.send_pointer_button(
                                pointer_position,
                                desktop_size,
                                event_time_ms(start_time),
                                MouseButton::Left,
                                ElementState::Pressed,
                            ) {
                                last_click = None;
                            } else if wayland
                                .compositor_mut()
                                .activate_taskbar_at(pointer_position, desktop_size)
                                .is_some()
                            {
                                last_click = None;
                            } else if wayland
                                .compositor()
                                .taskbar_contains(pointer_position, desktop_size)
                            {
                                last_click = None;
                            } else if wayland
                                .compositor_mut()
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
                                    && wayland
                                        .compositor_mut()
                                        .toggle_full_size_at(pointer_position, desktop_size)
                                {
                                    last_click = None;
                                } else {
                                    wayland.compositor_mut().pointer_down(pointer_position);
                                    last_click = Some((now, pointer_position));
                                }
                            }
                        }
                        ElementState::Released => {
                            let size = window.inner_size();
                            let desktop_size = Size::new(size.width as f32, size.height as f32);
                            wayland.send_pointer_button(
                                pointer_position,
                                desktop_size,
                                event_time_ms(start_time),
                                MouseButton::Left,
                                ElementState::Released,
                            );
                            wayland.compositor_mut().pointer_up();
                        }
                    }
                    window.request_redraw();
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    let size = window.inner_size();
                    let desktop_size = Size::new(size.width as f32, size.height as f32);
                    if wayland.send_pointer_scroll(
                        pointer_position,
                        desktop_size,
                        event_time_ms(start_time),
                        delta,
                    ) {
                        window.request_redraw();
                    }
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    if handle_keyboard_input(
                        &mut wayland,
                        &event,
                        modifiers,
                        event_time_ms(start_time),
                    ) {
                        window.request_redraw();
                    }
                }
                WindowEvent::ModifiersChanged(new_modifiers) => {
                    modifiers = new_modifiers.state();
                    wayland.send_keyboard_modifiers(modifiers);
                }
                WindowEvent::RedrawRequested => {
                    let size = window.inner_size();
                    let scene = wayland
                        .compositor()
                        .scene(Size::new(size.width as f32, size.height as f32));

                    unsafe {
                        renderer.render(&scene, size.width, size.height);
                    }

                    if let Err(err) = gl_surface.swap_buffers(&gl_context) {
                        eprintln!("swap buffers failed: {err}");
                        elwt.exit();
                    } else {
                        wayland.complete_frame_callbacks(event_time_ms(start_time));
                    }
                }
                _ => {}
            },
            Event::AboutToWait => {}
            Event::LoopExiting => {
                wayland.stop_launched_apps();
            }
            _ => {}
        }
    });

    match run_result {
        Ok(()) => Ok(()),
        Err(EventLoopError::ExitFailure(code)) => {
            eprintln!("host event loop exited with status {code}; nested compositor shut down");
            Ok(())
        }
        Err(err) => Err(err).context("host event loop failed"),
    }
}

fn print_startup(wayland: &NestedWaylandServer) {
    println!(
        "PuppyOS nested Wayland server listening on {}",
        wayland.socket_name()
    );
    println!("Launch clients with:");
    println!("  WAYLAND_DISPLAY={} <app>", wayland.socket_name());
    println!();
    println!("The nested host window renders compositor-core now.");
    if wayland.dmabuf_enabled() {
        println!(
            "Core Wayland globals are advertised; wl_shm and linux-dmabuf app buffers are uploaded as compositor surfaces."
        );
    } else {
        println!(
            "Core Wayland globals are advertised; wl_shm app buffers are uploaded as compositor surfaces."
        );
    }
}

fn dmabuf_requested() -> bool {
    std::env::var_os("PUPPYOS_ENABLE_DMABUF").is_some()
}

fn install_termination_signal_handlers() -> std::io::Result<()> {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = handle_termination_signal as *const () as usize;
        action.sa_flags = 0;
        libc::sigemptyset(&mut action.sa_mask);

        for signal in [libc::SIGINT, libc::SIGTERM] {
            if libc::sigaction(signal, &action, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
    }
    Ok(())
}

extern "C" fn handle_termination_signal(_signal: libc::c_int) {
    TERMINATION_REQUESTED.store(true, Ordering::SeqCst);
}

fn termination_requested() -> bool {
    TERMINATION_REQUESTED.load(Ordering::SeqCst)
}

struct NestedWaylandServer {
    display: Display<NestedState>,
    state: NestedState,
    socket: ListeningSocket,
    socket_name: String,
    launched_apps: Vec<LaunchedAppProcess>,
}

impl NestedWaylandServer {
    fn new(enable_dmabuf: bool) -> Result<Self> {
        let display = Display::new().context("failed to create Wayland display")?;
        let handle = display.handle();
        handle.create_global::<NestedState, wl_compositor::WlCompositor, _>(6, ());
        handle.create_global::<NestedState, wl_subcompositor::WlSubcompositor, _>(1, ());
        handle.create_global::<NestedState, wl_shm::WlShm, _>(1, ());
        handle.create_global::<NestedState, wl_data_device_manager::WlDataDeviceManager, _>(3, ());
        handle.create_global::<NestedState, wl_seat::WlSeat, _>(7, ());
        handle.create_global::<NestedState, wl_output::WlOutput, _>(4, ());
        handle.create_global::<NestedState, xdg_wm_base::XdgWmBase, _>(6, ());
        handle
            .create_global::<NestedState, zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, _>(
                1,
                (),
            );
        if enable_dmabuf {
            handle.create_global::<NestedState, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, _>(3, ());
        }
        let BoundSocket {
            socket,
            display_name,
        } = bind_socket().context("failed to bind PuppyOS Wayland socket")?;

        Ok(Self {
            display,
            state: NestedState::new(enable_dmabuf),
            socket,
            socket_name: display_name,
            launched_apps: Vec::new(),
        })
    }

    fn socket_name(&self) -> &str {
        &self.socket_name
    }

    fn dmabuf_enabled(&self) -> bool {
        self.state.dmabuf_enabled
    }

    fn compositor(&self) -> &DesktopCompositor {
        &self.state.compositor
    }

    fn compositor_mut(&mut self) -> &mut DesktopCompositor {
        &mut self.state.compositor
    }

    fn set_desktop_size(&mut self, size: Size) -> bool {
        self.state.desktop_size = size;
        self.state.compositor.resize_full_size_windows(size)
    }

    fn launch_app(&mut self, launch: LauncherLaunch) {
        match launch {
            LauncherLaunch::Window(_) => {}
            LauncherLaunch::External(ExternalApp::Telegram) => self.launch_telegram(),
            LauncherLaunch::External(ExternalApp::Chrome) => self.launch_chrome(),
            LauncherLaunch::External(ExternalApp::Brave) => self.launch_brave(),
        }
    }

    fn launch_telegram(&mut self) {
        let workdir = "/tmp/telegram-puppyos-nested";
        if let Err(err) = fs::create_dir_all(workdir) {
            eprintln!("failed to create Telegram workdir {workdir}: {err}");
            return;
        }

        let mut command = Command::new("flatpak");
        put_child_in_own_process_group(&mut command);
        remove_injected_library_env(&mut command);

        command
            .args([
                "run",
                "--socket=wayland",
                "--env=QT_QPA_PLATFORM=wayland",
                "--env=QT_WAYLAND_DISABLE_WINDOWDECORATION=1",
                "--filesystem=/tmp",
            ])
            .arg(format!("--env=WAYLAND_DISPLAY={}", self.socket_name))
            .args(["org.telegram.desktop", "-many", "-workdir", workdir])
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("QT_QPA_PLATFORM", "wayland")
            .env("QT_WAYLAND_DISABLE_WINDOWDECORATION", "1");

        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                eprintln!(
                    "launched Telegram in PuppyOS compositor on WAYLAND_DISPLAY={} pid={}",
                    self.socket_name, pid
                );
                self.launched_apps.push(LaunchedAppProcess::new(
                    "Telegram",
                    child,
                    process_group_from_pid(pid),
                ));
            }
            Err(err) => {
                eprintln!("failed to launch Telegram Flatpak: {err}");
            }
        }
    }

    fn launch_chrome(&mut self) {
        let user_data_dir = "/tmp/chrome-puppyos-nested";
        if let Err(err) = reset_chrome_profile(user_data_dir) {
            eprintln!("failed to create Chrome profile dir {user_data_dir}: {err}");
            return;
        }

        let chrome_binary = if Path::new("/opt/google/chrome/chrome").exists() {
            "/opt/google/chrome/chrome"
        } else {
            "google-chrome"
        };
        let mut command = Command::new(chrome_binary);
        put_child_in_own_process_group(&mut command);
        remove_injected_library_env(&mut command);
        command
            .env("WAYLAND_DISPLAY", &self.socket_name)
            .env("XDG_SESSION_TYPE", "wayland")
            .args([
                "--enable-features=UseOzonePlatform",
                "--ozone-platform=wayland",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-accelerated-video-decode",
                "--disable-gpu-memory-buffer-video-frames",
                "--disable-zero-copy",
            ]);
        if self.dmabuf_enabled() {
            command.args(["--disable-vulkan", "--disable-features=Vulkan"]);
        } else {
            command.arg("--disable-gpu");
        }
        command
            .arg(format!("--user-data-dir={user_data_dir}"))
            .arg("https://www.youtube.com/results?search_query=cat+video");

        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                eprintln!(
                    "launched Chrome in PuppyOS compositor on WAYLAND_DISPLAY={} pid={}",
                    self.socket_name, pid
                );
                self.launched_apps.push(LaunchedAppProcess::new(
                    "Chrome",
                    child,
                    process_group_from_pid(pid),
                ));
            }
            Err(err) => {
                eprintln!("failed to launch Chrome: {err}");
            }
        }
    }

    fn launch_brave(&mut self) {
        let user_data_dir = "/tmp/brave-puppyos-nested";
        if let Err(err) = reset_chrome_profile(user_data_dir) {
            eprintln!("failed to create Brave profile dir {user_data_dir}: {err}");
            return;
        }

        let mut command = if let Some(brave_binary) = find_first_existing_binary(&[
            "brave-browser",
            "brave",
            "/opt/brave.com/brave/brave-browser",
        ]) {
            let mut command = Command::new(brave_binary);
            command
                .env("WAYLAND_DISPLAY", &self.socket_name)
                .env("XDG_SESSION_TYPE", "wayland");
            command
        } else {
            let mut command = Command::new("flatpak");
            command
                .args(["run", "--socket=wayland", "--filesystem=/tmp"])
                .arg(format!("--env=WAYLAND_DISPLAY={}", self.socket_name))
                .arg("--env=XDG_SESSION_TYPE=wayland")
                .arg("com.brave.Browser")
                .env("WAYLAND_DISPLAY", &self.socket_name)
                .env("XDG_SESSION_TYPE", "wayland");
            command
        };
        put_child_in_own_process_group(&mut command);
        remove_injected_library_env(&mut command);
        command.args([
            "--enable-features=UseOzonePlatform",
            "--ozone-platform=wayland",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-accelerated-video-decode",
            "--disable-gpu-memory-buffer-video-frames",
            "--disable-zero-copy",
        ]);
        if self.dmabuf_enabled() {
            command.args(["--disable-vulkan", "--disable-features=Vulkan"]);
        } else {
            command.arg("--disable-gpu");
        }
        command
            .arg(format!("--user-data-dir={user_data_dir}"))
            .arg("about:blank");

        match command.spawn() {
            Ok(child) => {
                let pid = child.id();
                eprintln!(
                    "launched Brave in PuppyOS compositor on WAYLAND_DISPLAY={} pid={}",
                    self.socket_name, pid
                );
                self.launched_apps.push(LaunchedAppProcess::new(
                    "Brave",
                    child,
                    process_group_from_pid(pid),
                ));
            }
            Err(err) => {
                eprintln!("failed to launch Brave: {err}");
            }
        }
    }

    fn send_pointer_motion(&mut self, position: Point, desktop_size: Size, time: u32) -> bool {
        self.state.pointer_position = position;
        let Some(hit) = self.pointer_hit(position, desktop_size) else {
            self.clear_pointer_focus();
            return false;
        };

        self.enter_pointer_focus(hit);
        self.send_to_focused_pointers(|pointer, _surface| {
            pointer.motion(time, hit.position.x as f64, hit.position.y as f64);
            send_pointer_frame(pointer);
        });
        true
    }

    fn send_pointer_button(
        &mut self,
        position: Point,
        desktop_size: Size,
        time: u32,
        button: MouseButton,
        state: ElementState,
    ) -> bool {
        self.state.pointer_position = position;
        if state == ElementState::Pressed {
            let Some(hit) = self.pointer_hit(position, desktop_size) else {
                self.clear_pointer_focus();
                self.clear_keyboard_focus();
                return false;
            };
            self.enter_pointer_focus(hit);
            self.enter_keyboard_focus(hit.surface.0 as u32);
        } else if self.state.pointer_focus.is_none() {
            return false;
        }

        let Some(button) = pointer_button_code(button) else {
            return false;
        };
        let button_state = match state {
            ElementState::Pressed => wl_pointer::ButtonState::Pressed,
            ElementState::Released => wl_pointer::ButtonState::Released,
        };
        let serial = self.state.next_serial();

        self.send_to_focused_pointers(|pointer, _surface| {
            pointer.button(serial, time, button, button_state);
            send_pointer_frame(pointer);
        });
        true
    }

    fn send_keyboard_input(
        &mut self,
        event: &KeyEvent,
        modifiers: ModifiersState,
        time: u32,
    ) -> bool {
        self.send_keyboard_modifiers(modifiers);

        let Some(keycode) = evdev_keycode(event) else {
            return false;
        };
        let Some(_surface_id) = self.state.keyboard_focus else {
            return false;
        };

        let state = match event.state {
            ElementState::Pressed => wl_keyboard::KeyState::Pressed,
            ElementState::Released => wl_keyboard::KeyState::Released,
        };
        let serial = self.state.next_serial();
        self.send_to_focused_keyboards(|keyboard, _surface| {
            keyboard.key(serial, time, keycode, state);
        });
        if trace_wayland() {
            eprintln!(
                "keyboard key time={time} keycode={keycode} state={:?}",
                event.state
            );
        }
        true
    }

    fn send_keyboard_modifiers(&mut self, modifiers: ModifiersState) {
        if self.state.keyboard_modifiers == modifiers || self.state.keyboard_focus.is_none() {
            self.state.keyboard_modifiers = modifiers;
            return;
        }

        self.state.keyboard_modifiers = modifiers;
        self.send_current_keyboard_modifiers();
    }

    fn send_current_keyboard_modifiers(&mut self) {
        if self.state.keyboard_focus.is_none() {
            return;
        }

        let serial = self.state.next_serial();
        let depressed = xkb_depressed_modifiers(self.state.keyboard_modifiers);
        self.send_to_focused_keyboards(|keyboard, _surface| {
            keyboard.modifiers(serial, depressed, 0, 0, 0);
        });
        if trace_wayland() {
            eprintln!("keyboard modifiers depressed={depressed}");
        }
    }

    fn send_pointer_scroll(
        &mut self,
        position: Point,
        desktop_size: Size,
        time: u32,
        delta: MouseScrollDelta,
    ) -> bool {
        self.state.pointer_position = position;
        let Some(hit) = self.pointer_hit(position, desktop_size) else {
            self.clear_pointer_focus();
            return false;
        };
        self.enter_pointer_focus(hit);

        let (horizontal, vertical, discrete_x, discrete_y) = match delta {
            MouseScrollDelta::LineDelta(x, y) => {
                let units_per_line = 15.0;
                (
                    -(x as f64) * units_per_line,
                    -(y as f64) * units_per_line,
                    Some(-(x.round() as i32)),
                    Some(-(y.round() as i32)),
                )
            }
            MouseScrollDelta::PixelDelta(position) => (position.x, position.y, None, None),
        };

        if horizontal == 0.0 && vertical == 0.0 {
            return false;
        }

        self.send_to_focused_pointers(|pointer, _surface| {
            if pointer.version() >= 5 {
                pointer.axis_source(wl_pointer::AxisSource::Wheel);
            }

            if horizontal != 0.0 {
                send_pointer_axis(
                    pointer,
                    time,
                    wl_pointer::Axis::HorizontalScroll,
                    horizontal,
                );
                if let Some(discrete) = discrete_x.filter(|value| *value != 0) {
                    send_pointer_axis_discrete(
                        pointer,
                        wl_pointer::Axis::HorizontalScroll,
                        discrete,
                    );
                }
            }

            if vertical != 0.0 {
                send_pointer_axis(pointer, time, wl_pointer::Axis::VerticalScroll, vertical);
                if let Some(discrete) = discrete_y.filter(|value| *value != 0) {
                    send_pointer_axis_discrete(pointer, wl_pointer::Axis::VerticalScroll, discrete);
                }
            }

            send_pointer_frame(pointer);
        });
        true
    }

    fn pointer_hit(&self, position: Point, desktop_size: Size) -> Option<SurfaceHit> {
        self.state
            .compositor
            .surface_hit_at(position, desktop_size)
            .filter(|hit| {
                self.state
                    .surface_resources
                    .contains_key(&(hit.surface.0 as u32))
            })
    }

    fn enter_pointer_focus(&mut self, hit: SurfaceHit) {
        let surface_id = hit.surface.0 as u32;
        if self.state.pointer_focus == Some(surface_id) {
            return;
        }

        self.clear_pointer_focus();
        let Some(surface) = self.state.surface_resources.get(&surface_id).cloned() else {
            return;
        };
        let serial = self.state.next_serial();

        for pointer in self.pointers_for_surface(&surface) {
            pointer.enter(
                serial,
                &surface,
                hit.position.x as f64,
                hit.position.y as f64,
            );
            send_pointer_frame(&pointer);
        }
        self.state.pointer_focus = Some(surface_id);
    }

    fn clear_pointer_focus(&mut self) {
        let Some(surface_id) = self.state.pointer_focus.take() else {
            return;
        };
        let Some(surface) = self.state.surface_resources.get(&surface_id).cloned() else {
            return;
        };
        let serial = self.state.next_serial();

        for pointer in self.pointers_for_surface(&surface) {
            pointer.leave(serial, &surface);
            send_pointer_frame(&pointer);
        }
    }

    fn enter_keyboard_focus(&mut self, surface_id: u32) {
        if self.state.keyboard_focus == Some(surface_id) {
            return;
        }

        self.clear_keyboard_focus();
        let Some(surface) = self.state.surface_resources.get(&surface_id).cloned() else {
            return;
        };
        let serial = self.state.next_serial();

        for keyboard in self.keyboards_for_surface(&surface) {
            keyboard.enter(serial, &surface, Vec::new());
        }
        self.state.keyboard_focus = Some(surface_id);
        self.send_current_keyboard_modifiers();
        for (id, state) in &mut self.state.surface_toplevel_states {
            state.activated = *id == surface_id;
        }
        if trace_wayland() {
            eprintln!("keyboard focus entered surface_id={surface_id}");
        }
    }

    fn clear_keyboard_focus(&mut self) {
        let Some(surface_id) = self.state.keyboard_focus.take() else {
            return;
        };
        if let Some(state) = self.state.surface_toplevel_states.get_mut(&surface_id) {
            state.activated = false;
        }
        let Some(surface) = self.state.surface_resources.get(&surface_id).cloned() else {
            return;
        };
        let serial = self.state.next_serial();

        for keyboard in self.keyboards_for_surface(&surface) {
            keyboard.leave(serial, &surface);
        }
        if trace_wayland() {
            eprintln!("keyboard focus left surface_id={surface_id}");
        }
    }

    fn send_to_focused_pointers(
        &self,
        mut send: impl FnMut(&wl_pointer::WlPointer, &wl_surface::WlSurface),
    ) {
        let Some(surface_id) = self.state.pointer_focus else {
            return;
        };
        let Some(surface) = self.state.surface_resources.get(&surface_id) else {
            return;
        };

        for pointer in self.pointers_for_surface(surface) {
            send(&pointer, surface);
        }
    }

    fn send_to_focused_keyboards(
        &self,
        mut send: impl FnMut(&wl_keyboard::WlKeyboard, &wl_surface::WlSurface),
    ) {
        let Some(surface_id) = self.state.keyboard_focus else {
            return;
        };
        let Some(surface) = self.state.surface_resources.get(&surface_id) else {
            return;
        };

        for keyboard in self.keyboards_for_surface(surface) {
            send(&keyboard, surface);
        }
    }

    fn pointers_for_surface(&self, surface: &wl_surface::WlSurface) -> Vec<wl_pointer::WlPointer> {
        self.state
            .pointers
            .iter()
            .filter(|pointer| pointer.is_alive() && same_wayland_client(*pointer, surface))
            .cloned()
            .collect()
    }

    fn keyboards_for_surface(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> Vec<wl_keyboard::WlKeyboard> {
        self.state
            .keyboards
            .iter()
            .filter(|keyboard| keyboard.is_alive() && same_wayland_client(*keyboard, surface))
            .cloned()
            .collect()
    }

    fn configure_changed_surface_sizes(&mut self) {
        let surface_ids: Vec<u32> = self.state.surface_windows.keys().copied().collect();
        for surface_id in surface_ids {
            let surface = ResourceId(surface_id as u64);
            let Some(size) = self
                .state
                .compositor
                .surface_window_requested_content_size(surface)
            else {
                continue;
            };
            self.state.configure_surface_id(surface_id, size);
        }
    }

    fn drain_surface_uploads(&mut self) -> Vec<SurfaceUpload> {
        self.state.surface_uploads.drain(..).collect()
    }

    fn has_pending_frame_callbacks(&self) -> bool {
        self.state
            .pending_frame_callbacks
            .values()
            .any(|callbacks| !callbacks.is_empty())
    }

    fn complete_frame_callbacks(&mut self, time: u32) {
        for callbacks in std::mem::take(&mut self.state.pending_frame_callbacks).into_values() {
            for callback in callbacks {
                callback.done(time);
            }
        }
    }

    fn pump(&mut self) -> Result<()> {
        self.reap_launched_apps();
        self.accept_pending_clients()?;
        if let Err(err) = self.display.dispatch_clients(&mut self.state) {
            if is_client_disconnect_io(&err) {
                if trace_wayland() {
                    eprintln!("ignored Wayland client dispatch disconnect: {err}");
                }
            } else {
                return Err(err).context("failed to dispatch Wayland clients");
            }
        }
        if let Err(err) = self.display.flush_clients() {
            if is_client_disconnect_io(&err) {
                if trace_wayland() {
                    eprintln!("ignored Wayland client flush disconnect: {err}");
                }
            } else {
                return Err(err).context("failed to flush Wayland clients");
            }
        }

        Ok(())
    }

    fn reap_launched_apps(&mut self) {
        self.launched_apps
            .retain_mut(|app| match app.child.try_wait() {
                Ok(Some(status)) => {
                    if trace_wayland() {
                        eprintln!("{} exited with {status}", app.name);
                    }
                    false
                }
                Ok(None) => true,
                Err(err) => {
                    eprintln!("failed to poll {} process: {err}", app.name);
                    false
                }
            });
    }

    fn stop_launched_apps(&mut self) {
        for app in &mut self.launched_apps {
            app.terminate();
        }
        self.launched_apps.clear();
    }

    fn run_headless(&mut self) -> Result<()> {
        println!("Running without a host window.");

        while !termination_requested() {
            self.pump()?;
            std::thread::sleep(Duration::from_millis(1));
        }

        self.stop_launched_apps();
        Ok(())
    }

    fn accept_pending_clients(&mut self) -> Result<()> {
        let mut handle = self.display.handle();

        while let Some(stream) = self
            .socket
            .accept()
            .context("failed to accept Wayland client")?
        {
            handle
                .insert_client(stream, Arc::new(LoggedClient))
                .context("failed to initialize Wayland client")?;
        }

        Ok(())
    }
}

impl Drop for NestedWaylandServer {
    fn drop(&mut self) {
        self.stop_launched_apps();
    }
}

#[derive(Debug)]
struct LaunchedAppProcess {
    name: &'static str,
    child: Child,
    process_group: i32,
}

impl LaunchedAppProcess {
    fn new(name: &'static str, child: Child, process_group: i32) -> Self {
        Self {
            name,
            child,
            process_group,
        }
    }

    fn terminate(&mut self) {
        if matches!(self.child.try_wait(), Ok(Some(_))) {
            return;
        }

        signal_process_group(self.process_group, libc::SIGTERM);
        for _ in 0..10 {
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(err) => {
                    eprintln!("failed to wait for {} after SIGTERM: {err}", self.name);
                    return;
                }
            }
        }

        signal_process_group(self.process_group, libc::SIGKILL);
        if let Err(err) = self.child.wait() {
            eprintln!("failed to wait for {} after SIGKILL: {err}", self.name);
        }
    }
}

fn put_child_in_own_process_group(command: &mut Command) {
    unsafe {
        command.pre_exec(|| {
            if libc::setpgid(0, 0) == 0 {
                Ok(())
            } else {
                Err(std::io::Error::last_os_error())
            }
        });
    }
}

fn process_group_from_pid(pid: u32) -> i32 {
    pid.min(i32::MAX as u32) as i32
}

fn signal_process_group(process_group: i32, signal: i32) {
    if process_group <= 0 {
        return;
    }
    unsafe {
        libc::kill(-process_group, signal);
    }
}

fn reset_chrome_profile(path: &str) -> std::io::Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    fs::create_dir_all(path)
}

fn find_first_existing_binary(candidates: &[&str]) -> Option<String> {
    for candidate in candidates {
        let path = Path::new(candidate);
        if candidate.contains('/') {
            if is_executable_file(path) {
                return Some((*candidate).to_string());
            }
            continue;
        }

        let Some(paths) = std::env::var_os("PATH") else {
            continue;
        };
        for directory in std::env::split_paths(&paths) {
            let binary = directory.join(candidate);
            if is_executable_file(&binary) {
                return Some(binary.to_string_lossy().into_owned());
            }
        }
    }

    None
}

fn is_executable_file(path: &Path) -> bool {
    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn remove_injected_library_env(command: &mut Command) {
    for key in [
        "LD_LIBRARY_PATH",
        "LD_PRELOAD",
        "LD_AUDIT",
        "GI_TYPELIB_PATH",
        "GIO_EXTRA_MODULES",
        "GST_PLUGIN_PATH",
        "GST_PLUGIN_SYSTEM_PATH",
        "QT_PLUGIN_PATH",
        "QML2_IMPORT_PATH",
    ] {
        command.env_remove(key);
    }
}

fn is_client_disconnect_io(err: &std::io::Error) -> bool {
    matches!(
        err.kind(),
        std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
    )
}

#[derive(Debug)]
struct NestedState {
    compositor: DesktopCompositor,
    dmabuf_enabled: bool,
    desktop_size: Size,
    next_serial: u32,
    configured_surfaces: HashSet<u32>,
    pending_frame_callbacks: HashMap<u32, Vec<wl_callback::WlCallback>>,
    pending_attached_buffers: HashMap<u32, Option<wl_buffer::WlBuffer>>,
    surface_buffer_scales: HashMap<u32, i32>,
    surface_resources: HashMap<u32, wl_surface::WlSurface>,
    surface_xdg_surfaces: HashMap<u32, xdg_surface::XdgSurface>,
    surface_toplevels: HashMap<u32, xdg_toplevel::XdgToplevel>,
    surface_windows: HashMap<u32, WindowId>,
    surface_configured_sizes: HashMap<u32, Size>,
    surface_toplevel_states: HashMap<u32, ToplevelState>,
    surface_uploads: Vec<SurfaceUpload>,
    pointers: Vec<wl_pointer::WlPointer>,
    pointer_focus: Option<u32>,
    pointer_position: Point,
    keyboards: Vec<wl_keyboard::WlKeyboard>,
    keyboard_focus: Option<u32>,
    keyboard_modifiers: ModifiersState,
    keymap_files: Vec<File>,
}

impl NestedState {
    fn new(dmabuf_enabled: bool) -> Self {
        Self {
            compositor: DesktopCompositor::sample(),
            dmabuf_enabled,
            desktop_size: Size::new(1024.0, 700.0),
            next_serial: 1,
            configured_surfaces: HashSet::new(),
            pending_frame_callbacks: HashMap::new(),
            pending_attached_buffers: HashMap::new(),
            surface_buffer_scales: HashMap::new(),
            surface_resources: HashMap::new(),
            surface_xdg_surfaces: HashMap::new(),
            surface_toplevels: HashMap::new(),
            surface_windows: HashMap::new(),
            surface_configured_sizes: HashMap::new(),
            surface_toplevel_states: HashMap::new(),
            surface_uploads: Vec::new(),
            pointers: Vec::new(),
            pointer_focus: None,
            pointer_position: Point::new(0.0, 0.0),
            keyboards: Vec::new(),
            keyboard_focus: None,
            keyboard_modifiers: ModifiersState::empty(),
            keymap_files: Vec::new(),
        }
    }

    fn next_serial(&mut self) -> u32 {
        let serial = self.next_serial;
        self.next_serial = self.next_serial.wrapping_add(1).max(1);
        serial
    }

    fn configure_toplevel(
        &mut self,
        surface_id: u32,
        surface: &xdg_surface::XdgSurface,
        toplevel: &xdg_toplevel::XdgToplevel,
        size: Size,
    ) {
        let serial = self.next_serial();
        let width = size.width.round().max(1.0) as i32;
        let height = size.height.round().max(1.0) as i32;
        let states = self
            .surface_toplevel_states
            .get(&surface_id)
            .copied()
            .unwrap_or_default()
            .xdg_states();
        toplevel.configure(width, height, states);
        surface.configure(serial);
        self.configured_surfaces.insert(surface_id);
        self.surface_configured_sizes
            .insert(surface_id, Size::new(width as f32, height as f32));
        if trace_wayland() {
            let state = self
                .surface_toplevel_states
                .get(&surface_id)
                .copied()
                .unwrap_or_default();
            eprintln!(
                "Wayland xdg toplevel configured: surface={} serial={serial} size={}x{} state={:?}",
                surface_id, width, height, state
            );
        }
    }

    fn configure_surface_id(&mut self, surface_id: u32, size: Size) -> bool {
        let Some(surface) = self.surface_xdg_surfaces.get(&surface_id).cloned() else {
            return false;
        };
        let Some(toplevel) = self.surface_toplevels.get(&surface_id).cloned() else {
            return false;
        };

        if self
            .surface_configured_sizes
            .get(&surface_id)
            .is_some_and(|configured| {
                (configured.width - size.width).abs() < 0.5
                    && (configured.height - size.height).abs() < 0.5
            })
        {
            return false;
        }

        self.configure_toplevel(surface_id, &surface, &toplevel, size);
        true
    }

    fn remove_surface_window(&mut self, surface_id: u32) {
        if self.pointer_focus == Some(surface_id) {
            self.pointer_focus = None;
        }
        if self.keyboard_focus == Some(surface_id) {
            self.keyboard_focus = None;
        }

        if let Some(window) = self.surface_windows.remove(&surface_id) {
            self.compositor.remove_window(window);
        }

        self.configured_surfaces.remove(&surface_id);
        self.pending_frame_callbacks.remove(&surface_id);
        self.pending_attached_buffers.remove(&surface_id);
        self.surface_buffer_scales.remove(&surface_id);
        self.surface_xdg_surfaces.remove(&surface_id);
        self.surface_toplevels.remove(&surface_id);
        self.surface_configured_sizes.remove(&surface_id);
        self.surface_toplevel_states.remove(&surface_id);
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct ToplevelState {
    maximized: bool,
    fullscreen: bool,
    activated: bool,
}

impl ToplevelState {
    fn xdg_states(self) -> Vec<u8> {
        let mut states = Vec::new();
        if self.maximized {
            push_xdg_state(&mut states, xdg_toplevel::State::Maximized);
        }
        if self.fullscreen {
            push_xdg_state(&mut states, xdg_toplevel::State::Fullscreen);
        }
        if self.activated {
            push_xdg_state(&mut states, xdg_toplevel::State::Activated);
        }
        states
    }
}

fn push_xdg_state(states: &mut Vec<u8>, state: xdg_toplevel::State) {
    states.extend_from_slice(&(state as u32).to_ne_bytes());
}

fn xdg_wm_capabilities() -> Vec<u8> {
    let mut capabilities = Vec::new();
    push_xdg_wm_capability(&mut capabilities, xdg_toplevel::WmCapabilities::Maximize);
    push_xdg_wm_capability(&mut capabilities, xdg_toplevel::WmCapabilities::Fullscreen);
    push_xdg_wm_capability(&mut capabilities, xdg_toplevel::WmCapabilities::Minimize);
    capabilities
}

fn push_xdg_wm_capability(capabilities: &mut Vec<u8>, capability: xdg_toplevel::WmCapabilities) {
    capabilities.extend_from_slice(&(capability as u32).to_ne_bytes());
}

#[derive(Debug, Clone, Copy)]
struct CompositorData;

#[derive(Debug, Clone, Copy)]
struct SubcompositorData;

#[derive(Debug, Clone, Copy)]
struct SubsurfaceData;

#[derive(Debug, Clone)]
struct SurfaceData;

#[derive(Debug, Clone, Copy)]
struct RegionData;

#[derive(Debug, Clone)]
struct ShmPoolData {
    file: Arc<File>,
    size: i32,
}

#[derive(Debug, Clone)]
enum BufferData {
    Shm(ShmBufferData),
    Dmabuf(DmabufBufferData),
}

#[derive(Debug, Clone)]
struct ShmBufferData {
    width: i32,
    height: i32,
    stride: i32,
    format: u32,
    offset: i32,
    pool_size: i32,
    file: Arc<File>,
}

#[derive(Debug, Clone)]
struct DmabufBufferData {
    width: i32,
    height: i32,
    format: u32,
    flags: u32,
    planes: Vec<DmabufPlane>,
}

#[derive(Debug, Clone)]
struct DmabufPlane {
    file: Arc<File>,
    plane_idx: u32,
    offset: u32,
    stride: u32,
    modifier: u64,
}

#[derive(Debug, Clone, Default)]
struct DmabufParamsData {
    planes: Arc<Mutex<Vec<DmabufPlane>>>,
}

#[derive(Debug, Clone, Copy)]
struct SeatData;

#[derive(Debug, Clone, Copy)]
struct InputData;

#[derive(Debug, Clone, Copy)]
struct DataDeviceManagerData;

#[derive(Debug, Clone, Copy)]
struct DataDeviceData;

#[derive(Debug, Clone, Copy)]
struct DataSourceData;

#[derive(Debug, Clone, Copy)]
struct OutputData;

#[derive(Debug, Clone, Copy)]
struct DmabufData;

#[derive(Debug, Clone, Copy)]
struct XdgWmBaseData;

#[derive(Debug, Clone, Copy)]
struct XdgDecorationManagerData;

#[derive(Debug, Clone, Copy)]
struct XdgToplevelDecorationData;

#[derive(Debug, Clone)]
struct XdgSurfaceData {
    surface_id: u32,
}

#[derive(Debug, Clone)]
struct XdgToplevelData {
    surface: xdg_surface::XdgSurface,
    surface_id: u32,
}

#[derive(Debug, Clone)]
struct XdgPopupData {
    surface: xdg_surface::XdgSurface,
    surface_id: u32,
}

#[derive(Debug, Clone, Copy)]
struct XdgPositionerData;

#[derive(Debug, Clone, Copy)]
struct FrameCallbackData;

#[derive(Debug)]
enum SurfaceUpload {
    Pixels {
        id: ResourceId,
        width: u32,
        height: u32,
        format: SurfacePixelFormat,
        pixels: Vec<u8>,
    },
    Dmabuf {
        id: ResourceId,
        buffer: DmabufBufferData,
    },
}

impl GlobalDispatch<wl_compositor::WlCompositor, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_compositor::WlCompositor>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, CompositorData);
    }
}

impl Dispatch<wl_compositor::WlCompositor, CompositorData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &wl_compositor::WlCompositor,
        request: wl_compositor::Request,
        _data: &CompositorData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_compositor::Request::CreateSurface { id } => {
                let surface = data_init.init(id, SurfaceData);
                let id = surface_id(&surface);
                state.surface_resources.insert(id, surface);
                if trace_wayland() {
                    eprintln!("Wayland surface created: {id}");
                }
            }
            wl_compositor::Request::CreateRegion { id } => {
                data_init.init(id, RegionData);
            }
            wl_compositor::Request::Release => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_subcompositor::WlSubcompositor, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_subcompositor::WlSubcompositor>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, SubcompositorData);
    }
}

impl Dispatch<wl_subcompositor::WlSubcompositor, SubcompositorData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_subcompositor::WlSubcompositor,
        request: wl_subcompositor::Request,
        _data: &SubcompositorData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_subcompositor::Request::Destroy => {}
            wl_subcompositor::Request::GetSubsurface { id, .. } => {
                data_init.init(id, SubsurfaceData);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_subsurface::WlSubsurface, SubsurfaceData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_subsurface::WlSubsurface,
        request: wl_subsurface::Request,
        _data: &SubsurfaceData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_subsurface::Request::Destroy
            | wl_subsurface::Request::SetPosition { .. }
            | wl_subsurface::Request::PlaceAbove { .. }
            | wl_subsurface::Request::PlaceBelow { .. }
            | wl_subsurface::Request::SetSync
            | wl_subsurface::Request::SetDesync => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_surface::WlSurface, SurfaceData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &wl_surface::WlSurface,
        request: wl_surface::Request,
        _data: &SurfaceData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_surface::Request::Destroy => {
                let id = surface_id(resource);
                state.remove_surface_window(id);
                state.surface_resources.remove(&id);
            }
            wl_surface::Request::Attach { buffer, x, y } => {
                if let Some(buffer) = buffer {
                    if trace_wayland() {
                        if let Some(data) = buffer.data::<BufferData>() {
                            match data {
                                BufferData::Shm(data) => {
                                    eprintln!(
                                        "Wayland surface {} attached shm buffer {}x{} stride={} format={} offset={} pool={} at {x},{y}",
                                        surface_id(resource),
                                        data.width,
                                        data.height,
                                        data.stride,
                                        data.format,
                                        data.offset,
                                        data.pool_size
                                    );
                                }
                                BufferData::Dmabuf(data) => {
                                    eprintln!(
                                        "Wayland surface {} attached dmabuf buffer {}x{} format={} planes={} flags={} at {x},{y}",
                                        surface_id(resource),
                                        data.width,
                                        data.height,
                                        data.format,
                                        data.planes.len(),
                                        data.flags
                                    );
                                }
                            }
                        } else {
                            eprintln!(
                                "Wayland surface {} attached unknown buffer at {x},{y}",
                                surface_id(resource)
                            );
                        }
                    }
                    state
                        .pending_attached_buffers
                        .insert(surface_id(resource), Some(buffer));
                } else {
                    state
                        .pending_attached_buffers
                        .insert(surface_id(resource), None);
                    if trace_wayland() {
                        eprintln!("Wayland surface {} detached buffer", surface_id(resource));
                    }
                }
            }
            wl_surface::Request::Damage { .. }
            | wl_surface::Request::SetOpaqueRegion { .. }
            | wl_surface::Request::SetInputRegion { .. }
            | wl_surface::Request::SetBufferTransform { .. }
            | wl_surface::Request::DamageBuffer { .. }
            | wl_surface::Request::Offset { .. } => {}
            wl_surface::Request::SetBufferScale { scale } => {
                state
                    .surface_buffer_scales
                    .insert(surface_id(resource), scale.max(1));
            }
            wl_surface::Request::Frame { callback } => {
                let callback = data_init.init(callback, FrameCallbackData);
                state
                    .pending_frame_callbacks
                    .entry(surface_id(resource))
                    .or_default()
                    .push(callback);
            }
            wl_surface::Request::Commit => {
                if let Some(buffer) = state
                    .pending_attached_buffers
                    .remove(&surface_id(resource))
                    .flatten()
                {
                    if let Some(buffer_data) = buffer.data::<BufferData>() {
                        let surface_id = surface_id(resource);
                        let scale = state
                            .surface_buffer_scales
                            .get(&surface_id)
                            .copied()
                            .unwrap_or(1)
                            .max(1) as f32;
                        match buffer_data {
                            BufferData::Shm(buffer_data) => {
                                if let Some(pixels) = read_shm_buffer_pixels(buffer_data) {
                                    let logical_size = Size::new(
                                        buffer_data.width as f32 / scale,
                                        buffer_data.height as f32 / scale,
                                    );
                                    state.compositor.set_surface_window_content_size(
                                        ResourceId(surface_id as u64),
                                        logical_size,
                                    );
                                    state.surface_uploads.push(SurfaceUpload::Pixels {
                                        id: ResourceId(surface_id as u64),
                                        width: buffer_data.width as u32,
                                        height: buffer_data.height as u32,
                                        format: pixels.format,
                                        pixels: pixels.data,
                                    });
                                }
                            }
                            BufferData::Dmabuf(buffer_data) => {
                                if buffer_data.width > 0 && buffer_data.height > 0 {
                                    let logical_size = Size::new(
                                        buffer_data.width as f32 / scale,
                                        buffer_data.height as f32 / scale,
                                    );
                                    state.compositor.set_surface_window_content_size(
                                        ResourceId(surface_id as u64),
                                        logical_size,
                                    );
                                    state.surface_uploads.push(SurfaceUpload::Dmabuf {
                                        id: ResourceId(surface_id as u64),
                                        buffer: buffer_data.clone(),
                                    });
                                }
                            }
                        }
                    }
                    buffer.release();
                }

                if trace_wayland() {
                    eprintln!("Wayland surface committed: {}", surface_id(resource));
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        resource: &wl_surface::WlSurface,
        _data: &SurfaceData,
    ) {
        let id = surface_id(resource);
        state.remove_surface_window(id);
        state.surface_resources.remove(&id);
    }
}

impl Dispatch<wl_callback::WlCallback, FrameCallbackData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_callback::WlCallback,
        request: wl_callback::Request,
        _data: &FrameCallbackData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            _ => {}
        }
    }
}

impl Dispatch<wl_region::WlRegion, RegionData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_region::WlRegion,
        request: wl_region::Request,
        _data: &RegionData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_region::Request::Destroy
            | wl_region::Request::Add { .. }
            | wl_region::Request::Subtract { .. } => {}
            _ => {}
        }
    }
}

const DRM_FORMAT_ARGB8888: u32 = u32::from_ne_bytes(*b"AR24");
const DRM_FORMAT_XRGB8888: u32 = u32::from_ne_bytes(*b"XR24");
const DRM_FORMAT_ABGR8888: u32 = u32::from_ne_bytes(*b"AB24");
const DRM_FORMAT_XBGR8888: u32 = u32::from_ne_bytes(*b"XB24");
const DRM_FORMAT_MOD_INVALID_HI: u32 = 0x00ff_ffff;
const DRM_FORMAT_MOD_INVALID_LO: u32 = 0xffff_ffff;
const SUPPORTED_DMABUF_FORMATS: &[u32] = &[
    DRM_FORMAT_ARGB8888,
    DRM_FORMAT_XRGB8888,
    DRM_FORMAT_ABGR8888,
    DRM_FORMAT_XBGR8888,
];

fn dmabuf_buffer_from_params(
    data: &DmabufParamsData,
    width: i32,
    height: i32,
    format: u32,
    flags: u32,
) -> Option<BufferData> {
    if width <= 0 || height <= 0 || !SUPPORTED_DMABUF_FORMATS.contains(&format) {
        return None;
    }

    let mut planes = data
        .planes
        .lock()
        .expect("dmabuf params mutex poisoned")
        .clone();
    planes.sort_by_key(|plane| plane.plane_idx);
    if planes.len() != 1
        || planes
            .iter()
            .enumerate()
            .any(|(index, plane)| plane.plane_idx as usize != index)
    {
        return None;
    }

    Some(BufferData::Dmabuf(DmabufBufferData {
        width,
        height,
        format,
        flags,
        planes,
    }))
}

fn dmabuf_flags_bits(flags: WEnum<zwp_linux_buffer_params_v1::Flags>) -> u32 {
    match flags {
        WEnum::Value(flags) => flags.bits(),
        WEnum::Unknown(raw) => raw,
    }
}

impl GlobalDispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let dmabuf = data_init.init(resource, DmabufData);
        for format in SUPPORTED_DMABUF_FORMATS {
            if dmabuf.version() >= 3 {
                dmabuf.modifier(
                    *format,
                    DRM_FORMAT_MOD_INVALID_HI,
                    DRM_FORMAT_MOD_INVALID_LO,
                );
            } else {
                dmabuf.format(*format);
            }
        }
    }
}

impl Dispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
        request: zwp_linux_dmabuf_v1::Request,
        _data: &DmabufData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwp_linux_dmabuf_v1::Request::Destroy => {}
            zwp_linux_dmabuf_v1::Request::CreateParams { params_id } => {
                data_init.init(params_id, DmabufParamsData::default());
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, DmabufParamsData>
    for NestedState
{
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        request: zwp_linux_buffer_params_v1::Request,
        data: &DmabufParamsData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zwp_linux_buffer_params_v1::Request::Destroy => {}
            zwp_linux_buffer_params_v1::Request::Add {
                fd,
                plane_idx,
                offset,
                stride,
                modifier_hi,
                modifier_lo,
            } => {
                let modifier = ((modifier_hi as u64) << 32) | modifier_lo as u64;
                let mut planes = data.planes.lock().expect("dmabuf params mutex poisoned");
                if plane_idx >= 4 || planes.iter().any(|plane| plane.plane_idx == plane_idx) {
                    resource.failed();
                    return;
                }
                planes.push(DmabufPlane {
                    file: Arc::new(File::from(fd)),
                    plane_idx,
                    offset,
                    stride,
                    modifier,
                });
            }
            zwp_linux_buffer_params_v1::Request::Create {
                width,
                height,
                format,
                flags,
            } => {
                let _ = (width, height, format, dmabuf_flags_bits(flags));
                resource.failed();
            }
            zwp_linux_buffer_params_v1::Request::CreateImmed {
                buffer_id,
                width,
                height,
                format,
                flags,
            } => {
                let flags = dmabuf_flags_bits(flags);
                let Some(buffer_data) =
                    dmabuf_buffer_from_params(data, width, height, format, flags)
                else {
                    data_init.init(
                        buffer_id,
                        BufferData::Dmabuf(DmabufBufferData {
                            width: 0,
                            height: 0,
                            format,
                            flags,
                            planes: Vec::new(),
                        }),
                    );
                    resource.failed();
                    return;
                };
                data_init.init(buffer_id, buffer_data);
            }
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_shm::WlShm, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_shm::WlShm>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let shm = data_init.init(resource, ());
        shm.format(wl_shm::Format::Argb8888);
        shm.format(wl_shm::Format::Xrgb8888);
    }
}

impl Dispatch<wl_shm::WlShm, ()> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_shm::WlShm,
        request: wl_shm::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm::Request::CreatePool { id, fd, size } => {
                data_init.init(
                    id,
                    ShmPoolData {
                        file: Arc::new(File::from(fd)),
                        size,
                    },
                );
                if trace_wayland() {
                    eprintln!("Wayland shm pool created: {size} bytes");
                }
            }
            wl_shm::Request::Release => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ShmPoolData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_shm_pool::WlShmPool,
        request: wl_shm_pool::Request,
        data: &ShmPoolData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_shm_pool::Request::CreateBuffer {
                id,
                offset,
                width,
                height,
                stride,
                format,
            } => {
                let format = match format {
                    WEnum::Value(format) => format as u32,
                    WEnum::Unknown(raw) => raw,
                };
                data_init.init(
                    id,
                    BufferData::Shm(ShmBufferData {
                        width,
                        height,
                        stride,
                        format,
                        offset,
                        pool_size: data.size,
                        file: data.file.clone(),
                    }),
                );
                if trace_wayland() {
                    eprintln!(
                        "Wayland shm buffer created: {width}x{height} stride={stride} format={format}"
                    );
                }
            }
            wl_shm_pool::Request::Destroy => {}
            wl_shm_pool::Request::Resize { size } => {
                if trace_wayland() {
                    eprintln!("Wayland shm pool resize requested: {} -> {size}", data.size);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, BufferData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_buffer::WlBuffer,
        request: wl_buffer::Request,
        _data: &BufferData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_buffer::Request::Destroy => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_data_device_manager::WlDataDeviceManager, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_data_device_manager::WlDataDeviceManager>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, DataDeviceManagerData);
    }
}

impl Dispatch<wl_data_device_manager::WlDataDeviceManager, DataDeviceManagerData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_data_device_manager::WlDataDeviceManager,
        request: wl_data_device_manager::Request,
        _data: &DataDeviceManagerData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_device_manager::Request::CreateDataSource { id } => {
                data_init.init(id, DataSourceData);
            }
            wl_data_device_manager::Request::GetDataDevice { id, .. } => {
                data_init.init(id, DataDeviceData);
            }
            wl_data_device_manager::Request::Release => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, DataSourceData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_data_source::WlDataSource,
        request: wl_data_source::Request,
        _data: &DataSourceData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_source::Request::Offer { .. }
            | wl_data_source::Request::Destroy
            | wl_data_source::Request::SetActions { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_data_device::WlDataDevice, DataDeviceData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_data_device::WlDataDevice,
        request: wl_data_device::Request,
        _data: &DataDeviceData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_data_device::Request::StartDrag { .. }
            | wl_data_device::Request::SetSelection { .. }
            | wl_data_device::Request::Release => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_seat::WlSeat, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_seat::WlSeat>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let seat = data_init.init(resource, SeatData);
        seat.name("puppyos".into());
        seat.capabilities(wl_seat::Capability::Pointer | wl_seat::Capability::Keyboard);
    }
}

impl Dispatch<wl_seat::WlSeat, SeatData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &wl_seat::WlSeat,
        request: wl_seat::Request,
        _data: &SeatData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_seat::Request::GetPointer { id } => {
                let pointer = data_init.init(id, InputData);
                state.pointers.push(pointer);
            }
            wl_seat::Request::GetKeyboard { id } => {
                let keyboard = data_init.init(id, InputData);
                let serial = state.next_serial();
                match create_keymap_file(serial) {
                    Ok(file) => {
                        keyboard.keymap(
                            wl_keyboard::KeymapFormat::XkbV1,
                            file.as_fd(),
                            MINIMAL_XKB_KEYMAP.len() as u32,
                        );
                        state.keymap_files.push(file);
                        if trace_wayland() {
                            eprintln!(
                                "Wayland keyboard created: id={} keymap_bytes={}",
                                keyboard.id().protocol_id(),
                                MINIMAL_XKB_KEYMAP.len()
                            );
                        }
                    }
                    Err(err) => {
                        eprintln!(
                            "Wayland keyboard created without keymap: id={} error={err}",
                            keyboard.id().protocol_id()
                        );
                    }
                }
                if keyboard.version() >= 4 {
                    keyboard.repeat_info(25, 600);
                }
                if let Some(surface_id) = state.keyboard_focus
                    && let Some(surface) = state.surface_resources.get(&surface_id).cloned()
                    && same_wayland_client(&keyboard, &surface)
                {
                    let serial = state.next_serial();
                    keyboard.enter(serial, &surface, Vec::new());
                    let serial = state.next_serial();
                    keyboard.modifiers(
                        serial,
                        xkb_depressed_modifiers(state.keyboard_modifiers),
                        0,
                        0,
                        0,
                    );
                }
                state.keyboards.push(keyboard);
            }
            wl_seat::Request::GetTouch { id } => {
                data_init.init(id, InputData);
            }
            wl_seat::Request::Release => {}
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, InputData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &wl_pointer::WlPointer,
        request: wl_pointer::Request,
        _data: &InputData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_pointer::Request::SetCursor { .. } => {}
            wl_pointer::Request::Release => {
                let id = resource.id();
                state.pointers.retain(|pointer| pointer.id() != id);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, InputData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &wl_keyboard::WlKeyboard,
        request: wl_keyboard::Request,
        _data: &InputData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_keyboard::Request::Release => {
                let id = resource.id();
                state.keyboards.retain(|keyboard| keyboard.id() != id);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_touch::WlTouch, InputData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_touch::WlTouch,
        request: wl_touch::Request,
        _data: &InputData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_touch::Request::Release => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<wl_output::WlOutput, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<wl_output::WlOutput>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let output = data_init.init(resource, OutputData);
        output.geometry(
            0,
            0,
            300,
            190,
            wl_output::Subpixel::Unknown,
            "PuppyOS".into(),
            "Nested Display".into(),
            wl_output::Transform::Normal,
        );
        output.mode(wl_output::Mode::Current, 1024, 700, 60_000);
        output.scale(1);
        output.done();
    }
}

impl Dispatch<wl_output::WlOutput, OutputData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &wl_output::WlOutput,
        request: wl_output::Request,
        _data: &OutputData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wl_output::Request::Release => {}
            _ => {}
        }
    }
}

impl GlobalDispatch<xdg_wm_base::XdgWmBase, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<xdg_wm_base::XdgWmBase>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, XdgWmBaseData);
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, XdgWmBaseData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &xdg_wm_base::XdgWmBase,
        request: xdg_wm_base::Request,
        _data: &XdgWmBaseData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_wm_base::Request::Destroy => {}
            xdg_wm_base::Request::CreatePositioner { id } => {
                data_init.init(id, XdgPositionerData);
            }
            xdg_wm_base::Request::GetXdgSurface { id, surface } => {
                let surface_id = surface_id(&surface);
                data_init.init(id, XdgSurfaceData { surface_id });
                if trace_wayland() {
                    eprintln!("Wayland xdg surface created for wl_surface={surface_id}");
                }
            }
            xdg_wm_base::Request::Pong { serial } => {
                if trace_wayland() {
                    eprintln!("Wayland xdg pong: {serial}");
                }
            }
            _ => {}
        }
    }
}

impl GlobalDispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, ()> for NestedState {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, XdgDecorationManagerData);
    }
}

impl Dispatch<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1, XdgDecorationManagerData>
    for NestedState
{
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &zxdg_decoration_manager_v1::ZxdgDecorationManagerV1,
        request: zxdg_decoration_manager_v1::Request,
        _data: &XdgDecorationManagerData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zxdg_decoration_manager_v1::Request::GetToplevelDecoration { id, toplevel: _ } => {
                let decoration = data_init.init(id, XdgToplevelDecorationData);
                decoration.configure(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            }
            zxdg_decoration_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, XdgToplevelDecorationData>
    for NestedState
{
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        request: zxdg_toplevel_decoration_v1::Request,
        _data: &XdgToplevelDecorationData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            zxdg_toplevel_decoration_v1::Request::SetMode { .. }
            | zxdg_toplevel_decoration_v1::Request::UnsetMode => {
                resource.configure(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            }
            zxdg_toplevel_decoration_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<xdg_positioner::XdgPositioner, XdgPositionerData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &xdg_positioner::XdgPositioner,
        request: xdg_positioner::Request,
        _data: &XdgPositionerData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_positioner::Request::Destroy
            | xdg_positioner::Request::SetSize { .. }
            | xdg_positioner::Request::SetAnchorRect { .. }
            | xdg_positioner::Request::SetAnchor { .. }
            | xdg_positioner::Request::SetGravity { .. }
            | xdg_positioner::Request::SetConstraintAdjustment { .. }
            | xdg_positioner::Request::SetOffset { .. }
            | xdg_positioner::Request::SetReactive
            | xdg_positioner::Request::SetParentSize { .. }
            | xdg_positioner::Request::SetParentConfigure { .. } => {}
            _ => {}
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, XdgSurfaceData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &xdg_surface::XdgSurface,
        request: xdg_surface::Request,
        data: &XdgSurfaceData,
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_surface::Request::Destroy => {
                _state.remove_surface_window(data.surface_id);
            }
            xdg_surface::Request::GetToplevel { id } => {
                let toplevel = data_init.init(
                    id,
                    XdgToplevelData {
                        surface: resource.clone(),
                        surface_id: data.surface_id,
                    },
                );
                let window = _state.compositor.add_surface_window(
                    "Wayland App",
                    ResourceId(data.surface_id as u64),
                    Size::new(640.0, 420.0),
                );
                _state
                    .surface_xdg_surfaces
                    .insert(data.surface_id, resource.clone());
                _state
                    .surface_toplevel_states
                    .entry(data.surface_id)
                    .or_default();
                if toplevel.version() >= 5 {
                    toplevel.wm_capabilities(xdg_wm_capabilities());
                }
                _state.surface_toplevels.insert(data.surface_id, toplevel);
                _state.surface_windows.insert(data.surface_id, window);
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel created for wl_surface={}",
                        data.surface_id
                    );
                }
                _state.configure_surface_id(data.surface_id, Size::new(640.0, 420.0));
            }
            xdg_surface::Request::GetPopup { id, .. } => {
                let popup = data_init.init(
                    id,
                    XdgPopupData {
                        surface: resource.clone(),
                        surface_id: data.surface_id,
                    },
                );
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg popup requested for wl_surface={}",
                        data.surface_id
                    );
                }
                let serial = _state.next_serial();
                popup.configure(0, 0, 320, 240);
                resource.configure(serial);
            }
            xdg_surface::Request::SetWindowGeometry { .. } => {}
            xdg_surface::Request::AckConfigure { serial } => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg configure ack: surface={} serial={serial}",
                        data.surface_id
                    );
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        _resource: &xdg_surface::XdgSurface,
        data: &XdgSurfaceData,
    ) {
        state.remove_surface_window(data.surface_id);
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, XdgToplevelData> for NestedState {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &xdg_toplevel::XdgToplevel,
        request: xdg_toplevel::Request,
        data: &XdgToplevelData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel::Request::Destroy => {
                state.remove_surface_window(data.surface_id);
            }
            xdg_toplevel::Request::SetParent { .. }
            | xdg_toplevel::Request::SetAppId { .. }
            | xdg_toplevel::Request::ShowWindowMenu { .. }
            | xdg_toplevel::Request::Resize { .. }
            | xdg_toplevel::Request::SetMaxSize { .. }
            | xdg_toplevel::Request::SetMinSize { .. } => {}
            xdg_toplevel::Request::Move { .. } => {
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state
                        .compositor
                        .start_window_move(window, state.pointer_position);
                }
            }
            xdg_toplevel::Request::SetTitle { title } => {
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state.compositor.set_window_title(window, title);
                }
            }
            xdg_toplevel::Request::SetMaximized => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel request: set_maximized surface={}",
                        data.surface_id
                    );
                }
                state
                    .surface_toplevel_states
                    .entry(data.surface_id)
                    .or_default()
                    .maximized = true;
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state
                        .compositor
                        .set_window_full_size(window, state.desktop_size, true);
                }
                let size = state
                    .compositor
                    .surface_window_requested_content_size(ResourceId(data.surface_id as u64))
                    .unwrap_or(Size::new(640.0, 420.0));
                state.configure_toplevel(data.surface_id, &data.surface, resource, size);
            }
            xdg_toplevel::Request::SetFullscreen { .. } => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel request: set_fullscreen surface={}",
                        data.surface_id
                    );
                }
                state
                    .surface_toplevel_states
                    .entry(data.surface_id)
                    .or_default()
                    .fullscreen = true;
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state
                        .compositor
                        .set_window_full_size(window, state.desktop_size, true);
                }
                let size = state
                    .compositor
                    .surface_window_requested_content_size(ResourceId(data.surface_id as u64))
                    .unwrap_or(Size::new(640.0, 420.0));
                state.configure_toplevel(data.surface_id, &data.surface, resource, size);
            }
            xdg_toplevel::Request::UnsetMaximized => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel request: unset_maximized surface={}",
                        data.surface_id
                    );
                }
                if let Some(toplevel_state) =
                    state.surface_toplevel_states.get_mut(&data.surface_id)
                {
                    toplevel_state.maximized = false;
                }
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state
                        .compositor
                        .set_window_full_size(window, state.desktop_size, false);
                }
                let size = state
                    .compositor
                    .surface_window_requested_content_size(ResourceId(data.surface_id as u64))
                    .unwrap_or(Size::new(640.0, 420.0));
                state.configure_toplevel(data.surface_id, &data.surface, resource, size);
            }
            xdg_toplevel::Request::UnsetFullscreen => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel request: unset_fullscreen surface={}",
                        data.surface_id
                    );
                }
                if let Some(toplevel_state) =
                    state.surface_toplevel_states.get_mut(&data.surface_id)
                {
                    toplevel_state.fullscreen = false;
                }
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state
                        .compositor
                        .set_window_full_size(window, state.desktop_size, false);
                }
                let size = state
                    .compositor
                    .surface_window_requested_content_size(ResourceId(data.surface_id as u64))
                    .unwrap_or(Size::new(640.0, 420.0));
                state.configure_toplevel(data.surface_id, &data.surface, resource, size);
            }
            xdg_toplevel::Request::SetMinimized => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg toplevel request: set_minimized surface={}",
                        data.surface_id
                    );
                }
                if let Some(window) = state.surface_windows.get(&data.surface_id).copied() {
                    state.compositor.set_window_minimized(window, true);
                }
            }
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: ClientId,
        _resource: &xdg_toplevel::XdgToplevel,
        data: &XdgToplevelData,
    ) {
        state.remove_surface_window(data.surface_id);
    }
}

impl Dispatch<xdg_popup::XdgPopup, XdgPopupData> for NestedState {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &xdg_popup::XdgPopup,
        request: xdg_popup::Request,
        data: &XdgPopupData,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_popup::Request::Destroy => {}
            xdg_popup::Request::Grab { .. } => {
                if trace_wayland() {
                    eprintln!(
                        "Wayland xdg popup grab requested for wl_surface={}",
                        data.surface_id
                    );
                }
            }
            xdg_popup::Request::Reposition { .. } => {
                let serial = _state.next_serial();
                resource.configure(0, 0, 320, 240);
                data.surface.configure(serial);
            }
            _ => {}
        }
    }
}

fn surface_id(surface: &impl Resource) -> u32 {
    surface.id().protocol_id()
}

fn dmabuf_import_for_renderer(buffer: &DmabufBufferData) -> Option<DmabufImport> {
    if buffer.width <= 0 || buffer.height <= 0 || buffer.planes.is_empty() {
        return None;
    }

    let mut planes = buffer.planes.clone();
    planes.sort_by_key(|plane| plane.plane_idx);
    if planes
        .iter()
        .enumerate()
        .any(|(index, plane)| plane.plane_idx as usize != index)
    {
        return None;
    }

    Some(DmabufImport {
        width: buffer.width as u32,
        height: buffer.height as u32,
        format: buffer.format,
        planes: planes
            .iter()
            .map(|plane| RendererDmabufPlane {
                fd: plane.file.as_raw_fd(),
                offset: plane.offset,
                stride: plane.stride,
                modifier: plane.modifier,
            })
            .collect(),
    })
}

struct ShmPixels {
    format: SurfacePixelFormat,
    data: Vec<u8>,
}

fn read_shm_buffer_pixels(buffer: &ShmBufferData) -> Option<ShmPixels> {
    if buffer.width <= 0 || buffer.height <= 0 || buffer.stride <= 0 || buffer.offset < 0 {
        return None;
    }

    let width = buffer.width as usize;
    let height = buffer.height as usize;
    let stride = buffer.stride as usize;
    let offset = buffer.offset as u64;
    let bytes_per_pixel = 4;

    if stride < width.checked_mul(bytes_per_pixel)? {
        return None;
    }

    let row_bytes = width.checked_mul(bytes_per_pixel)?;
    let mut data = vec![0; width.checked_mul(height)?.checked_mul(bytes_per_pixel)?];

    if stride == row_bytes {
        read_exact_at(&buffer.file, &mut data, offset).ok()?;
    } else {
        let mut row = vec![0; stride];
        for y in 0..height {
            read_exact_at(&buffer.file, &mut row, offset + (y * stride) as u64).ok()?;
            let destination = y * row_bytes;
            data[destination..destination + row_bytes].copy_from_slice(&row[..row_bytes]);
        }
    }

    match buffer.format {
        0 => {}
        1 => {
            for pixel in data.chunks_exact_mut(bytes_per_pixel) {
                pixel[3] = 255;
            }
        }
        _ => return None,
    }

    Some(ShmPixels {
        format: SurfacePixelFormat::Bgra,
        data,
    })
}

fn read_exact_at(file: &File, mut buffer: &mut [u8], mut offset: u64) -> std::io::Result<()> {
    while !buffer.is_empty() {
        let read = file.read_at(buffer, offset)?;
        if read == 0 {
            return Err(std::io::ErrorKind::UnexpectedEof.into());
        }
        offset += read as u64;
        buffer = &mut buffer[read..];
    }

    Ok(())
}

const MINIMAL_XKB_KEYMAP: &[u8] = b"xkb_keymap {
xkb_keycodes \"evdev+aliases(qwerty)\" {
    include \"evdev+aliases(qwerty)\"
};
xkb_types \"complete\" {
    include \"complete\"
};
xkb_compatibility \"complete\" {
    include \"complete\"
};
xkb_symbols \"pc+us+inet(evdev)\" {
    include \"pc+us+inet(evdev)\"
};
};
\0";

fn trace_wayland() -> bool {
    static TRACE: OnceLock<bool> = OnceLock::new();
    *TRACE.get_or_init(|| std::env::var_os("PUPPYOS_TRACE_WAYLAND").is_some())
}

fn create_keymap_file(serial: u32) -> std::io::Result<File> {
    let path = std::env::temp_dir().join(format!(
        "puppyos-keymap-{}-{serial}.xkb",
        std::process::id()
    ));
    let mut file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(MINIMAL_XKB_KEYMAP)?;
    let _ = fs::remove_file(path);
    Ok(file)
}

#[derive(Debug)]
struct LoggedClient;

impl ClientData for LoggedClient {
    fn initialized(&self, client_id: ClientId) {
        eprintln!("Wayland client connected: {client_id:?}");
    }

    fn disconnected(&self, client_id: ClientId, reason: DisconnectReason) {
        eprintln!("Wayland client disconnected: {client_id:?}: {reason:?}");
    }
}

struct BoundSocket {
    socket: ListeningSocket,
    display_name: String,
}

fn bind_socket() -> Result<BoundSocket> {
    match ListeningSocket::bind_auto("puppyos", 0..32) {
        Ok(socket) => {
            let display_name = socket
                .socket_name()
                .context("auto-bound socket did not report a name")?
                .to_string_lossy()
                .into_owned();
            Ok(BoundSocket {
                socket,
                display_name,
            })
        }
        Err(BindError::RuntimeDirNotSet | BindError::PermissionDenied) => bind_workspace_socket()
            .context("failed to bind fallback socket under workspace target directory"),
        Err(err) => Err(err).context("failed to bind socket in XDG_RUNTIME_DIR"),
    }
}

fn bind_workspace_socket() -> Result<BoundSocket> {
    let runtime_dir = std::env::current_dir()
        .context("failed to read current directory")?
        .join("target")
        .join("wayland-runtime");
    fs::create_dir_all(&runtime_dir).context("failed to create fallback runtime directory")?;
    fs::set_permissions(&runtime_dir, fs::Permissions::from_mode(0o700))
        .context("failed to set fallback runtime directory permissions")?;

    for index in 0..32 {
        let path = runtime_dir.join(format!("puppyos-{index}"));
        match ListeningSocket::bind_absolute(path.clone()) {
            Ok(socket) => {
                return Ok(BoundSocket {
                    socket,
                    display_name: display_path(&path),
                });
            }
            Err(BindError::AlreadyInUse) => {}
            Err(err) => return Err(err).context("failed to bind fallback socket"),
        }
    }

    anyhow::bail!("no fallback socket name was available")
}

fn display_path(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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

fn event_time_ms(start_time: Instant) -> u32 {
    start_time.elapsed().as_millis() as u32
}

fn pointer_button_code(button: MouseButton) -> Option<u32> {
    match button {
        MouseButton::Left => Some(0x110),
        MouseButton::Right => Some(0x111),
        MouseButton::Middle => Some(0x112),
        MouseButton::Back => Some(0x116),
        MouseButton::Forward => Some(0x115),
        MouseButton::Other(code) => Some(code.into()),
    }
}

fn evdev_keycode(event: &KeyEvent) -> Option<u32> {
    let PhysicalKey::Code(code) = event.physical_key else {
        return None;
    };

    let key = match code {
        KeyCode::Escape => 1,
        KeyCode::Digit1 => 2,
        KeyCode::Digit2 => 3,
        KeyCode::Digit3 => 4,
        KeyCode::Digit4 => 5,
        KeyCode::Digit5 => 6,
        KeyCode::Digit6 => 7,
        KeyCode::Digit7 => 8,
        KeyCode::Digit8 => 9,
        KeyCode::Digit9 => 10,
        KeyCode::Digit0 => 11,
        KeyCode::Minus => 12,
        KeyCode::Equal => 13,
        KeyCode::Backspace => 14,
        KeyCode::Tab => 15,
        KeyCode::KeyQ => 16,
        KeyCode::KeyW => 17,
        KeyCode::KeyE => 18,
        KeyCode::KeyR => 19,
        KeyCode::KeyT => 20,
        KeyCode::KeyY => 21,
        KeyCode::KeyU => 22,
        KeyCode::KeyI => 23,
        KeyCode::KeyO => 24,
        KeyCode::KeyP => 25,
        KeyCode::BracketLeft => 26,
        KeyCode::BracketRight => 27,
        KeyCode::Enter => 28,
        KeyCode::ControlLeft => 29,
        KeyCode::KeyA => 30,
        KeyCode::KeyS => 31,
        KeyCode::KeyD => 32,
        KeyCode::KeyF => 33,
        KeyCode::KeyG => 34,
        KeyCode::KeyH => 35,
        KeyCode::KeyJ => 36,
        KeyCode::KeyK => 37,
        KeyCode::KeyL => 38,
        KeyCode::Semicolon => 39,
        KeyCode::Quote => 40,
        KeyCode::Backquote => 41,
        KeyCode::ShiftLeft => 42,
        KeyCode::Backslash => 43,
        KeyCode::KeyZ => 44,
        KeyCode::KeyX => 45,
        KeyCode::KeyC => 46,
        KeyCode::KeyV => 47,
        KeyCode::KeyB => 48,
        KeyCode::KeyN => 49,
        KeyCode::KeyM => 50,
        KeyCode::Comma => 51,
        KeyCode::Period => 52,
        KeyCode::Slash => 53,
        KeyCode::ShiftRight => 54,
        KeyCode::AltLeft => 56,
        KeyCode::Space => 57,
        KeyCode::CapsLock => 58,
        KeyCode::F1 => 59,
        KeyCode::F2 => 60,
        KeyCode::F3 => 61,
        KeyCode::F4 => 62,
        KeyCode::F5 => 63,
        KeyCode::F6 => 64,
        KeyCode::F7 => 65,
        KeyCode::F8 => 66,
        KeyCode::F9 => 67,
        KeyCode::F10 => 68,
        KeyCode::F11 => 87,
        KeyCode::F12 => 88,
        KeyCode::ControlRight => 97,
        KeyCode::AltRight => 100,
        KeyCode::Home => 102,
        KeyCode::ArrowUp => 103,
        KeyCode::PageUp => 104,
        KeyCode::ArrowLeft => 105,
        KeyCode::ArrowRight => 106,
        KeyCode::End => 107,
        KeyCode::ArrowDown => 108,
        KeyCode::PageDown => 109,
        KeyCode::Insert => 110,
        KeyCode::Delete => 111,
        KeyCode::SuperLeft => 125,
        KeyCode::SuperRight => 126,
        _ => return None,
    };

    Some(key)
}

fn xkb_depressed_modifiers(modifiers: ModifiersState) -> u32 {
    let mut depressed = 0;
    if modifiers.shift_key() {
        depressed |= 1;
    }
    if modifiers.control_key() {
        depressed |= 4;
    }
    if modifiers.alt_key() {
        depressed |= 8;
    }
    if modifiers.super_key() {
        depressed |= 64;
    }
    depressed
}

fn send_pointer_frame(pointer: &wl_pointer::WlPointer) {
    if pointer.version() >= 5 {
        pointer.frame();
    }
}

fn send_pointer_axis(
    pointer: &wl_pointer::WlPointer,
    time: u32,
    axis: wl_pointer::Axis,
    value: f64,
) {
    pointer.axis(time, axis, value);
}

fn send_pointer_axis_discrete(
    pointer: &wl_pointer::WlPointer,
    axis: wl_pointer::Axis,
    discrete: i32,
) {
    if pointer.version() >= 5 {
        pointer.axis_discrete(axis, discrete);
    }
}

fn same_wayland_client(a: &impl Resource, b: &impl Resource) -> bool {
    a.client()
        .zip(b.client())
        .is_some_and(|(a, b)| a.id() == b.id())
}

fn handle_keyboard_input(
    wayland: &mut NestedWaylandServer,
    event: &KeyEvent,
    modifiers: ModifiersState,
    time: u32,
) -> bool {
    let is_pressed = event.state == ElementState::Pressed;
    match &event.logical_key {
        Key::Named(NamedKey::F1) if is_pressed => {
            let compositor = wayland.compositor_mut();
            compositor.toggle_launcher();
            true
        }
        Key::Named(NamedKey::Space) if is_pressed && modifiers.control_key() => {
            let compositor = wayland.compositor_mut();
            compositor.toggle_launcher();
            true
        }
        Key::Named(NamedKey::Escape) if is_pressed && wayland.compositor().launcher_is_open() => {
            let compositor = wayland.compositor_mut();
            compositor.close_launcher();
            true
        }
        Key::Named(NamedKey::Enter) if is_pressed && wayland.compositor().launcher_is_open() => {
            let compositor = wayland.compositor_mut();
            if let Some(launch) = compositor.launcher_launch_selected() {
                wayland.launch_app(launch);
            }
            true
        }
        Key::Named(NamedKey::Backspace)
            if is_pressed && wayland.compositor().launcher_is_open() =>
        {
            let compositor = wayland.compositor_mut();
            compositor.launcher_backspace();
            true
        }
        Key::Named(NamedKey::ArrowDown)
            if is_pressed && wayland.compositor().launcher_is_open() =>
        {
            let compositor = wayland.compositor_mut();
            compositor.launcher_select_next();
            true
        }
        Key::Named(NamedKey::ArrowUp) if is_pressed && wayland.compositor().launcher_is_open() => {
            let compositor = wayland.compositor_mut();
            compositor.launcher_select_previous();
            true
        }
        _ if is_pressed && wayland.compositor().launcher_is_open() => {
            if let Some(text) = event.text.as_deref() {
                let compositor = wayland.compositor_mut();
                compositor.launcher_insert_text(text);
                true
            } else {
                false
            }
        }
        _ if !wayland.compositor().launcher_is_open() => {
            wayland.send_keyboard_input(event, modifiers, time)
        }
        _ => false,
    }
}
