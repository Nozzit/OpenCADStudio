use crate::scene::view::camera::Camera;
use iced::Rectangle;

/// Face highlight strength when the visual style sets none: a soft sheen
/// rather than a polished hotspot.
pub const DEFAULT_HIGHLIGHT: f32 = 0.3;

#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct Uniforms {
    pub viewport_size: [f32; 2],
    /// World units per screen pixel at the current zoom. Used by the
    /// hatch shader to substitute solid fill when pattern line spacing
    /// falls below ~2 px (Phase 3.3 LOD).
    pub world_per_pixel: f32,
    /// LWDISPLAY toggle (1.0 = show lineweights, 0.0 = force 1 px).
    /// Read by the wire shader so the toggle does not require a retessellate.
    pub lwdisplay_enable: f32,
    /// 1.0 → mesh fragment shader replaces the interpolated vertex
    /// normal with `normalize(cross(dpdx(pos), dpdy(pos)))` so each
    /// triangle gets a uniform shade (FlatShaded mode); 0.0 → keeps the
    /// per-vertex normal interpolation (GouraudShaded-style).
    pub flat_shade: f32,
    /// Transparency-display toggle (1.0 = honour entity transparency,
    /// 0.0 = force opaque). Read by the wire shader so the toggle does not
    /// require a retessellate.
    pub transparency_enable: f32,
    /// Per-viewport PSLTSCALE factor. Kept in the shared frame uniform so
    /// zooming inside MSPACE changes one scalar instead of re-tessellating and
    /// re-uploading every dashed wire.
    pub linetype_scale: f32,
    /// Positive values scale paper-space lineweights directly. Negative values
    /// carry the model-space preview scale; lineweight shaders then apply the
    /// model preview curve before multiplying by its magnitude.
    pub lineweight_scale: f32,

    // ── Relative-to-eye (double-single) additions ───────────────────────────
    // Appended at the end so existing field offsets are unchanged; shaders that
    // still read only the legacy fields keep working. Pipelines migrate to RTE
    // one at a time. `view_rot` is the rotation-only view-projection; vertices
    // pre-subtract the eye (via `eye_high`/`eye_low`, two f32 emulating f64) so
    // the large eye translation never enters the f32 matrix → no large-coord
    // jitter.
    pub view_rot: glam::Mat4,
    pub eye_high: [f32; 3],
    /// Extra screen pixels added to every line width. Only the selection
    /// overlay pass sees a non-zero value (`Pipeline::upload_uniforms`).
    pub highlight_widen: f32,
    pub eye_low: [f32; 3],
    /// Edge colour of the filled-with-edges modes, `0x01RRGGBB` in the
    /// bits (see [`edge_rgb_bits`]); any other value draws them black.
    pub edge_rgb: f32,
    /// Largest clip-space depth shift one unit of draw order may make
    /// (`draw_order.wgsl`). `f32::MAX` leaves draw order to its own bounds,
    /// the CAD convention of 2D Wireframe; the 3-D styles cap it at one
    /// pixel of depth ([`Uniforms::limit_draw_order_to_pixel`]).
    pub draw_order_limit: f32,
    pub _pad_draw_order: [f32; 3],

    // Up to four native AcDbLight/Sun sources. Positions are uploaded relative
    // to the current eye, preserving large-coordinate precision without
    // changing mesh buffers.
    /// xyz = eye-relative source position, w = light type (1 distant, 2 point,
    /// 3 spot).
    pub light_position_type: [[f32; 4]; 4],
    /// xyz = source-to-target direction, w = intensity.
    pub light_direction_intensity: [[f32; 4]; 4],
    /// rgb = source colour, w = cosine of hotspot angle.
    pub light_color_hotspot: [[f32; 4]; 4],
    /// x = attenuation mode, y/z = start/end limits, w = cosine of falloff.
    pub light_attenuation: [[f32; 4]; 4],
    /// x = active light count, yzw = per-viewport ambient-light RGB.
    pub lighting: [f32; 4],
    /// x = viewport face-colour mode, y = viewport face opacity,
    /// z = highlight intensity for faces without an attached material,
    /// w = visual-style brightness.
    pub visual_style: [f32; 4],
    /// Viewport visual-style mono/tint colour.
    pub visual_style_color: [f32; 4],
    /// RGB background selected by the viewport; alpha is one when a drawing
    /// background object, rather than the application canvas, supplies it.
    pub viewport_background: [f32; 4],
    /// x/y = normalized linear brightness/contrast in the documented
    /// -10..10 viewport range; z/w are reserved for exposure controls.
    pub view_tone: [f32; 4],
    pub background_top: [f32; 4],
    pub background_middle: [f32; 4],
    pub background_bottom: [f32; 4],
    pub background_aux0: [f32; 4],
    pub background_aux1: [f32; 4],
    /// x = mode, y = horizon, z = height, w = rotation in radians.
    pub background_params: [f32; 4],
    /// x = fit, y = keep aspect, z = tile, w = decoded image aspect.
    pub background_image_params: [f32; 4],
    /// xy = image offset, zw = image scale.
    pub background_image_transform: [f32; 4],
    /// x = environment enabled, y = rotation, z = diffuse strength,
    /// w = reflection strength.
    pub environment_params: [f32; 4],
    /// View-to-world rotation used to derive environment lookup directions.
    pub environment_view: glam::Mat4,
    /// Eye-relative light projection for the first shadow-casting source.
    pub shadow_view_proj: glam::Mat4,
    /// x = enabled, y = depth bias, z = softness, w = light index.
    pub shadow_params: [f32; 4],
    /// RGB = document render-environment fog colour.
    pub fog_color: [f32; 4],
    /// x = enabled, y = affect background, z/w = near/far density.
    pub fog_params: [f32; 4],
    /// x/y = near/far eye distance used to interpolate the density.
    pub fog_distances: [f32; 4],
    /// Eight normalized vertical-candela samples for each photometric light.
    pub light_web_profile_a: [[f32; 4]; 4],
    pub light_web_profile_b: [[f32; 4]; 4],
    /// xyz = web rotation, w = profile enabled.
    pub light_web_rotation: [[f32; 4]; 4],
}

impl Uniforms {
    pub fn new(camera: &Camera, bounds: Rectangle, lwdisplay_enable: bool) -> Self {
        let half_h = camera.ortho_size();
        let world_per_pixel = if bounds.height > 0.0 {
            (2.0 * half_h) / bounds.height
        } else {
            0.0
        };
        let (eye_high, eye_low) = camera.eye_high_low();
        Self {
            viewport_size: [bounds.width, bounds.height],
            world_per_pixel,
            lwdisplay_enable: if lwdisplay_enable { 1.0 } else { 0.0 },
            flat_shade: 0.0,
            transparency_enable: 1.0,
            linetype_scale: 1.0,
            lineweight_scale: 1.0,
            view_rot: camera.view_proj_rte(bounds),
            eye_high,
            highlight_widen: 0.0,
            eye_low,
            edge_rgb: 0.0,
            draw_order_limit: f32::MAX,
            _pad_draw_order: [0.0; 3],
            light_position_type: [[0.0; 4]; 4],
            light_direction_intensity: [[0.0; 4]; 4],
            light_color_hotspot: [[0.0; 4]; 4],
            light_attenuation: [[0.0; 4]; 4],
            lighting: [0.0, 0.18, 0.18, 0.18],
            visual_style: [1.0, 1.0, DEFAULT_HIGHLIGHT, 0.0],
            visual_style_color: [1.0, 1.0, 1.0, 1.0],
            viewport_background: [0.0, 0.0, 0.0, 0.0],
            view_tone: [0.0; 4],
            background_top: [0.0; 4],
            background_middle: [0.0; 4],
            background_bottom: [0.0; 4],
            background_aux0: [0.0; 4],
            background_aux1: [0.0; 4],
            background_params: [0.0; 4],
            background_image_params: [0.0; 4],
            background_image_transform: [0.0, 0.0, 1.0, 1.0],
            environment_params: [0.0, 0.0, 0.25, 0.35],
            environment_view: glam::Mat4::IDENTITY,
            shadow_view_proj: glam::Mat4::IDENTITY,
            shadow_params: [0.0, 0.0015, 1.0, 0.0],
            fog_color: [0.0, 0.0, 0.0, 1.0],
            fog_params: [0.0; 4],
            fog_distances: [0.0, 1.0, 0.0, 0.0],
            light_web_profile_a: [[1.0; 4]; 4],
            light_web_profile_b: [[1.0; 4]; 4],
            light_web_rotation: [[0.0; 4]; 4],
        }
    }
}

impl Uniforms {
    /// Caps draw order at one pixel of depth, for the 3-D styles. There the
    /// true depth decides what is in front, as the reference keeps draw order
    /// to 2D Wireframe; draw order only still breaks the tie between surfaces
    /// less than a pixel apart along the view, such as a hatch and its
    /// boundary in one plane. Uncapped, the shift is up to a fiftieth of the
    /// depth range: tens of metres in a survey-sized drawing, which brought a
    /// raster below the ground in front of a point cloud above it.
    pub fn limit_draw_order_to_pixel(&mut self, camera_distance: f32) {
        let m = self.view_rot;
        self.draw_order_limit = if m.w_axis.w.abs() < 0.5 {
            // Perspective: clip z = A·w + B, with the eye at the origin.
            // Sliding a vertex the fraction k of its way to the eye moves its
            // depth as a clip z shift of -B·k (to first order) at any depth,
            // and one pixel at a depth is the fraction world_per_pixel /
            // distance of it.
            m.w_axis.z.abs() * self.world_per_pixel / camera_distance.max(f32::MIN_POSITIVE)
        } else {
            // Parallel: clip z changes by the depth row's length per world
            // unit along the view, and w is one.
            glam::vec3(m.x_axis.z, m.y_axis.z, m.z_axis.z).length() * self.world_per_pixel
        };
    }
}

/// The `edge_rgb` uniform for an edge colour (`None`: black).
pub fn edge_rgb_bits(rgb: Option<[u8; 3]>) -> f32 {
    f32::from_bits(rgb.map_or(0, |[r, g, b]| {
        0x0100_0000 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::view::camera::Projection;
    use glam::{DMat4, DVec3, DVec4, Quat, Vec3};

    /// Clip position of a world point under the uniforms' relative-to-eye
    /// projection, in f64 so a one-pixel depth step stays measurable.
    fn clip(u: &Uniforms, camera: &Camera, point: DVec3) -> DVec4 {
        DMat4::from_cols_array(&u.view_rot.to_cols_array().map(f64::from))
            * (point - camera.eye()).extend(1.0)
    }

    /// A 2D Wireframe view keeps draw order as the CAD convention: the cap is
    /// open, so the shader's own bounds alone apply.
    #[test]
    fn draw_order_is_uncapped_until_a_3d_style_caps_it() {
        let u = Uniforms::new(
            &Camera::default(),
            Rectangle::with_size(iced::Size::new(800.0, 600.0)),
            false,
        );
        assert_eq!(u.draw_order_limit, f32::MAX);
    }

    /// In a 3-D style, one unit of draw order moves geometry no further than
    /// one pixel towards the eye. A raster a few metres below a point cloud
    /// stays behind it, while two surfaces in one plane still follow their
    /// draw order.
    #[test]
    fn a_3d_style_caps_draw_order_at_one_pixel_of_depth() {
        for projection in [Projection::Orthographic, Projection::Perspective] {
            let camera = Camera {
                target: DVec3::new(108_000.0, 447_000.0, -1.5),
                rotation: Quat::from_rotation_z(-0.8) * Quat::from_rotation_x(0.95),
                distance: 2_500.0,
                projection,
                model_bounds: Some((
                    DVec3::new(105_000.0, 444_000.0, -10.0),
                    DVec3::new(111_000.0, 450_000.0, 40.0),
                )),
                ..Camera::default()
            };
            let bounds = Rectangle::with_size(iced::Size::new(1600.0, 900.0));
            let mut u = Uniforms::new(&camera, bounds, false);
            u.limit_draw_order_to_pixel(camera.distance);

            let point = camera.target + DVec3::new(300.0, -200.0, 0.0);
            let eye = camera.eye();
            let pixel = f64::from(u.world_per_pixel);
            // One pixel towards the eye: along the view axis in parallel
            // projection, along the sight line (one pixel at that depth) in
            // perspective.
            let nearer = match projection {
                Projection::Orthographic => point + (camera.rotation * Vec3::Z).as_dvec3() * pixel,
                Projection::Perspective => {
                    point + (eye - point) * (pixel / f64::from(camera.distance))
                }
            };
            let at = clip(&u, &camera, point);
            let moved = clip(&u, &camera, nearer);
            let pixel_depth = at.z / at.w - moved.z / moved.w;
            let capped_depth = f64::from(u.draw_order_limit) / at.w;
            assert!(
                (capped_depth - pixel_depth).abs() < 0.02 * pixel_depth,
                "{projection:?}: the cap moves {capped_depth:e} of depth, \
                 one pixel is {pixel_depth:e}"
            );
        }
    }
}
