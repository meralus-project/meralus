#version 140

in vec2 v_uv;
in vec4 v_color;
flat in vec4 v_clip;

out vec4 f_color;
out vec4 f_bright_color;

uniform sampler2D atlas;
uniform vec2 resolution;

void main() {
    vec2 frag_coord = gl_FragCoord.xy / resolution;

    if (frag_coord.x < v_clip.x || frag_coord.y < v_clip.y || frag_coord.x > v_clip.z || frag_coord.y > v_clip.w) {
        discard;
    }
    
    f_color = v_color * texture(atlas, v_uv);
    f_bright_color = vec4(vec3(0.0), 1.0);
}
