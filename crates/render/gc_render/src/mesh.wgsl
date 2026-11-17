struct View {
    clip_from_relative: mat4x4<f32>,
    // Direction the light travels, and the direction the camera looks along.
    light: vec4<f32>,
    forward: vec4<f32>,
    // Width and height of the target in pixels.
    viewport: vec4<f32>,
};
@group(0) @binding(0) var<uniform> view: View;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) relative: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) position: vec3<f32>,
    @location(1) model_0: vec4<f32>,
    @location(2) model_1: vec4<f32>,
    @location(3) model_2: vec4<f32>,
    @location(4) model_3: vec4<f32>,
    @location(5) color: vec4<f32>,
) -> VertexOut {
    let model = mat4x4<f32>(model_0, model_1, model_2, model_3);
    let relative = model * vec4<f32>(position, 1.0);
    var out: VertexOut;
    out.clip = view.clip_from_relative * relative;
    out.relative = relative.xyz;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    // Face normal from the surface itself: no vertex normals are needed and flat faces stay flat.
    var normal = normalize(cross(dpdx(in.relative), dpdy(in.relative)));
    if (dot(normal, view.forward.xyz) > 0.0) {
        normal = -normal;
    }
    // A light from above and behind the viewer, sky and ground light from all around (the positions are
    // world-oriented, so +Z is up), and a soft highlight: the look of CAD programs.
    let to_light = -view.light.xyz;
    let to_eye = -view.forward.xyz;
    let sky = vec3<f32>(0.95, 0.97, 1.0);
    let ground = vec3<f32>(0.50, 0.48, 0.46);
    let ambient = mix(ground, sky, 0.5 + 0.5 * normal.z);
    let diffuse = max(dot(normal, to_light), 0.0);
    let halfway = normalize(to_light + to_eye);
    let highlight = pow(max(dot(normal, halfway), 0.0), 40.0) * 0.16;
    let rgb = in.color.rgb * (0.40 * ambient + 0.62 * diffuse) + vec3<f32>(highlight);
    return vec4<f32>(min(rgb, vec3<f32>(1.0)), in.color.a);
}

// Lines of constant width on screen, tested against the same depth as the solids.

struct LineOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_line(
    @builtin(vertex_index) corner: u32,
    @location(0) a: vec3<f32>,
    @location(1) b: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) width: f32,
) -> LineOut {
    let clip_a = view.clip_from_relative * vec4<f32>(a, 1.0);
    let clip_b = view.clip_from_relative * vec4<f32>(b, 1.0);
    let half_view = view.viewport.xy * 0.5;
    // The direction of the line on screen, in pixels.
    var along = clip_b.xy / clip_b.w * half_view - clip_a.xy / clip_a.w * half_view;
    let length_px = length(along);
    if (length_px < 1e-5) {
        along = vec2<f32>(1.0, 0.0);
    } else {
        along = along / length_px;
    }
    let across = vec2<f32>(-along.y, along.x);
    // Two triangles: corners 0, 1, 2 and 3, 4, 5 of the quad round the segment.
    var ends = array<f32, 6>(0.0, 1.0, 1.0, 0.0, 1.0, 0.0);
    var sides = array<f32, 6>(-1.0, -1.0, 1.0, -1.0, 1.0, 1.0);
    let centre = mix(clip_a, clip_b, ends[corner]);
    let offset = across * sides[corner] * width * 0.5 / half_view;
    var out: LineOut;
    // Pulled a little towards the eye, so that a line lying on a surface is not swallowed by it.
    out.clip = vec4<f32>(centre.xy + offset * centre.w, centre.z + 2e-4 * centre.w, centre.w);
    out.color = color;
    return out;
}

@fragment
fn fs_line(in: LineOut) -> @location(0) vec4<f32> {
    return in.color;
}

// Edges: one-pixel lines drawn by the hardware, a little towards the eye so that a surface does not hide
// its own edges.

struct EdgeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_edge(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> EdgeOut {
    let clip = view.clip_from_relative * vec4<f32>(position, 1.0);
    var out: EdgeOut;
    out.clip = vec4<f32>(clip.xy, clip.z + 3e-4 * clip.w, clip.w);
    out.color = color;
    return out;
}

@fragment
fn fs_edge(in: EdgeOut) -> @location(0) vec4<f32> {
    return in.color;
}
