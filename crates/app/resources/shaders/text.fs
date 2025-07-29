#version 140

in vec2 v_character;

out vec4 f_color;

uniform sampler2D font;
uniform vec4 text_color;

vec4 toLinear(vec4 sRGB) {
  bvec3 cutoff = lessThan(sRGB.rgb, vec3(0.04045));
  vec3 higher = pow((sRGB.rgb + vec3(0.055)) / vec3(1.055), vec3(2.4));
  vec3 lower = sRGB.rgb / vec3(12.92);

  return vec4(mix(higher, lower, cutoff), sRGB.a);
}

void main() { f_color = texture2D(font, v_character) * /* toLinear( */text_color/* ) */; }
