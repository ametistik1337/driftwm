//_DEFINES_
precision highp float;
varying vec2 v_coords;
uniform sampler2D tex;
uniform float alpha;
#if defined(DEBUG_FLAGS)
uniform float tint;
#endif
void main() {
    float tex_a = texture2D(tex, v_coords).a;
    float a = smoothstep(0.08, 0.75, tex_a);
    gl_FragColor = vec4(a) * alpha;
}
