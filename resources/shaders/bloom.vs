#version 330 core

in vec3 position;
in vec2 uv;

out vec2 f_uv;

void main() {
    gl_Position = vec4(position, 1.0);
    
    f_uv = uv;
}