pub mod types;

pub use types::*;

const BUILTIN_FONT_ROWS: f32 = 7.0;
const BUILTIN_FONT_ADVANCE_COLUMNS: f32 = 6.0;
const BUILTIN_FONT_SPACE_COLUMNS: f32 = 4.0;
const BUILTIN_FONT_LINE_ADVANCE_COLUMNS: f32 = 9.0;
const BUILTIN_TEXT_ELLIPSIS: &str = "...";

pub fn measure_builtin_text(text: &str, size: f32) -> Size {
    let pixel = builtin_font_pixel(size);
    let mut line_width: f32 = 0.0;
    let mut max_width: f32 = 0.0;
    let mut lines = 1;

    for ch in text.chars() {
        if ch == '\n' {
            max_width = max_width.max(line_width);
            line_width = 0.0;
            lines += 1;
        } else if ch == ' ' {
            line_width += pixel * BUILTIN_FONT_SPACE_COLUMNS;
        } else {
            line_width += pixel * BUILTIN_FONT_ADVANCE_COLUMNS;
        }
    }

    max_width = max_width.max(line_width);
    Size::new(
        max_width,
        pixel * (BUILTIN_FONT_ROWS + BUILTIN_FONT_LINE_ADVANCE_COLUMNS * (lines - 1) as f32),
    )
}

pub fn fit_builtin_text_to_width(text: &str, size: f32, max_width: f32) -> String {
    if max_width <= 0.0 {
        return String::new();
    }

    if measure_builtin_text(text, size).width <= max_width {
        return text.to_string();
    }

    let mut suffix = BUILTIN_TEXT_ELLIPSIS;
    while !suffix.is_empty() && measure_builtin_text(suffix, size).width > max_width {
        suffix = &suffix[..suffix.len() - 1];
    }

    if suffix.is_empty() {
        return String::new();
    }

    let suffix_width = measure_builtin_text(suffix, size).width;
    let mut fitted = String::new();
    let mut fitted_width = 0.0;

    for ch in text.chars() {
        let ch_width = measure_builtin_text(&ch.to_string(), size).width;
        if fitted_width + ch_width + suffix_width > max_width {
            break;
        }

        fitted.push(ch);
        fitted_width += ch_width;
    }

    fitted.push_str(suffix);
    fitted
}

fn builtin_font_pixel(size: f32) -> f32 {
    (size / BUILTIN_FONT_ROWS).max(1.0)
}

#[derive(Clone, Debug)]
pub struct DesktopCompositor {
    windows: Vec<Window>,
    next_window_id: u64,
    grab: Option<PointerGrab>,
    launcher: Launcher,
}

impl DesktopCompositor {
    const DEFAULT_TITLEBAR_HEIGHT: f32 = 30.0;
    const DEFAULT_MIN_WINDOW_SIZE: Size = Size::new(140.0, 90.0);
    const TOOLBAR_HEIGHT: f32 = 44.0;
    const TASKBAR_HEIGHT: f32 = 44.0;
    const TASKBAR_BUTTON_HEIGHT: f32 = 30.0;
    const TASKBAR_BUTTON_WIDTH: f32 = 142.0;
    const TASKBAR_BUTTON_GAP: f32 = 8.0;
    const TASKBAR_BUTTON_LEFT_INSET: f32 = 10.0;
    const TASKBAR_BUTTON_TOP_INSET: f32 = 7.0;

    pub fn new() -> Self {
        Self {
            windows: Vec::new(),
            next_window_id: 1,
            grab: None,
            launcher: Launcher::default(),
        }
    }

    pub fn sample() -> Self {
        let mut compositor = Self::new();
        compositor.add_window(WindowSpec {
            title: "Terminal".to_string(),
            bounds: Rect::from_xywh(72.0, 86.0, 360.0, 230.0),
            min_size: None,
            body_color: Color::rgb(0.96, 0.97, 0.98),
            titlebar_color: Color::rgb(0.23, 0.46, 0.78),
            surface: None,
        });
        compositor.add_window(WindowSpec {
            title: "Editor".to_string(),
            bounds: Rect::from_xywh(230.0, 170.0, 420.0, 260.0),
            min_size: None,
            body_color: Color::rgb(0.99, 0.98, 0.94),
            titlebar_color: Color::rgb(0.76, 0.31, 0.27),
            surface: None,
        });
        compositor
    }

    pub fn add_window(&mut self, spec: WindowSpec) -> WindowId {
        let id = WindowId(self.next_window_id);
        self.next_window_id += 1;

        self.windows.push(Window {
            id,
            title: spec.title,
            bounds: spec.bounds,
            restore_bounds: None,
            minimized: false,
            min_size: spec.min_size.unwrap_or(Self::DEFAULT_MIN_WINDOW_SIZE),
            body_color: spec.body_color,
            titlebar_color: spec.titlebar_color,
            titlebar_height: Self::DEFAULT_TITLEBAR_HEIGHT,
            surface: spec.surface,
            surface_content_size: None,
        });

        id
    }

    pub fn add_surface_window(
        &mut self,
        title: impl Into<String>,
        surface: ResourceId,
        content_size: Size,
    ) -> WindowId {
        let id = self.add_window(WindowSpec {
            title: title.into(),
            bounds: Rect::from_xywh(
                118.0 + (self.next_window_id % 5) as f32 * 24.0,
                104.0 + (self.next_window_id % 5) as f32 * 24.0,
                content_size.width.max(Self::DEFAULT_MIN_WINDOW_SIZE.width),
                content_size
                    .height
                    .max(Self::DEFAULT_MIN_WINDOW_SIZE.height),
            ),
            min_size: Some(Self::DEFAULT_MIN_WINDOW_SIZE),
            body_color: Color::rgb(0.05, 0.055, 0.06),
            titlebar_color: Color::rgb(0.23, 0.46, 0.78),
            surface: Some(surface),
        });

        if let Some(window) = self.windows.iter_mut().find(|window| window.id == id) {
            window.titlebar_height = 0.0;
            window.surface_content_size = Some(content_size);
        }

        id
    }

    pub fn remove_window(&mut self, id: WindowId) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };

        self.windows.remove(index);
        self.grab = match self.grab {
            Some(PointerGrab::Move { window, .. }) | Some(PointerGrab::Resize { window, .. })
                if window == id =>
            {
                None
            }
            grab => grab,
        };
        true
    }

    pub fn set_surface_window_content_size(
        &mut self,
        surface: ResourceId,
        content_size: Size,
    ) -> bool {
        let Some(window) = self
            .windows
            .iter_mut()
            .find(|window| window.surface == Some(surface))
        else {
            return false;
        };

        window.surface_content_size = Some(content_size);
        true
    }

    pub fn surface_window_requested_content_size(&self, surface: ResourceId) -> Option<Size> {
        let window = self
            .windows
            .iter()
            .find(|window| window.surface == Some(surface))?;
        Some(Size::new(
            window.bounds.size.width,
            (window.bounds.size.height - window.titlebar_height).max(0.0),
        ))
    }

    pub fn start_window_move(&mut self, id: WindowId, position: Point) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };

        let window = self.windows.remove(index);
        let bounds = window.bounds;
        self.windows.push(window);
        self.grab = Some(PointerGrab::Move {
            window: id,
            offset: Vector::new(position.x - bounds.origin.x, position.y - bounds.origin.y),
        });
        true
    }

    pub fn set_window_title(&mut self, id: WindowId, title: impl Into<String>) -> bool {
        let Some(window) = self.windows.iter_mut().find(|window| window.id == id) else {
            return false;
        };
        window.title = title.into();
        true
    }

    pub fn pointer_down(&mut self, position: Point) {
        let Some(index) = self.hit_test_window(position) else {
            self.grab = None;
            return;
        };

        let window = self.windows.remove(index);
        let id = window.id;
        let bounds = window.bounds;
        let resize_edge = window.resize_edge_at(position);
        let move_offset = Vector::new(position.x - bounds.origin.x, position.y - bounds.origin.y);
        let is_titlebar = window.titlebar_bounds().contains(position);
        let is_control = window.control_at(position).is_some();

        self.windows.push(window);

        if is_control {
            self.grab = None;
        } else if let Some(edge) = resize_edge {
            self.grab = Some(PointerGrab::Resize {
                window: id,
                edge,
                start_position: position,
                start_bounds: bounds,
            });
        } else if is_titlebar {
            self.grab = Some(PointerGrab::Move {
                window: id,
                offset: move_offset,
            });
        } else {
            self.grab = None;
        }
    }

    pub fn activate_control_at(
        &mut self,
        position: Point,
        desktop_size: Size,
    ) -> Option<WindowControl> {
        let index = self.hit_test_window(position)?;
        let control = self.windows[index].control_at(position)?;
        let mut window = self.windows.remove(index);
        let id = window.id;

        match control {
            WindowControl::Close => {
                self.grab = None;
            }
            WindowControl::FullSize => {
                window.toggle_full_size(Self::work_area(desktop_size));
                self.windows.push(window);
                self.grab = None;
            }
            WindowControl::Minimize => {
                window.minimized = true;
                self.windows.push(window);
                self.grab = None;
            }
        }

        if matches!(control, WindowControl::Close) {
            self.grab = match self.grab {
                Some(PointerGrab::Move { window, .. })
                | Some(PointerGrab::Resize { window, .. })
                    if window == id =>
                {
                    None
                }
                grab => grab,
            };
        }

        Some(control)
    }

    pub fn activate_taskbar_at(&mut self, position: Point, desktop_size: Size) -> Option<WindowId> {
        let index = self.hit_test_taskbar_button(position, desktop_size)?;
        let mut window = self.windows.remove(index);
        window.minimized = false;
        let id = window.id;
        self.windows.push(window);
        self.grab = None;
        Some(id)
    }

    pub fn taskbar_contains(&self, position: Point, desktop_size: Size) -> bool {
        Self::taskbar_bounds(desktop_size).contains(position)
    }

    pub fn toggle_full_size_at(&mut self, position: Point, desktop_size: Size) -> bool {
        let Some(index) = self.hit_test_window(position) else {
            return false;
        };

        let window = &self.windows[index];
        let is_titlebar = window.titlebar_bounds().contains(position);
        let is_resize_edge = window.resize_edge_at(position).is_some();
        let is_control = window.control_at(position).is_some();

        if !is_titlebar || is_resize_edge || is_control {
            return false;
        }

        let mut window = self.windows.remove(index);
        window.toggle_full_size(Self::work_area(desktop_size));
        self.windows.push(window);
        self.grab = None;
        true
    }

    pub fn set_window_full_size(
        &mut self,
        id: WindowId,
        desktop_size: Size,
        full_size: bool,
    ) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };

        let mut window = self.windows.remove(index);
        if full_size {
            if window.restore_bounds.is_none() {
                window.restore_bounds = Some(window.bounds);
            }
            window.bounds = Self::work_area(desktop_size);
        } else if let Some(restore_bounds) = window.restore_bounds.take() {
            window.bounds = restore_bounds;
        }

        self.windows.push(window);
        self.grab = None;
        true
    }

    pub fn set_window_minimized(&mut self, id: WindowId, minimized: bool) -> bool {
        let Some(index) = self.windows.iter().position(|window| window.id == id) else {
            return false;
        };

        let mut window = self.windows.remove(index);
        window.minimized = minimized;
        self.windows.push(window);
        self.grab = None;
        true
    }

    pub fn pointer_move(&mut self, position: Point) {
        let Some(grab) = self.grab else {
            return;
        };

        match grab {
            PointerGrab::Move { window, offset } => {
                if let Some(window) = self.windows.iter_mut().find(|item| item.id == window) {
                    window.bounds.origin =
                        Point::new(position.x - offset.dx, position.y - offset.dy);
                }
            }
            PointerGrab::Resize {
                window,
                edge,
                start_position,
                start_bounds,
            } => {
                if let Some(window) = self.windows.iter_mut().find(|item| item.id == window) {
                    let delta =
                        Vector::new(position.x - start_position.x, position.y - start_position.y);
                    window.bounds = resize_bounds(start_bounds, window.min_size, edge, delta);
                }
            }
        }
    }

    pub fn pointer_up(&mut self) {
        self.grab = None;
    }

    pub fn pointer_grab_active(&self) -> bool {
        self.grab.is_some()
    }

    pub fn pointer_interaction(&self, position: Point, desktop_size: Size) -> PointerInteraction {
        if self.launcher.open {
            return PointerInteraction::Default;
        }

        if self.taskbar_contains(position, desktop_size) {
            return PointerInteraction::Default;
        }

        let Some(index) = self.hit_test_window(position) else {
            return PointerInteraction::Default;
        };

        let window = &self.windows[index];
        if window.control_at(position).is_some() {
            PointerInteraction::Default
        } else if let Some(edge) = window.resize_edge_at(position) {
            PointerInteraction::Resize(edge)
        } else if window.titlebar_bounds().contains(position) {
            PointerInteraction::Move
        } else {
            PointerInteraction::Default
        }
    }

    pub fn surface_hit_at(&self, position: Point, desktop_size: Size) -> Option<SurfaceHit> {
        if self.launcher.open || self.taskbar_contains(position, desktop_size) {
            return None;
        }

        self.windows.iter().rev().find_map(|window| {
            if window.minimized || !window.bounds.contains(position) {
                return None;
            }

            let surface = window.surface?;
            let local = Point::new(
                position.x - window.bounds.origin.x,
                position.y - window.bounds.origin.y,
            );
            let surface_bounds = window.local_surface_bounds();
            surface_bounds.contains(local).then_some(SurfaceHit {
                window: window.id,
                surface,
                position: Point::new(
                    local.x - surface_bounds.origin.x,
                    local.y - surface_bounds.origin.y,
                ),
            })
        })
    }

    pub fn scene(&self, size: Size) -> Scene {
        desktop_scene(size, &self.windows, &self.launcher)
    }

    pub fn windows(&self) -> &[Window] {
        &self.windows
    }

    pub fn toggle_launcher(&mut self) {
        self.launcher.open = !self.launcher.open;
        self.launcher.query.clear();
        self.launcher.selected = 0;
        self.grab = None;
    }

    pub fn close_launcher(&mut self) {
        self.launcher.open = false;
        self.launcher.query.clear();
        self.launcher.selected = 0;
    }

    pub fn launcher_is_open(&self) -> bool {
        self.launcher.open
    }

    pub fn launcher_insert_text(&mut self, text: &str) {
        if !self.launcher.open {
            return;
        }

        for ch in text.chars().filter(|ch| !ch.is_control()) {
            self.launcher.query.push(ch);
        }
        self.launcher.selected = 0;
    }

    pub fn launcher_backspace(&mut self) {
        if self.launcher.open {
            self.launcher.query.pop();
            self.launcher.selected = 0;
        }
    }

    pub fn launcher_select_next(&mut self) {
        if !self.launcher.open {
            return;
        }

        let count = matching_apps(&self.launcher.query).len();
        if count > 0 {
            self.launcher.selected = (self.launcher.selected + 1) % count;
        }
    }

    pub fn launcher_select_previous(&mut self) {
        if !self.launcher.open {
            return;
        }

        let count = matching_apps(&self.launcher.query).len();
        if count > 0 {
            self.launcher.selected = if self.launcher.selected == 0 {
                count - 1
            } else {
                self.launcher.selected - 1
            };
        }
    }

    pub fn launcher_launch_selected(&mut self) -> Option<LauncherLaunch> {
        if !self.launcher.open {
            return None;
        }

        let matches = matching_apps(&self.launcher.query);
        let app = matches.get(self.launcher.selected).copied()?;
        if let Some(external) = app.external {
            self.close_launcher();
            return Some(LauncherLaunch::External(external));
        }

        let id = self.add_window(app.window_spec(self.next_window_id));
        self.close_launcher();
        Some(LauncherLaunch::Window(id))
    }

    fn hit_test_window(&self, position: Point) -> Option<usize> {
        if self.launcher.open {
            return None;
        }

        self.windows
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, window)| {
                (!window.minimized && window.bounds.contains(position)).then_some(index)
            })
    }

    fn hit_test_taskbar_button(&self, position: Point, desktop_size: Size) -> Option<usize> {
        if !Self::taskbar_bounds(desktop_size).contains(position) {
            return None;
        }

        self.windows.iter().enumerate().find_map(|(index, _)| {
            Self::taskbar_button_bounds(index, desktop_size)
                .contains(position)
                .then_some(index)
        })
    }

    fn work_area(size: Size) -> Rect {
        Rect::from_xywh(
            0.0,
            Self::TOOLBAR_HEIGHT,
            size.width,
            (size.height - Self::TOOLBAR_HEIGHT - Self::TASKBAR_HEIGHT).max(0.0),
        )
    }

    fn taskbar_bounds(size: Size) -> Rect {
        Rect::from_xywh(
            0.0,
            (size.height - Self::TASKBAR_HEIGHT).max(0.0),
            size.width,
            Self::TASKBAR_HEIGHT,
        )
    }

    fn taskbar_button_bounds(index: usize, desktop_size: Size) -> Rect {
        let x = Self::TASKBAR_BUTTON_LEFT_INSET
            + index as f32 * (Self::TASKBAR_BUTTON_WIDTH + Self::TASKBAR_BUTTON_GAP);
        let taskbar = Self::taskbar_bounds(desktop_size);

        Rect::from_xywh(
            x,
            taskbar.origin.y + Self::TASKBAR_BUTTON_TOP_INSET,
            Self::TASKBAR_BUTTON_WIDTH,
            Self::TASKBAR_BUTTON_HEIGHT,
        )
    }
}

impl Default for DesktopCompositor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WindowId(pub u64);

#[derive(Clone, Debug)]
pub struct Window {
    pub id: WindowId,
    pub title: String,
    pub bounds: Rect,
    pub restore_bounds: Option<Rect>,
    pub minimized: bool,
    pub min_size: Size,
    pub body_color: Color,
    pub titlebar_color: Color,
    pub titlebar_height: f32,
    pub surface: Option<ResourceId>,
    pub surface_content_size: Option<Size>,
}

impl Window {
    const RESIZE_BORDER_WIDTH: f32 = 8.0;
    const HANDLE_SIZE: f32 = 16.0;
    const CONTROL_BUTTON_SIZE: f32 = 16.0;
    const CONTROL_BUTTON_GAP: f32 = 6.0;
    const CONTROL_BUTTON_RIGHT_INSET: f32 = 8.0;
    const CONTROL_BUTTON_TOP_INSET: f32 = 7.0;

    pub fn is_decorated(&self) -> bool {
        self.titlebar_height > 0.0
    }

    pub fn titlebar_bounds(&self) -> Rect {
        Rect::from_xywh(
            self.bounds.origin.x,
            self.bounds.origin.y,
            self.bounds.size.width,
            self.titlebar_height,
        )
    }

    pub fn resize_edge_at(&self, position: Point) -> Option<ResizeEdge> {
        if !self.bounds.contains(position) {
            return None;
        }

        let left = position.x - self.bounds.origin.x <= Self::RESIZE_BORDER_WIDTH;
        let right =
            self.bounds.origin.x + self.bounds.size.width - position.x <= Self::RESIZE_BORDER_WIDTH;
        let top = position.y - self.bounds.origin.y <= Self::RESIZE_BORDER_WIDTH;
        let bottom = self.bounds.origin.y + self.bounds.size.height - position.y
            <= Self::RESIZE_BORDER_WIDTH;

        match (top, right, bottom, left) {
            (true, true, _, _) => Some(ResizeEdge::TopRight),
            (true, _, _, true) => Some(ResizeEdge::TopLeft),
            (_, true, true, _) => Some(ResizeEdge::BottomRight),
            (_, _, true, true) => Some(ResizeEdge::BottomLeft),
            (true, _, _, _) => Some(ResizeEdge::Top),
            (_, true, _, _) => Some(ResizeEdge::Right),
            (_, _, true, _) => Some(ResizeEdge::Bottom),
            (_, _, _, true) => Some(ResizeEdge::Left),
            _ => None,
        }
    }

    pub fn resize_handle_bounds(&self) -> Rect {
        Rect::from_xywh(
            self.bounds.size.width - Self::HANDLE_SIZE,
            self.bounds.size.height - Self::HANDLE_SIZE,
            Self::HANDLE_SIZE,
            Self::HANDLE_SIZE,
        )
    }

    pub fn control_at(&self, position: Point) -> Option<WindowControl> {
        if !self.is_decorated() {
            return None;
        }

        self.control_bounds()
            .into_iter()
            .find_map(|(control, bounds)| bounds.contains(position).then_some(control))
    }

    pub fn control_bounds(&self) -> [(WindowControl, Rect); 3] {
        [
            (WindowControl::Minimize, self.control_bounds_for_index(2)),
            (WindowControl::FullSize, self.control_bounds_for_index(1)),
            (WindowControl::Close, self.control_bounds_for_index(0)),
        ]
    }

    pub fn local_control_bounds(&self) -> [(WindowControl, Rect); 3] {
        self.control_bounds().map(|(control, bounds)| {
            (
                control,
                Rect::from_xywh(
                    bounds.origin.x - self.bounds.origin.x,
                    bounds.origin.y - self.bounds.origin.y,
                    bounds.size.width,
                    bounds.size.height,
                ),
            )
        })
    }

    pub fn local_surface_bounds(&self) -> Rect {
        let available = Size::new(
            self.bounds.size.width,
            (self.bounds.size.height - self.titlebar_height).max(0.0),
        );
        let size = self.surface_content_size.unwrap_or(available);

        Rect::from_xywh(
            0.0,
            self.titlebar_height,
            size.width.min(available.width),
            size.height.min(available.height),
        )
    }

    fn toggle_full_size(&mut self, bounds: Rect) {
        if let Some(restore_bounds) = self.restore_bounds.take() {
            self.bounds = restore_bounds;
        } else {
            self.restore_bounds = Some(self.bounds);
            self.bounds = bounds;
        }
    }

    fn control_bounds_for_index(&self, index_from_right: usize) -> Rect {
        let size = Self::CONTROL_BUTTON_SIZE;
        let step = Self::CONTROL_BUTTON_SIZE + Self::CONTROL_BUTTON_GAP;
        Rect::from_xywh(
            self.bounds.origin.x + self.bounds.size.width
                - Self::CONTROL_BUTTON_RIGHT_INSET
                - size
                - step * index_from_right as f32,
            self.bounds.origin.y + Self::CONTROL_BUTTON_TOP_INSET,
            size,
            size,
        )
    }
}

#[derive(Clone, Debug)]
pub struct WindowSpec {
    pub title: String,
    pub bounds: Rect,
    pub min_size: Option<Size>,
    pub body_color: Color,
    pub titlebar_color: Color,
    pub surface: Option<ResourceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WindowControl {
    Minimize,
    FullSize,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceHit {
    pub window: WindowId,
    pub surface: ResourceId,
    pub position: Point,
}

#[derive(Clone, Debug, Default)]
struct Launcher {
    open: bool,
    query: String,
    selected: usize,
}

#[derive(Clone, Copy, Debug)]
struct AppDefinition {
    name: &'static str,
    body_color: Color,
    titlebar_color: Color,
    size: Size,
    external: Option<ExternalApp>,
}

impl AppDefinition {
    const fn internal(
        name: &'static str,
        body_color: Color,
        titlebar_color: Color,
        size: Size,
    ) -> Self {
        Self {
            name,
            body_color,
            titlebar_color,
            size,
            external: None,
        }
    }

    const fn external(name: &'static str, external: ExternalApp) -> Self {
        Self {
            name,
            body_color: Color::rgb(0.96, 0.97, 0.98),
            titlebar_color: Color::rgb(0.23, 0.46, 0.78),
            size: Size::new(520.0, 360.0),
            external: Some(external),
        }
    }

    fn window_spec(self, next_window_id: u64) -> WindowSpec {
        let offset = ((next_window_id - 1) % 6) as f32 * 28.0;
        WindowSpec {
            title: self.name.to_string(),
            bounds: Rect::from_xywh(
                92.0 + offset,
                92.0 + offset,
                self.size.width,
                self.size.height,
            ),
            min_size: None,
            body_color: self.body_color,
            titlebar_color: self.titlebar_color,
            surface: None,
        }
    }
}

const APP_CATALOG: [AppDefinition; 10] = [
    AppDefinition::internal(
        "Terminal",
        Color::rgb(0.96, 0.97, 0.98),
        Color::rgb(0.23, 0.46, 0.78),
        Size::new(380.0, 240.0),
    ),
    AppDefinition::internal(
        "Editor",
        Color::rgb(0.99, 0.98, 0.94),
        Color::rgb(0.76, 0.31, 0.27),
        Size::new(430.0, 280.0),
    ),
    AppDefinition::internal(
        "Files",
        Color::rgb(0.94, 0.97, 0.99),
        Color::rgb(0.22, 0.58, 0.62),
        Size::new(420.0, 260.0),
    ),
    AppDefinition::internal(
        "Browser",
        Color::rgb(0.98, 0.98, 0.99),
        Color::rgb(0.42, 0.40, 0.72),
        Size::new(500.0, 320.0),
    ),
    AppDefinition::internal(
        "Settings",
        Color::rgb(0.96, 0.96, 0.95),
        Color::rgb(0.42, 0.47, 0.52),
        Size::new(360.0, 240.0),
    ),
    AppDefinition::internal(
        "Monitor",
        Color::rgb(0.92, 0.96, 0.92),
        Color::rgb(0.34, 0.62, 0.34),
        Size::new(420.0, 250.0),
    ),
    AppDefinition::internal(
        "Calendar",
        Color::rgb(0.98, 0.95, 0.97),
        Color::rgb(0.74, 0.35, 0.56),
        Size::new(390.0, 260.0),
    ),
    AppDefinition::internal(
        "Calculator",
        Color::rgb(0.95, 0.95, 0.97),
        Color::rgb(0.35, 0.45, 0.64),
        Size::new(260.0, 280.0),
    ),
    AppDefinition::external("Telegram", ExternalApp::Telegram),
    AppDefinition::external("Chrome", ExternalApp::Chrome),
];

fn matching_apps(query: &str) -> Vec<AppDefinition> {
    let query = query.trim().to_lowercase();
    APP_CATALOG
        .iter()
        .copied()
        .filter(|app| query.is_empty() || app.name.to_lowercase().contains(&query))
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LauncherLaunch {
    Window(WindowId),
    External(ExternalApp),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalApp {
    Telegram,
    Chrome,
}

#[derive(Clone, Copy, Debug)]
enum PointerGrab {
    Move {
        window: WindowId,
        offset: Vector,
    },
    Resize {
        window: WindowId,
        edge: ResizeEdge,
        start_position: Point,
        start_bounds: Rect,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerInteraction {
    Default,
    Move,
    Resize(ResizeEdge),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResizeEdge {
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn affects_left(self) -> bool {
        matches!(self, Self::Left | Self::TopLeft | Self::BottomLeft)
    }

    fn affects_right(self) -> bool {
        matches!(self, Self::Right | Self::TopRight | Self::BottomRight)
    }

    fn affects_top(self) -> bool {
        matches!(self, Self::Top | Self::TopLeft | Self::TopRight)
    }

    fn affects_bottom(self) -> bool {
        matches!(self, Self::Bottom | Self::BottomLeft | Self::BottomRight)
    }
}

pub fn sample_desktop_scene(size: Size) -> Scene {
    DesktopCompositor::sample().scene(size)
}

fn desktop_scene(size: Size, windows: &[Window], launcher: &Launcher) -> Scene {
    let mut scene = Scene::new();

    let background = scene.add_node(Node::rect(RectStyle {
        bounds: Rect::from_xywh(0.0, 0.0, size.width, size.height),
        fill: Some(Color::rgb(0.11, 0.12, 0.14)),
        stroke: None,
        radius: 0.0,
    }));

    let toolbar = scene.add_node(Node::rect(RectStyle {
        bounds: Rect::from_xywh(0.0, 0.0, size.width, DesktopCompositor::TOOLBAR_HEIGHT),
        fill: Some(Color::rgb(0.84, 0.86, 0.89)),
        stroke: Some(Stroke {
            color: Color::rgb(0.58, 0.60, 0.64),
            width: 1.0,
        }),
        radius: 0.0,
    }));

    scene.roots = vec![background, toolbar];
    add_text(
        &mut scene,
        "PuppyOS   Ctrl+Space",
        Point::new(14.0, 14.0),
        13.0,
        Color::rgb(0.10, 0.11, 0.13),
    );

    for window in windows {
        if window.minimized {
            continue;
        }

        let body = scene.add_node(Node::rect(RectStyle {
            bounds: Rect::from_xywh(
                0.0,
                0.0,
                window.bounds.size.width,
                window.bounds.size.height,
            ),
            fill: Some(window.body_color),
            stroke: Some(Stroke {
                color: Color::rgb(0.22, 0.24, 0.28),
                width: 2.0,
            }),
            radius: 0.0,
        }));

        let mut children = vec![body];
        if window.is_decorated() {
            let titlebar = scene.add_node(Node::rect(RectStyle {
                bounds: Rect::from_xywh(0.0, 0.0, window.bounds.size.width, window.titlebar_height),
                fill: Some(window.titlebar_color),
                stroke: None,
                radius: 0.0,
            }));
            children.push(titlebar);
        }

        if let Some(surface) = window.surface {
            let surface = scene.add_node(Node::new(NodeKind::Surface(SurfaceRef {
                id: surface,
                bounds: window.local_surface_bounds(),
            })));
            children.push(surface);
        }

        if window.is_decorated() {
            let title_max_width = (window.bounds.size.width
                - 10.0
                - Window::CONTROL_BUTTON_RIGHT_INSET
                - Window::CONTROL_BUTTON_SIZE * 3.0
                - Window::CONTROL_BUTTON_GAP * 2.0
                - 10.0)
                .max(0.0);
            let title = fit_builtin_text_to_width(&window.title, 12.0, title_max_width);
            let title = scene.add_node(Node::new(NodeKind::Text(TextRun {
                text: title,
                position: Point::new(10.0, 9.0),
                color: Color::rgb(0.05, 0.06, 0.07),
                size: 12.0,
                font: "builtin".to_string(),
            })));
            children.push(title);

            for (control, bounds) in window.local_control_bounds() {
                let button = scene.add_node(Node::rect(RectStyle {
                    bounds,
                    fill: Some(control_color(control)),
                    stroke: Some(Stroke {
                        color: Color::rgba(0.0, 0.0, 0.0, 0.35),
                        width: 1.0,
                    }),
                    radius: 0.0,
                }));
                children.push(button);

                for icon_bounds in control_icon_bounds(control, bounds) {
                    let icon = scene.add_node(Node::rect(RectStyle {
                        bounds: icon_bounds,
                        fill: Some(Color::rgba(0.0, 0.0, 0.0, 0.70)),
                        stroke: None,
                        radius: 0.0,
                    }));
                    children.push(icon);
                }
            }
        }

        if window.is_decorated() {
            let resize_handle = scene.add_node(Node::rect(RectStyle {
                bounds: window.resize_handle_bounds(),
                fill: Some(Color::rgba(0.0, 0.0, 0.0, 0.18)),
                stroke: None,
                radius: 0.0,
            }));
            children.push(resize_handle);
        } else {
            let resize_frame = scene.add_node(Node::rect(RectStyle {
                bounds: Rect::from_xywh(
                    0.0,
                    0.0,
                    window.bounds.size.width,
                    window.bounds.size.height,
                ),
                fill: None,
                stroke: Some(Stroke {
                    color: Color::rgba(0.0, 0.0, 0.0, 0.35),
                    width: 2.0,
                }),
                radius: 0.0,
            }));
            children.push(resize_frame);
        }

        let mut group = Node::group(children);
        group.transform.translation = window.bounds.origin;
        let group = scene.add_node(group);
        scene.roots.push(group);
    }

    let taskbar = scene.add_node(Node::rect(RectStyle {
        bounds: DesktopCompositor::taskbar_bounds(size),
        fill: Some(Color::rgb(0.20, 0.21, 0.24)),
        stroke: Some(Stroke {
            color: Color::rgb(0.45, 0.46, 0.50),
            width: 1.0,
        }),
        radius: 0.0,
    }));
    scene.roots.push(taskbar);

    for (index, window) in windows.iter().enumerate() {
        let button_bounds = DesktopCompositor::taskbar_button_bounds(index, size);
        let button = scene.add_node(Node::rect(RectStyle {
            bounds: button_bounds,
            fill: Some(if window.minimized {
                Color::rgb(0.28, 0.29, 0.31)
            } else {
                Color::rgb(0.88, 0.89, 0.91)
            }),
            stroke: Some(Stroke {
                color: if window.minimized {
                    Color::rgb(0.45, 0.46, 0.49)
                } else {
                    Color::rgb(0.72, 0.74, 0.78)
                },
                width: 1.0,
            }),
            radius: 0.0,
        }));
        scene.roots.push(button);

        let color_strip = scene.add_node(Node::rect(RectStyle {
            bounds: Rect::from_xywh(
                button_bounds.origin.x + 6.0,
                button_bounds.origin.y + 6.0,
                8.0,
                button_bounds.size.height - 12.0,
            ),
            fill: Some(window.titlebar_color),
            stroke: None,
            radius: 0.0,
        }));
        scene.roots.push(color_strip);

        let status_line = scene.add_node(Node::rect(RectStyle {
            bounds: Rect::from_xywh(
                button_bounds.origin.x + 22.0,
                button_bounds.origin.y + button_bounds.size.height - 8.0,
                button_bounds.size.width - 34.0,
                3.0,
            ),
            fill: Some(if window.minimized {
                Color::rgb(0.58, 0.59, 0.62)
            } else {
                window.titlebar_color
            }),
            stroke: None,
            radius: 0.0,
        }));
        scene.roots.push(status_line);

        let taskbar_title =
            fit_builtin_text_to_width(&window.title, 11.0, button_bounds.size.width - 28.0);
        add_text(
            &mut scene,
            &taskbar_title,
            Point::new(button_bounds.origin.x + 22.0, button_bounds.origin.y + 9.0),
            11.0,
            if window.minimized {
                Color::rgb(0.78, 0.79, 0.82)
            } else {
                Color::rgb(0.12, 0.13, 0.15)
            },
        );
    }

    if launcher.open {
        add_launcher_scene(&mut scene, size, launcher);
    }

    scene
}

fn add_launcher_scene(scene: &mut Scene, size: Size, launcher: &Launcher) {
    let overlay = scene.add_node(Node::rect(RectStyle {
        bounds: Rect::from_xywh(0.0, 0.0, size.width, size.height),
        fill: Some(Color::rgba(0.0, 0.0, 0.0, 0.35)),
        stroke: None,
        radius: 0.0,
    }));
    scene.roots.push(overlay);

    let panel_width = 520.0_f32.min((size.width - 48.0).max(280.0));
    let panel_height = 360.0_f32.min((size.height - 96.0).max(220.0));
    let panel = Rect::from_xywh(
        (size.width - panel_width) / 2.0,
        (size.height - panel_height) / 2.0,
        panel_width,
        panel_height,
    );

    let panel_node = scene.add_node(Node::rect(RectStyle {
        bounds: panel,
        fill: Some(Color::rgb(0.94, 0.95, 0.96)),
        stroke: Some(Stroke {
            color: Color::rgb(0.28, 0.30, 0.34),
            width: 2.0,
        }),
        radius: 0.0,
    }));
    scene.roots.push(panel_node);

    add_text(
        scene,
        "Launch  Ctrl+Space",
        Point::new(panel.origin.x + 20.0, panel.origin.y + 18.0),
        18.0,
        Color::rgb(0.12, 0.13, 0.15),
    );

    let search = Rect::from_xywh(
        panel.origin.x + 20.0,
        panel.origin.y + 52.0,
        panel.size.width - 40.0,
        38.0,
    );
    let search_node = scene.add_node(Node::rect(RectStyle {
        bounds: search,
        fill: Some(Color::rgb(1.0, 1.0, 1.0)),
        stroke: Some(Stroke {
            color: Color::rgb(0.62, 0.64, 0.68),
            width: 1.0,
        }),
        radius: 0.0,
    }));
    scene.roots.push(search_node);

    let query = if launcher.query.is_empty() {
        "type to search"
    } else {
        launcher.query.as_str()
    };
    let query = fit_builtin_text_to_width(query, 14.0, search.size.width - 24.0);
    add_text(
        scene,
        &query,
        Point::new(search.origin.x + 12.0, search.origin.y + 11.0),
        14.0,
        if launcher.query.is_empty() {
            Color::rgb(0.48, 0.50, 0.54)
        } else {
            Color::rgb(0.09, 0.10, 0.12)
        },
    );

    let matches = matching_apps(&launcher.query);
    let row_height = 38.0;
    let rows_top = panel.origin.y + 106.0;
    let max_rows =
        ((panel.origin.y + panel.size.height - rows_top - 18.0) / row_height).max(0.0) as usize;

    if matches.is_empty() {
        add_text(
            scene,
            "No apps found",
            Point::new(panel.origin.x + 22.0, rows_top + 10.0),
            14.0,
            Color::rgb(0.42, 0.44, 0.48),
        );
    }

    for (row, app) in matches.iter().take(max_rows).enumerate() {
        let y = rows_top + row as f32 * row_height;
        let row_bounds = Rect::from_xywh(
            panel.origin.x + 14.0,
            y,
            panel.size.width - 28.0,
            row_height - 4.0,
        );

        let row_node = scene.add_node(Node::rect(RectStyle {
            bounds: row_bounds,
            fill: Some(if row == launcher.selected {
                Color::rgb(0.78, 0.86, 0.96)
            } else {
                Color::rgb(0.94, 0.95, 0.96)
            }),
            stroke: None,
            radius: 0.0,
        }));
        scene.roots.push(row_node);

        let strip = scene.add_node(Node::rect(RectStyle {
            bounds: Rect::from_xywh(
                row_bounds.origin.x + 8.0,
                row_bounds.origin.y + 8.0,
                7.0,
                row_bounds.size.height - 16.0,
            ),
            fill: Some(app.titlebar_color),
            stroke: None,
            radius: 0.0,
        }));
        scene.roots.push(strip);

        let app_name = fit_builtin_text_to_width(app.name, 14.0, row_bounds.size.width - 36.0);
        add_text(
            scene,
            &app_name,
            Point::new(row_bounds.origin.x + 26.0, row_bounds.origin.y + 10.0),
            14.0,
            Color::rgb(0.11, 0.12, 0.14),
        );
    }
}

fn add_text(scene: &mut Scene, text: &str, position: Point, size: f32, color: Color) {
    let node = scene.add_node(Node::new(NodeKind::Text(TextRun {
        text: text.to_string(),
        position,
        color,
        size,
        font: "builtin".to_string(),
    })));
    scene.roots.push(node);
}

fn control_color(control: WindowControl) -> Color {
    match control {
        WindowControl::Minimize => Color::rgb(0.94, 0.75, 0.22),
        WindowControl::FullSize => Color::rgb(0.32, 0.72, 0.38),
        WindowControl::Close => Color::rgb(0.86, 0.24, 0.22),
    }
}

fn control_icon_bounds(control: WindowControl, bounds: Rect) -> Vec<Rect> {
    match control {
        WindowControl::Minimize => vec![Rect::from_xywh(
            bounds.origin.x + 4.0,
            bounds.origin.y + 11.0,
            8.0,
            2.0,
        )],
        WindowControl::FullSize => vec![
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 4.0, 8.0, 2.0),
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 10.0, 8.0, 2.0),
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 4.0, 2.0, 8.0),
            Rect::from_xywh(bounds.origin.x + 10.0, bounds.origin.y + 4.0, 2.0, 8.0),
        ],
        WindowControl::Close => vec![
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 4.0, 8.0, 2.0),
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 10.0, 8.0, 2.0),
            Rect::from_xywh(bounds.origin.x + 4.0, bounds.origin.y + 4.0, 2.0, 8.0),
            Rect::from_xywh(bounds.origin.x + 10.0, bounds.origin.y + 4.0, 2.0, 8.0),
        ],
    }
}

fn resize_bounds(start: Rect, min_size: Size, edge: ResizeEdge, delta: Vector) -> Rect {
    let start_left = start.origin.x;
    let start_top = start.origin.y;
    let start_right = start.origin.x + start.size.width;
    let start_bottom = start.origin.y + start.size.height;

    let mut left = start_left;
    let mut top = start_top;
    let mut right = start_right;
    let mut bottom = start_bottom;

    if edge.affects_left() {
        left = (start_left + delta.dx).min(start_right - min_size.width);
    }

    if edge.affects_right() {
        right = (start_right + delta.dx).max(start_left + min_size.width);
    }

    if edge.affects_top() {
        top = (start_top + delta.dy).min(start_bottom - min_size.height);
    }

    if edge.affects_bottom() {
        bottom = (start_bottom + delta.dy).max(start_top + min_size.height);
    }

    Rect::from_xywh(left, top, right - left, bottom - top)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_drag_moves_window_by_titlebar() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds.origin;

        compositor.pointer_down(Point::new(initial.x + 12.0, initial.y + 10.0));
        compositor.pointer_move(Point::new(initial.x + 42.0, initial.y + 55.0));
        compositor.pointer_up();

        assert_eq!(
            compositor.windows()[1].bounds.origin,
            Point::new(initial.x + 30.0, initial.y + 45.0)
        );
    }

    #[test]
    fn pointer_down_raises_hit_window() {
        let mut compositor = DesktopCompositor::sample();
        let first = compositor.windows()[0].id;

        compositor.pointer_down(Point::new(90.0, 100.0));

        assert_eq!(
            compositor.windows().last().map(|window| window.id),
            Some(first)
        );
    }

    #[test]
    fn pointer_drag_ignores_window_body() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds.origin;

        compositor.pointer_down(Point::new(initial.x + 12.0, initial.y + 80.0));
        compositor.pointer_move(Point::new(initial.x + 120.0, initial.y + 150.0));
        compositor.pointer_up();

        assert_eq!(compositor.windows()[1].bounds.origin, initial);
    }

    #[test]
    fn pointer_drag_resizes_from_bottom_right() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let start = Point::new(
            initial.origin.x + initial.size.width - 3.0,
            initial.origin.y + initial.size.height - 3.0,
        );

        compositor.pointer_down(start);
        compositor.pointer_move(Point::new(start.x + 35.0, start.y + 45.0));
        compositor.pointer_up();

        assert_eq!(
            compositor.windows()[1].bounds,
            Rect::from_xywh(
                initial.origin.x,
                initial.origin.y,
                initial.size.width + 35.0,
                initial.size.height + 45.0
            )
        );
    }

    #[test]
    fn pointer_drag_resizes_from_left_and_preserves_right_edge() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let start = Point::new(initial.origin.x + 2.0, initial.origin.y + 120.0);

        compositor.pointer_down(start);
        compositor.pointer_move(Point::new(start.x + 25.0, start.y));
        compositor.pointer_up();

        assert_eq!(
            compositor.windows()[1].bounds,
            Rect::from_xywh(
                initial.origin.x + 25.0,
                initial.origin.y,
                initial.size.width - 25.0,
                initial.size.height
            )
        );
    }

    #[test]
    fn pointer_resize_clamps_to_min_size() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let min_size = compositor.windows()[1].min_size;
        let start = Point::new(initial.origin.x + 2.0, initial.origin.y + 120.0);

        compositor.pointer_down(start);
        compositor.pointer_move(Point::new(start.x + initial.size.width, start.y));
        compositor.pointer_up();

        let resized = compositor.windows()[1].bounds;
        assert_eq!(resized.size.width, min_size.width);
        assert_eq!(
            resized.origin.x,
            initial.origin.x + initial.size.width - min_size.width
        );
    }

    #[test]
    fn titlebar_double_click_toggles_full_size() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let titlebar_position = Point::new(initial.origin.x + 30.0, initial.origin.y + 15.0);

        assert!(compositor.toggle_full_size_at(titlebar_position, Size::new(900.0, 700.0)));
        assert_eq!(
            compositor.windows()[1].bounds,
            Rect::from_xywh(0.0, DesktopCompositor::TOOLBAR_HEIGHT, 900.0, 612.0)
        );

        let restored_titlebar_position = Point::new(30.0, DesktopCompositor::TOOLBAR_HEIGHT + 15.0);
        assert!(
            compositor.toggle_full_size_at(restored_titlebar_position, Size::new(900.0, 700.0))
        );
        assert_eq!(compositor.windows()[1].bounds, initial);
    }

    #[test]
    fn set_window_full_size_by_id_toggles_bounds() {
        let mut compositor = DesktopCompositor::sample();
        let id = compositor.windows()[1].id;
        let initial = compositor.windows()[1].bounds;

        assert!(compositor.set_window_full_size(id, Size::new(900.0, 700.0), true));
        assert_eq!(
            compositor.windows().last().unwrap().bounds,
            Rect::from_xywh(0.0, DesktopCompositor::TOOLBAR_HEIGHT, 900.0, 612.0)
        );

        assert!(compositor.set_window_full_size(id, Size::new(900.0, 700.0), false));
        assert_eq!(compositor.windows().last().unwrap().bounds, initial);
    }

    #[test]
    fn full_size_toggle_ignores_body_and_resize_border() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let body_position = Point::new(initial.origin.x + 30.0, initial.origin.y + 80.0);
        let border_position = Point::new(initial.origin.x + 2.0, initial.origin.y + 15.0);

        assert!(!compositor.toggle_full_size_at(body_position, Size::new(900.0, 700.0)));
        assert!(!compositor.toggle_full_size_at(border_position, Size::new(900.0, 700.0)));
        assert_eq!(compositor.windows()[1].bounds, initial);
    }

    #[test]
    fn close_control_removes_window() {
        let mut compositor = DesktopCompositor::sample();
        let id = compositor.windows()[1].id;
        let position = control_center(&compositor.windows()[1], WindowControl::Close);

        assert_eq!(
            compositor.activate_control_at(position, Size::new(900.0, 700.0)),
            Some(WindowControl::Close)
        );

        assert_eq!(compositor.windows().len(), 1);
        assert!(compositor.windows().iter().all(|window| window.id != id));
    }

    #[test]
    fn remove_window_removes_matching_window() {
        let mut compositor = DesktopCompositor::sample();

        assert!(compositor.remove_window(WindowId(1)));

        assert_eq!(compositor.windows().len(), 1);
        assert_eq!(compositor.windows()[0].id, WindowId(2));
        assert!(!compositor.remove_window(WindowId(99)));
    }

    #[test]
    fn full_size_control_toggles_window_size() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds;
        let position = control_center(&compositor.windows()[1], WindowControl::FullSize);

        assert_eq!(
            compositor.activate_control_at(position, Size::new(900.0, 700.0)),
            Some(WindowControl::FullSize)
        );
        assert_eq!(
            compositor.windows()[1].bounds,
            Rect::from_xywh(0.0, DesktopCompositor::TOOLBAR_HEIGHT, 900.0, 612.0)
        );

        let restore_position = control_center(&compositor.windows()[1], WindowControl::FullSize);
        assert_eq!(
            compositor.activate_control_at(restore_position, Size::new(900.0, 700.0)),
            Some(WindowControl::FullSize)
        );
        assert_eq!(compositor.windows()[1].bounds, initial);
    }

    #[test]
    fn minimize_control_hides_window_from_scene() {
        let mut compositor = DesktopCompositor::sample();
        let position = control_center(&compositor.windows()[1], WindowControl::Minimize);

        assert_eq!(
            compositor.activate_control_at(position, Size::new(900.0, 700.0)),
            Some(WindowControl::Minimize)
        );

        assert!(compositor.windows()[1].minimized);
        let scene = compositor.scene(Size::new(900.0, 700.0));
        assert!(scene.roots.len() > 10);
    }

    #[test]
    fn set_window_minimized_by_id_updates_window_state() {
        let mut compositor = DesktopCompositor::sample();
        let id = compositor.windows()[1].id;

        assert!(compositor.set_window_minimized(id, true));
        assert!(compositor.windows().last().unwrap().minimized);

        assert!(compositor.set_window_minimized(id, false));
        assert!(!compositor.windows().last().unwrap().minimized);
        assert!(!compositor.set_window_minimized(WindowId(99), true));
    }

    #[test]
    fn controls_do_not_start_window_drag() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds.origin;
        let position = control_center(&compositor.windows()[1], WindowControl::FullSize);

        compositor.pointer_down(position);
        compositor.pointer_move(Point::new(position.x + 50.0, position.y + 50.0));
        compositor.pointer_up();

        assert_eq!(compositor.windows()[1].bounds.origin, initial);
    }

    #[test]
    fn taskbar_button_restores_minimized_window() {
        let mut compositor = DesktopCompositor::sample();
        let minimize_position = control_center(&compositor.windows()[1], WindowControl::Minimize);
        compositor.activate_control_at(minimize_position, Size::new(900.0, 700.0));

        let taskbar_position = taskbar_button_center(1, Size::new(900.0, 700.0));
        let restored = compositor.activate_taskbar_at(taskbar_position, Size::new(900.0, 700.0));

        assert_eq!(restored, Some(WindowId(2)));
        assert!(!compositor.windows().last().unwrap().minimized);
        assert_eq!(compositor.windows().last().unwrap().id, WindowId(2));
    }

    #[test]
    fn taskbar_button_raises_visible_window() {
        let mut compositor = DesktopCompositor::sample();
        let first = compositor.windows()[0].id;

        let taskbar_position = taskbar_button_center(0, Size::new(900.0, 700.0));
        let raised = compositor.activate_taskbar_at(taskbar_position, Size::new(900.0, 700.0));

        assert_eq!(raised, Some(first));
        assert_eq!(
            compositor.windows().last().map(|window| window.id),
            Some(first)
        );
    }

    #[test]
    fn taskbar_empty_space_does_not_activate_window() {
        let mut compositor = DesktopCompositor::sample();
        let position = Point::new(890.0, 680.0);

        assert!(compositor.taskbar_contains(position, Size::new(900.0, 700.0)));
        assert_eq!(
            compositor.activate_taskbar_at(position, Size::new(900.0, 700.0)),
            None
        );
        assert_eq!(
            compositor.windows().last().map(|window| window.id),
            Some(WindowId(2))
        );
    }

    #[test]
    fn launcher_filters_and_launches_selected_app() {
        let mut compositor = DesktopCompositor::sample();
        compositor.toggle_launcher();
        compositor.launcher_insert_text("term");

        let launched = compositor.launcher_launch_selected();

        assert_eq!(launched, Some(LauncherLaunch::Window(WindowId(3))));
        assert!(!compositor.launcher_is_open());
        assert_eq!(compositor.windows().last().unwrap().title, "Terminal");
    }

    #[test]
    fn launcher_can_request_external_app_launch() {
        let mut compositor = DesktopCompositor::sample();
        compositor.toggle_launcher();
        compositor.launcher_insert_text("telegram");

        let launched = compositor.launcher_launch_selected();

        assert_eq!(
            launched,
            Some(LauncherLaunch::External(ExternalApp::Telegram))
        );
        assert!(!compositor.launcher_is_open());
        assert_eq!(compositor.windows().len(), 2);
    }

    #[test]
    fn launcher_can_request_chrome_launch() {
        let mut compositor = DesktopCompositor::sample();
        compositor.toggle_launcher();
        compositor.launcher_insert_text("chrome");

        let launched = compositor.launcher_launch_selected();

        assert_eq!(
            launched,
            Some(LauncherLaunch::External(ExternalApp::Chrome))
        );
        assert!(!compositor.launcher_is_open());
        assert_eq!(compositor.windows().len(), 2);
    }

    #[test]
    fn launcher_arrow_selection_changes_launched_app() {
        let mut compositor = DesktopCompositor::sample();
        compositor.toggle_launcher();
        compositor.launcher_select_next();

        compositor.launcher_launch_selected();

        assert_eq!(compositor.windows().last().unwrap().title, "Editor");
    }

    #[test]
    fn launcher_backspace_updates_query() {
        let mut compositor = DesktopCompositor::sample();
        compositor.toggle_launcher();
        compositor.launcher_insert_text("calcx");
        compositor.launcher_backspace();

        compositor.launcher_launch_selected();

        assert_eq!(compositor.windows().last().unwrap().title, "Calculator");
    }

    #[test]
    fn launcher_blocks_window_dragging() {
        let mut compositor = DesktopCompositor::sample();
        let initial = compositor.windows()[1].bounds.origin;

        compositor.toggle_launcher();
        compositor.pointer_down(Point::new(initial.x + 20.0, initial.y + 15.0));
        compositor.pointer_move(Point::new(initial.x + 80.0, initial.y + 80.0));
        compositor.pointer_up();

        assert_eq!(compositor.windows()[1].bounds.origin, initial);
    }

    #[test]
    fn surface_hit_uses_surface_local_coordinates() {
        let mut compositor = DesktopCompositor::new();
        let surface = ResourceId(42);
        let window = compositor.add_surface_window("App", surface, Size::new(640.0, 420.0));
        let bounds = compositor.windows()[0].bounds;

        let hit = compositor.surface_hit_at(
            Point::new(bounds.origin.x + 20.0, bounds.origin.y + 50.0),
            Size::new(900.0, 700.0),
        );

        assert_eq!(
            hit,
            Some(SurfaceHit {
                window,
                surface,
                position: Point::new(20.0, 50.0),
            })
        );
        assert!(!compositor.windows()[0].is_decorated());
        assert_eq!(compositor.windows()[0].titlebar_height, 0.0);
    }

    #[test]
    fn surface_window_content_size_updates_surface_bounds_without_resizing_window() {
        let mut compositor = DesktopCompositor::new();
        let surface = ResourceId(42);
        compositor.add_surface_window("App", surface, Size::new(640.0, 420.0));

        assert!(compositor.set_surface_window_content_size(surface, Size::new(600.0, 390.0)));

        let window = &compositor.windows()[0];
        assert_eq!(window.bounds.size.width, 640.0);
        assert_eq!(window.bounds.size.height, 420.0);
        assert_eq!(
            window.local_surface_bounds(),
            Rect::from_xywh(0.0, 0.0, 600.0, 390.0)
        );
        assert_eq!(
            compositor.surface_window_requested_content_size(surface),
            Some(Size::new(640.0, 420.0))
        );
    }

    #[test]
    fn builtin_text_measurement_matches_renderer_advance() {
        assert_eq!(measure_builtin_text("ABC", 14.0), Size::new(36.0, 14.0));
        assert_eq!(measure_builtin_text("A C", 14.0), Size::new(32.0, 14.0));
        assert_eq!(measure_builtin_text("A\nBC", 14.0), Size::new(24.0, 32.0));
    }

    #[test]
    fn builtin_text_fitting_preserves_umlaut_characters_that_fit() {
        assert_eq!(fit_builtin_text_to_width("Näyttö", 14.0, 80.0), "Näyttö");
    }

    #[test]
    fn builtin_text_fitting_truncates_at_character_boundaries() {
        let fitted = fit_builtin_text_to_width("Björnin näyttöasetukset", 11.0, 110.0);

        assert!(fitted.ends_with("..."));
        assert!(fitted.is_char_boundary(fitted.len()));
        assert!(measure_builtin_text(&fitted, 11.0).width <= 110.0);
    }

    fn control_center(window: &Window, control: WindowControl) -> Point {
        let bounds = window
            .control_bounds()
            .into_iter()
            .find_map(|(candidate, bounds)| (candidate == control).then_some(bounds))
            .expect("window control bounds were missing");

        Point::new(
            bounds.origin.x + bounds.size.width / 2.0,
            bounds.origin.y + bounds.size.height / 2.0,
        )
    }

    fn taskbar_button_center(index: usize, desktop_size: Size) -> Point {
        let bounds = DesktopCompositor::taskbar_button_bounds(index, desktop_size);

        Point::new(
            bounds.origin.x + bounds.size.width / 2.0,
            bounds.origin.y + bounds.size.height / 2.0,
        )
    }
}
