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
uniform vec4 u_color;
uniform float u_seed;
uniform float u_scale;
uniform float u_strength;
uniform float u_speed;

// 1 out, 2 in hash (David Hoskins)
float hash12(vec2 p) {
    vec3 p3 = fract(vec3(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 2 out, 2 in hash
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

float easeInQuad(float x) {
    return x * x;
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
    float progress = easeInQuad(u_for_opening > 0.5 ? 1.0 - u_progress : u_progress);
    float time     = progress * u_duration * u_speed;
    float strength = u_strength * progress;
    float displace = 1000.0 * strength / u_size.x;
    float yPos     = u_scale * u_size.y * (v_coords.y + u_seed * 10.0);

    // Create large noise waves and add some smaller noise waves
    float noise = clamp(simplex2D(vec2(time, yPos * 0.002)) - 0.5, 0.0, 1.0);
    noise += (simplex2D(vec2(time * 10.0, yPos * 0.05)) - 0.5) * 0.15;

    // Apply noise as x displacement for every line
    float xPos  = clamp(v_coords.x - displace * noise * noise, 0.0, 1.0);
    vec4 oColor = getInputColor(vec2(xPos, v_coords.y));

    // Mix in random interference lines
    vec3 interference          = u_color.rgb * hash12(vec2(yPos * time, yPos * 0.01));
    float interferenceStrength = noise * min(strength, 1.0);
    oColor.rgb = mix(oColor.rgb, interference, u_color.a * interferenceStrength);

    // Mix in grainy noise
    vec3 grain          = u_color.rgb * simplex2D(u_size * v_coords + vec2(time * 100.0));
    float grainStrength = 0.2 * min(strength, 1.0);
    oColor.rgb          = mix(oColor.rgb, grain, u_color.a * grainStrength);

    // Add subtle scanline pattern every 4 pixels
    if (floor(mod(yPos * 0.25, 2.0)) == 0.0) {
        oColor.rgb = mix(oColor.rgb, u_color.rgb, u_color.a * (0.15 * noise));
    }

    // Shift green/blue channels (chromatic aberration)
    float offset = 0.1 * noise * displace;
    oColor.g     = mix(oColor.g, getInputColor(vec2(xPos + offset, v_coords.y)).g, 0.25);
    oColor.b     = mix(oColor.b, getInputColor(vec2(xPos - offset, v_coords.y)).b, 0.25);

    // Dissolve the window
    float fadeDelay = 1.5;
    float a = clamp(noise + 1.0 + fadeDelay - (3.0 + fadeDelay) * progress, 0.0, 1.0);
    oColor.a *= a;

    gl_FragColor = vec4(oColor.rgb * oColor.a * alpha, oColor.a * alpha);
}
