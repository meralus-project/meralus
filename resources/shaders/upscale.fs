#version 330

out highp vec4 f_color;

uniform sampler2D texture;
uniform vec2 resolution;
uniform vec2 half_pixel;

const vec2 offset = vec2(1.0);

void main() {
    vec2 uv = vec2(gl_FragCoord.xy / resolution);

    vec4 sum = texture2D(texture, uv + vec2(-half_pixel.x * 2.0, 0.0) * offset);

    sum += texture2D(texture, uv + vec2(-half_pixel.x, half_pixel.y) * offset) * 2.0;
    sum += texture2D(texture, uv + vec2(0.0, half_pixel.y * 2.0) * offset);
    sum += texture2D(texture, uv + vec2(half_pixel.x, half_pixel.y) * offset) * 2.0;
    sum += texture2D(texture, uv + vec2(half_pixel.x * 2.0, 0.0) * offset);
    sum += texture2D(texture, uv + vec2(half_pixel.x, -half_pixel.y) * offset) * 2.0;
    sum += texture2D(texture, uv + vec2(0.0, -half_pixel.y * 2.0) * offset);
    sum += texture2D(texture, uv + vec2(-half_pixel.x, -half_pixel.y) * offset) * 2.0;

    f_color = sum / 12.0;
}