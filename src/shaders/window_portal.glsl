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
uniform vec3 u_color;
uniform vec2 u_seed;
uniform float u_rotation_speed;
uniform float u_whirling;
uniform float u_details;

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

vec2 whirl(vec2 coords, float warping, float rotation) {
    float angle = pow(1.0 - clamp(length(coords), 0.0, 1.0), 2.0) * warping + rotation;
    float s = sin(angle);
    float c = cos(angle);
    return vec2(coords.x * c - coords.y * s, coords.x * s + coords.y * c);
}

float easeInBack(float x, float e) {
    return x * x * ((e + 1.0) * x - e);
}

float easeOutBack(float x, float e) {
    float p = x - 1.0;
    return p * p * ((e + 1.0) * p + e) + 1.0;
}

vec3 darken(vec3 c, float fac) {
    return c * (1.0 - clamp(fac, 0.0, 1.0));
}

vec3 lighten(vec3 c, float fac) {
    return c + (vec3(1.0) - c) * clamp(fac, 0.0, 1.0);
}

vec4 alphaOver(vec4 under, vec4 over) {
    if (under.a == 0.0 && over.a == 0.0) {
        return vec4(0.0);
    }
    float a = mix(under.a, 1.0, over.a);
    return vec4(mix(under.rgb * under.a, over.rgb, over.a) / max(a, 0.0001), a);
}

const float PORTAL_WOBBLE_TIME     = 0.8;
const float PORTAL_WOBBLE_STRENGTH = 1.2;
const float GLOW_EDGE_WIDTH        = 5.0;
const float WINDOW_SCALE           = 0.25;
const float WINDOW_SQUISH          = 1.0;
const float WINDOW_TILT            = -1.0;

const float PORTAL_OPEN_TIME  = 0.35;
const float PORTAL_CLOSE_TIME = 0.35;
const float WINDOW_OPEN_TIME  = 0.35;

vec2 getPortalWobble(vec2 coords) {
    float progress = (u_for_opening > 0.5 ? (1.0 - u_progress) : u_progress) / WINDOW_OPEN_TIME * u_duration;
    progress = clamp(1.0 - abs((progress - 1.0) / PORTAL_WOBBLE_TIME), 0.0, 1.0);
    progress = easeInBack(progress, 1.7);
    float dist = length(coords);
    return coords * (1.0 - dist) * exp(-dist) * progress * PORTAL_WOBBLE_STRENGTH;
}

float getPortalScale() {
    float scale = 1.0;
    float closeTime = PORTAL_CLOSE_TIME / max(u_duration, 0.001);
    float openTime  = PORTAL_OPEN_TIME / max(u_duration, 0.001);
    if (u_progress < openTime) {
        scale = easeOutBack(clamp(u_progress / openTime, 0.0, 1.0), 1.5);
    } else if (u_progress > 1.0 - closeTime) {
        scale = easeOutBack(clamp(1.0 - (u_progress - 1.0 + closeTime) / closeTime, 0.0, 1.0), 1.5);
    }
    return max(scale, 0.0);
}

vec2 getRandomDisplace(vec2 seed, float scale) {
    return vec2(simplex2D(seed * scale + u_seed) - 0.5,
                simplex2D(seed * scale + u_seed + vec2(7.89, 123.0)) - 0.5);
}

vec2 getWhirledCoords(vec2 coords, float speedMultiplier, float warpMultiplier) {
    float rotation = u_rotation_speed * u_progress * u_duration * speedMultiplier;
    float warping  = u_whirling * (6.0 + 1.5 * u_progress) * warpMultiplier;
    return whirl(coords, warping, rotation);
}

vec4 getPortalColor() {
    vec2 coords = (v_coords - vec2(0.5)) * 2.0;
    coords *= 1.4;

    float scale = getPortalScale();
    coords /= max(scale * 0.5 + 0.5, 0.01);

    vec2 wobble = getPortalWobble(coords);
    float detailScale = 10000.0 / max(u_size.x + u_size.y, 100.0) / max(u_details, 0.1);

    // Layer 1: глубина и градиент ядра портала
    vec2 layerCoords = getWhirledCoords(coords - wobble * 1.0, 0.25, 1.0);
    vec2 displace    = getRandomDisplace(layerCoords, 2.1);
    layerCoords += displace * 0.1;
    float dist  = length(layerCoords);
    float p_alpha = dist > 1.0 ? 0.0 : 1.0;
    vec4 color = vec4(mix(darken(u_color, 0.85), darken(u_color, 0.15), pow(clamp(dist, 0.0, 1.0), 5.0)), p_alpha);
    float rand = dot(displace, displace);

    // Layer 2: первый слой вихревых полос
    float noise = simplex2D(layerCoords / detailScale * 1.0 + vec2(12.3, 56.4) + u_seed);
    vec4 layer  = vec4(darken(u_color, 0.3), noise > 0.6 ? p_alpha : 0.0);
    color       = alphaOver(color, layer);

    // Layer 3: второй более яркий слой полос
    layerCoords = getWhirledCoords(coords - wobble * 1.5, 0.75, 0.5);
    displace    = getRandomDisplace(layerCoords, 12.2);
    layerCoords += displace * 0.1;
    noise = simplex2D(layerCoords / detailScale * 1.3 + u_seed);
    layer = vec4(u_color, noise > 0.6 ? p_alpha : 0.0);
    color = alphaOver(color, layer);
    color = clamp(color, 0.0, 1.0);

    // Layer 4: неоновое свечение внешнего контура
    float edge = mix(1.0, 5.0, rand) * GLOW_EDGE_WIDTH * detailScale - 150.0 * abs(dist - 1.0);
    layer = vec4(u_color, clamp(edge, 0.0, 1.0));
    color = alphaOver(color, layer);

    // Layer 5: искры и частицы плазмы
    layerCoords = getWhirledCoords(coords - wobble * 1.8, 1.25, 0.0);
    noise       = simplex2D(layerCoords / detailScale * 3.0 + u_seed);
    layer.rgb   = lighten(u_color, 0.8) * clamp(pow(clamp(noise * rand + 0.9, 0.0, 2.0), 50.0), 0.0, 1.0);
    color.rgb += layer.rgb;

    color.a *= pow(clamp(scale, 0.0, 1.0), 2.0);
    return clamp(color, 0.0, 1.0);
}

vec4 getWindowColor() {
    float progress = (u_for_opening > 0.5 ? (1.0 - u_progress) : u_progress) / WINDOW_OPEN_TIME * u_duration;
    progress = easeInBack(clamp(progress, 0.0, 1.0), 1.2);

    vec2 coords = v_coords * 2.0 - 1.0;
    coords /= mix(1.0, WINDOW_SCALE, progress);
    coords.y /= mix(1.0, (1.0 - 0.2 * WINDOW_SQUISH), progress);
    coords.x /= mix(1.0, 1.0 - 0.1 * WINDOW_TILT * coords.y, progress);
    coords = coords * 0.5 + 0.5;

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

    texColor.a *= clamp((1.0 - progress) * 3.0, 0.0, 1.0);
    return texColor;
}

void main() {
    vec4 portal = getPortalColor();
    vec4 window = getWindowColor();
    vec4 final_color = alphaOver(portal, window);

    gl_FragColor = vec4(final_color.rgb * final_color.a * alpha, final_color.a * alpha);
}
