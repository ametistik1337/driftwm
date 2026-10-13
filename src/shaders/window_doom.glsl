//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;

#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform float u_progress;
uniform float u_for_opening;
uniform float u_duration;
uniform vec2 u_size;
uniform float u_horizontal_scale;
uniform float u_vertical_scale;
uniform float u_pixel_size;

// 2 out, 2 in hash (David Hoskins)
vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// 2D Simplex Noise (Inigo Quilez, MIT)
float simplex2D(vec2 p) {
    const float K1 = 0.366025404; // (sqrt(3)-1)/2
    const float K2 = 0.211324865; // (3-sqrt(3))/6

    vec2 i = floor(p + (p.x + p.y) * K1);
    vec2 a = p - i + (i.x + i.y) * K2;
    float m = step(a.y, a.x);
    vec2 o = vec2(m, 1.0 - m);
    vec2 b = a - o + K2;
    vec2 c = a - 1.0 + 2.0 * K2;
    vec3 h = max(0.5 - vec3(dot(a, a), dot(b, b), dot(c, c)), 0.0);
    vec3 n = h * h * h * h * vec3(
        dot(a, -1.0 + 2.0 * hash22(i + 0.0)),
        dot(b, -1.0 + 2.0 * hash22(i + o)),
        dot(c, -1.0 + 2.0 * hash22(i + 1.0))
    );
    return 0.5 + 0.5 * dot(n, vec3(70.0));
}

float simplex2DFractal(vec2 p) {
    mat2 m  = mat2(1.6, 1.2, -1.2, 1.6);
    float f = 0.5000 * simplex2D(p);
    p       = m * p;
    f += 0.2500 * simplex2D(p);
    p = m * p;
    f += 0.1250 * simplex2D(p);
    p = m * p;
    f += 0.0625 * simplex2D(p);
    return f;
}

float getEdgeMask(vec2 uv, float fadeWidth) {
    float mask = 1.0;
    mask *= smoothstep(0.0, 1.0, clamp(uv.y / fadeWidth, 0.0, 1.0));
    mask *= smoothstep(0.0, 1.0, clamp((1.0 - uv.y) / fadeWidth, 0.0, 1.0));
    return mask;
}

vec4 getInputColor(vec2 coords) {
    if (coords.x < 0.0 || coords.x > 1.0 || coords.y < 0.0 || coords.y > 1.0) {
        return vec4(0.0);
    }
    vec4 texColor = texture2D(tex, coords);
    #ifdef NO_ALPHA
    texColor = vec4(texColor.rgb, 1.0);
    #endif
    if (texColor.a > 0.0) {
        texColor.rgb /= texColor.a;
    }
    return texColor;
}

void main() {
    float pixelateProgress = u_for_opening > 0.5 ? 0.9 - u_progress : u_progress;
    float pixelSize        = max(1.0, ceil(u_pixel_size * pixelateProgress + 1.0));
    vec2 pixelGrid         = vec2(pixelSize) / u_size;

    vec2 texcoord = v_coords;
    texcoord -= mod(v_coords, pixelGrid);
    texcoord += pixelGrid * 0.5;

    float hScale = u_horizontal_scale * u_size.x * 0.001;
    float noise  = simplex2DFractal(texcoord.xx * hScale) * 2.0 - 0.5;

    float vScale        = u_vertical_scale * u_size.y * 0.00003;
    float shiftProgress = u_for_opening > 0.5 ? mix(-vScale - 1.0, 0.0, u_progress)
                                              : mix(-vScale, 1.0 + vScale, u_progress);
    float shift         = noise * vScale + shiftProgress;

    if (u_for_opening > 0.5) {
        texcoord.y -= 0.5 * min(shift, 0.0);
    } else {
        texcoord.y -= 0.5 * max(shift, 0.0);
    }

    vec4 oColor = getInputColor(texcoord);
    oColor.a *= getEdgeMask(v_coords, 0.05);
    oColor.a *= clamp((1.0 - u_progress) * 1.5, 0.0, 1.0);

    gl_FragColor = vec4(oColor.rgb * oColor.a * alpha, oColor.a * alpha);
}
