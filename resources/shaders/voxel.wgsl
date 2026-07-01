struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<u32>,
    @location(3) light: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) light_intensity: f32,
    @location(2) @interpolate(flat) color: vec4<f32>,
    @location(3) spherical_dist: f32,
    @location(4) cylindrical_dist: f32,
};

fn fog_spherical_distance(pos: vec3<f32>) -> f32 { return length(pos); }
fn fog_cylindrical_distance(pos: vec3<f32>) -> f32 { return max(length(pos.xz), abs(pos.y)); }

struct VoxelUniform {
    sun_position: vec3<f32>
}

@group(0) @binding(0)
var<uniform> voxel: VoxelUniform;

struct VoxelImm {
    matrix: mat4x4<f32>,
    chunk_offset: vec3<f32>,
}

var<immediate> voxel_imm: VoxelImm;

const COLOR_TO_LINEAR = array(
    0.00000000,
    0.00030353,
    0.00060705,
    0.00091058,
    0.00121411,
    0.00151763,
    0.00182116,
    0.00212469,
    0.00242822,
    0.00273174,
    0.00303527,
    0.00334654,
    0.00367651,
    0.00402472,
    0.00439144,
    0.00477695,
    0.00518152,
    0.00560539,
    0.00604883,
    0.00651209,
    0.00699541,
    0.00749903,
    0.00802319,
    0.00856813,
    0.00913406,
    0.00972122,
    0.01032982,
    0.01096009,
    0.01161225,
    0.01228649,
    0.01298303,
    0.01370208,
    0.01444384,
    0.01520851,
    0.01599629,
    0.01680738,
    0.01764195,
    0.01850022,
    0.01938236,
    0.02028856,
    0.02121901,
    0.02217388,
    0.02315337,
    0.02415763,
    0.02518686,
    0.02624122,
    0.02732089,
    0.02842604,
    0.02955683,
    0.03071344,
    0.03189603,
    0.03310477,
    0.03433981,
    0.03560131,
    0.03688945,
    0.03820437,
    0.03954624,
    0.0409152,
    0.04231141,
    0.04373503,
    0.0451862,
    0.04666509,
    0.04817182,
    0.04970657,
    0.05126946,
    0.05286065,
    0.05448028,
    0.05612849,
    0.05780543,
    0.05951124,
    0.06124605,
    0.06301002,
    0.06480327,
    0.06662594,
    0.06847817,
    0.0703601,
    0.07227185,
    0.07421357,
    0.07618538,
    0.07818742,
    0.08021982,
    0.08228271,
    0.08437621,
    0.08650046,
    0.08865559,
    0.09084171,
    0.09305896,
    0.09530747,
    0.09758735,
    0.09989873,
    0.10224173,
    0.10461648,
    0.1070231,
    0.10946171,
    0.11193243,
    0.11443537,
    0.11697067,
    0.11953843,
    0.12213877,
    0.12477182,
    0.12743768,
    0.13013648,
    0.13286832,
    0.13563333,
    0.13843162,
    0.14126329,
    0.14412847,
    0.14702727,
    0.14995979,
    0.15292615,
    0.15592646,
    0.15896084,
    0.16202938,
    0.1651322,
    0.1682694,
    0.1714411,
    0.1746474,
    0.17788842,
    0.18116424,
    0.18447499,
    0.18782077,
    0.19120168,
    0.19461783,
    0.19806932,
    0.20155625,
    0.20507874,
    0.20863687,
    0.21223076,
    0.2158605,
    0.2195262,
    0.22322796,
    0.22696587,
    0.23074005,
    0.23455058,
    0.23839757,
    0.24228112,
    0.24620133,
    0.25015828,
    0.2541521,
    0.25818285,
    0.26225066,
    0.2663556,
    0.2704978,
    0.2746773,
    0.27889426,
    0.28314874,
    0.28744084,
    0.29177065,
    0.29613827,
    0.30054379,
    0.3049873,
    0.30946892,
    0.31398871,
    0.31854678,
    0.3231432,
    0.3277781,
    0.33245154,
    0.33716362,
    0.34191442,
    0.34670406,
    0.3515326,
    0.35640014,
    0.3613068,
    0.3662526,
    0.37123768,
    0.37626212,
    0.38132601,
    0.38642943,
    0.39157248,
    0.39675523,
    0.40197778,
    0.4072402,
    0.4125426,
    0.41788507,
    0.42326767,
    0.4286905,
    0.43415364,
    0.43965717,
    0.4452012,
    0.4507858,
    0.45641102,
    0.462077,
    0.4677838,
    0.4735315,
    0.47932018,
    0.48514994,
    0.49102085,
    0.496933,
    0.5028865,
    0.50888132,
    0.5149177,
    0.52099557,
    0.5271151,
    0.5332764,
    0.5394795,
    0.54572446,
    0.5520114,
    0.5583404,
    0.5647115,
    0.57112483,
    0.57758044,
    0.58407842,
    0.59061884,
    0.59720179,
    0.60382734,
    0.61049557,
    0.6172066,
    0.6239604,
    0.63075714,
    0.63759687,
    0.6444797,
    0.65140564,
    0.65837482,
    0.6653873,
    0.67244316,
    0.6795425,
    0.6866853,
    0.69387176,
    0.7011019,
    0.70837578,
    0.7156935,
    0.7230551,
    0.73046074,
    0.7379104,
    0.7454042,
    0.7529422,
    0.7605245,
    0.76815115,
    0.7758222,
    0.7835378,
    0.7912979,
    0.7991027,
    0.80695226,
    0.8148466,
    0.82278575,
    0.8307699,
    0.838799,
    0.8468732,
    0.8549926,
    0.8631572,
    0.8713671,
    0.8796224,
    0.8879231,
    0.8962693,
    0.9046612,
    0.91309865,
    0.92158186,
    0.9301109,
    0.9386857,
    0.9473065,
    0.9559733,
    0.9646863,
    0.9734453,
    0.9822506,
    0.9911021,
    1.00000000,
);

@vertex
fn vs_main(
    in: VertexInput
) -> VertexOutput {
    var out: VertexOutput;

    let block_light = (f32(in.light & u32(15)) + 1.0) / 16.0;
    var sun_light = (f32((in.light >> u32(4)) & u32(15)) + 1.0) / 16.0;

    sun_light *= max(voxel.sun_position.y * 0.45 + 0.5, 0.02);

    let light_intensity = min(max(block_light, sun_light), 1.0);

    let linear_color = vec4(COLOR_TO_LINEAR[in.color.r], COLOR_TO_LINEAR[in.color.g], COLOR_TO_LINEAR[in.color.b], COLOR_TO_LINEAR[in.color.a]);
    let pos = voxel_imm.chunk_offset + in.position;

    out.position = voxel_imm.matrix * vec4(pos, 1.0);
    out.spherical_dist = fog_spherical_distance(out.position.xyz);
    out.cylindrical_dist = fog_cylindrical_distance(out.position.xyz);
    out.color = linear_color;
    out.light_intensity = light_intensity;
    out.uv = in.uv;

    return out;
}

@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var lightmap: texture_2d<f32>;
@group(1) @binding(2) var base_sampler: sampler;

struct FogUniform {
    fog_color: vec4<f32>,
    fog_env_start: f32,
    fog_env_end: f32,
    fog_render_dist_start: f32,
    fog_render_dist_end: f32,
    with_fog: u32,
}

@group(2) @binding(0) var<uniform> fog: FogUniform;

fn linear_value(dist: f32, start: f32, end: f32) -> f32 {
    if dist <= start { return 0.0; }
  else if dist >= end { return 1.0; }

    return (dist - start) / (end - start);
}

fn total_fog_value(spherical_dist: f32, cylindrical_dist: f32) -> f32 {
    return max(
        linear_value(spherical_dist, fog.fog_env_start, fog.fog_env_end),
        linear_value(cylindrical_dist, fog.fog_render_dist_start, fog.fog_render_dist_end)
    );
}

fn apply_fog(in_color: vec4<f32>, spherical_dist: f32, cylindrical_dist: f32) -> vec4<f32> {
    return vec4(mix(
        in_color.rgb,
        fog.fog_color.rgb,
        total_fog_value(spherical_dist, cylindrical_dist) * fog.fog_color.a
    ), in_color.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    //   if (with_tex) {
    let lightmap_intensity = textureSample(lightmap, base_sampler, in.uv) * vec4(0.21, 0.71, 0.07, 0.0);
    let gray = lightmap_intensity.r + lightmap_intensity.g + lightmap_intensity.b;
    let light_intensity = max(gray, in.light_intensity);

    let sampled_raw = textureSample(tex, base_sampler, in.uv);
    let linear_rgb = pow(sampled_raw.rgb, vec3<f32>(2.2));
    let tex_color = vec4<f32>(linear_rgb, sampled_raw.a);
    var f_color = sampled_raw * vec4(in.color.rgb * light_intensity, in.color.a);

    if fog.with_fog == 1 {
        f_color = apply_fog(
            f_color,
            in.spherical_dist,
            in.cylindrical_dist,
        );
    }

    return f_color;

    // float brightness = dot(f_color.rgb, vec3(0.2126, 0.7152, 0.0722));

    // f_bright_color = vec4(f_color.rgb * gray, 1.0);
    //   } else{
    // f_color = v_color;
    // f_bright_color = vec4(0.0, 0.0, 0.0, 1.0);
    //   }
}
