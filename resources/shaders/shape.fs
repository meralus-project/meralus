#version 140

in vec2 v_uv;
in vec4 v_color;

out vec4 f_color;
out vec4 f_bright_color;

uniform sampler2D atlas;

void main() {
    f_color = v_color * texture2D(atlas, v_uv);
    f_bright_color = vec4(vec3(0.0), 1.0);
}
