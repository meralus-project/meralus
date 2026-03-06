#version 330

out highp vec4 f_color;

uniform sampler2D texture;
uniform vec2 resolution;
uniform vec2 half_pixel;

const vec2 offset = vec2(1.0);

void main() {
    vec2 uv = vec2(gl_FragCoord.xy / resolution);
    vec4 sum = texture2D(texture, uv) * 4.0;
    vec2 h_p = half_pixel;
    vec2 h_p_neg_y = vec2(h_p.x, -h_p.y);

    sum += texture2D(texture, uv - h_p);
    sum += texture2D(texture, uv + h_p);
    sum += texture2D(texture, uv + h_p_neg_y);
    sum += texture2D(texture, uv - h_p_neg_y);

    f_color = sum / 8.0;
}
