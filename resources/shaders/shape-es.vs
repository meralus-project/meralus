#version 300 es

in vec3 position;
in vec2 uv;
in vec4 color;

out vec2 v_uv;
out vec4 v_color;

uniform mat4 matrix;

vec3 linearize(vec3 rgb) {
  return mix(pow((rgb + 0.055) * (1.0 / 1.055), vec3(2.4)), rgb * (1.0 / 12.92),
             lessThanEqual(rgb, vec3(0.04045)));
}

void main() {
  gl_Position = matrix * vec4(position, 1.0);

  vec4 f_color = color / 255.0;

  v_uv = uv;
  v_color = f_color;
}
