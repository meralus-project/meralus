#version 330 core

in vec3 v_color;

out vec4 f_bright_color;
out vec4 f_color;

void main() {
    f_color = vec4(v_color, 1.0);
    f_bright_color = vec4(vec3(0.0), 1.0);
}