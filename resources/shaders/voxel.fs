#version 140

in highp vec2 v_tex_coords;
in float v_light_intensity;
in vec4 v_color;

// fog
in float v_spherical_dist;
in float v_cylindrical_dist;

out vec4 f_color;
out vec4 f_bright_color;

uniform sampler2D tex;
uniform sampler2D lightmap;
uniform bool with_tex;

uniform vec4 fog_color;
uniform float fog_env_start;
uniform float fog_env_end;
uniform float fog_render_dist_start;
uniform float fog_render_dist_end;

float linear_value(float dist, float start, float end) {
  if (dist <= start) { return 0.0; }
  else if (dist >= end) { return 1.0; }

  return (dist - start) / (end - start);
}

float total_fog_value(float spherical_dist, float cylindrical_dist) {
  return max(
    linear_value(spherical_dist, fog_env_start, fog_env_end),
    linear_value(cylindrical_dist, fog_render_dist_start, fog_render_dist_end)
  );
}

vec4 apply_fog(vec4 in_color, float spherical_dist, float cylindrical_dist) {
  return vec4(mix(
    in_color.rgb,
    fog_color.rgb,
    total_fog_value(spherical_dist, cylindrical_dist) * fog_color.a
  ), in_color.a);
}


void main() {
  if (with_tex) {
    vec4 lightmap_intensity = texture(lightmap, v_tex_coords) * vec4(0.21, 0.71, 0.07, 0.0);
    float gray = lightmap_intensity.r + lightmap_intensity.g + lightmap_intensity.b;
    float light_intensity = max(gray, v_light_intensity);
    vec4 color = texture(tex, v_tex_coords) * vec4(v_color.rgb * light_intensity, v_color.a);

    f_color = apply_fog(color, v_spherical_dist, v_cylindrical_dist);

    // float brightness = dot(f_color.rgb, vec3(0.2126, 0.7152, 0.0722));

    f_bright_color = vec4(f_color.rgb * gray, 1.0);
  } else{
    f_color = v_color;
    f_bright_color = vec4(0.0, 0.0, 0.0, 1.0);
  }
}
