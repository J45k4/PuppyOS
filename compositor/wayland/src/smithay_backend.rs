use std::{os::unix::io::OwnedFd, sync::Arc, time::Instant};

use anyhow::{Context, Result, anyhow};
use smithay::{
    backend::{
        input::{InputEvent, KeyboardKeyEvent},
        renderer::{
            Color32F, Frame, Renderer,
            element::{
                Kind,
                surface::{WaylandSurfaceRenderElement, render_elements_from_surface_tree},
            },
            gles::GlesRenderer,
            utils::{draw_render_elements, on_commit_buffer_handler},
        },
        winit::{self, WinitEvent},
    },
    delegate_compositor, delegate_data_device, delegate_seat, delegate_shm, delegate_xdg_shell,
    input::{
        Seat, SeatHandler, SeatState,
        keyboard::{FilterResult, KeyboardHandle},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            Client, Display, ListeningSocket,
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::{
                wl_buffer, wl_seat,
                wl_surface::{self, WlSurface},
            },
        },
        winit::platform::pump_events::PumpStatus,
    },
    utils::{Rectangle, Serial, Transform},
    wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
            TraversalAction, with_surface_tree_downward,
        },
        selection::{
            SelectionHandler,
            data_device::{
                ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
            },
        },
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
        },
        shm::{ShmHandler, ShmState},
    },
};

pub fn run() -> Result<()> {
    let mut display: Display<SmithayState> = Display::new().context("failed to create display")?;
    let handle = display.handle();

    let compositor_state = CompositorState::new::<SmithayState>(&handle);
    let xdg_shell_state = XdgShellState::new::<SmithayState>(&handle);
    let shm_state = ShmState::new::<SmithayState>(&handle, Vec::new());
    let data_device_state = DataDeviceState::new::<SmithayState>(&handle);
    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(&handle, "puppy-seat");
    let keyboard = seat
        .add_keyboard(Default::default(), 200, 25)
        .context("failed to create Smithay keyboard")?;

    let mut state = SmithayState {
        compositor_state,
        xdg_shell_state,
        shm_state,
        data_device_state,
        seat_state,
        _seat: seat,
        keyboard,
    };

    let socket = ListeningSocket::bind_auto("puppyos", 0..32)
        .context("failed to bind PuppyOS Smithay Wayland socket")?;
    let socket_name = socket
        .socket_name()
        .and_then(|name| name.to_str())
        .unwrap_or("puppyos-unknown")
        .to_string();
    println!("PuppyOS Smithay nested compositor listening on {socket_name}");
    println!("Launch clients with:");
    println!("  WAYLAND_DISPLAY={socket_name} <app>");
    println!();
    println!("Smithay owns wl_compositor, wl_shm, xdg_shell, frame callbacks, and buffer imports.");

    let (mut backend, mut winit) = winit::init::<GlesRenderer>()
        .map_err(|err| anyhow!("failed to initialize Smithay winit backend: {err:?}"))?;
    backend
        .window()
        .set_title("PuppyOS Smithay Nested Compositor");

    let mut clients = Vec::new();
    let start_time = Instant::now();

    loop {
        match winit.dispatch_new_events(|event| match event {
            WinitEvent::Input(event) => match event {
                InputEvent::Keyboard { event } => {
                    let keyboard = state.keyboard.clone();
                    keyboard.input::<(), _>(
                        &mut state,
                        event.key_code(),
                        event.state(),
                        Serial::from(0),
                        start_time.elapsed().as_millis() as u32,
                        |_, _, _| FilterResult::Forward,
                    );
                }
                InputEvent::PointerMotionAbsolute { .. } => {
                    focus_first_toplevel(&mut state);
                }
                _ => {}
            },
            WinitEvent::Resized { .. } => {}
            WinitEvent::CloseRequested => {}
            _ => {}
        }) {
            PumpStatus::Continue => {}
            PumpStatus::Exit(_) => break,
        }

        while let Some(stream) = socket.accept().context("failed to accept Wayland client")? {
            let client = display
                .handle()
                .insert_client(stream, Arc::new(SmithayClientState::default()))
                .context("failed to insert Wayland client")?;
            clients.push(client);
        }

        display
            .dispatch_clients(&mut state)
            .context("failed to dispatch Wayland clients")?;
        display
            .flush_clients()
            .context("failed to flush Wayland clients")?;

        render(&mut backend, &state, start_time)?;
    }

    Ok(())
}

fn render(
    backend: &mut winit::WinitGraphicsBackend<GlesRenderer>,
    state: &SmithayState,
    start_time: Instant,
) -> Result<()> {
    let size = backend.window_size();
    let damage = Rectangle::from_size(size);
    {
        let (renderer, mut framebuffer) = backend
            .bind()
            .context("failed to bind Smithay framebuffer")?;
        let elements = state
            .xdg_shell_state
            .toplevel_surfaces()
            .iter()
            .flat_map(|surface| {
                render_elements_from_surface_tree(
                    renderer,
                    surface.wl_surface(),
                    (24, 24),
                    1.0,
                    1.0,
                    Kind::Unspecified,
                )
            })
            .collect::<Vec<WaylandSurfaceRenderElement<GlesRenderer>>>();

        let mut frame = renderer
            .render(&mut framebuffer, size, Transform::Flipped180)
            .context("failed to start Smithay render frame")?;
        frame
            .clear(Color32F::new(0.03, 0.035, 0.04, 1.0), &[damage])
            .context("failed to clear Smithay frame")?;
        draw_render_elements(&mut frame, 1.0, &elements, &[damage])
            .context("failed to draw Smithay surfaces")?;
        let _ = frame.finish().context("failed to finish Smithay frame")?;
    }

    let time = start_time.elapsed().as_millis() as u32;
    for surface in state.xdg_shell_state.toplevel_surfaces() {
        send_frames_surface_tree(surface.wl_surface(), time);
    }

    backend
        .submit(Some(&[damage]))
        .context("failed to submit Smithay frame")?;
    Ok(())
}

fn focus_first_toplevel(state: &mut SmithayState) {
    let Some(surface) = state
        .xdg_shell_state
        .toplevel_surfaces()
        .iter()
        .next()
        .map(|surface| surface.wl_surface().clone())
    else {
        return;
    };
    let keyboard = state.keyboard.clone();
    keyboard.set_focus(state, Some(surface), Serial::from(0));
}

fn send_frames_surface_tree(surface: &wl_surface::WlSurface, time: u32) {
    with_surface_tree_downward(
        surface,
        (),
        |_, _, &()| TraversalAction::DoChildren(()),
        |_surface, states, &()| {
            for callback in states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .frame_callbacks
                .drain(..)
            {
                callback.done(time);
            }
        },
        |_, _, &()| true,
    );
}

struct SmithayState {
    compositor_state: CompositorState,
    xdg_shell_state: XdgShellState,
    shm_state: ShmState,
    seat_state: SeatState<Self>,
    data_device_state: DataDeviceState,
    _seat: Seat<Self>,
    keyboard: KeyboardHandle<Self>,
}

impl BufferHandler for SmithayState {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl CompositorHandler for SmithayState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<SmithayClientState>()
            .expect("Smithay client data missing")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
    }
}

impl XdgShellHandler for SmithayState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Activated);
        });
        surface.send_configure();
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {}

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }
}

impl ShmHandler for SmithayState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl SeatHandler for SmithayState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {}

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        _image: smithay::input::pointer::CursorImageStatus,
    ) {
    }
}

impl SelectionHandler for SmithayState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for SmithayState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for SmithayState {}

impl ServerDndGrabHandler for SmithayState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

#[derive(Default)]
struct SmithayClientState {
    compositor_state: CompositorClientState,
}

impl ClientData for SmithayClientState {
    fn initialized(&self, client_id: ClientId) {
        eprintln!("Smithay Wayland client connected: {client_id:?}");
    }

    fn disconnected(&self, client_id: ClientId, reason: DisconnectReason) {
        eprintln!("Smithay Wayland client disconnected: {client_id:?}: {reason:?}");
    }
}

delegate_xdg_shell!(SmithayState);
delegate_compositor!(SmithayState);
delegate_shm!(SmithayState);
delegate_seat!(SmithayState);
delegate_data_device!(SmithayState);
