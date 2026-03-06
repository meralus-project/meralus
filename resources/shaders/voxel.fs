#version 140

in highp vec2 v_tex_coords;
in float v_light_intensity;
in vec4 v_color;

out vec4 f_color;
out vec4 f_bright_color;

uniform sampler2D tex;
uniform sampler2D lightmap;
uniform bool with_tex;

void main() {
  if (with_tex) {
    vec4 lightmap_intensity = texture2D(lightmap, v_tex_coords) * vec4(0.21, 0.71, 0.07, 0.0);
    float gray = lightmap_intensity.r + lightmap_intensity.g + lightmap_intensity.b;
    float light_intensity = max(gray, v_light_intensity);

    f_color = texture2D(tex, v_tex_coords) * vec4(v_color.rgb * light_intensity, v_color.a);

    // float brightness = dot(f_color.rgb, vec3(0.2126, 0.7152, 0.0722));

    f_bright_color = vec4(f_color.rgb * gray, 1.0);
  } else{
    f_color = v_color;
    f_bright_color = vec4(0.0, 0.0, 0.0, 1.0);
  }
}
