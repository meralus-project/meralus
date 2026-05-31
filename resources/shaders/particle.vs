#version 330 core

in vec3 position;
in vec3 world_position;
in vec3 color;

out vec3 v_position;
out vec3 v_color;

uniform mat4 matrix;

void main() {
    v_position = position;
    v_color = color;

    gl_Position = matrix * vec4(position + world_position, 1.0);
}