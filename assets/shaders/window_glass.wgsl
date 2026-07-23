// Ombreval window panes: leaded glass that goes see-through up close.
// Extends the StandardMaterial fragment: within `fade.y` metres of the
// camera the pane's alpha slides down toward `fade.z`, so the room shell
// behind the glass reads; past `fade.y` it is the plain opaque pane and
// the hollow buildings are never on screen.

#import bevy_pbr::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
}

#ifdef PREPASS_PIPELINE
#import bevy_pbr::{
    prepass_io::{VertexOutput, FragmentOutput},
    pbr_deferred_functions::deferred_output,
}
#else
#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::{apply_pbr_lighting, main_pass_post_lighting_processing},
    mesh_view_bindings::view,
}
#endif

struct WindowGlassMaterial {
    // x: metres where the pane is at its clearest, y: metres where it is
    // fully opaque again, z: the point-blank alpha, w: padding.
    fade: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> window_glass: WindowGlassMaterial;

@fragment
fn fragment(
    in: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);

#ifdef PREPASS_PIPELINE
    // Depth-only views (the shadow maps) keep the pane fully opaque, so
    // glass still stops the sun from streaming straight through a building.
    let out = deferred_output(in, pbr_input);
#else
    let dist = distance(in.world_position.xyz, view.world_position);
    let opacity = mix(
        window_glass.fade.z,
        1.0,
        smoothstep(window_glass.fade.x, window_glass.fade.y, dist),
    );
    pbr_input.material.base_color.a = pbr_input.material.base_color.a * opacity;
    pbr_input.material.base_color =
        alpha_discard(pbr_input.material, pbr_input.material.base_color);

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
#endif

    return out;
}
