struct View {
    clip_from_relative: mat4x4<f32>,
    // Direction the light travels, and the direction the camera looks along.
    light: vec4<f32>,
    forward: vec4<f32>,
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
    let diffuse = max(dot(normal, -view.light.xyz), 0.0);
    let shade = 0.42 + 0.58 * diffuse;
    return vec4<f32>(in.color.rgb * shade, in.color.a);
}
