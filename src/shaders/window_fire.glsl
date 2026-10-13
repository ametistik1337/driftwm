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
uniform float u_scale;
uniform float u_movement_speed;
uniform vec4 u_gradient1;
uniform vec4 u_gradient2;
uniform vec4 u_gradient3;
uniform vec4 u_gradient4;
uniform vec4 u_gradient5;
uniform float u_closing_bottom_to_top;

// 2 out, 2 in hash (David Hoskins)
vec2 hash22(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// 3 out, 3 in hash
vec3 hash33(vec3 p3) {
    p3 = fract(p3 * vec3(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// 3D Simplex Noise (Nikita Miropolskiy, MIT)
float simplex3D(vec3 p) {
    const float F3 = 0.3333333;
    const float G3 = 0.1666667;

    vec3 s = floor(p + dot(p, vec3(F3)));
    vec3 x = p - s + dot(s, vec3(G3));

    vec3 e  = step(vec3(0.0), x - x.yzx);
    vec3 i1 = e * (1.0 - e.zxy);
    vec3 i2 = 1.0 - e.zxy * (1.0 - e);

    vec3 x1 = x - i1 + G3;
    vec3 x2 = x - i2 + 2.0 * G3;
    vec3 x3 = x - 1.0 + 3.0 * G3;

    vec4 w;
    w.x = dot(x, x);
    w.y = dot(x1, x1);
    w.z = dot(x2, x2);
    w.w = dot(x3, x3);

    w = max(0.6 - w, 0.0);

    vec4 d;
    d.x = dot(-0.5 + hash33(s), x);
    d.y = dot(-0.5 + hash33(s + i1), x1);
    d.z = dot(-0.5 + hash33(s + i2), x2);
    d.w = dot(-0.5 + hash33(s + 1.0), x3);

    w *= w;
    w *= w;
    d *= w;

    return dot(d, vec4(52.0)) * 0.5 + 0.5;
}

// Directional artifacts reduction by rotating octaves
float simplex3DFractal(vec3 m) {
    const mat3 rot1 = mat3(-0.37, 0.36, 0.85, -0.14, -0.93, 0.34, 0.92, 0.01, 0.4);
    const mat3 rot2 = mat3(-0.55, -0.39, 0.74, 0.33, -0.91, -0.24, 0.77, 0.12, 0.63);
    const mat3 rot3 = mat3(-0.71, 0.52, -0.47, -0.08, -0.72, -0.68, -0.7, -0.45, 0.56);

    return 0.5333333 * simplex3D(m * rot1) + 0.2666667 * simplex3D(2.0 * m * rot2) +
           0.1333333 * simplex3D(4.0 * m * rot3) + 0.0666667 * simplex3D(8.0 * m);
}

float easeOutQuad(float x) {
    return -1.0 * x * (x - 2.0);
}

vec4 alphaOver(vec4 under, vec4 over) {
    if (under.a == 0.0 && over.a == 0.0) {
        return vec4(0.0);
    }
    float a = mix(under.a, 1.0, over.a);
    return vec4(mix(under.rgb * under.a, over.rgb, over.a) / max(a, 0.0001), a);
}

vec4 getFireColor(float v, vec4 c0, vec4 c1, vec4 c2, vec4 c3, vec4 c4) {
    float steps[5];
    steps[0] = 0.0;
    steps[1] = 0.2;
    steps[2] = 0.35;
    steps[3] = 0.5;
    steps[4] = 0.8;

    vec4 colors[5];
    colors[0] = c0;
    colors[1] = c1;
    colors[2] = c2;
    colors[3] = c3;
    colors[4] = c4;

    if (v < steps[0]) {
        return colors[0];
    }
    for (int i = 0; i < 4; ++i) {
        if (v <= steps[i + 1]) {
            return mix(colors[i], colors[i + 1],
                       vec4(v - steps[i]) / (steps[i + 1] - steps[i]));
        }
    }
    return colors[4];
}

float getEdgeMask(vec2 uv, vec2 maxUV, float fadeWidth) {
    float mask = 1.0;
    mask *= smoothstep(0.0, 1.0, clamp(uv.x / fadeWidth, 0.0, 1.0));
    mask *= smoothstep(0.0, 1.0, clamp(uv.y / fadeWidth, 0.0, 1.0));
    mask *= smoothstep(0.0, 1.0, clamp((maxUV.x - uv.x) / fadeWidth, 0.0, 1.0));
    mask *= smoothstep(0.0, 1.0, clamp((maxUV.y - uv.y) / fadeWidth, 0.0, 1.0));
    return mask;
}

float getAbsoluteEdgeMask(float fadePixels) {
    vec2 uv = v_coords * u_size;
    return getEdgeMask(uv, u_size, fadePixels);
}

const float EDGE_FADE  = 70.0;
const float FADE_WIDTH = 0.1;
const float HIDE_TIME  = 0.4;

vec2 effectMask(float hideTime, float fadeWidth, float edgeFadeWidth) {
    float progress = easeOutQuad(u_progress);

    float burnProgress      = clamp(progress / hideTime, 0.0, 1.0);
    float afterBurnProgress = clamp((progress - hideTime) / (1.0 - hideTime), 0.0, 1.0);

    bool bottomToTop = (u_closing_bottom_to_top > 0.5);
    float y          = bottomToTop ? 1.0 - v_coords.y : v_coords.y;
    float t          = y * (1.0 - fadeWidth);

    float windowMask = 1.0 - clamp((burnProgress - t) / fadeWidth, 0.0, 1.0);
    float eMask = clamp(t * (1.0 - windowMask) / max(burnProgress, 0.0001), 0.0, 1.0);

    if (progress > hideTime) {
        float fade = sqrt(clamp(1.0 - afterBurnProgress * afterBurnProgress, 0.0, 1.0));
        eMask *= mix(1.0, 1.0 - t, afterBurnProgress) * fade;
    }

    eMask *= getAbsoluteEdgeMask(edgeFadeWidth);

    if (u_for_opening > 0.5) {
        windowMask = 1.0 - windowMask;
    }

    return vec2(windowMask, eMask);
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
    vec2 uv = v_coords * u_size / vec2(400.0, 600.0) / u_scale;
    uv.y += u_progress * u_duration * u_movement_speed;

    float noise = simplex3DFractal(vec3(uv * 4.0, u_progress * u_duration * u_movement_speed * 1.5));

    vec2 e_mask = effectMask(HIDE_TIME, FADE_WIDTH, EDGE_FADE);
    noise *= e_mask.y;

    vec4 fire = getFireColor(noise, u_gradient1, u_gradient2, u_gradient3, u_gradient4, u_gradient5);

    vec4 oColor = getInputColor(v_coords);
    oColor.a *= e_mask.x;

    oColor = alphaOver(oColor, fire);

    gl_FragColor = vec4(oColor.rgb * oColor.a * alpha, oColor.a * alpha);
}
