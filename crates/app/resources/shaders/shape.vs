#version 140

in vec3 position;
in vec4 color;
in mat4 transform;

out vec4 v_color;

uniform mat4 matrix;

vec3 linearize(vec3 rgb) {
  return mix(pow((rgb + 0.055) * (1.0 / 1.055), vec3(2.4)), rgb * (1.0 / 12.92),
             lessThanEqual(rgb, vec3(0.04045)));
}

void main() {
  gl_Position = matrix * transform * vec4(position, 1.0);

  vec4 f_color = color / 255.0;

  v_color = f_color;//  vec4(linearize(f_color.rgb), f_color.a);
}
