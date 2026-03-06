#version 140

in vec3 position;
in uint light;
in highp vec2 uv;
in vec4 color;
in int visible;

out highp vec2 v_tex_coords;
out float v_light_intensity;
out vec4 v_color;

uniform mat4 matrix;
uniform vec3 sun_position;

vec4 toLinear(vec4 sRGB) {
    bvec3 cutoff = lessThan(sRGB.rgb, vec3(0.04045));
    vec3 higher = pow((sRGB.rgb + vec3(0.055)) / vec3(1.055), vec3(2.4));
    vec3 lower = sRGB.rgb / vec3(12.92);

    return vec4(mix(higher, lower, cutoff), sRGB.a);
}

void main() {
    if (visible == 1) {
        float block_light = (float(light & uint(15)) + 1.0) / 16.0;
        float sun_light = (float((light >> uint(4)) & uint(15)) + 1.0) / 16.0;
        float sunlight = sun_light * max(sun_position.y * 0.45 + 0.5, 0.02);
        float light_intensity = min(max(block_light, sunlight), 1.0);

        vec4 linear_color = toLinear(color / 255.0);

        gl_Position = matrix * vec4(position, 1.0);

        v_color = linear_color;
        v_light_intensity = light_intensity;
        v_tex_coords = uv;
    }
}
