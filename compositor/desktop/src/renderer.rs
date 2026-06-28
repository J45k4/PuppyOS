use std::{
    collections::HashMap,
    ffi::{CStr, c_void},
    os::fd::RawFd,
};

use anyhow::{Context, Result, bail};
use compositor_core::{
    Color, NodeId, NodeKind, Rect, ResourceId, Scene, Stroke, SurfaceRef, TextRun, Transform,
};
use glow::HasContext;

pub struct GlRenderer {
    gl: glow::Context,
    program: glow::NativeProgram,
    vao: glow::NativeVertexArray,
    vbo: glow::NativeBuffer,
    viewport_uniform: glow::NativeUniformLocation,
    texture_program: glow::NativeProgram,
    texture_vao: glow::NativeVertexArray,
    texture_vbo: glow::NativeBuffer,
    texture_viewport_uniform: glow::NativeUniformLocation,
    textures: HashMap<ResourceId, SurfaceTexture>,
    #[allow(dead_code)]
    dmabuf_importer: Option<EglDmabufImporter>,
}

struct SurfaceTexture {
    texture: glow::NativeTexture,
    width: u32,
    height: u32,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SurfacePixelFormat {
    Rgba,
    Bgra,
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct EglDmabufImporter {
    display: *const c_void,
    create_image: EglCreateImageKhr,
    destroy_image: EglDestroyImageKhr,
    image_target_texture_2d: GlEglImageTargetTexture2DOes,
}

#[allow(dead_code)]
impl EglDmabufImporter {
    pub unsafe fn load(
        display: *const c_void,
        mut load: impl FnMut(&CStr) -> *const c_void,
    ) -> Option<Self> {
        if display.is_null() {
            return None;
        }

        let create_image = load(c"eglCreateImageKHR");
        let destroy_image = load(c"eglDestroyImageKHR");
        let image_target_texture_2d = load(c"glEGLImageTargetTexture2DOES");
        if create_image.is_null() || destroy_image.is_null() || image_target_texture_2d.is_null() {
            return None;
        }

        Some(Self {
            display,
            create_image: unsafe {
                std::mem::transmute::<*const c_void, EglCreateImageKhr>(create_image)
            },
            destroy_image: unsafe {
                std::mem::transmute::<*const c_void, EglDestroyImageKhr>(destroy_image)
            },
            image_target_texture_2d: unsafe {
                std::mem::transmute::<*const c_void, GlEglImageTargetTexture2DOes>(
                    image_target_texture_2d,
                )
            },
        })
    }
}

#[allow(dead_code)]
pub struct DmabufImport {
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub planes: Vec<DmabufPlane>,
}

#[allow(dead_code)]
pub struct DmabufPlane {
    pub fd: RawFd,
    pub offset: u32,
    pub stride: u32,
    pub modifier: u64,
}

type EglImage = *const c_void;
type EglCreateImageKhr = unsafe extern "C" fn(
    display: *const c_void,
    context: *const c_void,
    target: u32,
    buffer: *mut c_void,
    attrib_list: *const i32,
) -> EglImage;
type EglDestroyImageKhr = unsafe extern "C" fn(display: *const c_void, image: EglImage) -> u32;
type GlEglImageTargetTexture2DOes = unsafe extern "C" fn(target: u32, image: EglImage);

impl GlRenderer {
    pub unsafe fn new(gl: glow::Context) -> Result<Self> {
        let program = unsafe { create_program(&gl, VERTEX_SHADER, FRAGMENT_SHADER)? };
        let vao = unsafe { gl.create_vertex_array().map_err(gl_error)? };
        let vbo = unsafe { gl.create_buffer().map_err(gl_error)? };

        unsafe {
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));

            let stride = (6 * std::mem::size_of::<f32>()) as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(
                1,
                4,
                glow::FLOAT,
                false,
                stride,
                (2 * std::mem::size_of::<f32>()) as i32,
            );

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }

        let viewport_uniform = unsafe {
            gl.get_uniform_location(program, "u_viewport")
                .context("shader is missing u_viewport uniform")?
        };

        let texture_program =
            unsafe { create_program(&gl, TEXTURE_VERTEX_SHADER, TEXTURE_FRAGMENT_SHADER)? };
        let texture_vao = unsafe { gl.create_vertex_array().map_err(gl_error)? };
        let texture_vbo = unsafe { gl.create_buffer().map_err(gl_error)? };

        unsafe {
            gl.bind_vertex_array(Some(texture_vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(texture_vbo));

            let stride = (5 * std::mem::size_of::<f32>()) as i32;
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, stride, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_f32(
                1,
                2,
                glow::FLOAT,
                false,
                stride,
                (2 * std::mem::size_of::<f32>()) as i32,
            );
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_f32(
                2,
                1,
                glow::FLOAT,
                false,
                stride,
                (4 * std::mem::size_of::<f32>()) as i32,
            );

            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
        }

        let texture_viewport_uniform = unsafe {
            gl.get_uniform_location(texture_program, "u_viewport")
                .context("texture shader is missing u_viewport uniform")?
        };

        Ok(Self {
            gl,
            program,
            vao,
            vbo,
            viewport_uniform,
            texture_program,
            texture_vao,
            texture_vbo,
            texture_viewport_uniform,
            textures: HashMap::new(),
            dmabuf_importer: None,
        })
    }

    #[allow(dead_code)]
    pub fn set_egl_dmabuf_importer(&mut self, importer: EglDmabufImporter) {
        self.dmabuf_importer = Some(importer);
    }

    #[allow(dead_code)]
    pub fn supports_dmabuf_import(&self) -> bool {
        self.dmabuf_importer.is_some()
    }

    #[allow(dead_code)]
    pub unsafe fn set_surface_pixels(
        &mut self,
        id: ResourceId,
        width: u32,
        height: u32,
        format: SurfacePixelFormat,
        rgba: &[u8],
    ) {
        if width == 0 || height == 0 || rgba.len() != width as usize * height as usize * 4 {
            return;
        }
        let gl_format = match format {
            SurfacePixelFormat::Rgba => glow::RGBA,
            SurfacePixelFormat::Bgra => glow::BGRA,
        };

        let texture = match self.textures.get(&id) {
            Some(surface) => surface.texture,
            None => {
                let texture = match unsafe { self.gl.create_texture() } {
                    Ok(texture) => texture,
                    Err(_) => return,
                };
                self.textures.insert(
                    id,
                    SurfaceTexture {
                        texture,
                        width: 0,
                        height: 0,
                    },
                );
                texture
            }
        };

        unsafe {
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            let needs_allocation = self
                .textures
                .get(&id)
                .is_none_or(|surface| surface.width != width || surface.height != height);
            if needs_allocation {
                self.gl.tex_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    glow::RGBA as i32,
                    width as i32,
                    height as i32,
                    0,
                    gl_format,
                    glow::UNSIGNED_BYTE,
                    Some(rgba),
                );
                if let Some(surface) = self.textures.get_mut(&id) {
                    surface.width = width;
                    surface.height = height;
                }
            } else {
                self.gl.tex_sub_image_2d(
                    glow::TEXTURE_2D,
                    0,
                    0,
                    0,
                    width as i32,
                    height as i32,
                    gl_format,
                    glow::UNSIGNED_BYTE,
                    glow::PixelUnpackData::Slice(rgba),
                );
            }
            self.gl.bind_texture(glow::TEXTURE_2D, None);
        }
    }

    #[allow(dead_code)]
    pub unsafe fn set_surface_dmabuf(&mut self, id: ResourceId, buffer: &DmabufImport) -> bool {
        if buffer.width == 0 || buffer.height == 0 || buffer.planes.is_empty() {
            return false;
        }

        let Some(importer) = self.dmabuf_importer else {
            return false;
        };

        let texture = match self.textures.get(&id) {
            Some(surface) => surface.texture,
            None => {
                let texture = match unsafe { self.gl.create_texture() } {
                    Ok(texture) => texture,
                    Err(_) => return false,
                };
                self.textures.insert(
                    id,
                    SurfaceTexture {
                        texture,
                        width: 0,
                        height: 0,
                    },
                );
                texture
            }
        };

        let attrs = match dmabuf_egl_attrs(buffer) {
            Some(attrs) => attrs,
            None => return false,
        };

        unsafe {
            let image = (importer.create_image)(
                importer.display,
                std::ptr::null(),
                EGL_LINUX_DMA_BUF_EXT,
                std::ptr::null_mut(),
                attrs.as_ptr(),
            );
            if image.is_null() {
                return false;
            }

            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            (importer.image_target_texture_2d)(glow::TEXTURE_2D, image);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            (importer.destroy_image)(importer.display, image);
            if let Some(surface) = self.textures.get_mut(&id) {
                surface.width = buffer.width;
                surface.height = buffer.height;
            }
        }

        true
    }

    pub unsafe fn render(&mut self, scene: &Scene, width: u32, height: u32) {
        let mut commands = Vec::new();
        let mut vertices = Vec::new();
        let transform = Affine2::identity();

        for root in &scene.roots {
            collect_node(scene, *root, transform, 1.0, &mut vertices, &mut commands);
        }
        flush_color_vertices(&mut vertices, &mut commands);

        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.clear_color(0.03, 0.035, 0.04, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.enable(glow::BLEND);
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);

            for command in &commands {
                match command {
                    DrawCommand::Color(vertices) => {
                        self.draw_color_vertices(vertices, width, height)
                    }
                    DrawCommand::Surface(quad) => self.draw_surface_quad(quad, width, height),
                }
            }
        }
    }

    unsafe fn draw_color_vertices(&self, vertices: &[f32], width: u32, height: u32) {
        if vertices.is_empty() {
            return;
        }

        unsafe {
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_2_f32(Some(&self.viewport_uniform), width as f32, height as f32);
            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));

            let bytes = std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(vertices),
            );
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            self.gl
                .draw_arrays(glow::TRIANGLES, 0, (vertices.len() / 6) as i32);

            self.gl.bind_vertex_array(None);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.use_program(None);
        }
    }

    unsafe fn draw_surface_quad(&self, quad: &SurfaceQuad, width: u32, height: u32) {
        let Some(surface) = self.textures.get(&quad.id) else {
            return;
        };

        let mut vertices = Vec::with_capacity(30);
        push_texture_quad(&mut vertices, quad);

        unsafe {
            self.gl.use_program(Some(self.texture_program));
            self.gl.uniform_2_f32(
                Some(&self.texture_viewport_uniform),
                width as f32,
                height as f32,
            );
            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(surface.texture));
            self.gl.bind_vertex_array(Some(self.texture_vao));
            self.gl
                .bind_buffer(glow::ARRAY_BUFFER, Some(self.texture_vbo));

            let bytes = std::slice::from_raw_parts(
                vertices.as_ptr().cast::<u8>(),
                std::mem::size_of_val(vertices.as_slice()),
            );
            self.gl
                .buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            self.gl.draw_arrays(glow::TRIANGLES, 0, 6);

            self.gl.bind_vertex_array(None);
            self.gl.bind_buffer(glow::ARRAY_BUFFER, None);
            self.gl.bind_texture(glow::TEXTURE_2D, None);
            self.gl.use_program(None);
        }
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.vbo);
            self.gl.delete_vertex_array(self.vao);
            self.gl.delete_program(self.program);
            self.gl.delete_buffer(self.texture_vbo);
            self.gl.delete_vertex_array(self.texture_vao);
            self.gl.delete_program(self.texture_program);
            for surface in self.textures.values() {
                self.gl.delete_texture(surface.texture);
            }
        }
    }
}

enum DrawCommand {
    Color(Vec<f32>),
    Surface(SurfaceQuad),
}

struct SurfaceQuad {
    id: ResourceId,
    corners: [(f32, f32); 4],
    opacity: f32,
}

fn collect_node(
    scene: &Scene,
    id: NodeId,
    parent_transform: Affine2,
    parent_opacity: f32,
    vertices: &mut Vec<f32>,
    commands: &mut Vec<DrawCommand>,
) {
    let Some(node) = scene.node(id) else {
        return;
    };

    if !node.visible {
        return;
    }

    let transform = parent_transform.mul(Affine2::from_transform(node.transform));
    let opacity = parent_opacity * node.opacity;

    match &node.kind {
        NodeKind::Group { children } => {
            for child in children {
                collect_node(scene, *child, transform, opacity, vertices, commands);
            }
        }
        NodeKind::Rect(style) => {
            if let Some(fill) = style.fill {
                push_rect(
                    vertices,
                    style.bounds,
                    fill.with_opacity(opacity),
                    transform,
                );
            }

            if let Some(stroke) = style.stroke {
                push_stroke(vertices, style.bounds, stroke, opacity, transform);
            }
        }
        NodeKind::Text(run) => push_text(vertices, run, opacity, transform),
        NodeKind::Image(_) => {}
        NodeKind::Surface(surface) => {
            flush_color_vertices(vertices, commands);
            commands.push(DrawCommand::Surface(surface_quad(
                surface, opacity, transform,
            )));
        }
    }
}

fn flush_color_vertices(vertices: &mut Vec<f32>, commands: &mut Vec<DrawCommand>) {
    if vertices.is_empty() {
        return;
    }

    commands.push(DrawCommand::Color(std::mem::take(vertices)));
}

fn surface_quad(surface: &SurfaceRef, opacity: f32, transform: Affine2) -> SurfaceQuad {
    let x = surface.bounds.origin.x;
    let y = surface.bounds.origin.y;
    let w = surface.bounds.size.width;
    let h = surface.bounds.size.height;

    SurfaceQuad {
        id: surface.id,
        corners: [
            transform.apply(x, y),
            transform.apply(x + w, y),
            transform.apply(x + w, y + h),
            transform.apply(x, y + h),
        ],
        opacity,
    }
}

fn push_texture_quad(vertices: &mut Vec<f32>, quad: &SurfaceQuad) {
    push_texture_vertex(vertices, quad.corners[0], 0.0, 0.0, quad.opacity);
    push_texture_vertex(vertices, quad.corners[1], 1.0, 0.0, quad.opacity);
    push_texture_vertex(vertices, quad.corners[2], 1.0, 1.0, quad.opacity);
    push_texture_vertex(vertices, quad.corners[0], 0.0, 0.0, quad.opacity);
    push_texture_vertex(vertices, quad.corners[2], 1.0, 1.0, quad.opacity);
    push_texture_vertex(vertices, quad.corners[3], 0.0, 1.0, quad.opacity);
}

fn push_texture_vertex(vertices: &mut Vec<f32>, point: (f32, f32), u: f32, v: f32, opacity: f32) {
    vertices.extend_from_slice(&[point.0, point.1, u, v, opacity]);
}

#[allow(dead_code)]
fn dmabuf_egl_attrs(buffer: &DmabufImport) -> Option<Vec<i32>> {
    if buffer.planes.len() > 4 {
        return None;
    }

    let mut attrs = vec![
        EGL_WIDTH,
        i32::try_from(buffer.width).ok()?,
        EGL_HEIGHT,
        i32::try_from(buffer.height).ok()?,
        EGL_LINUX_DRM_FOURCC_EXT,
        i32::from_ne_bytes(buffer.format.to_ne_bytes()),
    ];

    for (index, plane) in buffer.planes.iter().enumerate() {
        let [
            fd_attr,
            offset_attr,
            pitch_attr,
            modifier_lo_attr,
            modifier_hi_attr,
        ] = EGL_DMA_BUF_PLANE_ATTRS[index];
        attrs.extend_from_slice(&[
            fd_attr,
            plane.fd,
            offset_attr,
            i32::try_from(plane.offset).ok()?,
            pitch_attr,
            i32::try_from(plane.stride).ok()?,
            modifier_lo_attr,
            (plane.modifier & 0xffff_ffff) as i32,
            modifier_hi_attr,
            (plane.modifier >> 32) as i32,
        ]);
    }

    attrs.push(EGL_NONE);
    Some(attrs)
}

fn push_text(vertices: &mut Vec<f32>, run: &TextRun, opacity: f32, transform: Affine2) {
    let pixel = (run.size / 7.0).max(1.0);
    let color = run.color.with_opacity(opacity);
    let mut x = run.position.x;
    let mut y = run.position.y;

    for ch in run.text.chars() {
        if ch == '\n' {
            x = run.position.x;
            y += pixel * 9.0;
            continue;
        }

        if ch == ' ' {
            x += pixel * 4.0;
            continue;
        }

        if let Some(glyph) = glyph(ch) {
            for (row, line) in glyph.iter().enumerate() {
                for (col, cell) in line.as_bytes().iter().enumerate() {
                    if *cell == b'1' {
                        push_rect(
                            vertices,
                            Rect::from_xywh(
                                x + col as f32 * pixel,
                                y + row as f32 * pixel,
                                pixel,
                                pixel,
                            ),
                            color,
                            transform,
                        );
                    }
                }
            }
        }

        x += pixel * 6.0;
    }
}

fn push_stroke(
    vertices: &mut Vec<f32>,
    bounds: Rect,
    stroke: Stroke,
    opacity: f32,
    transform: Affine2,
) {
    let x = bounds.origin.x;
    let y = bounds.origin.y;
    let width = bounds.size.width;
    let height = bounds.size.height;
    let stroke_width = stroke.width;
    let color = stroke.color.with_opacity(opacity);

    push_rect(
        vertices,
        Rect::from_xywh(x, y, width, stroke_width),
        color,
        transform,
    );
    push_rect(
        vertices,
        Rect::from_xywh(x, y + height - stroke_width, width, stroke_width),
        color,
        transform,
    );
    push_rect(
        vertices,
        Rect::from_xywh(x, y, stroke_width, height),
        color,
        transform,
    );
    push_rect(
        vertices,
        Rect::from_xywh(x + width - stroke_width, y, stroke_width, height),
        color,
        transform,
    );
}

fn push_rect(vertices: &mut Vec<f32>, rect: Rect, color: Color, transform: Affine2) {
    let x = rect.origin.x;
    let y = rect.origin.y;
    let w = rect.size.width;
    let h = rect.size.height;

    let p0 = transform.apply(x, y);
    let p1 = transform.apply(x + w, y);
    let p2 = transform.apply(x + w, y + h);
    let p3 = transform.apply(x, y + h);

    push_vertex(vertices, p0, color);
    push_vertex(vertices, p1, color);
    push_vertex(vertices, p2, color);
    push_vertex(vertices, p0, color);
    push_vertex(vertices, p2, color);
    push_vertex(vertices, p3, color);
}

fn push_vertex(vertices: &mut Vec<f32>, point: (f32, f32), color: Color) {
    vertices.extend_from_slice(&[point.0, point.1, color.r, color.g, color.b, color.a]);
}

fn glyph(ch: char) -> Option<[&'static str; 7]> {
    let ch = glyph_char(ch);
    let glyph = match ch {
        'A' => [
            "01110", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        '\u{00c4}' => [
            "01010", "01110", "10001", "11111", "10001", "10001", "10001",
        ],
        '\u{00c5}' => [
            "00100", "01010", "01110", "10001", "11111", "10001", "10001",
        ],
        'B' => [
            "11110", "10001", "10001", "11110", "10001", "10001", "11110",
        ],
        'C' => [
            "01111", "10000", "10000", "10000", "10000", "10000", "01111",
        ],
        'D' => [
            "11110", "10001", "10001", "10001", "10001", "10001", "11110",
        ],
        'E' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "11111",
        ],
        'F' => [
            "11111", "10000", "10000", "11110", "10000", "10000", "10000",
        ],
        'G' => [
            "01111", "10000", "10000", "10111", "10001", "10001", "01111",
        ],
        'H' => [
            "10001", "10001", "10001", "11111", "10001", "10001", "10001",
        ],
        'I' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "11111",
        ],
        'J' => [
            "00111", "00010", "00010", "00010", "00010", "10010", "01100",
        ],
        'K' => [
            "10001", "10010", "10100", "11000", "10100", "10010", "10001",
        ],
        'L' => [
            "10000", "10000", "10000", "10000", "10000", "10000", "11111",
        ],
        'M' => [
            "10001", "11011", "10101", "10101", "10001", "10001", "10001",
        ],
        'N' => [
            "10001", "11001", "10101", "10011", "10001", "10001", "10001",
        ],
        'O' => [
            "01110", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        '\u{00d6}' => [
            "01010", "01110", "10001", "10001", "10001", "10001", "01110",
        ],
        'P' => [
            "11110", "10001", "10001", "11110", "10000", "10000", "10000",
        ],
        'Q' => [
            "01110", "10001", "10001", "10001", "10101", "10010", "01101",
        ],
        'R' => [
            "11110", "10001", "10001", "11110", "10100", "10010", "10001",
        ],
        'S' => [
            "01111", "10000", "10000", "01110", "00001", "00001", "11110",
        ],
        'T' => [
            "11111", "00100", "00100", "00100", "00100", "00100", "00100",
        ],
        'U' => [
            "10001", "10001", "10001", "10001", "10001", "10001", "01110",
        ],
        'V' => [
            "10001", "10001", "10001", "10001", "10001", "01010", "00100",
        ],
        'W' => [
            "10001", "10001", "10001", "10101", "10101", "10101", "01010",
        ],
        'X' => [
            "10001", "10001", "01010", "00100", "01010", "10001", "10001",
        ],
        'Y' => [
            "10001", "10001", "01010", "00100", "00100", "00100", "00100",
        ],
        'Z' => [
            "11111", "00001", "00010", "00100", "01000", "10000", "11111",
        ],
        '0' => [
            "01110", "10001", "10011", "10101", "11001", "10001", "01110",
        ],
        '1' => [
            "00100", "01100", "00100", "00100", "00100", "00100", "01110",
        ],
        '2' => [
            "01110", "10001", "00001", "00010", "00100", "01000", "11111",
        ],
        '3' => [
            "11110", "00001", "00001", "01110", "00001", "00001", "11110",
        ],
        '4' => [
            "00010", "00110", "01010", "10010", "11111", "00010", "00010",
        ],
        '5' => [
            "11111", "10000", "10000", "11110", "00001", "00001", "11110",
        ],
        '6' => [
            "01110", "10000", "10000", "11110", "10001", "10001", "01110",
        ],
        '7' => [
            "11111", "00001", "00010", "00100", "01000", "01000", "01000",
        ],
        '8' => [
            "01110", "10001", "10001", "01110", "10001", "10001", "01110",
        ],
        '9' => [
            "01110", "10001", "10001", "01111", "00001", "00001", "01110",
        ],
        '-' => [
            "00000", "00000", "00000", "11111", "00000", "00000", "00000",
        ],
        '_' => [
            "00000", "00000", "00000", "00000", "00000", "00000", "11111",
        ],
        ':' => [
            "00000", "00100", "00100", "00000", "00100", "00100", "00000",
        ],
        '/' => [
            "00001", "00001", "00010", "00100", "01000", "10000", "10000",
        ],
        '.' => [
            "00000", "00000", "00000", "00000", "00000", "01100", "01100",
        ],
        _ => return None,
    };
    Some(glyph)
}

fn glyph_char(ch: char) -> char {
    match ch {
        '\u{00e4}' | '\u{00c4}' => '\u{00c4}',
        '\u{00e5}' | '\u{00c5}' => '\u{00c5}',
        '\u{00f6}' | '\u{00d6}' => '\u{00d6}',
        _ => ch.to_ascii_uppercase(),
    }
}

#[derive(Clone, Copy)]
struct Affine2 {
    m11: f32,
    m12: f32,
    m21: f32,
    m22: f32,
    tx: f32,
    ty: f32,
}

impl Affine2 {
    const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    fn from_transform(transform: Transform) -> Self {
        let cos = transform.rotation.cos();
        let sin = transform.rotation.sin();

        Self {
            m11: cos * transform.scale.x,
            m12: sin * transform.scale.x,
            m21: -sin * transform.scale.y,
            m22: cos * transform.scale.y,
            tx: transform.translation.x,
            ty: transform.translation.y,
        }
    }

    fn mul(self, rhs: Self) -> Self {
        Self {
            m11: self.m11 * rhs.m11 + self.m21 * rhs.m12,
            m12: self.m12 * rhs.m11 + self.m22 * rhs.m12,
            m21: self.m11 * rhs.m21 + self.m21 * rhs.m22,
            m22: self.m12 * rhs.m21 + self.m22 * rhs.m22,
            tx: self.m11 * rhs.tx + self.m21 * rhs.ty + self.tx,
            ty: self.m12 * rhs.tx + self.m22 * rhs.ty + self.ty,
        }
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.m11 * x + self.m21 * y + self.tx,
            self.m12 * x + self.m22 * y + self.ty,
        )
    }
}

trait ColorExt {
    fn with_opacity(self, opacity: f32) -> Self;
}

impl ColorExt for Color {
    fn with_opacity(self, opacity: f32) -> Self {
        Self {
            a: self.a * opacity,
            ..self
        }
    }
}

unsafe fn create_program(
    gl: &glow::Context,
    vertex_source: &str,
    fragment_source: &str,
) -> Result<glow::NativeProgram> {
    let program = unsafe { gl.create_program().map_err(gl_error)? };
    let vertex_shader = unsafe { compile_shader(gl, glow::VERTEX_SHADER, vertex_source)? };
    let fragment_shader = unsafe { compile_shader(gl, glow::FRAGMENT_SHADER, fragment_source)? };

    unsafe {
        gl.attach_shader(program, vertex_shader);
        gl.attach_shader(program, fragment_shader);
        gl.link_program(program);
    }

    if unsafe { !gl.get_program_link_status(program) } {
        let log = unsafe { gl.get_program_info_log(program) };
        unsafe {
            gl.delete_shader(vertex_shader);
            gl.delete_shader(fragment_shader);
            gl.delete_program(program);
        }
        bail!("program link failed: {log}");
    }

    unsafe {
        gl.delete_shader(vertex_shader);
        gl.delete_shader(fragment_shader);
    }

    Ok(program)
}

unsafe fn compile_shader(
    gl: &glow::Context,
    shader_type: u32,
    source: &str,
) -> Result<glow::NativeShader> {
    let shader = unsafe { gl.create_shader(shader_type).map_err(gl_error)? };

    unsafe {
        gl.shader_source(shader, source);
        gl.compile_shader(shader);
    }

    if unsafe { !gl.get_shader_compile_status(shader) } {
        let log = unsafe { gl.get_shader_info_log(shader) };
        unsafe {
            gl.delete_shader(shader);
        }
        bail!("shader compile failed: {log}");
    }

    Ok(shader)
}

fn gl_error(error: String) -> anyhow::Error {
    anyhow::anyhow!(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_lookup_supports_scandinavian_letters() {
        assert_eq!(glyph_char('\u{00e4}'), '\u{00c4}');
        assert_eq!(glyph_char('\u{00f6}'), '\u{00d6}');
        assert_eq!(glyph_char('\u{00e5}'), '\u{00c5}');
        assert!(glyph('\u{00e4}').is_some());
        assert!(glyph('\u{00f6}').is_some());
        assert!(glyph('\u{00e5}').is_some());
    }
}

const VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec2 a_position;
layout (location = 1) in vec4 a_color;

uniform vec2 u_viewport;

out vec4 v_color;

void main() {
    vec2 zero_to_one = a_position / u_viewport;
    vec2 clip = zero_to_one * 2.0 - 1.0;
    gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
    v_color = a_color;
}
"#;

const FRAGMENT_SHADER: &str = r#"#version 330 core
in vec4 v_color;

out vec4 frag_color;

void main() {
    frag_color = v_color;
}
"#;

#[allow(dead_code)]
const EGL_NONE: i32 = 0x3038;
#[allow(dead_code)]
const EGL_WIDTH: i32 = 0x3057;
#[allow(dead_code)]
const EGL_HEIGHT: i32 = 0x3056;
#[allow(dead_code)]
const EGL_LINUX_DMA_BUF_EXT: u32 = 0x3270;
#[allow(dead_code)]
const EGL_LINUX_DRM_FOURCC_EXT: i32 = 0x3271;
#[allow(dead_code)]
const EGL_DMA_BUF_PLANE_ATTRS: [[i32; 5]; 4] = [
    [0x3272, 0x3273, 0x3274, 0x3443, 0x3444],
    [0x3275, 0x3276, 0x3277, 0x3445, 0x3446],
    [0x3278, 0x3279, 0x327a, 0x3447, 0x3448],
    [0x3440, 0x3441, 0x3442, 0x3449, 0x344a],
];

const TEXTURE_VERTEX_SHADER: &str = r#"#version 330 core
layout (location = 0) in vec2 a_position;
layout (location = 1) in vec2 a_tex_coord;
layout (location = 2) in float a_opacity;

uniform vec2 u_viewport;

out vec2 v_tex_coord;
out float v_opacity;

void main() {
    vec2 zero_to_one = a_position / u_viewport;
    vec2 clip = zero_to_one * 2.0 - 1.0;
    gl_Position = vec4(clip.x, -clip.y, 0.0, 1.0);
    v_tex_coord = a_tex_coord;
    v_opacity = a_opacity;
}
"#;

const TEXTURE_FRAGMENT_SHADER: &str = r#"#version 330 core
in vec2 v_tex_coord;
in float v_opacity;

uniform sampler2D u_texture;

out vec4 frag_color;

void main() {
    vec4 color = texture(u_texture, v_tex_coord);
    frag_color = vec4(color.rgb, color.a * v_opacity);
}
"#;
