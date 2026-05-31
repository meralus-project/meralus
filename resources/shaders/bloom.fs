#version 330 core

in vec2 f_uv;

out vec4 f_color;

uniform sampler2D scene;
uniform sampler2D bright;

const float gamma = 2.2;

void main() {
    vec3 hdrColor = texture(scene, f_uv).rgb;      
    vec3 bloomColor = texture(bright, f_uv).rgb;

    vec3 result = vec3(1.0) - ((vec3(1.0) - hdrColor) * (vec3(1.0) - bloomColor));

    f_color = vec4(pow(result, vec3(1.0 / gamma)), 1.0);
}
