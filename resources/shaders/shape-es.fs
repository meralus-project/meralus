#version 300 es

in highp vec2 v_uv;
in highp vec4 v_color;

out highp vec4 f_color;

uniform sampler2D atlas;

void main() {
    f_color = v_color * texture(atlas, v_uv);
}
