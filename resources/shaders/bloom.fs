#version 330 core

in vec2 f_uv;

out vec4 f_color;

uniform sampler2D scene;
uniform sampler2D bright;

const float gamma = 1.4;

void main() {
    vec3 hdrColor = texture(scene, f_uv).rgb;      
    vec3 bloomColor = texture(bright, f_uv).rgb;

    vec3 result = vec3(1.0) - ((vec3(1.0) - hdrColor) * (vec3(1.0) - bloomColor));
    // vec3 result = vec3(1.0) - exp(-hdrColor);

    // result = pow(result, vec3(1.0 / gamma));
    
    f_color = vec4(result, 1.0);
}
