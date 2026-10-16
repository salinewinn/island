//! Screen post-processing: the camcorder look and the horror effects (heartbeat pulse, closing
//! vignette, glitch tearing, chromatic fringing, the world swimming when your sanity is low).
//! The 3D scene is drawn into a render target, then that image is drawn to the screen through a
//! shader. It is only switched on when something needs it, so normal play keeps the window's
//! antialiasing.

use super::*;

const POST_VS: &str = "#version 100
attribute vec3 position; attribute vec2 texcoord; attribute vec4 color0;
varying lowp vec2 uv;
uniform mat4 Model; uniform mat4 Projection;
void main(){ gl_Position = Projection * Model * vec4(position, 1.0); uv = texcoord; }";

const POST_FS: &str = "#version 100
precision highp float;
varying lowp vec2 uv;
uniform sampler2D Texture;
uniform float Time; uniform float Cam; uniform float Dread; uniform float Calm; uniform float Grain;
uniform float Pulse; uniform float Blood; uniform float Glitch; uniform float Bright; uniform vec2 Res;
float h11(float p){ return fract(sin(p*127.1)*43758.5453); }
float h21(vec2 p){ return fract(sin(dot(p, vec2(12.9898,78.233)))*43758.5453); }
void main(){
  vec2 d = uv - 0.5;
  float r2 = dot(d,d);
  // the image swells a little with every heartbeat; the camcorder lens bends the edges
  vec2 q = 0.5 + d*(1.0 - Pulse*0.014*Dread);
  q += d*r2*0.05*Cam;
  q.x += sin(Time*1.7)*0.0007*Cam;
  // horizontal tearing
  float row = floor(uv.y*60.0);
  float bt = floor(Time*10.0);
  float g = step(1.0 - 0.06*Glitch, h11(row*0.37+bt)) * Glitch;
  q.x += (h11(row+bt*3.1)-0.5)*0.08*g;
  // the world swims when your mind goes
  float sw = 1.0 - Calm;
  q += vec2(sin(uv.y*18.0+Time*2.3), cos(uv.x*14.0+Time*1.9))*0.0016*sw*sw;
  // colour fringing grows with fear
  float ca = (0.0004 + 0.0035*Dread + 0.0022*Cam + 0.004*g)*(0.4 + 2.0*sqrt(r2));
  vec2 dir = normalize(d + vec2(0.0001));
  vec3 col;
  col.r = texture2D(Texture, q + dir*ca).r;
  col.g = texture2D(Texture, q).g;
  col.b = texture2D(Texture, q - dir*ca).b;
  float grey = dot(col, vec3(0.299,0.587,0.114));
  col = mix(col, vec3(grey), clamp(sw*0.7 + Dread*0.25, 0.0, 0.85));
  col = mix(col, col*vec3(1.05,1.0,0.80) + vec3(0.0,0.015,0.0), Cam*0.55);
  col = mix(col, (col-0.5)*1.12+0.5, Cam*0.4);
  col *= mix(vec3(1.0), vec3(1.12,0.88,0.88), Blood*0.6);
  col *= Bright;
  float scan = 0.5 + 0.5*sin(uv.y*Res.y*3.14159);
  col *= 1.0 - Cam*0.10*scan;
  float n = h21(uv*Res + fract(Time)*91.7) - 0.5;
  col += n*(0.02*Grain + 0.10*Cam*Grain + 0.06*Dread);
  // a vignette that closes in and throbs
  float vig = smoothstep(0.30, 0.95, sqrt(r2)*1.45);
  float vstr = 0.18 + Cam*0.25 + Dread*(0.45 + 0.4*Pulse) + sw*0.3;
  col *= 1.0 - clamp(vig*vstr, 0.0, 0.95);
  col += vec3(0.25,0.0,0.0)*vig*Dread*Pulse*0.35;
  // the camcorder's tracking line rolling up the picture
  float roll = fract(Time*0.13);
  col += smoothstep(0.012, 0.0, abs(uv.y-roll)) * Cam * 0.12;
  gl_FragColor = vec4(col, 1.0);
}";

/// What the shader needs each frame.
pub struct FxParams {
    pub time: f32,
    /// 0 or 1: the camcorder look
    pub cam: f32,
    /// 0..1 fear
    pub dread: f32,
    /// 1 = clear-headed, 0 = unhinged
    pub calm: f32,
    /// film grain strength (a setting)
    pub grain: f32,
    /// 0..1 heartbeat flash
    pub pulse: f32,
    pub blood: f32,
    /// 0..1 tearing / signal trouble
    pub glitch: f32,
    pub bright: f32,
}

pub struct PostFx {
    mat: Material,
    rt: Option<(RenderTarget, u32, u32)>,
}

impl PostFx {
    /// `None` when the shader can't be built: the game just plays without the effects.
    pub fn new() -> Option<PostFx> {
        let mat = load_material(
            ShaderSource::Glsl { vertex: POST_VS, fragment: POST_FS },
            MaterialParams {
                pipeline_params: PipelineParams::default(),
                uniforms: vec![
                    UniformDesc::new("Time", UniformType::Float1),
                    UniformDesc::new("Cam", UniformType::Float1),
                    UniformDesc::new("Dread", UniformType::Float1),
                    UniformDesc::new("Calm", UniformType::Float1),
                    UniformDesc::new("Grain", UniformType::Float1),
                    UniformDesc::new("Pulse", UniformType::Float1),
                    UniformDesc::new("Blood", UniformType::Float1),
                    UniformDesc::new("Glitch", UniformType::Float1),
                    UniformDesc::new("Bright", UniformType::Float1),
                    UniformDesc::new("Res", UniformType::Float2),
                ],
                textures: vec![],
            },
        )
        .ok()?;
        Some(PostFx { mat, rt: None })
    }

    /// The render target to draw the 3D scene into (recreated when the window changes size).
    pub fn target(&mut self) -> RenderTarget {
        let dpi = screen_dpi_scale();
        let (w, h) = (((screen_width() * dpi) as u32).max(16), ((screen_height() * dpi) as u32).max(16));
        let stale = match &self.rt {
            Some((_, rw, rh)) => *rw != w || *rh != h,
            None => true,
        };
        if stale {
            // the 3D scene needs a depth buffer; 4x multisampling keeps the edges as smooth as the window's
            let rt = render_target_ex(w, h, RenderTargetParams { sample_count: 4, depth: true });
            rt.texture.set_filter(FilterMode::Linear);
            self.rt = Some((rt, w, h));
        }
        self.rt.as_ref().map(|(r, _, _)| r.clone()).expect("render target")
    }

    /// Draw the captured scene to the screen through the effect. Call after `set_default_camera()`.
    pub fn present(&self, p: &FxParams) {
        let Some((rt, _, _)) = &self.rt else { return };
        let (sw, sh) = (screen_width(), screen_height());
        self.mat.set_uniform("Time", p.time);
        self.mat.set_uniform("Cam", p.cam);
        self.mat.set_uniform("Dread", p.dread);
        self.mat.set_uniform("Calm", p.calm);
        self.mat.set_uniform("Grain", p.grain);
        self.mat.set_uniform("Pulse", p.pulse);
        self.mat.set_uniform("Blood", p.blood);
        self.mat.set_uniform("Glitch", p.glitch);
        self.mat.set_uniform("Bright", p.bright);
        self.mat.set_uniform("Res", vec2(sw, sh));
        gl_use_material(&self.mat);
        draw_texture_ex(&rt.texture, 0.0, 0.0, WHITE, DrawTextureParams { dest_size: Some(vec2(sw, sh)), flip_y: true, ..Default::default() });
        gl_use_default_material();
    }
}
